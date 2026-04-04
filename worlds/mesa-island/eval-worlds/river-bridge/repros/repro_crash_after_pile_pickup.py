#!/usr/bin/env python3

import argparse
import json
import os
import shutil
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path


BASE = "http://localhost:8099"
SESSION_NAME = "river-bridge-pile-crash"
SOURCE_RECORDING = "/home/naclo/Code/clawblox/worlds/mesa-island/eval-worlds/river-bridge/results/20260404T010118Z/worlds/world-0/recordings/20260404_010120.clawrec"
SOURCE_RESET = {"tick": 8828, "time_s": 142.922}
SOURCE_CUTOFF = {"tick": 18652, "time_s": 301.834}
WORLD_DIR = Path(__file__).resolve().parents[1]
STARTUP_TIMEOUT_S = 30.0
ACTION_TIMEOUT_S = 30.0
POST_PICKUP_WATCH_S = 3.0
WATCH_POLL_S = 0.1

INPUTS = [
    # Start from a fresh world; this is equivalent to the post-reset baseline from
    # the source recording, so we do not replay the Reset input itself.
    {"tick": 9705, "time_ms": 157133, "delay_s": 0.0, "type": "Pickup", "data": {"position": [-8.5, 0.19, 14.0]}},
    {"tick": 11195, "time_ms": 181239, "delay_s": 24.106, "type": "MoveTo", "data": {"position": [-8.0, 0.0, 13.0]}},
    {"tick": 11729, "time_ms": 189865, "delay_s": 8.626, "type": "Pickup", "data": {"position": [-8.0, 0.19, 14.0]}},
    {"tick": 12276, "time_ms": 198716, "delay_s": 8.851, "type": "MoveTo", "data": {"position": [-19.0, 0.0, -14.0]}},
    {
        "tick": 12939,
        "time_ms": 209432,
        "delay_s": 10.716,
        "type": "Place",
        "data": {"position": [-22.0, 0.75, -14.0], "rotation": {"pitch": 0, "roll": 0, "yaw": 0}},
    },
    {"tick": 15549, "time_ms": 251642, "delay_s": 42.21, "type": "MoveTo", "data": {"position": [-8.0, 0.0, 13.0]}},
    {"tick": 15861, "time_ms": 256696, "delay_s": 5.054, "type": "Pickup", "data": {"position": [-8.0, 0.19, 14.0]}},
    {"tick": 16398, "time_ms": 265380, "delay_s": 8.684, "type": "MoveTo", "data": {"position": [-19.0, 0.0, -14.0]}},
    {
        "tick": 16771,
        "time_ms": 271416,
        "delay_s": 6.036,
        "type": "Place",
        "data": {"position": [-22.0, 0.37, -14.0], "rotation": {"pitch": 0, "roll": 0, "yaw": 0}},
    },
    {"tick": 18652, "time_ms": 301834, "delay_s": 30.418, "type": "Pickup", "data": {"position": [-21.2, 0.37, -14.0]}},
]


def port_from_base_url(base_url):
    if not base_url.startswith("http://localhost:") and not base_url.startswith("http://127.0.0.1:"):
        raise ValueError(f"only localhost base URLs are supported, got {base_url}")
    return int(base_url.rsplit(":", 1)[1].rstrip("/"))


def find_repo_clawblox_bin():
    current = Path(__file__).resolve()
    for parent in current.parents:
        cargo_toml = parent / "Cargo.toml"
        debug_bin = parent / "target" / "debug" / "clawblox"
        release_bin = parent / "target" / "release" / "clawblox"
        if cargo_toml.exists():
            if debug_bin.exists():
                return debug_bin
            if release_bin.exists():
                return release_bin
    return None


def can_connect():
    try:
        with urllib.request.urlopen(BASE + "/skill.md", timeout=1.5) as response:
            return response.status == 200
    except Exception:
        return False


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


def tail_text(path, limit=4000):
    if path is None or not path.exists():
        return ""
    text = path.read_text(errors="replace")
    if len(text) <= limit:
        return text
    return text[-limit:]


def capture_process_backtrace(process):
    if process is None:
        return None
    pid = process.pid
    if process.poll() is not None:
        return f"process already exited with status {process.poll()}"
    try:
        result = subprocess.run(
            ["gdb", "-batch", "-ex", "thread apply all bt", "-p", str(pid)],
            capture_output=True,
            text=True,
            timeout=10,
            check=False,
        )
    except FileNotFoundError:
        return "gdb not found"
    except subprocess.TimeoutExpired:
        return "gdb backtrace timed out"
    combined = []
    if result.stdout:
        combined.append(result.stdout)
    if result.stderr:
        combined.append(result.stderr)
    output = "".join(combined).strip()
    return output or f"gdb exited with code {result.returncode} and produced no output"


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
    stdout_handle = artifacts.get("stdout_handle")
    stderr_handle = artifacts.get("stderr_handle")
    if stdout_handle is not None and not stdout_handle.closed:
        stdout_handle.close()
    if stderr_handle is not None and not stderr_handle.closed:
        stderr_handle.close()
    record_dir = artifacts.get("record_dir")
    log_dir = artifacts.get("log_dir")
    if record_dir is not None:
        shutil.rmtree(record_dir, ignore_errors=True)
    if log_dir is not None:
        shutil.rmtree(log_dir, ignore_errors=True)


def start_world():
    if can_connect():
        print("world", f"reusing existing server at {BASE}")
        return {
            "process": None,
            "record_dir": None,
            "log_dir": None,
            "stdout_path": None,
            "stderr_path": None,
            "stdout_handle": None,
            "stderr_handle": None,
            "reused_existing": True,
        }
    clawblox_bin = os.environ.get("CLAWBLOX_BIN")
    if not clawblox_bin:
        repo_bin = find_repo_clawblox_bin()
        clawblox_bin = str(repo_bin) if repo_bin is not None else "clawblox"
    port = port_from_base_url(BASE)
    record_dir = Path(tempfile.mkdtemp(prefix="river-bridge-crash-repro-", dir=str(WORLD_DIR / ".clawblox")))
    log_dir = Path(tempfile.mkdtemp(prefix="river-bridge-crash-repro-logs-", dir=str(WORLD_DIR / ".clawblox")))
    stdout_path = log_dir / "world.stdout.log"
    stderr_path = log_dir / "world.stderr.log"
    stdout_handle = stdout_path.open("wb")
    stderr_handle = stderr_path.open("wb")
    print("world", f"starting local server at {BASE} using {clawblox_bin}")
    print("record_dir", str(record_dir))
    print("log_dir", str(log_dir))
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
        "reused_existing": False,
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
    players = obs.get("players")
    if players:
        return players[0].get("attributes", {}).get("LastActionResult")
    return obs.get("player", {}).get("attributes", {}).get("LastActionResult")


def wait_for_result(session, predicate, timeout_s=ACTION_TIMEOUT_S):
    deadline = time.time() + timeout_s
    last = None
    while time.time() < deadline:
        obs = observe(session)
        last = last_action_result(obs)
        if predicate(last):
            return obs
        time.sleep(0.05)
    raise RuntimeError(f"timeout waiting for action result, last={last!r}")


def wait_for_action_transition(session, previous_result, timeout_s=ACTION_TIMEOUT_S):
    if previous_result is None:
        return observe(session)
    return wait_for_result(session, lambda result: result != previous_result, timeout_s=timeout_s)


def wait_for_action_completion(session, action_type, previous_result, immediate_result):
    if action_type == "MoveTo":
        if previous_result is None:
            if isinstance(immediate_result, str) and immediate_result.startswith("MoveToFinished"):
                return observe(session)
            return wait_for_result(session, lambda result: isinstance(result, str) and result.startswith("MoveToFinished"))
        transitioned_obs = wait_for_action_transition(session, previous_result)
        transitioned_result = last_action_result(transitioned_obs)
        if isinstance(transitioned_result, str) and transitioned_result.startswith("MoveToFinished"):
            return transitioned_obs
        return wait_for_result(session, lambda result: isinstance(result, str) and result.startswith("MoveToFinished"))
    if action_type == "Pickup":
        if previous_result is None:
            if isinstance(immediate_result, str) and immediate_result.startswith("Pickup"):
                return observe(session)
            return wait_for_result(session, lambda result: isinstance(result, str) and result.startswith("Pickup"))
        transitioned_obs = wait_for_action_transition(session, previous_result)
        transitioned_result = last_action_result(transitioned_obs)
        if isinstance(transitioned_result, str) and transitioned_result.startswith("Pickup"):
            return transitioned_obs
        return wait_for_result(session, lambda result: isinstance(result, str) and result.startswith("Pickup"))
    if action_type == "Place":
        if previous_result is None:
            if isinstance(immediate_result, str) and immediate_result.startswith("Place"):
                return observe(session)
            return wait_for_result(session, lambda result: isinstance(result, str) and result.startswith("Place"))
        transitioned_obs = wait_for_action_transition(session, previous_result)
        transitioned_result = last_action_result(transitioned_obs)
        if isinstance(transitioned_result, str) and transitioned_result.startswith("Place"):
            return transitioned_obs
        return wait_for_result(session, lambda result: isinstance(result, str) and result.startswith("Place"))
    if action_type == "Reset":
        if previous_result is None:
            return wait_for_result(session, lambda result: result == "ResetSuccess")
        return wait_for_result(session, lambda result: result == "ResetSuccess" and result != previous_result)
    return observe(session)


def server_alive():
    try:
        with urllib.request.urlopen(BASE + "/skill.md", timeout=1.0) as response:
            return response.status == 200
    except Exception:
        return False


def fetch_skill_md_status():
    try:
        with urllib.request.urlopen(BASE + "/skill.md", timeout=1.0) as response:
            return f"http_{response.status}"
    except Exception as exc:
        return f"{type(exc).__name__}: {exc}"


def lifted_blocks(obs):
    entities = obs.get("entities")
    if entities is None:
        entities = (obs.get("world") or {}).get("entities", [])
    blocks = []
    for entity in entities:
        attrs = entity.get("attributes") or {}
        if attrs.get("CarryKind") != "Block":
            continue
        pos = entity.get("position") or [None, None, None]
        if pos[1] is not None and pos[1] > 2.0:
            blocks.append((entity.get("name"), pos))
    return blocks


def player_position(obs):
    players = obs.get("players") or []
    if players:
        return players[0].get("position")
    return (obs.get("player") or {}).get("position")


def parse_args():
    parser = argparse.ArgumentParser(description="Replay the post-reset pile pickup crash repro.")
    parser.add_argument(
        "--delay-scale",
        type=float,
        default=float(os.environ.get("REPRO_DELAY_SCALE", "0.0")),
        help="Scale recorded delays by this factor; default is 0.0 with event-driven waiting.",
    )
    parser.add_argument(
        "--watch-seconds",
        type=float,
        default=POST_PICKUP_WATCH_S,
        help="How long to watch for a crash after the final pickup.",
    )
    parser.add_argument(
        "--attempts",
        type=int,
        default=1,
        help="How many fresh-world attempts to run before giving up.",
    )
    parser.add_argument(
        "--keep-success-artifacts",
        action="store_true",
        help="Keep recording/log directories even for surviving attempts.",
    )
    parser.add_argument(
        "--keep-crashed-process",
        action="store_true",
        help="Do not terminate a wedged/crashed process after a failing attempt; useful for attaching a debugger.",
    )
    parser.add_argument(
        "--pause-after-crash-seconds",
        type=float,
        default=0.0,
        help="After detecting a failing attempt, sleep for this many seconds before exiting so you can attach a debugger.",
    )
    return parser.parse_args()


def run_attempt(args, attempt_index):
    artifacts = start_world()
    preserve_artifacts = False
    try:
        session = join()
        process = artifacts["process"]
        record_dir = artifacts["record_dir"]
        log_dir = artifacts["log_dir"]
        print("session", session)
        if process is not None:
            print("pid", process.pid)
        print("recording", SOURCE_RECORDING)
        print("sequence_start", {"from_fresh_world_equivalent_to": SOURCE_RESET})
        print("sequence_end", SOURCE_CUTOFF)
        print("actions", len(INPUTS))
        print("attempt", attempt_index)

        previous_result = None
        final_obs = None
        for index, item in enumerate(INPUTS, start=1):
            delay_s = item["delay_s"] * args.delay_scale
            if delay_s > 0:
                time.sleep(delay_s)
            action_obs = action(session, item["type"], item["data"])
            immediate_result = last_action_result(action_obs)
            completed_obs = wait_for_action_completion(session, item["type"], previous_result, immediate_result)
            result = last_action_result(completed_obs)
            previous_result = result
            final_obs = completed_obs
            print(
                "step",
                index,
                {"tick": item["tick"], "time_s": round(item["time_ms"] / 1000.0, 3), "type": item["type"], "result": result},
            )

        if final_obs is None:
            raise RuntimeError("no actions were replayed")

        lifted = lifted_blocks(final_obs)
        print("post_pickup_player", player_position(final_obs))
        print("post_pickup_action", last_action_result(final_obs))
        print("post_pickup_lifted_block_count", len(lifted))
        print("post_pickup_lifted_blocks", lifted)

        deadline = time.time() + args.watch_seconds
        while time.time() < deadline:
            try:
                obs = observe(session)
            except Exception as exc:
                backtrace = capture_process_backtrace(process)
                if process is not None:
                    stdout_handle = artifacts.get("stdout_handle")
                    stderr_handle = artifacts.get("stderr_handle")
                    if stdout_handle is not None and not stdout_handle.closed:
                        stdout_handle.flush()
                    if stderr_handle is not None and not stderr_handle.closed:
                        stderr_handle.flush()
                print(
                    "CRASH_REPRODUCED",
                    {
                        "attempt": attempt_index,
                        "watch_seconds": args.watch_seconds,
                        "failure": f"{type(exc).__name__}: {exc}",
                        "skill_md_status": fetch_skill_md_status(),
                        "process_status": None if process is None else process.poll(),
                        "record_dir": None if record_dir is None else str(record_dir),
                        "log_dir": None if log_dir is None else str(log_dir),
                    },
                )
                if artifacts.get("stderr_path") is not None:
                    print("world_stderr_tail", tail_text(artifacts["stderr_path"]))
                if artifacts.get("stdout_path") is not None:
                    print("world_stdout_tail", tail_text(artifacts["stdout_path"]))
                if backtrace:
                    print("process_backtrace", backtrace)
                preserve_artifacts = True
                if args.pause_after_crash_seconds > 0:
                    print("pause_after_crash_seconds", args.pause_after_crash_seconds)
                    time.sleep(args.pause_after_crash_seconds)
                return True
            if not server_alive():
                backtrace = capture_process_backtrace(process)
                if process is not None:
                    stdout_handle = artifacts.get("stdout_handle")
                    stderr_handle = artifacts.get("stderr_handle")
                    if stdout_handle is not None and not stdout_handle.closed:
                        stdout_handle.flush()
                    if stderr_handle is not None and not stderr_handle.closed:
                        stderr_handle.flush()
                print(
                    "CRASH_REPRODUCED",
                    {
                        "attempt": attempt_index,
                        "watch_seconds": args.watch_seconds,
                        "failure": "server_unreachable",
                        "skill_md_status": fetch_skill_md_status(),
                        "process_status": None if process is None else process.poll(),
                        "record_dir": None if record_dir is None else str(record_dir),
                        "log_dir": None if log_dir is None else str(log_dir),
                    },
                )
                if artifacts.get("stderr_path") is not None:
                    print("world_stderr_tail", tail_text(artifacts["stderr_path"]))
                if artifacts.get("stdout_path") is not None:
                    print("world_stdout_tail", tail_text(artifacts["stdout_path"]))
                if backtrace:
                    print("process_backtrace", backtrace)
                preserve_artifacts = True
                if args.pause_after_crash_seconds > 0:
                    print("pause_after_crash_seconds", args.pause_after_crash_seconds)
                    time.sleep(args.pause_after_crash_seconds)
                return True
            time.sleep(WATCH_POLL_S)

        obs = observe(session)
        print("server_survived", {"watch_seconds": args.watch_seconds, "last_action": last_action_result(obs)})
        return False
    finally:
        if artifacts.get("reused_existing"):
            return
        if preserve_artifacts and args.keep_crashed_process:
            stdout_handle = artifacts.get("stdout_handle")
            stderr_handle = artifacts.get("stderr_handle")
            if stdout_handle is not None and not stdout_handle.closed:
                stdout_handle.close()
            if stderr_handle is not None and not stderr_handle.closed:
                stderr_handle.close()
        elif not preserve_artifacts and not args.keep_success_artifacts:
            cleanup_artifacts(artifacts)
        else:
            stop_world(artifacts.get("process"))
            stdout_handle = artifacts.get("stdout_handle")
            stderr_handle = artifacts.get("stderr_handle")
            if stdout_handle is not None and not stdout_handle.closed:
                stdout_handle.close()
            if stderr_handle is not None and not stderr_handle.closed:
                stderr_handle.close()


def main():
    args = parse_args()
    for attempt_index in range(1, args.attempts + 1):
        crashed = run_attempt(args, attempt_index)
        if crashed:
            raise AssertionError(
                f"expected the server to remain healthy after the pile pickup, but attempt {attempt_index} became unavailable or unresponsive"
            )
    print(
        "PASS",
        {
            "attempts": args.attempts,
            "expectation": "successful 12-block pile pickup with server survival",
        },
    )


if __name__ == "__main__":
    main()
