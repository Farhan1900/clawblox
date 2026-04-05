#!/usr/bin/env python3

import argparse
import importlib.util
import json
import os
import re
import shutil
import subprocess
import tempfile
import time
import urllib.request
from pathlib import Path


REPRO_PATH = Path(__file__).with_name("repro_crash_long_bridge_exact.py")
spec = importlib.util.spec_from_file_location("repro_crash_long_bridge_exact", REPRO_PATH)
module = importlib.util.module_from_spec(spec)
assert spec is not None and spec.loader is not None
spec.loader.exec_module(module)

BASE = os.environ.get("REPRO_BASE_URL", module.BASE)
SESSION_NAME = module.SESSION_NAME
SOURCE_RECORDING = module.SOURCE_RECORDING
SOURCE_RESET = module.SOURCE_RESET
SOURCE_CUTOFF_TICK = module.SOURCE_CUTOFF_TICK
SOURCE_CUTOFF_TIME_S = module.SOURCE_CUTOFF_TIME_S
ACTIONS = module.ACTIONS

WORLD_DIR = Path(__file__).resolve().parents[1]
STARTUP_TIMEOUT_S = 30.0
WATCH_POLL_S = 0.1


def port_from_base_url(base_url):
    if not base_url.startswith("http://localhost:") and not base_url.startswith("http://127.0.0.1:"):
        raise ValueError(f"only localhost base URLs are supported, got {base_url}")
    return int(base_url.rsplit(":", 1)[1].rstrip("/"))


def find_repo_clawblox_bin():
    current = Path(__file__).resolve()
    for parent in current.parents:
        if (parent / "Cargo.toml").exists():
            for rel in ("target/debug/clawblox", "target/release/clawblox"):
                path = parent / rel
                if path.exists():
                    return path
    return None


def tail_text(path, limit=4000):
    if path is None or not path.exists():
        return ""
    text = path.read_text(errors="replace")
    return text if len(text) <= limit else text[-limit:]


def stop_world(process):
    if process is None:
        return
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)


def cleanup_artifacts(artifacts):
    if artifacts is None:
        return
    stop_world(artifacts.get("process"))
    for key in ("stdout_handle", "stderr_handle"):
        handle = artifacts.get(key)
        if handle is not None and not handle.closed:
            handle.close()
    for key in ("record_dir", "log_dir"):
        path = artifacts.get(key)
        if path is not None:
            shutil.rmtree(path, ignore_errors=True)


def wait_for_http(timeout_s=STARTUP_TIMEOUT_S):
    deadline = time.time() + timeout_s
    last_error = None
    while time.time() < deadline:
        try:
            with urllib.request.urlopen(BASE + "/skill.md", timeout=2) as response:
                if response.status == 200:
                    return
        except Exception as exc:
            last_error = exc
        time.sleep(0.25)
    raise RuntimeError(f"world did not become ready at {BASE} within {timeout_s:.1f}s; last_error={last_error}")


def start_world():
    clawblox_bin = os.environ.get("CLAWBLOX_BIN")
    if not clawblox_bin:
        repo_bin = find_repo_clawblox_bin()
        clawblox_bin = str(repo_bin) if repo_bin is not None else "clawblox"
    port = port_from_base_url(BASE)
    record_dir = Path(tempfile.mkdtemp(prefix="river-bridge-long-bridge-crash-", dir=str(WORLD_DIR / ".clawblox")))
    log_dir = Path(tempfile.mkdtemp(prefix="river-bridge-long-bridge-crash-logs-", dir=str(WORLD_DIR / ".clawblox")))
    stdout_path = log_dir / "world.stdout.log"
    stderr_path = log_dir / "world.stderr.log"
    stdout_handle = stdout_path.open("wb")
    stderr_handle = stderr_path.open("wb")
    process = subprocess.Popen(
        [clawblox_bin, "run", str(WORLD_DIR), "--port", str(port), "--record", "--record-dir", str(record_dir)],
        stdout=stdout_handle,
        stderr=stderr_handle,
    )
    try:
        wait_for_http()
    except Exception:
        stop_world(process)
        stdout_handle.close()
        stderr_handle.close()
        raise
    return {
        "process": process,
        "record_dir": record_dir,
        "log_dir": log_dir,
        "stdout_path": stdout_path,
        "stderr_path": stderr_path,
        "stdout_handle": stdout_handle,
        "stderr_handle": stderr_handle,
    }


def reqj(session, path, method="GET", data=None, timeout=5):
    headers = {"X-Session": session}
    body = None
    if data is not None:
        body = json.dumps(data).encode()
        headers["Content-Type"] = "application/json"
    req = urllib.request.Request(BASE + path, data=body, headers=headers, method=method)
    with urllib.request.urlopen(req, timeout=timeout) as response:
        return json.load(response)


def join(name=SESSION_NAME):
    with urllib.request.urlopen(BASE + f"/join?name={name}", data=b"", timeout=5) as response:
        return json.load(response)["session"]


def action(session, kind, data):
    return reqj(session, "/input", method="POST", data={"type": kind, "data": data})


def observe(session):
    return reqj(session, "/observe")


def last_action_result(obs):
    return (obs.get("player") or {}).get("attributes", {}).get("LastActionResult")


def parse_args():
    parser = argparse.ArgumentParser(description="Replay the exact long-bridge crash sequence from the latest recording.")
    parser.add_argument(
        "--base-url",
        default=BASE,
        help="Base URL for the local river-bridge server to use for this repro.",
    )
    parser.add_argument(
        "--delay-scale",
        type=float,
        default=float(os.environ.get("REPRO_DELAY_SCALE", "0.2")),
        help="Scale recorded delays by this factor. Default 0.2 to preserve timing shape while cutting long idle gaps.",
    )
    parser.add_argument(
        "--min-delay",
        type=float,
        default=float(os.environ.get("REPRO_MIN_DELAY", "0.05")),
        help="Minimum delay to apply between recorded actions that originally had a non-zero gap.",
    )
    parser.add_argument(
        "--max-delay",
        type=float,
        default=float(os.environ.get("REPRO_MAX_DELAY", "2.0")),
        help="Maximum delay to apply between actions after scaling. Keeps long recorded waits from dominating runtime.",
    )
    parser.add_argument(
        "--keep-artifacts",
        action="store_true",
        help="Keep recording/log directories after the run.",
    )
    return parser.parse_args()


def parse_reset_time_s(label):
    match = re.search(r",\s*([0-9]+(?:\.[0-9]+)?)s$", label)
    if not match:
        return None
    return float(match.group(1))


def main():
    args = parse_args()
    global BASE
    BASE = args.base_url
    artifacts = start_world()
    keep = args.keep_artifacts
    try:
        session = join()
        print("session", session)
        print("pid", artifacts["process"].pid)
        print("recording", SOURCE_RECORDING)
        print("sequence_start", SOURCE_RESET)
        print("sequence_end", {"tick": SOURCE_CUTOFF_TICK, "time_s": SOURCE_CUTOFF_TIME_S})
        print("actions", len(ACTIONS))

        reset_obs = action(session, "Reset", {})
        print("boundary_reset", {"result": last_action_result(reset_obs)})

        reset_time_s = parse_reset_time_s(SOURCE_RESET)
        if reset_time_s is not None and ACTIONS:
            initial_delay_s = max(0.0, ACTIONS[0]["time_ms"] / 1000.0 - reset_time_s)
            if initial_delay_s > 0:
                print("initial_delay_s", initial_delay_s)
                time.sleep(initial_delay_s * args.delay_scale)

        for index, item in enumerate(ACTIONS, start=1):
            raw_delay_s = item["delay_s"] * args.delay_scale
            if item["delay_s"] > 0:
                delay_s = max(args.min_delay, min(args.max_delay, raw_delay_s))
            else:
                delay_s = 0.0
            if delay_s > 0:
                time.sleep(delay_s)
            try:
                action_obs = action(session, item["type"], item["data"])
            except Exception as exc:
                print("CRASH_REPRODUCED", {"step": index, "phase": "submit", "failure": f"{type(exc).__name__}: {exc}"})
                print("world_stdout_tail", tail_text(artifacts["stdout_path"]))
                print("world_stderr_tail", tail_text(artifacts["stderr_path"]))
                keep = True
                return

            print(
                "step",
                index,
                {
                    "tick": item["tick"],
                    "time_s": round(item["time_ms"] / 1000.0, 3),
                    "type": item["type"],
                    "result": last_action_result(action_obs),
                },
            )

        deadline = time.time() + 10.0
        last_error = None
        while time.time() < deadline:
            try:
                obs = observe(session)
                print("post_replay", {"result": last_action_result(obs), "player": (obs.get("player") or {}).get("position")})
            except Exception as exc:
                print("CRASH_REPRODUCED", {"phase": "post-replay-watch", "failure": f"{type(exc).__name__}: {exc}"})
                print("world_stdout_tail", tail_text(artifacts["stdout_path"]))
                print("world_stderr_tail", tail_text(artifacts["stderr_path"]))
                keep = True
                return
            time.sleep(WATCH_POLL_S)
        raise AssertionError("expected the exact long-bridge replay to crash the server, but the server survived replay and post-watch")
    finally:
        if not keep:
            cleanup_artifacts(artifacts)
        else:
            stop_world(artifacts.get("process"))
            for key in ("stdout_handle", "stderr_handle"):
                handle = artifacts.get(key)
                if handle is not None and not handle.closed:
                    handle.close()


if __name__ == "__main__":
    main()
