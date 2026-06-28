#!/usr/bin/env python3

import argparse
import json
import os
import shutil
import subprocess
import tempfile
import time
import urllib.request
from pathlib import Path


DEFAULT_SOURCE_SCRIPT = Path(
    "/home/naclo/Code/clawblox/worlds/mesa-island/eval-worlds/river-bridge/results/20260404T154729Z/worlds/world-0/agents/Eko-r20260404T154729Z-w0-a0/workspace/build_bridge.py"
)
DEFAULT_BASE_URL = "http://localhost:18100"
WORLD_DIR = Path(__file__).resolve().parents[1]
STARTUP_TIMEOUT_S = 30.0
WATCH_POLL_S = 0.2


def port_from_base_url(base_url):
    if not base_url.startswith("http://localhost:") and not base_url.startswith("http://127.0.0.1:"):
        raise ValueError(f"only localhost base URLs are supported, got {base_url}")
    return int(base_url.rsplit(":", 1)[1].rstrip("/"))


def find_repo_clawblox_bin():
    current = Path(__file__).resolve()
    for parent in current.parents:
        if (parent / "Cargo.toml").exists():
            for rel in ("target/release/clawblox", "target/debug/clawblox"):
                path = parent / rel
                if path.exists():
                    return path
    return None


def tail_text(path, limit=6000):
    if path is None or not path.exists():
        return ""
    text = path.read_text(errors="replace")
    return text if len(text) <= limit else text[-limit:]


def stop_process(process):
    if process is None:
        return
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)


def wait_for_http(base_url, timeout_s=STARTUP_TIMEOUT_S):
    deadline = time.time() + timeout_s
    last_error = None
    while time.time() < deadline:
        try:
            with urllib.request.urlopen(base_url + "/skill.md", timeout=2) as response:
                if response.status == 200:
                    return
        except Exception as exc:
            last_error = exc
        time.sleep(0.25)
    raise RuntimeError(f"world did not become ready at {base_url} within {timeout_s:.1f}s; last_error={last_error}")


def start_world(base_url):
    clawblox_bin = os.environ.get("CLAWBLOX_BIN")
    if not clawblox_bin:
        repo_bin = find_repo_clawblox_bin()
        clawblox_bin = str(repo_bin) if repo_bin is not None else "clawblox"
    port = port_from_base_url(base_url)
    record_dir = Path(tempfile.mkdtemp(prefix="river-bridge-build-bridge-crash-", dir=str(WORLD_DIR / ".clawblox")))
    log_dir = Path(tempfile.mkdtemp(prefix="river-bridge-build-bridge-crash-logs-", dir=str(WORLD_DIR / ".clawblox")))
    stdout_path = log_dir / "world.stdout.log"
    stderr_path = log_dir / "world.stderr.log"
    stdout_handle = stdout_path.open("wb")
    stderr_handle = stderr_path.open("wb")
    cmd = [clawblox_bin, "run", str(WORLD_DIR), "--port", str(port)]
    if os.environ.get("REPRO_RECORD", "0") == "1":
        cmd.extend(["--record", "--record-dir", str(record_dir)])
    process = subprocess.Popen(cmd, stdout=stdout_handle, stderr=stderr_handle)
    try:
        wait_for_http(base_url)
    except Exception:
        stop_process(process)
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


def reqj(base_url, session, path, method="GET", data=None, timeout=5):
    headers = {"X-Session": session}
    body = None
    if data is not None:
        body = json.dumps(data).encode()
        headers["Content-Type"] = "application/json"
    req = urllib.request.Request(base_url + path, data=body, headers=headers, method=method)
    with urllib.request.urlopen(req, timeout=timeout) as response:
        return json.load(response)


def join(base_url, name):
    with urllib.request.urlopen(base_url + f"/join?name={name}", data=b"", timeout=5) as response:
        return json.load(response)["session"]


def observe(base_url, session):
    return reqj(base_url, session, "/observe")


def rewrite_script(source_script, base_url, session, target_path):
    text = source_script.read_text()
    text = text.replace('URL = "http://localhost:8099"', f'URL = "{base_url}"')
    text = text.replace('"X-Session": "eddcbc49-4043-4e86-b1bb-b59fe553502e"', f'"X-Session": "{session}"')
    target_path.write_text(text)
    target_path.chmod(0o755)


def cleanup(artifacts, script_process=None, script_dir=None, keep=False):
    stop_process(script_process)
    stop_process(artifacts.get("process"))
    for key in ("stdout_handle", "stderr_handle"):
        handle = artifacts.get(key)
        if handle is not None and not handle.closed:
            handle.close()
    if not keep:
        for key in ("record_dir", "log_dir"):
            path = artifacts.get(key)
            if path is not None:
                shutil.rmtree(path, ignore_errors=True)
        if script_dir is not None:
            shutil.rmtree(script_dir, ignore_errors=True)


def parse_args():
    parser = argparse.ArgumentParser(description="Run the saved crashing build_bridge.py against a clean local world.")
    parser.add_argument("--base-url", default=DEFAULT_BASE_URL, help="Base URL for the local river-bridge server.")
    parser.add_argument("--source-script", type=Path, default=DEFAULT_SOURCE_SCRIPT, help="Path to the saved build_bridge.py to replay.")
    parser.add_argument("--session-name", default="build-bridge-crash-repro", help="Name to use when joining the world.")
    parser.add_argument("--keep-artifacts", action="store_true", help="Keep temp world logs/recordings and the rewritten script.")
    return parser.parse_args()


def main():
    args = parse_args()
    if not args.source_script.exists():
        raise FileNotFoundError(args.source_script)

    artifacts = start_world(args.base_url)
    script_process = None
    script_dir = None
    keep = args.keep_artifacts
    try:
        session = join(args.base_url, args.session_name)
        print("session", session, flush=True)
        print("world_pid", artifacts["process"].pid, flush=True)
        print("source_script", str(args.source_script), flush=True)

        script_dir = Path(tempfile.mkdtemp(prefix="river-bridge-build-bridge-script-", dir=str(WORLD_DIR / ".clawblox")))
        rewritten_script = script_dir / "build_bridge_runtime.py"
        rewrite_script(args.source_script, args.base_url, session, rewritten_script)
        print("rewritten_script", str(rewritten_script), flush=True)

        script_log = script_dir / "script.log"
        log_handle = script_log.open("wb")
        script_process = subprocess.Popen(
            ["python3", "-u", str(rewritten_script)],
            stdout=log_handle,
            stderr=subprocess.STDOUT,
            cwd=str(script_dir),
        )
        print("script_pid", script_process.pid, flush=True)

        while True:
            if artifacts["process"].poll() is not None:
                print("CRASH_REPRODUCED", {"world_exit": artifacts["process"].poll()}, flush=True)
                print("world_stdout_tail", tail_text(artifacts["stdout_path"]), flush=True)
                print("world_stderr_tail", tail_text(artifacts["stderr_path"]), flush=True)
                print("script_log_tail", tail_text(script_log), flush=True)
                keep = True
                return

            if script_process.poll() is not None:
                obs = observe(args.base_url, session)
                raise AssertionError(
                    {
                        "message": "saved build_bridge.py exited before crashing the world",
                        "script_exit": script_process.poll(),
                        "last_action": (obs.get("player") or {}).get("attributes", {}).get("LastActionResult"),
                        "player": (obs.get("player") or {}).get("position"),
                        "script_log_tail": tail_text(script_log),
                    }
                )

            time.sleep(WATCH_POLL_S)
    finally:
        cleanup(artifacts, script_process=script_process, script_dir=script_dir, keep=keep)


if __name__ == "__main__":
    main()
