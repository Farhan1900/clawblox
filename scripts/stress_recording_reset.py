#!/usr/bin/env python3
import argparse
import glob
import sqlite3
import sys
import time

import requests


def parse_args():
    p = argparse.ArgumentParser(description="Stress test local recording across resets.")
    p.add_argument("--url", required=True, help="Base world URL, e.g. http://127.0.0.1:8102")
    p.add_argument("--record-dir", required=True, help="Directory containing the .clawrec")
    p.add_argument("--name", default="RecordingStress", help="Agent name for /join")
    p.add_argument("--iterations", type=int, default=300, help="Number of reset loop iterations")
    p.add_argument("--sample-every", type=int, default=10, help="Check DB every N iterations")
    p.add_argument("--gap-threshold-ms", type=int, default=2000, help="Fail if input time exceeds delta time by more than this")
    p.add_argument("--settle-ms", type=int, default=50, help="Small settle delay between action bursts")
    return p.parse_args()


def get_recording_path(record_dir):
    files = sorted(glob.glob(f"{record_dir}/*.clawrec"))
    if len(files) != 1:
        raise RuntimeError(f"expected exactly 1 .clawrec in {record_dir}, found {files}")
    return files[0]


def stats(recording_path):
    con = sqlite3.connect(recording_path)
    cur = con.cursor()
    max_delta = cur.execute("select coalesce(max(time_ms), 0) from deltas").fetchone()[0]
    max_input = cur.execute("select coalesce(max(time_ms), 0) from inputs").fetchone()[0]
    checkpoints = cur.execute("select count(*) from checkpoints").fetchone()[0]
    deltas = cur.execute("select count(*) from deltas").fetchone()[0]
    inputs = cur.execute("select count(*) from inputs").fetchone()[0]
    con.close()
    return {
        "max_delta_ms": max_delta,
        "max_input_ms": max_input,
        "checkpoint_count": checkpoints,
        "delta_count": deltas,
        "input_count": inputs,
    }


def main():
    args = parse_args()
    url = args.url.rstrip("/")
    join = requests.post(f"{url}/join?name={args.name}", timeout=10)
    join.raise_for_status()
    session = join.json()["session"]
    headers = {"X-Session": session}
    json_headers = {"X-Session": session, "Content-Type": "application/json"}

    recording_path = None
    for _ in range(50):
        try:
            recording_path = get_recording_path(args.record_dir)
            break
        except Exception:
            time.sleep(0.1)
    if recording_path is None:
        raise RuntimeError("recording file not found")

    print(f"session={session}")
    print(f"recording={recording_path}")

    last = stats(recording_path)
    print(f"start={last}")

    for i in range(args.iterations):
        move_target = {
            "x": -12 + (i % 5),
            "y": 0,
            "z": ((i % 7) - 3) * 1.25,
        }
        requests.post(
            f"{url}/input",
            headers=json_headers,
            json={"type": "MoveTo", "data": move_target},
            timeout=10,
        ).raise_for_status()
        time.sleep(args.settle_ms / 1000.0)

        if i % 3 == 0:
            requests.post(f"{url}/reset", headers=headers, timeout=10).raise_for_status()
        else:
            requests.post(
                f"{url}/input",
                headers=json_headers,
                json={"type": "Reset", "data": {}},
                timeout=10,
            ).raise_for_status()

        if i % 7 == 0:
            requests.post(
                f"{url}/input",
                headers=json_headers,
                json={"type": "Jump", "data": {}},
                timeout=10,
            ).raise_for_status()

        if i % args.sample_every == 0:
            current = stats(recording_path)
            gap = current["max_input_ms"] - current["max_delta_ms"]
            print(f"iter={i} stats={current} gap_ms={gap}")
            if gap > args.gap_threshold_ms:
                print(
                    f"FAIL: input/delta divergence exceeded threshold ({gap}ms > {args.gap_threshold_ms}ms)",
                    file=sys.stderr,
                )
                return 1
            if current["delta_count"] <= last["delta_count"] and current["input_count"] > last["input_count"]:
                print(
                    "FAIL: inputs advanced while deltas stalled",
                    file=sys.stderr,
                )
                return 1
            last = current
            time.sleep(args.settle_ms / 1000.0)

    final = stats(recording_path)
    gap = final["max_input_ms"] - final["max_delta_ms"]
    print(f"final={final} gap_ms={gap}")
    if gap > args.gap_threshold_ms:
        print(
            f"FAIL: final input/delta divergence exceeded threshold ({gap}ms > {args.gap_threshold_ms}ms)",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
