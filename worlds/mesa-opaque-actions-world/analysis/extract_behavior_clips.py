#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import math
import os
import signal
import socket
import subprocess
import sys
import time
import urllib.error
import urllib.request
from collections import deque
from dataclasses import dataclass, field
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
WORLD_DIR_DEFAULT = SCRIPT_DIR.parent
PROJECT_ROOT_DEFAULT = SCRIPT_DIR.parents[2]
from typing import Any

try:
    from .analyze_recording import apply_delta, decode_blob, is_block_entity
except ImportError:
    from analyze_recording import apply_delta, decode_blob, is_block_entity


SPIRE_CAMERA_ARGS = [
    "--camera-mode",
    "spectator",
    "--camera-pan-x",
    "-30",
    "--camera-pan-y",
    "20",
    "--camera-pan-z",
    "-33",
    "--camera-zoom",
    "1.0",
    "--camera-yaw",
    "-1.0",
    "--camera-pitch",
    "0.75",
]

ORIGIN_CAMERA_ARGS = [
    "--camera-mode",
    "spectator",
    "--camera-pan-x",
    "0",
    "--camera-pan-y",
    "12",
    "--camera-pan-z",
    "0",
    "--camera-zoom",
    "0.85",
    "--camera-yaw",
    "0.0",
    "--camera-pitch",
    "1.25",
]

ORIGIN_CROSS_CAMERA_ARGS = [
    "--camera-mode",
    "spectator",
    "--camera-pan-x",
    "3",
    "--camera-pan-y",
    "10",
    "--camera-pan-z",
    "-7.5",
    "--camera-zoom",
    "0.80",
    "--camera-yaw",
    "0.0",
    "--camera-pitch",
    "1.20",
]

ORIGIN_TOWER_CAMERA_ARGS = [
    "--camera-mode",
    "spectator",
    "--camera-pan-x",
    "-2",
    "--camera-pan-y",
    "10",
    "--camera-pan-z",
    "-5",
    "--camera-zoom",
    "0.80",
    "--camera-yaw",
    "0.0",
    "--camera-pitch",
    "1.20",
]

FOLLOW_CAMERA_ARGS = [
    "--camera-mode",
    "follow",
]

CAMERA_PROFILES = {
    "spire_spectator": SPIRE_CAMERA_ARGS,
    "origin_spectator": ORIGIN_CAMERA_ARGS,
    "origin_cross_spectator": ORIGIN_CROSS_CAMERA_ARGS,
    "origin_tower_spectator": ORIGIN_TOWER_CAMERA_ARGS,
    "follow": FOLLOW_CAMERA_ARGS,
}


@dataclass(frozen=True)
class Region:
    xmin: float
    xmax: float
    ymin: float
    ymax: float
    zmin: float
    zmax: float

    def contains(self, x: float, y: float, z: float) -> bool:
        return self.xmin <= x <= self.xmax and self.ymin <= y <= self.ymax and self.zmin <= z <= self.zmax


@dataclass(frozen=True)
class LayoutTarget:
    x: float
    y: float
    z: float


@dataclass(frozen=True)
class ClipSpec:
    run_name: str
    label: str
    title: str
    description: str
    category: str
    detector: str
    params: dict[str, Any]
    pre_ticks: int
    post_ticks: int
    camera_profile: str
    speed: float


@dataclass
class MatchResult:
    spec: ClipSpec
    event_tick: int
    start_tick: int
    end_tick: int
    evidence: dict[str, Any]
    recording: Path
    output_mp4: Path | None = None
    elapsed_s: float | None = None
    skipped_existing: bool = False


@dataclass
class RunContext:
    run_dir: Path
    run_name: str
    recording: Path
    first_tick: int
    last_tick: int


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Extract behavior-driven mesa-world replay clips from report-grounded replay signatures."
    )
    parser.add_argument(
        "runs_dir",
        type=Path,
        nargs="?",
        default=Path.cwd(),
        help="Directory containing generation run folders (default: current directory)",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=None,
        help="Directory for extracted clips and manifest (default: <runs_dir>/clips/behavior)",
    )
    parser.add_argument(
        "--world-dir",
        type=Path,
        default=WORLD_DIR_DEFAULT,
        help="World directory for replay server assets (default: this script's parent world directory)",
    )
    parser.add_argument(
        "--clawblox-bin",
        type=Path,
        default=PROJECT_ROOT_DEFAULT / "target/debug/clawblox",
        help="Path to clawblox binary",
    )
    parser.add_argument(
        "--capture-script",
        type=Path,
        default=SCRIPT_DIR / "replay_capture.py",
        help="Path to replay capture script",
    )
    parser.add_argument("--width", type=int, default=1280)
    parser.add_argument("--height", type=int, default=720)
    parser.add_argument("--output-fps", type=int, default=12)
    parser.add_argument("--probe-timeout-ms", type=int, default=120000)
    parser.add_argument("--tail-ms", type=int, default=1000)
    parser.add_argument("--dry-run", action="store_true", help="Only write the manifest, do not capture clips")
    parser.add_argument(
        "--skip-existing",
        action="store_true",
        default=True,
        help="Skip capture when the target mp4 already exists",
    )
    parser.add_argument(
        "--labels",
        nargs="*",
        default=None,
        help="Optional subset of clip labels to scan and capture",
    )
    return parser.parse_args()


def choose_free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


def wait_for_replay_server(port: int, timeout_s: float = 30.0) -> None:
    deadline = time.time() + timeout_s
    url = f"http://127.0.0.1:{port}/replay/info"
    while time.time() < deadline:
        try:
            with urllib.request.urlopen(url, timeout=5) as response:
                if response.status == 200:
                    return
        except (urllib.error.URLError, ConnectionResetError, OSError):
            time.sleep(0.2)
    raise RuntimeError(f"Replay server on port {port} did not become ready in {timeout_s:.1f}s")


def launch_replay_server(args: argparse.Namespace, recording: Path, port: int) -> subprocess.Popen:
    process = subprocess.Popen(
        [
            str(args.clawblox_bin),
            "replay",
            str(recording),
            "--port",
            str(port),
            "--world-dir",
            str(args.world_dir),
        ],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    wait_for_replay_server(port)
    return process


def stop_process(process: subprocess.Popen | None) -> None:
    if process is None:
        return
    try:
        os.kill(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass


def select_recording(run_dir: Path) -> Path:
    recordings_dir = run_dir / "worlds" / "world-0" / "recordings"
    analysis_dirs = sorted(recordings_dir.glob("*_analysis"))
    if analysis_dirs:
        analysis_stem = analysis_dirs[0].name.removesuffix("_analysis")
        candidate = recordings_dir / f"{analysis_stem}.clawrec"
        if candidate.exists():
            return candidate.resolve()
    recordings = sorted(recordings_dir.glob("*.clawrec"))
    if not recordings:
        raise FileNotFoundError(f"No recording found in {recordings_dir}")
    return recordings[0].resolve()


def discover_runs(runs_dir: Path) -> dict[str, RunContext]:
    runs: dict[str, RunContext] = {}
    for run_dir in sorted(runs_dir.glob("genexp-*")):
        recording = select_recording(run_dir)
        first_tick, last_tick = read_tick_bounds(recording)
        runs[run_dir.name] = RunContext(
            run_dir=run_dir.resolve(),
            run_name=run_dir.name,
            recording=recording,
            first_tick=first_tick,
            last_tick=last_tick,
        )
    return runs


def read_tick_bounds(recording: Path) -> tuple[int, int]:
    import sqlite3

    conn = sqlite3.connect(str(recording))
    conn.row_factory = sqlite3.Row
    first = conn.execute("SELECT tick FROM checkpoints ORDER BY tick LIMIT 1").fetchone()
    max_row = conn.execute(
        """
        SELECT COALESCE(MAX(t), 0) AS max_tick
        FROM (
          SELECT MAX(tick) AS t FROM checkpoints
          UNION ALL
          SELECT MAX(tick) AS t FROM deltas
        )
        """
    ).fetchone()
    if first is None or max_row is None:
        raise RuntimeError(f"Recording has no tick data: {recording}")
    return int(first["tick"]), int(max_row["max_tick"])


def iter_observations(recording: Path):
    import sqlite3

    conn = sqlite3.connect(str(recording))
    conn.row_factory = sqlite3.Row
    checkpoints = conn.execute("SELECT tick, data FROM checkpoints ORDER BY tick").fetchall()
    for index, cp_row in enumerate(checkpoints):
        observation = decode_blob(cp_row["data"])
        tick = int(cp_row["tick"])
        yield tick, observation
        next_tick = int(checkpoints[index + 1]["tick"]) if index + 1 < len(checkpoints) else None
        query = "SELECT tick, data FROM deltas WHERE tick > ?"
        params: list[Any] = [tick]
        if next_tick is not None:
            query += " AND tick < ?"
            params.append(next_tick)
        query += " ORDER BY tick"
        for delta_row in conn.execute(query, params):
            apply_delta(observation, decode_blob(delta_row["data"]))
            yield int(delta_row["tick"]), observation


def block_positions(observation: dict[str, Any]) -> list[tuple[float, float, float]]:
    blocks: list[tuple[float, float, float]] = []
    for entity in observation.get("entities", []):
        if not is_block_entity(entity):
            continue
        position = entity.get("position") or []
        if len(position) < 3:
            continue
        blocks.append((float(position[0]), float(position[1]), float(position[2])))
    return blocks


def player_position(observation: dict[str, Any]) -> tuple[float, float, float] | None:
    players = observation.get("players", [])
    if not players:
        return None
    position = players[0].get("position") or []
    if len(position) < 3:
        return None
    return float(position[0]), float(position[1]), float(position[2])


def count_targets(blocks: list[tuple[float, float, float]], targets: list[LayoutTarget], tolerance: float) -> int:
    remaining = list(blocks)
    matched = 0
    for target in targets:
        best_index = None
        best_distance = None
        for index, (x, y, z) in enumerate(remaining):
            distance = math.dist((x, y, z), (target.x, target.y, target.z))
            if distance <= tolerance and (best_distance is None or distance < best_distance):
                best_index = index
                best_distance = distance
        if best_index is not None:
            matched += 1
            remaining.pop(best_index)
    return matched


def count_blocks_in_region(blocks: list[tuple[float, float, float]], region: Region) -> int:
    return sum(1 for x, y, z in blocks if region.contains(x, y, z))


def stack_height_in_region(blocks: list[tuple[float, float, float]], region: Region) -> int:
    ys = sorted(y for x, y, z in blocks if region.xmin <= x <= region.xmax and region.zmin <= z <= region.zmax)
    best = 0
    current = 0
    previous = None
    for y in ys:
        if previous is None or 0.45 <= y - previous <= 1.0:
            current = 1 if previous is None else current + 1
        else:
            current = 1
        best = max(best, current)
        previous = y
    return best


def load_specs() -> list[ClipSpec]:
    spire_region = Region(-31.6, -28.4, 19.8, 22.4, -34.6, -31.0)
    spire_stack_region = Region(-31.4, -28.6, 19.8, 24.5, -34.4, -31.6)
    summit_stack_region = Region(-28.5, -25.5, 10.0, 15.5, -29.5, -26.5)
    origin_region = Region(-1.5, 1.5, 0.0, 8.5, -1.5, 1.5)
    origin_tower_region = Region(-3.2, -0.8, 0.0, 7.5, -6.2, -3.8)
    g001_cross = [
        LayoutTarget(3.0, 0.37, -9.0),
        LayoutTarget(3.0, 0.37, -8.0),
        LayoutTarget(3.0, 0.37, -7.0),
        LayoutTarget(3.0, 0.37, -6.0),
        LayoutTarget(2.0, 0.37, -7.0),
        LayoutTarget(4.0, 0.37, -7.0),
        LayoutTarget(3.0, 1.12, -8.0),
    ]
    g005_cross = [
        LayoutTarget(-30.0, 20.37, -32.0),
        LayoutTarget(-31.0, 20.37, -33.0),
        LayoutTarget(-30.0, 21.12, -33.0),
        LayoutTarget(-29.0, 20.37, -33.0),
        LayoutTarget(-30.0, 20.37, -34.0),
    ]
    g008_compass = [
        LayoutTarget(0.0, 0.37, 0.0),
        LayoutTarget(0.0, 1.12, 0.0),
        LayoutTarget(0.0, 1.87, 0.0),
        LayoutTarget(0.0, 0.37, -5.0),
        LayoutTarget(5.0, 0.37, 0.0),
        LayoutTarget(-5.0, 0.37, 0.0),
        LayoutTarget(3.5, 0.37, 3.5),
        LayoutTarget(3.5, 0.37, -3.5),
        LayoutTarget(-4.0, 0.37, 4.0),
        LayoutTarget(-3.5, 0.37, -3.5),
        LayoutTarget(-3.5, 1.12, -3.5),
        LayoutTarget(-5.5, 0.37, -5.5),
    ]
    return [
        ClipSpec(
            run_name="genexp-20260315T021154Z-g001",
            label="g001_origin_cross_art",
            title="Origin Cross Pattern",
            description="The first deliberate floor pattern near origin: a 3D cross assembled from seven blocks.",
            category="construction_art",
            detector="target_layout",
            params={"targets": g001_cross, "tolerance": 0.60, "min_matches": 6},
            pre_ticks=2400,
            post_ticks=600,
            camera_profile="origin_cross_spectator",
            speed=6.0,
        ),
        ClipSpec(
            run_name="genexp-20260315T021154Z-g002",
            label="g002_spire_top_seed_blocks",
            title="First Spire-Top Seeding",
            description="Blocks placed onto the Spire top from wedge positions before direct access existed.",
            category="experimentation",
            detector="region_block_count",
            params={"region": spire_region, "threshold": 4, "player_y_max": 20.0},
            pre_ticks=2400,
            post_ticks=600,
            camera_profile="spire_spectator",
            speed=4.0,
        ),
        ClipSpec(
            run_name="genexp-20260315T021154Z-g003",
            label="g003_structured_spire_assault",
            title="Structured Spire Assault",
            description="A more controlled wedge attack that leaves seeded blocks on the Spire top without standing there.",
            category="experimentation",
            detector="region_block_count",
            params={"region": spire_region, "threshold": 2, "player_y_max": 20.0},
            pre_ticks=2400,
            post_ticks=600,
            camera_profile="spire_spectator",
            speed=4.0,
        ),
        ClipSpec(
            run_name="genexp-20260315T021154Z-g004",
            label="g004_ne_corner_slide",
            title="NE Corner Slide",
            description="The diagonal northeast corner slip experiment around the Spire box collision.",
            category="experimentation",
            detector="player_region",
            params={
                "region": Region(-27.2, -26.7, 18.0, 25.0, -32.0, -31.1),
            },
            pre_ticks=1200,
            post_ticks=600,
            camera_profile="spire_spectator",
            speed=4.0,
        ),
        ClipSpec(
            run_name="genexp-20260315T021154Z-g005",
            label="g005_spire_five_block_tower",
            title="Spire Five-Block Tower",
            description="The agent turns the Spire summit into a five-block vertical tower before converting it into a monument.",
            category="breakthrough_build",
            detector="region_stack_height",
            params={"region": spire_stack_region, "threshold": 5},
            pre_ticks=1800,
            post_ticks=900,
            camera_profile="spire_spectator",
            speed=4.0,
        ),
        ClipSpec(
            run_name="genexp-20260315T021154Z-g005",
            label="g005_spire_cross_monument",
            title="Spire Cross Monument",
            description="The five-block tower is dismantled into a deliberate cross monument on the Spire top.",
            category="construction_art",
            detector="target_layout",
            params={"targets": g005_cross, "tolerance": 0.60, "min_matches": 5},
            pre_ticks=1800,
            post_ticks=900,
            camera_profile="spire_spectator",
            speed=4.0,
        ),
        ClipSpec(
            run_name="genexp-20260315T021154Z-g006",
            label="g006_spire_tower_collapse",
            title="Spire Tower Collapse Limit",
            description="The added Spire-top tower reaches its limit and collapses when a further block is attempted.",
            category="engineering_failure",
            detector="region_stack_collapse",
            params={
                "region": spire_stack_region,
                "peak_threshold": 5,
                "collapsed_threshold": 2,
                "collapse_window_ticks": 1200,
            },
            pre_ticks=1200,
            post_ticks=600,
            camera_profile="spire_spectator",
            speed=4.0,
        ),
        ClipSpec(
            run_name="genexp-20260315T021154Z-g007",
            label="g007_origin_to_spire_speedrun",
            title="Origin To Spire Speedrun",
            description="The optimized speedrun route from true origin to the Spire top in under 1000 ticks.",
            category="route_mastery",
            detector="speedrun",
            params={
                "start_region": Region(-1.0, 1.0, 0.0, 3.2, -1.0, 1.0),
                "end_region": Region(-31.5, -28.0, 22.0, 24.5, -35.5, -31.0),
                "min_delta_ticks": 900,
                "max_delta_ticks": 1400,
            },
            pre_ticks=120,
            post_ticks=180,
            camera_profile="follow",
            speed=1.5,
        ),
        ClipSpec(
            run_name="genexp-20260315T021154Z-g008",
            label="g008_compass_rose_build",
            title="Compass Rose Build",
            description="The aesthetic rebuild at origin: failed tight spacing gives way to a precise wide compass rose.",
            category="construction_art",
            detector="target_layout",
            params={"targets": g008_compass, "tolerance": 0.60, "min_matches": 11},
            pre_ticks=12000,
            post_ticks=900,
            camera_profile="origin_spectator",
            speed=12.0,
        ),
        ClipSpec(
            run_name="genexp-20260315T021154Z-g009",
            label="g009_head_carry_glitch",
            title="Head-Carry Glitch",
            description="The carried block gets stuck above the player and moves with them, becoming a transport mechanism.",
            category="mechanics_discovery",
            detector="head_carry_persistence",
            params={
                "horizontal_tolerance": 0.80,
                "vertical_min": 3.0,
                "vertical_max": 4.2,
                "min_duration_ticks": 45,
                "min_player_motion": 1.5,
            },
            pre_ticks=600,
            post_ticks=900,
            camera_profile="follow",
            speed=4.0,
        ),
        ClipSpec(
            run_name="genexp-20260315T021154Z-g009",
            label="g009_nine_block_tower",
            title="Nine-Block Tower",
            description="A careful origin-area tower reaches nine blocks and becomes a launch platform and engineering proof point.",
            category="tower_engineering",
            detector="region_stack_height",
            params={"region": origin_tower_region, "threshold": 9},
            pre_ticks=3600,
            post_ticks=600,
            camera_profile="origin_tower_spectator",
            speed=8.0,
        ),
        ClipSpec(
            run_name="genexp-20260315T021154Z-g010",
            label="g010_twelve_block_attempt_collapse",
            title="Twelve-Block Attempt Collapse",
            description="The jump-place extension succeeds for the ninth block, then the overextended origin tower detonates into a collapse.",
            category="engineering_failure",
            detector="region_stack_collapse",
            params={
                "region": origin_region,
                "peak_threshold": 9,
                "collapsed_threshold": 2,
                "collapse_window_ticks": 1200,
            },
            pre_ticks=3000,
            post_ticks=900,
            camera_profile="origin_spectator",
            speed=8.0,
        ),
        ClipSpec(
            run_name="genexp-20260315T021154Z-g010",
            label="g010_spire_three_block_tower",
            title="Three-Block Spire Tower",
            description="After the failed giant tower push, the effort shifts to precise high-altitude delivery and a three-block Spire tower.",
            category="tower_engineering",
            detector="region_stack_height",
            params={"region": spire_stack_region, "threshold": 3},
            pre_ticks=1800,
            post_ticks=900,
            camera_profile="spire_spectator",
            speed=4.0,
        ),
    ]


def maybe_match_target_layout(spec: ClipSpec, blocks: list[tuple[float, float, float]]) -> dict[str, Any] | None:
    matched = count_targets(blocks, spec.params["targets"], spec.params["tolerance"])
    if matched >= spec.params["min_matches"]:
        return {"matched_targets": matched}
    return None


def maybe_match_region_block_count(
    spec: ClipSpec,
    blocks: list[tuple[float, float, float]],
    player: tuple[float, float, float] | None,
) -> dict[str, Any] | None:
    count = count_blocks_in_region(blocks, spec.params["region"])
    player_y = None if player is None else player[1]
    if count >= spec.params["threshold"] and (
        player_y is None or player_y <= spec.params.get("player_y_max", float("inf"))
    ):
        return {"region_count": count, "player_y": player_y}
    return None


def maybe_match_region_stack_height(spec: ClipSpec, blocks: list[tuple[float, float, float]]) -> dict[str, Any] | None:
    stack_height = stack_height_in_region(blocks, spec.params["region"])
    if stack_height >= spec.params["threshold"]:
        return {"stack_height": stack_height}
    return None


def maybe_match_player_region(spec: ClipSpec, player: tuple[float, float, float] | None) -> dict[str, Any] | None:
    if player is None:
        return None
    region = spec.params["region"]
    if region.contains(*player):
        return {"player_position": [round(player[0], 3), round(player[1], 3), round(player[2], 3)]}
    return None


def scan_run(context: RunContext, specs: list[ClipSpec]) -> list[MatchResult]:
    pending = list(specs)
    matches: list[MatchResult] = []
    collapse_states: dict[str, dict[str, Any]] = {}
    speedrun_states: dict[str, deque[int]] = {}
    head_states: dict[str, dict[str, Any]] = {}

    for spec in pending:
        if spec.detector == "region_stack_collapse":
            collapse_states[spec.label] = {"peak_tick": None, "peak_height": 0}
        elif spec.detector == "speedrun":
            speedrun_states[spec.label] = deque()
        elif spec.detector == "head_carry_persistence":
            head_states[spec.label] = {
                "active_start_tick": None,
                "last_tick": None,
                "start_player": None,
            }

    for tick, observation in iter_observations(context.recording):
        blocks = block_positions(observation)
        player = player_position(observation)
        remaining: list[ClipSpec] = []

        for spec in pending:
            evidence = None

            if spec.detector == "target_layout":
                evidence = maybe_match_target_layout(spec, blocks)
            elif spec.detector == "region_block_count":
                evidence = maybe_match_region_block_count(spec, blocks, player)
            elif spec.detector == "region_stack_height":
                evidence = maybe_match_region_stack_height(spec, blocks)
            elif spec.detector == "player_region":
                evidence = maybe_match_player_region(spec, player)
            elif spec.detector == "region_stack_collapse":
                state = collapse_states[spec.label]
                height = stack_height_in_region(blocks, spec.params["region"])
                if height > state["peak_height"]:
                    state["peak_height"] = height
                    state["peak_tick"] = tick
                peak_tick = state["peak_tick"]
                if (
                    peak_tick is not None
                    and state["peak_height"] >= spec.params["peak_threshold"]
                    and height <= spec.params["collapsed_threshold"]
                    and tick > peak_tick
                    and (tick - peak_tick) <= spec.params["collapse_window_ticks"]
                ):
                    evidence = {
                        "peak_tick": peak_tick,
                        "peak_height": state["peak_height"],
                        "collapsed_height": height,
                    }
            elif spec.detector == "speedrun":
                if player is not None:
                    start_region = spec.params["start_region"]
                    end_region = spec.params["end_region"]
                    starts = speedrun_states[spec.label]
                    if start_region.contains(*player):
                        starts.append(tick)
                    while starts and tick - starts[0] > spec.params["max_delta_ticks"]:
                        starts.popleft()
                    if end_region.contains(*player):
                        for start_tick in reversed(starts):
                            delta = tick - start_tick
                            if spec.params["min_delta_ticks"] <= delta <= spec.params["max_delta_ticks"]:
                                evidence = {"start_tick": start_tick, "delta_ticks": delta}
                                break
            elif spec.detector == "head_carry_persistence":
                state = head_states[spec.label]
                candidate = None
                if player is not None:
                    px, py, pz = player
                    for x, y, z in blocks:
                        horizontal = math.dist((px, pz), (x, z))
                        vertical = y - py
                        if (
                            horizontal <= spec.params["horizontal_tolerance"]
                            and spec.params["vertical_min"] <= vertical <= spec.params["vertical_max"]
                        ):
                            candidate = (x, y, z)
                            break
                if candidate is not None:
                    if state["active_start_tick"] is None or state["last_tick"] is None or tick - state["last_tick"] > 2:
                        state["active_start_tick"] = tick
                        state["start_player"] = player
                    state["last_tick"] = tick
                    start_tick = state["active_start_tick"]
                    start_player = state["start_player"]
                    if start_tick is not None and start_player is not None and player is not None:
                        player_motion = math.dist(start_player, player)
                        if (
                            tick - start_tick >= spec.params["min_duration_ticks"]
                            and player_motion >= spec.params["min_player_motion"]
                        ):
                            evidence = {
                                "start_tick": start_tick,
                                "player_motion": round(player_motion, 3),
                                "block_position": [round(candidate[0], 3), round(candidate[1], 3), round(candidate[2], 3)],
                            }
                else:
                    state["active_start_tick"] = None
                    state["last_tick"] = None
                    state["start_player"] = None
            else:
                raise ValueError(f"Unknown detector: {spec.detector}")

            if evidence is None:
                remaining.append(spec)
                continue

            event_tick = int(evidence.get("start_tick", tick))
            if spec.detector != "speedrun":
                event_tick = tick
            if spec.detector == "head_carry_persistence" and "start_tick" in evidence:
                event_tick = int(evidence["start_tick"])
            if spec.detector == "speedrun" and "start_tick" in evidence:
                event_tick = int(evidence["start_tick"])

            if spec.detector == "speedrun":
                clip_start = max(context.first_tick, int(evidence["start_tick"]) - spec.pre_ticks)
                clip_end = min(context.last_tick, tick + spec.post_ticks)
            else:
                clip_start = max(context.first_tick, tick - spec.pre_ticks)
                clip_end = min(context.last_tick, tick + spec.post_ticks)

            matches.append(
                MatchResult(
                    spec=spec,
                    event_tick=tick,
                    start_tick=clip_start,
                    end_tick=clip_end,
                    evidence=evidence,
                    recording=context.recording,
                )
            )

        pending = remaining
        if not pending:
            break

    return matches


def capture_clip(
    args: argparse.Namespace,
    match: MatchResult,
    output_dir: Path,
    replay_port: int,
) -> None:
    output_path = output_dir / f"{match.spec.label}.mp4"
    match.output_mp4 = output_path
    if args.skip_existing and output_path.exists():
        match.skipped_existing = True
        return

    command = [
        sys.executable,
        str(args.capture_script),
        f"http://127.0.0.1:{replay_port}/",
        str(output_path),
        str(match.start_tick),
        str(match.end_tick),
        "--width",
        str(args.width),
        "--height",
        str(args.height),
        "--output-fps",
        str(args.output_fps),
        "--capture-mode",
        "stepped",
        "--probe-timeout-ms",
        str(args.probe_timeout_ms),
        "--capture-interpolation-delay-ms",
        "0",
        "--follow-position-smoothing",
        "0",
        "--follow-rotation-smoothing",
        "0",
        *CAMERA_PROFILES[match.spec.camera_profile],
    ]
    started_at = time.time()
    subprocess.run(command, check=True)
    match.elapsed_s = round(time.time() - started_at, 2)


def write_manifest(output_dir: Path, manifest: dict[str, Any]) -> None:
    (output_dir / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")


def build_manifest(runs_dir: Path, output_dir: Path, specs: list[ClipSpec], matches: list[MatchResult]) -> dict[str, Any]:
    return {
        "runs_dir": str(runs_dir),
        "output_dir": str(output_dir),
        "generated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "selection_basis": "report-driven replay signatures",
        "specs": [
            {
                "run_name": spec.run_name,
                "label": spec.label,
                "title": spec.title,
                "description": spec.description,
                "category": spec.category,
                "detector": spec.detector,
                "camera_profile": spec.camera_profile,
                "speed": spec.speed,
            }
            for spec in specs
        ],
        "clips": [
            {
                "run_name": match.spec.run_name,
                "label": match.spec.label,
                "title": match.spec.title,
                "description": match.spec.description,
                "category": match.spec.category,
                "detector": match.spec.detector,
                "recording": str(match.recording),
                "event_tick": match.event_tick,
                "start_tick": match.start_tick,
                "end_tick": match.end_tick,
                "camera_profile": match.spec.camera_profile,
                "speed": match.spec.speed,
                "evidence": match.evidence,
                "output_mp4": None if match.output_mp4 is None else str(match.output_mp4),
                "elapsed_s": match.elapsed_s,
                "skipped_existing": match.skipped_existing,
            }
            for match in matches
        ],
    }


def main() -> None:
    args = parse_args()
    runs_dir = args.runs_dir.resolve()
    output_dir = (
        args.output_dir.resolve() if args.output_dir is not None else (runs_dir / "clips" / "behavior").resolve()
    )
    output_dir.mkdir(parents=True, exist_ok=True)

    specs = load_specs()
    if args.labels:
        label_set = set(args.labels)
        specs = [spec for spec in specs if spec.label in label_set]
        if not specs:
            raise RuntimeError("No matching labels found.")

    runs = discover_runs(runs_dir)
    grouped_specs: dict[str, list[ClipSpec]] = {}
    for spec in specs:
        grouped_specs.setdefault(spec.run_name, []).append(spec)

    matches: list[MatchResult] = []
    for run_name, run_specs in grouped_specs.items():
        context = runs.get(run_name)
        if context is None:
            raise RuntimeError(f"Run {run_name} not found under {runs_dir}")
        run_matches = scan_run(context, run_specs)
        missing = {spec.label for spec in run_specs} - {match.spec.label for match in run_matches}
        if missing:
            raise RuntimeError(f"Failed to detect clips for {run_name}: {', '.join(sorted(missing))}")
        matches.extend(run_matches)

    matches.sort(key=lambda item: (item.spec.run_name, item.start_tick, item.spec.label))
    manifest = build_manifest(runs_dir, output_dir, specs, matches)
    write_manifest(output_dir, manifest)

    if args.dry_run:
        return

    for match in matches:
        replay_process = None
        replay_port = choose_free_port()
        try:
            print(
                f"[clip] {match.spec.label} "
                f"ticks={match.start_tick}-{match.end_tick} "
                f"detector={match.spec.detector}"
            )
            replay_process = launch_replay_server(args, match.recording, replay_port)
            capture_clip(args, match, output_dir, replay_port)
            manifest = build_manifest(runs_dir, output_dir, specs, matches)
            write_manifest(output_dir, manifest)
        finally:
            stop_process(replay_process)


if __name__ == "__main__":
    main()
