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


WORLD_DIR = Path(__file__).resolve().parents[1]
SOURCE_V2_SCRIPT = Path(
    "/home/naclo/Code/clawblox/worlds/mesa-island/eval-worlds/river-bridge/results/20260404T154729Z/worlds/world-0/agents/Eko-r20260404T154729Z-w0-a0/workspace/build_bridge.py"
)
DEFAULT_BASE_URL = "http://localhost:18112"
STARTUP_TIMEOUT_S = 30.0
POLL_S = 0.2


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


def req(base_url, path, headers=None, data=None, timeout=10):
    headers = dict(headers or {})
    body = None
    if data is not None:
        body = json.dumps(data).encode()
        headers["Content-Type"] = "application/json"
    request = urllib.request.Request(base_url + path, data=body, headers=headers)
    with urllib.request.urlopen(request, timeout=timeout) as response:
        return response.read().decode()


def reqj(base_url, path, headers=None, data=None, timeout=10):
    return json.loads(req(base_url, path, headers=headers, data=data, timeout=timeout))


def wait_for_http(base_url, timeout_s=STARTUP_TIMEOUT_S):
    deadline = time.time() + timeout_s
    last_error = None
    while time.time() < deadline:
        try:
            if req(base_url, "/skill.md", timeout=2):
                return
        except Exception as exc:
            last_error = exc
        time.sleep(0.25)
    raise RuntimeError(f"world did not become ready at {base_url} within {timeout_s:.1f}s; last_error={last_error}")


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


def start_world(base_url):
    clawblox_bin = os.environ.get("CLAWBLOX_BIN")
    if not clawblox_bin:
        repo_bin = find_repo_clawblox_bin()
        clawblox_bin = str(repo_bin) if repo_bin is not None else "clawblox"
    port = port_from_base_url(base_url)
    log_dir = Path(tempfile.mkdtemp(prefix="river-bridge-exact-repro-", dir=str(WORLD_DIR / ".clawblox")))
    stdout_path = log_dir / "world.stdout.log"
    stderr_path = log_dir / "world.stderr.log"
    stdout_handle = stdout_path.open("wb")
    stderr_handle = stderr_path.open("wb")
    cmd = [clawblox_bin, "run", str(WORLD_DIR), "--port", str(port)]
    if os.environ.get("REPRO_RECORD", "0") == "1":
        record_dir = Path(tempfile.mkdtemp(prefix="river-bridge-exact-rec-", dir=str(WORLD_DIR / ".clawblox")))
        cmd.extend(["--record", "--record-dir", str(record_dir)])
    else:
        record_dir = None
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
        "log_dir": log_dir,
        "record_dir": record_dir,
        "stdout_path": stdout_path,
        "stderr_path": stderr_path,
        "stdout_handle": stdout_handle,
        "stderr_handle": stderr_handle,
    }


def rewrite_runtime_script(source_script, base_url, session, target_path):
    text = source_script.read_text()
    text = text.replace('URL = "http://localhost:8099"', f'URL = "{base_url}"')
    text = text.replace('"X-Session": "eddcbc49-4043-4e86-b1bb-b59fe553502e"', f'"X-Session": "{session}"')
    target_path.write_text(text)
    target_path.chmod(0o755)


def write_followup_script(base_url, session, target_path):
    target_path.write_text(
        f"""#!/usr/bin/env python3
import requests
import time

URL = "{base_url}"
H = {{"X-Session": "{session}", "Content-Type": "application/json"}}

def act(t, d=None):
    requests.post(f"{{URL}}/input", headers=H, json={{"type": t, "data": d or {{}}}})

def obs():
    return requests.get(f"{{URL}}/observe", headers=H).json()

def wait(timeout=25):
    time.sleep(0.3)
    prev = None
    for _ in range(timeout * 4):
        o = obs()
        r = o["player"]["attributes"]["LastActionResult"]
        if r != prev and any(k in r for k in ("Finished", "Success", "Failed")):
            return o, r
        prev = r
        time.sleep(0.25)
    o = obs()
    return o, o["player"]["attributes"]["LastActionResult"]

def do(action, data=None, label=""):
    act(action, data)
    o, r = wait()
    print(f"  {{label}}: {{r}}", flush=True)
    return o, r

o = obs()
bridge = [e for e in o["world"]["entities"] if "Block" in e["name"] and abs(e["position"][2]) < 1 and not e.get("anchored", False) and e["position"][1] < 2]
bridge.sort(key=lambda e: e["position"][0])
print(f"Bridge: {{len(bridge)}} blocks, x={{bridge[0]['position'][0]:.1f}} to {{bridge[-1]['position'][0]:.1f}}", flush=True)

west = bridge[0]
wx, wy, wz = west["position"]
print(f"\\nPicking up from west end: {{west['name']}} at x={{wx:.2f}}", flush=True)
do("MoveTo", {{"position": [wx, 2.55, wz - 3]}}, "Move to west end")
do("Pickup", {{"position": [wx, wy, wz]}}, "Pickup west end")

o = obs()
held = [e for e in o["world"]["entities"] if "Block" in e["name"] and e.get("anchored", False) and e["position"][1] > 3]
print(f"Blocks held: {{len(held)}}", flush=True)

print("\\nPlacing bridge across river...", flush=True)
do("MoveTo", {{"position": [-13.0, 2.55, 3.0]}}, "Move to placement")
do("Place", {{"position": [-13.0, 0.37, 0.0], "rotation": {{"yaw": 0, "pitch": 0, "roll": 0}}}}, "Place bridge")

o = obs()
bridge = [e for e in o["world"]["entities"] if "Block" in e["name"] and abs(e["position"][2]) < 1 and not e.get("anchored", False) and e["position"][1] < 2]
if bridge:
    bridge.sort(key=lambda e: e["position"][0])
    print(f"\\nBridge now: {{len(bridge)}} blocks, x={{bridge[0]['position'][0]:.1f}} to {{bridge[-1]['position'][0]:.1f}}", flush=True)

print("\\nCrossing...", flush=True)
do("MoveTo", {{"position": [20.0, 2.55, 0.0]}}, "Cross bridge")
o = obs()
print(f"Position: {{o['player']['position']}}", flush=True)
print(f"Game status: {{o['game_status']}}", flush=True)
"""
    )
    target_path.chmod(0o755)


def tail_text(path, limit=8000):
    if path is None or not path.exists():
        return ""
    text = path.read_text(errors="replace")
    return text if len(text) <= limit else text[-limit:]


def world_panicked(stderr_path):
    if stderr_path is None or not stderr_path.exists():
        return False
    text = stderr_path.read_text(errors="replace")
    return "[FATAL][instance_tick]" in text or "No element at index" in text


def parse_args():
    parser = argparse.ArgumentParser(description="Replay the exact transcript sequence that crashed the river-bridge world.")
    parser.add_argument("--base-url", default=DEFAULT_BASE_URL)
    parser.add_argument("--source-script", type=Path, default=SOURCE_V2_SCRIPT)
    parser.add_argument("--keep-artifacts", action="store_true")
    return parser.parse_args()


def main():
    args = parse_args()
    artifacts = start_world(args.base_url)
    work_dir = Path(tempfile.mkdtemp(prefix="river-bridge-exact-work-", dir=str(WORLD_DIR / ".clawblox")))
    build_process = None
    follow_process = None
    keep = args.keep_artifacts
    try:
        session = reqj(args.base_url, "/join?name=exact-crash-repro", data="", timeout=5)["session"]
        print("session", session, flush=True)
        print("world_pid", artifacts["process"].pid, flush=True)
        print("log_dir", artifacts["log_dir"], flush=True)
        if artifacts["record_dir"] is not None:
            print("record_dir", artifacts["record_dir"], flush=True)

        runtime_script = work_dir / "build_bridge_runtime.py"
        follow_script = work_dir / "followup_exact.py"
        build_log = work_dir / "build.log"
        follow_log = work_dir / "follow.log"

        rewrite_runtime_script(args.source_script, args.base_url, session, runtime_script)
        write_followup_script(args.base_url, session, follow_script)

        with build_log.open("wb") as build_handle:
            build_process = subprocess.Popen(
                ["python3", "-u", str(runtime_script)],
                stdout=build_handle,
                stderr=subprocess.STDOUT,
                cwd=str(work_dir),
            )
            while True:
                if artifacts["process"].poll() is not None:
                    print("CRASH_REPRODUCED_DURING_BUILD", flush=True)
                    print(tail_text(build_log), flush=True)
                    print(tail_text(artifacts["stderr_path"]), flush=True)
                    keep = True
                    return
                if build_process.poll() is not None:
                    break
                time.sleep(POLL_S)

        print("build_exit", build_process.poll(), flush=True)
        print(tail_text(build_log), flush=True)
        if build_process.poll() != 0:
            raise AssertionError({"message": "build stage failed", "build_log": tail_text(build_log)})

        with follow_log.open("wb") as follow_handle:
            follow_process = subprocess.Popen(
                ["python3", "-u", str(follow_script)],
                stdout=follow_handle,
                stderr=subprocess.STDOUT,
                cwd=str(work_dir),
            )
            while True:
                if artifacts["process"].poll() is not None:
                    print("CRASH_REPRODUCED", flush=True)
                    print(tail_text(follow_log), flush=True)
                    print(tail_text(artifacts["stderr_path"]), flush=True)
                    keep = True
                    return
                if follow_process.poll() is not None:
                    break
                time.sleep(POLL_S)

        if world_panicked(artifacts["stderr_path"]):
            print("CRASH_REPRODUCED", flush=True)
            print(tail_text(follow_log), flush=True)
            print(tail_text(artifacts["stderr_path"]), flush=True)
            keep = True
            return

        print("follow_exit", follow_process.poll(), flush=True)
        print(tail_text(follow_log), flush=True)
        raise AssertionError(
            {
                "message": "exact transcript sequence finished without crashing the world",
                "follow_log": tail_text(follow_log),
                "world_log": tail_text(artifacts["stderr_path"]),
            }
        )
    finally:
        stop_process(follow_process)
        stop_process(build_process)
        stop_process(artifacts["process"])
        for key in ("stdout_handle", "stderr_handle"):
            handle = artifacts.get(key)
            if handle is not None and not handle.closed:
                handle.close()
        if not keep:
            shutil.rmtree(work_dir, ignore_errors=True)
            shutil.rmtree(artifacts["log_dir"], ignore_errors=True)
            if artifacts["record_dir"] is not None:
                shutil.rmtree(artifacts["record_dir"], ignore_errors=True)


if __name__ == "__main__":
    main()
