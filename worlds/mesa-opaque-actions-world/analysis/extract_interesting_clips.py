#!/usr/bin/env python3
from __future__ import annotations

import argparse
import csv
import json
import os
import signal
import socket
import subprocess
import sys
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
WORLD_DIR_DEFAULT = SCRIPT_DIR.parent
PROJECT_ROOT_DEFAULT = SCRIPT_DIR.parents[2]

DEFAULT_CAMERA_ARGS = [
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


@dataclass
class RunMetrics:
    run_dir: Path
    run_name: str
    recording: Path
    summary: dict
    final_sustained: float
    final_sustained_tick: int
    peak_y: float
    peak_tick: int
    peak_gap: float


@dataclass
class ClipCandidate:
    run: RunMetrics
    kind: str
    label: str
    score: float
    event_tick: int
    start_tick: int
    end_tick: int
    reasons: list[str]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Inspect mesa-world runs, find breakthrough/interesting moments, and extract replay clips."
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
        help="Directory for extracted clips and manifest (default: <runs_dir>/clips/interesting)",
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
    parser.add_argument("--port", type=int, default=10181, help="Replay server port")
    parser.add_argument(
        "--generation-improvement-threshold",
        type=float,
        default=0.5,
        help="Minimum sustained-height improvement over previous generations to mark a breakthrough",
    )
    parser.add_argument(
        "--transient-gap-threshold",
        type=float,
        default=5.0,
        help="Minimum peak-minus-sustained gap to mark a transient interesting clip",
    )
    parser.add_argument(
        "--sustained-pre-ticks",
        type=int,
        default=2400,
        help="Context before a sustained breakthrough event",
    )
    parser.add_argument(
        "--sustained-post-ticks",
        type=int,
        default=600,
        help="Context after a sustained breakthrough event",
    )
    parser.add_argument(
        "--peak-pre-ticks",
        type=int,
        default=1800,
        help="Context before a transient peak event",
    )
    parser.add_argument(
        "--peak-post-ticks",
        type=int,
        default=300,
        help="Context after a transient peak event",
    )
    parser.add_argument("--width", type=int, default=1280)
    parser.add_argument("--height", type=int, default=720)
    parser.add_argument("--output-fps", type=int, default=12)
    parser.add_argument("--speed", type=float, default=4.0)
    parser.add_argument("--probe-timeout-ms", type=int, default=120000)
    parser.add_argument("--dry-run", action="store_true", help="Write manifest without capturing clips")
    parser.add_argument(
        "--skip-existing",
        action="store_true",
        default=True,
        help="Skip capture when the target mp4 already exists",
    )
    return parser.parse_args()


def read_csv_floats(path: Path, tick_key: str, value_key: str) -> list[tuple[int, float]]:
    rows = []
    with path.open() as handle:
        for row in csv.DictReader(handle):
            value = row.get(value_key, "")
            if not value:
                continue
            rows.append((int(row[tick_key]), float(value)))
    return rows


def discover_runs(runs_dir: Path) -> list[RunMetrics]:
    run_metrics = []
    for run_dir in sorted(runs_dir.glob("genexp-*")):
        recording = next(run_dir.glob("worlds/world-0/recordings/*.clawrec"), None)
        analysis_dir = next(run_dir.glob("worlds/world-0/recordings/*_analysis"), None)
        if recording is None or analysis_dir is None:
            continue
        summary_path = analysis_dir / "summary.json"
        sustained_csv = analysis_dir / "agent_max_y_sustained.csv"
        vertical_csv = analysis_dir / "agent_vertical.csv"
        if not summary_path.exists() or not sustained_csv.exists() or not vertical_csv.exists():
            continue

        summary = json.loads(summary_path.read_text())
        sustained_rows = read_csv_floats(sustained_csv, "tick", "max_agent_y_sustained_2s")
        vertical_rows = read_csv_floats(vertical_csv, "tick", "agent_y")
        if not sustained_rows or not vertical_rows:
            continue

        final_sustained = sustained_rows[-1][1]
        final_sustained_tick = next(
            tick for tick, value in sustained_rows if abs(value - final_sustained) < 1e-9
        )
        peak_tick, peak_y = max(vertical_rows, key=lambda item: item[1])
        run_metrics.append(
            RunMetrics(
                run_dir=run_dir.resolve(),
                run_name=run_dir.name,
                recording=recording.resolve(),
                summary=summary,
                final_sustained=final_sustained,
                final_sustained_tick=final_sustained_tick,
                peak_y=peak_y,
                peak_tick=peak_tick,
                peak_gap=peak_y - final_sustained,
            )
        )
    return run_metrics


def select_candidates(args: argparse.Namespace, runs: list[RunMetrics]) -> list[ClipCandidate]:
    candidates: list[ClipCandidate] = []
    best_so_far = float("-inf")

    for run in runs:
        improvement = run.final_sustained - best_so_far
        if improvement >= args.generation_improvement_threshold or best_so_far == float("-inf"):
            candidates.append(
                ClipCandidate(
                    run=run,
                    kind="sustained_breakthrough",
                    label=f"{run.run_name}_sustained_breakthrough",
                    score=run.final_sustained,
                    event_tick=run.final_sustained_tick,
                    start_tick=max(60, run.final_sustained_tick - args.sustained_pre_ticks),
                    end_tick=run.final_sustained_tick + args.sustained_post_ticks,
                    reasons=[
                        f"final_sustained={run.final_sustained:.2f}",
                        f"improvement_over_prev={improvement:.2f}",
                    ],
                )
            )
            best_so_far = run.final_sustained

        if run.peak_gap >= args.transient_gap_threshold:
            candidates.append(
                ClipCandidate(
                    run=run,
                    kind="transient_peak",
                    label=f"{run.run_name}_transient_peak",
                    score=run.peak_gap,
                    event_tick=run.peak_tick,
                    start_tick=max(60, run.peak_tick - args.peak_pre_ticks),
                    end_tick=run.peak_tick + args.peak_post_ticks,
                    reasons=[
                        f"peak_y={run.peak_y:.2f}",
                        f"final_sustained={run.final_sustained:.2f}",
                        f"peak_gap={run.peak_gap:.2f}",
                    ],
                )
            )

    candidates.sort(key=lambda item: (item.run.run_name, item.start_tick, item.kind))
    merged: list[ClipCandidate] = []
    for candidate in candidates:
        if not merged or merged[-1].run.run_name != candidate.run.run_name:
            merged.append(candidate)
            continue
        prev = merged[-1]
        if candidate.start_tick <= prev.end_tick:
            prev.end_tick = max(prev.end_tick, candidate.end_tick)
            prev.event_tick = min(prev.event_tick, candidate.event_tick)
            prev.kind = f"{prev.kind}+{candidate.kind}"
            prev.label = f"{prev.run.run_name}_{prev.kind.replace('+', '_')}"
            prev.score = max(prev.score, candidate.score)
            prev.reasons.extend(candidate.reasons)
        else:
            merged.append(candidate)
    return merged


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


def choose_free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


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


def capture_candidate(
    args: argparse.Namespace,
    candidate: ClipCandidate,
    output_dir: Path,
    replay_port: int,
) -> dict:
    output_path = output_dir / f"{candidate.label}.mp4"
    replay_url = f"http://127.0.0.1:{replay_port}/"
    if args.skip_existing and output_path.exists():
        return {
            "run_name": candidate.run.run_name,
            "recording": str(candidate.run.recording),
            "kind": candidate.kind,
            "label": candidate.label,
            "event_tick": candidate.event_tick,
            "start_tick": candidate.start_tick,
            "end_tick": candidate.end_tick,
            "reasons": candidate.reasons,
            "output_mp4": str(output_path),
            "elapsed_s": 0.0,
            "final_sustained": candidate.run.final_sustained,
            "peak_y": candidate.run.peak_y,
            "peak_gap": candidate.run.peak_gap,
            "skipped_existing": True,
        }
    command = [
        sys.executable,
        str(args.capture_script),
        replay_url,
        str(output_path),
        str(candidate.start_tick),
        str(candidate.end_tick),
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
        *DEFAULT_CAMERA_ARGS,
    ]
    started_at = time.time()
    subprocess.run(command, check=True)
    elapsed_s = time.time() - started_at
    return {
        "run_name": candidate.run.run_name,
        "recording": str(candidate.run.recording),
        "kind": candidate.kind,
        "label": candidate.label,
        "event_tick": candidate.event_tick,
        "start_tick": candidate.start_tick,
        "end_tick": candidate.end_tick,
        "reasons": candidate.reasons,
        "output_mp4": str(output_path),
        "elapsed_s": round(elapsed_s, 2),
        "final_sustained": candidate.run.final_sustained,
        "peak_y": candidate.run.peak_y,
        "peak_gap": candidate.run.peak_gap,
    }


def main() -> None:
    args = parse_args()
    runs_dir = args.runs_dir.resolve()
    output_dir = (
        args.output_dir.resolve()
        if args.output_dir is not None
        else (runs_dir / "clips" / "interesting").resolve()
    )
    output_dir.mkdir(parents=True, exist_ok=True)

    runs = discover_runs(runs_dir)
    if not runs:
        raise RuntimeError(f"No analyzed runs found under {runs_dir}")

    candidates = select_candidates(args, runs)
    manifest = {
        "runs_dir": str(runs_dir),
        "output_dir": str(output_dir),
        "selection": {
            "generation_improvement_threshold": args.generation_improvement_threshold,
            "transient_gap_threshold": args.transient_gap_threshold,
            "sustained_pre_ticks": args.sustained_pre_ticks,
            "sustained_post_ticks": args.sustained_post_ticks,
            "peak_pre_ticks": args.peak_pre_ticks,
            "peak_post_ticks": args.peak_post_ticks,
        },
        "runs": [
            {
                "run_name": run.run_name,
                "recording": str(run.recording),
                "final_sustained": run.final_sustained,
                "final_sustained_tick": run.final_sustained_tick,
                "peak_y": run.peak_y,
                "peak_tick": run.peak_tick,
                "peak_gap": run.peak_gap,
            }
            for run in runs
        ],
        "selected_candidates": [
            {
                "run_name": candidate.run.run_name,
                "kind": candidate.kind,
                "label": candidate.label,
                "event_tick": candidate.event_tick,
                "start_tick": candidate.start_tick,
                "end_tick": candidate.end_tick,
                "reasons": candidate.reasons,
            }
            for candidate in candidates
        ],
        "clips": [],
    }
    (output_dir / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")

    if args.dry_run:
        return

    for candidate in candidates:
        print(
            f"[clip] {candidate.label} "
            f"ticks={candidate.start_tick}-{candidate.end_tick} "
            f"reasons={'; '.join(candidate.reasons)}"
        )
        replay_process = None
        replay_port = choose_free_port()
        try:
            replay_process = launch_replay_server(args, candidate.run.recording, replay_port)
            clip_info = capture_candidate(args, candidate, output_dir, replay_port)
            manifest["clips"].append(clip_info)
            (output_dir / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
        finally:
            stop_process(replay_process)


if __name__ == "__main__":
    main()
