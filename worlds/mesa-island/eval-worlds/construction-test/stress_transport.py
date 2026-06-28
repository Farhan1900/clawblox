#!/usr/bin/env python3

import json
import math
import time
import urllib.request


BASE = "http://localhost:8093"


def reqj(session, path, method="GET", data=None, timeout=3):
    headers = {"X-Session": session}
    body = None
    if data is not None:
        body = json.dumps(data).encode()
        headers["Content-Type"] = "application/json"
    req = urllib.request.Request(BASE + path, data=body, headers=headers, method=method)
    with urllib.request.urlopen(req, timeout=timeout) as response:
        return json.load(response)


def join(name="latest-stress"):
    with urllib.request.urlopen(BASE + f"/join?name={name}", data=b"", timeout=2) as response:
        return json.load(response)["session"]


def action(session, kind, data):
    obs = reqj(session, "/input", method="POST", data={"type": kind, "data": data})
    return obs["player"]["attributes"].get("LastActionResult")


def wait_for(session, substring, timeout=10):
    deadline = time.time() + timeout
    last = None
    while time.time() < deadline:
        obs = reqj(session, "/observe")
        last = obs["player"]["attributes"].get("LastActionResult")
        if substring in (last or ""):
            return obs
        time.sleep(0.05)
    raise RuntimeError(f"timeout waiting for {substring}, last={last}")


def ent(obs, name):
    for entity in obs["world"]["entities"]:
        if entity["name"] == name:
            return entity
    raise KeyError(name)


def pos(obs, name):
    return ent(obs, name)["position"]


def positions(obs, names):
    return {name: pos(obs, name) for name in names}


def dist(a, b):
    return math.sqrt(sum((a[i] - b[i]) ** 2 for i in range(3)))


def main():
    session = join()
    obs = reqj(session, "/observe")
    print("session", session)
    print("start", obs["player"]["position"], obs["player"]["attributes"].get("LastActionResult"))

    print("pickup_root", action(session, "Pickup", {"position": pos(obs, "Block_11")}))
    obs = wait_for(session, "PickupSuccess")
    print("place_root_open", action(session, "Place", {"position": [0.0, 2.0, 4.0]}))
    obs = wait_for(session, "PlaceSuccess")
    for delay in [0.2, 0.6, 1.2]:
        time.sleep(delay)
        obs = reqj(session, "/observe")
    print("root_open_pos", pos(obs, "Block_11"))

    print("pickup_clump7", action(session, "Pickup", {"position": pos(obs, "Clump_7")}))
    obs = wait_for(session, "PickupSuccess")
    print("place_clump_on_root", action(session, "Place", {"position": pos(obs, "Block_11")}))
    obs = wait_for(session, "PlaceSuccess")
    print("root_material", ent(obs, "Block_11")["material"])

    for name, off in [
        ("Block_18", [-0.8, 0.38, 0.0]),
        ("Block_19", [0.8, 0.38, 0.0]),
        ("Block_20", [0.0, 0.38, 0.8]),
    ]:
        source = pos(obs, name)
        print("move_source", name, action(session, "MoveTo", {"position": [source[0], 0.0, source[2] + 1.0]}))
        obs = wait_for(session, "MoveToFinished")
        print("pickup", name, action(session, "Pickup", {"position": pos(obs, name)}))
        obs = wait_for(session, "PickupSuccess")
        root = pos(obs, "Block_11")
        print("move_root", name, action(session, "MoveTo", {"position": [root[0], 0.0, root[2] - 1.5]}))
        obs = wait_for(session, "MoveToFinished")
        root = pos(obs, "Block_11")
        target = [root[0] + off[0], off[1], root[2] + off[2]]
        print("place", name, target, action(session, "Place", {"position": target}))
        obs = wait_for(session, "PlaceSuccess")
        time.sleep(0.8)
        obs = reqj(session, "/observe")
        print("placed", name, pos(obs, name))

    names = ["Block_11", "Block_18", "Block_19", "Block_20"]
    print("built", positions(obs, names))

    print("pickup_arm", action(session, "Pickup", {"position": pos(obs, "Block_18")}))
    obs = wait_for(session, "PickupSuccess")
    held = positions(obs, names)
    print("held", held)

    print("move_transport", action(session, "MoveTo", {"position": [5.5, 0.0, 7.5]}))
    obs = wait_for(session, "MoveToFinished")
    carried = positions(obs, names)
    print("carried", carried)
    for name in names:
        print("transport_delta", name, round(dist(held[name], carried[name]), 2))

    place_target = [obs["player"]["position"][0] + 2.5, 2.5, obs["player"]["position"][2] + 1.5]
    print(
        "place_rotated",
        action(
            session,
            "Place",
            {"position": place_target, "rotation": {"yaw": 90.0, "pitch": 0.0, "roll": 0.0}},
        ),
    )
    for delay in [0.1, 0.4, 0.8, 1.6]:
        time.sleep(delay)
        obs = reqj(session, "/observe")
        print("settle", delay, {name: [round(x, 2) for x in pos(obs, name)] for name in names})


if __name__ == "__main__":
    main()
