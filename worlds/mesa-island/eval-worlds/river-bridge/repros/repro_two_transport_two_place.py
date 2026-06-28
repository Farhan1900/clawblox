#!/usr/bin/env python3

import importlib.util
import time
from pathlib import Path


BASE_REPRO = Path(__file__).with_name("repro_crash_after_pile_pickup.py")
SPEC = importlib.util.spec_from_file_location("pile_repro", BASE_REPRO)
PILE_REPRO = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PILE_REPRO)


def world_blocks(obs):
    entities = (obs.get("world") or {}).get("entities", [])
    out = {}
    for entity in entities:
        name = entity.get("name") or ""
        position = entity.get("position")
        if name.startswith("Block_") and position:
            out[name] = position
    return out


def lifted_blocks(obs, min_y=2.0):
    return {name: pos for name, pos in world_blocks(obs).items() if pos[1] > min_y}


def centroid(points):
    values = list(points.values())
    return [sum(point[i] for point in values) / len(values) for i in range(3)]


def summary(label, obs):
    blocks = world_blocks(obs)
    lifted = lifted_blocks(obs)
    center = centroid(lifted) if lifted else None
    print(
        label,
        {
            "result": PILE_REPRO.last_action_result(obs),
            "total_blocks": len(blocks),
            "lifted_blocks": len(lifted),
            "centroid": None if center is None else [round(v, 2) for v in center],
        },
        flush=True,
    )


def wait_settle(session, samples=12, delay_s=0.2):
    last = None
    for _ in range(samples):
        time.sleep(delay_s)
        last = PILE_REPRO.observe(session)
    return last


def replay_initial_pickup(session, delay_scale):
    prev = None
    obs = None
    for item in PILE_REPRO.INPUTS:
        if item["delay_s"] > 0:
            time.sleep(item["delay_s"] * delay_scale)
        action_obs = PILE_REPRO.action(session, item["type"], item["data"])
        obs = PILE_REPRO.wait_for_action_completion(
            session, item["type"], prev, PILE_REPRO.last_action_result(action_obs)
        )
        prev = PILE_REPRO.last_action_result(obs)
    return obs


def move_to(session, prev_result, target):
    action_obs = PILE_REPRO.action(session, "MoveTo", {"position": target})
    obs = PILE_REPRO.wait_for_action_completion(
        session, "MoveTo", prev_result, PILE_REPRO.last_action_result(action_obs)
    )
    return obs


def pickup_at(session, prev_result, target):
    action_obs = PILE_REPRO.action(session, "Pickup", {"position": target})
    obs = PILE_REPRO.wait_for_action_completion(
        session, "Pickup", prev_result, PILE_REPRO.last_action_result(action_obs)
    )
    return obs


def place_at(session, position):
    PILE_REPRO.action(
        session,
        "Place",
        {"position": position, "rotation": {"pitch": 0, "roll": 0, "yaw": 0}},
    )
    time.sleep(0.4)
    return PILE_REPRO.observe(session)


def choose_clear_place_targets(player_position):
    px, _, pz = player_position
    return [
        [round(px - 4.0, 2), 0.37, round(pz, 2)],
        [round(px - 3.0, 2), 0.37, round(pz - 2.0, 2)],
        [round(px - 2.5, 2), 0.37, round(pz - 3.0, 2)],
    ]


def choose_approach_target(player_position, target_position, stand_off=2.0):
    px, _, pz = player_position
    tx, _, tz = target_position
    dx = tx - px
    dz = tz - pz
    planar = (dx * dx + dz * dz) ** 0.5
    if planar <= stand_off + 0.25:
        return [round(tx, 2), 0.0, round(tz, 2)]
    scale = max(planar - stand_off, 0.0) / planar
    return [round(px + dx * scale, 2), 0.0, round(pz + dz * scale, 2)]


def settle_and_choose_repickup(obs):
    high_blocks = lifted_blocks(obs, min_y=1.5)
    if not high_blocks:
        raise AssertionError("expected settled pile to remain observable for repick, found no elevated blocks")
    target = centroid(high_blocks)
    target[1] = max(pos[1] for pos in high_blocks.values())
    return target


def assert_total_blocks(obs, expected=22):
    actual = len(world_blocks(obs))
    if actual != expected:
        raise AssertionError(f"expected {expected} total blocks, found {actual}")


def run_cycle(session, prev_result, move_target, label_prefix):
    obs = move_to(session, prev_result, move_target)
    prev_result = PILE_REPRO.last_action_result(obs)
    summary(f"{label_prefix}_transport", obs)
    assert_total_blocks(obs)

    player_position = (obs.get("player") or {}).get("position")
    if not player_position:
        raise AssertionError(f"{label_prefix} missing player position after transport")
    place_targets = choose_clear_place_targets(player_position)

    placed = None
    for position in place_targets:
        obs = place_at(session, position)
        prev_result = PILE_REPRO.last_action_result(obs)
        summary(f"{label_prefix}_place_{position}", obs)
        assert_total_blocks(obs)
        if prev_result == "PlaceSuccess":
            placed = obs
            break
    if placed is None:
        raise AssertionError(f"{label_prefix} failed to find a valid placement target")

    settled = wait_settle(session)
    summary(f"{label_prefix}_settled", settled)
    assert_total_blocks(settled)
    return settled


def main():
    delay_scale = 0.2
    artifacts = PILE_REPRO.start_world()
    try:
        session = PILE_REPRO.join("river-bridge-two-place")
        obs = replay_initial_pickup(session, delay_scale)
        summary("after_initial_pickup", obs)
        assert_total_blocks(obs)

        prev = PILE_REPRO.last_action_result(obs)
        obs = run_cycle(
            session,
            prev,
            [-10.0, 0.0, -8.0],
            "first",
        )

        repick_target = settle_and_choose_repickup(obs)
        print("repick_target", [round(v, 2) for v in repick_target], flush=True)

        prev = PILE_REPRO.last_action_result(obs)
        player_position = (obs.get("player") or {}).get("position")
        if not player_position:
            raise AssertionError("missing player position before second approach")
        approach_target = choose_approach_target(player_position, repick_target)
        print("approach_target", approach_target, flush=True)
        obs = move_to(session, prev, approach_target)
        prev = PILE_REPRO.last_action_result(obs)
        summary("second_approach", obs)
        assert_total_blocks(obs)
        if prev != "MoveToFinished:Reached":
            raise AssertionError(f"expected second approach to reach stand-off target, got {prev}")

        obs = pickup_at(session, prev, repick_target)
        prev = PILE_REPRO.last_action_result(obs)
        summary("after_second_pickup", obs)
        assert_total_blocks(obs)
        if prev != "PickupSuccess":
            raise AssertionError(f"expected second pickup to succeed, got {prev}")

        obs = run_cycle(
            session,
            prev,
            [-2.0, 0.0, -2.0],
            "second",
        )

        print(
            "PASS",
            {
                "expectation": "two successful transport + placement cycles with all blocks preserved",
                "total_blocks": len(world_blocks(obs)),
            },
            flush=True,
        )
    finally:
        if not artifacts.get("reused_existing"):
            PILE_REPRO.cleanup_artifacts(artifacts)


if __name__ == "__main__":
    main()
