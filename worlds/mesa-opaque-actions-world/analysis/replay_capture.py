#!/usr/bin/env python3
import argparse
import bisect
import csv
import base64
import json
import math
import os
import shutil
import signal
import socket
import subprocess
import time
import urllib.parse
import urllib.request
from pathlib import Path

import websocket

CHROME = Path(
    "/afs/csail.mit.edu/u/n/nacloos/.cache/ms-playwright/chromium-1200/chrome-linux64/chrome"
)
FFMPEG = Path(
    "/afs/csail.mit.edu/u/n/nacloos/ffmpeg_build/ffmpeg-7.0.2-amd64-static/ffmpeg"
)


def parse_args():
    parser = argparse.ArgumentParser(
        description="Capture a replay clip from mesa-world with hidden player-follow camera support."
    )
    parser.add_argument("replay_url", help="Replay server URL, for example http://localhost:10081/")
    parser.add_argument("output_mp4", help="Path to output MP4")
    parser.add_argument("start_tick", type=int, help="Inclusive start tick")
    parser.add_argument("end_tick", type=int, help="Inclusive end tick")
    parser.add_argument("--width", type=int, default=1280, help="Viewport width")
    parser.add_argument("--height", type=int, default=720, help="Viewport height")
    parser.add_argument("--fps", type=int, default=30, help="Canvas capture fps")
    parser.add_argument("--output-fps", type=int, default=12, help="Final encoded video fps after deduping")
    parser.add_argument("--speed", type=float, default=1.0, help="Replay speed during capture")
    parser.add_argument("--show-ui", action="store_true", help="Keep replay UI visible")
    parser.add_argument("--load-wait-ms", type=int, default=5000, help="Delay after page load")
    parser.add_argument("--warmup-ms", type=int, default=300, help="Delay after seek before recording")
    parser.add_argument("--tail-ms", type=int, default=750, help="Extra time to record after end tick")
    parser.add_argument(
        "--probe-timeout-ms",
        type=int,
        default=20000,
        help="How long to wait for the replay page to report the requested tick",
    )
    parser.add_argument(
        "--video-bitrate",
        type=int,
        default=6_000_000,
        help="MediaRecorder videoBitsPerSecond",
    )
    parser.add_argument(
        "--keep-webm",
        action="store_true",
        help="Keep the intermediate WebM file",
    )
    parser.add_argument(
        "--focus-player",
        help="Replay player id or name to focus via the hidden replay API",
    )
    parser.add_argument(
        "--camera-mode",
        choices=["follow", "spectator", "first_person"],
        help="Override the renderer camera mode during capture",
    )
    parser.add_argument("--camera-zoom", type=float, help="Spectator zoom override")
    parser.add_argument("--camera-yaw", type=float, help="Spectator yaw override in radians")
    parser.add_argument("--camera-pitch", type=float, help="Spectator pitch override in radians")
    parser.add_argument("--camera-pan-x", type=float, help="Spectator pan X override")
    parser.add_argument("--camera-pan-y", type=float, help="Spectator pan Y override")
    parser.add_argument("--camera-pan-z", type=float, help="Spectator pan Z override")
    parser.add_argument(
        "--capture-interpolation-delay-ms",
        type=int,
        help="Override replay snapshot interpolation delay during capture",
    )
    parser.add_argument(
        "--follow-position-smoothing",
        type=float,
        help="Override follow-camera position smoothing during capture",
    )
    parser.add_argument(
        "--follow-rotation-smoothing",
        type=float,
        help="Override follow-camera rotation smoothing during capture",
    )
    parser.add_argument(
        "--capture-mode",
        choices=["stepped", "realtime"],
        default="realtime",
        help="Capture exact tick-stepped screenshots or use realtime canvas recording",
    )
    parser.add_argument(
        "--tick-step",
        type=int,
        help="Replay ticks between captured frames in stepped mode; defaults to tick_rate / output_fps",
    )
    parser.add_argument(
        "--debug-dir",
        help="Directory for debug screenshots and JSON logs",
    )
    parser.add_argument(
        "--debug-sample-ms",
        type=int,
        default=500,
        help="Polling interval for debug state samples during playback",
    )
    return parser.parse_args()


def ensure_tools():
    if not CHROME.exists():
        raise SystemExit(f"Chrome not found: {CHROME}")
    if not FFMPEG.exists():
        raise SystemExit(f"ffmpeg not found: {FFMPEG}")


def get_json(url: str, method: str = "GET"):
    req = urllib.request.Request(url, method=method)
    with urllib.request.urlopen(req, timeout=15) as response:
        return json.loads(response.read().decode())


def post_text(url: str):
    req = urllib.request.Request(url, method="POST")
    with urllib.request.urlopen(req, timeout=15) as response:
        return response.read().decode()


class CdpClient:
    def __init__(self, ws_url: str):
        self.ws = websocket.create_connection(ws_url, timeout=20)
        self.ws.settimeout(120)
        self.next_id = 1
        self.pending = {}

    def _recv_one(self):
        payload = json.loads(self.ws.recv())
        if "id" in payload:
            self.pending[payload["id"]] = payload
        return payload

    def send(self, method: str, params=None):
        msg_id = self.next_id
        self.next_id += 1
        self.ws.send(json.dumps({"id": msg_id, "method": method, "params": params or {}}))
        while True:
            if msg_id in self.pending:
                payload = self.pending.pop(msg_id)
            else:
                payload = self._recv_one()
                if payload.get("id") != msg_id:
                    continue
            if "error" in payload:
                raise RuntimeError(f"CDP {method} failed: {payload['error']}")
            return payload["result"]

    def close(self):
        self.ws.close()


def build_capture_config(args):
    config = {}
    if args.capture_interpolation_delay_ms is not None:
        config["interpolationDelayMs"] = int(args.capture_interpolation_delay_ms)
    if args.follow_position_smoothing is not None:
        config["followPositionSmoothing"] = float(args.follow_position_smoothing)
    if args.follow_rotation_smoothing is not None:
        config["followRotationSmoothing"] = float(args.follow_rotation_smoothing)
    return config


def build_preload_script(hide_ui: bool, capture_config: dict):
    css = ""
    if hide_ui:
        css = """
          const style = document.createElement('style');
          style.textContent = `
            #player-list,
            #chat-panel,
            #chat-toggle,
            #agent-log-toggle,
            #agent-log-panel,
            #replay-panel,
            #renderer-meta,
            .runtime-meta,
            [aria-label="Chat"],
            [aria-label="Connected players"],
            [aria-label="Agent messages"],
            [aria-label="Replay controls"] {
              display: none !important;
              visibility: hidden !important;
              opacity: 0 !important;
            }
            #app, .stage-wrap, #stage {
              inset: 0 !important;
              width: 100vw !important;
              height: 100vh !important;
              max-width: none !important;
              max-height: none !important;
            }
            body {
              overflow: hidden !important;
              background: #000 !important;
            }
          `;
          document.addEventListener('DOMContentLoaded', () => document.head.appendChild(style), { once: true });
        """
    return f"""
      (() => {{
        window.__mesaCaptureConfig = {json.dumps(capture_config, sort_keys=True)};
        const originalGetContext = HTMLCanvasElement.prototype.getContext;
        HTMLCanvasElement.prototype.getContext = function(type, attrs) {{
          const isGl = type === 'webgl' || type === 'webgl2' || type === 'experimental-webgl';
          if (isGl && this.__mesaCaptureGlContext) return this.__mesaCaptureGlContext;
          const ctx = originalGetContext.call(
            this,
            type,
            isGl ? {{ ...(attrs || {{}}), preserveDrawingBuffer: true }} : attrs,
          );
          if (isGl && ctx) this.__mesaCaptureGlContext = ctx;
          return ctx;
        }};
        {css}

        const probe = {{
          messageCount: 0,
          lastTick: null,
          lastSelectedPlayerId: null,
          lastSelectedPlayerName: null,
          lastSelectedPlayerPos: null,
          errors: [],
          reset() {{
            this.messageCount = 0;
            this.lastTick = null;
            this.lastSelectedPlayerId = null;
            this.lastSelectedPlayerName = null;
            this.lastSelectedPlayerPos = null;
            this.errors = [];
          }},
        }};
        window.__mesaReplayCaptureProbe = probe;

        if (!window.__mesaReplayCaptureWrappedWebSocket) {{
          const NativeWebSocket = window.WebSocket;
          function WrappedWebSocket(...args) {{
            const ws = new NativeWebSocket(...args);
            ws.addEventListener('message', (event) => {{
              try {{
                if (typeof event.data !== 'string') return;
                const data = JSON.parse(event.data);
                if (!data || typeof data.tick !== 'number' || !Array.isArray(data.players)) return;
                probe.messageCount += 1;
                probe.lastTick = data.tick;
                const selected = data.selected_player_id ?? (data.players[0] && data.players[0].id) ?? null;
                probe.lastSelectedPlayerId = selected == null ? null : String(selected);
                const player = data.players.find((p) => String(p?.id || '') === String(selected)) || data.players[0] || null;
                probe.lastSelectedPlayerName = player?.name ?? null;
                probe.lastSelectedPlayerPos = Array.isArray(player?.position) ? player.position : null;
              }} catch (err) {{
                probe.errors.push(String(err));
              }}
            }});
            return ws;
          }}
          WrappedWebSocket.prototype = NativeWebSocket.prototype;
          Object.setPrototypeOf(WrappedWebSocket, NativeWebSocket);
          window.WebSocket = WrappedWebSocket;
          window.__mesaReplayCaptureWrappedWebSocket = true;
        }}
      }})();
    """


def eval_js(client, expression: str, await_promise: bool = False, timeout_s: float | None = None):
    old_timeout = client.ws.gettimeout()
    if timeout_s is not None:
        client.ws.settimeout(timeout_s)
    try:
        return client.send(
            "Runtime.evaluate",
            {
                "expression": expression,
                "awaitPromise": await_promise,
                "returnByValue": True,
            },
        )
    finally:
        client.ws.settimeout(old_timeout)


def camera_pose_args(args):
    pose = {}
    if args.camera_zoom is not None:
        pose["zoom"] = args.camera_zoom
    if args.camera_yaw is not None:
        pose["orbitYaw"] = args.camera_yaw
    if args.camera_pitch is not None:
        pose["orbitPitch"] = args.camera_pitch
    if any(v is not None for v in (args.camera_pan_x, args.camera_pan_y, args.camera_pan_z)):
        pan = {}
        if args.camera_pan_x is not None:
            pan["x"] = args.camera_pan_x
        if args.camera_pan_y is not None:
            pan["y"] = args.camera_pan_y
        if args.camera_pan_z is not None:
            pan["z"] = args.camera_pan_z
        pose["pan"] = pan
    return pose


def wait_for_hidden_capture_apis(client, require_replay: bool, require_camera: bool, timeout_s: float = 30.0):
    deadline = time.time() + timeout_s
    last_value = None
    while time.time() < deadline:
        result = eval_js(
            client,
            """
              (() => {
                const internal = window.__clawbloxInternal;
                return {
                  hasInternal: !!internal,
                  keys: internal && internal.keys ? internal.keys() : [],
                  hasReplay: !!internal?.get?.('replay'),
                  hasCamera: !!internal?.get?.('camera'),
                };
              })()
            """,
        )
        last_value = result.get("result", {}).get("value")
        if last_value is not None:
            if (not require_replay or last_value.get("hasReplay")) and (
                not require_camera or last_value.get("hasCamera")
            ):
                return last_value
        time.sleep(0.2)
    raise RuntimeError(
        "Hidden capture APIs unavailable: "
        f"require_replay={require_replay} require_camera={require_camera} state={json.dumps(last_value)}"
    )


def apply_hidden_capture_controls(client, args):
    pose = camera_pose_args(args)
    if not args.focus_player and not args.camera_mode and not pose:
        return None
    wait_for_hidden_capture_apis(
        client,
        require_replay=True,
        require_camera=bool(args.camera_mode or pose),
    )
    player_json = json.dumps(args.focus_player) if args.focus_player is not None else "null"
    mode_json = json.dumps(args.camera_mode) if args.camera_mode is not None else "null"
    pose_json = json.dumps(pose)
    script = f"""
      (() => {{
        const internal = window.__clawbloxInternal;
        if (!internal) throw new Error('window.__clawbloxInternal unavailable');
        const replay = internal.get('replay');
        const camera = internal.get('camera');
        if (!replay) throw new Error('hidden replay api unavailable');
        if (!camera) throw new Error('hidden camera api unavailable');
        const result = {{
          available: internal.keys ? internal.keys() : [],
          before: camera.getState ? camera.getState() : null,
          players: replay.listPlayers ? replay.listPlayers() : [],
          selectedBefore: replay.getSelectedPlayerId ? replay.getSelectedPlayerId() : null,
        }};
        const focusPlayer = {player_json};
        if (focusPlayer !== null && replay.selectPlayer) {{
          result.selectedAfter = replay.selectPlayer(focusPlayer);
        }}
        const mode = {mode_json};
        if (mode !== null && camera.setMode) {{
          result.modeAfter = camera.setMode(mode);
        }}
        const pose = {pose_json};
        if (pose && Object.keys(pose).length && camera.setSpectatorPose) {{
          result.poseAfter = camera.setSpectatorPose(pose);
        }}
        if (replay.rerender) replay.rerender();
        result.after = camera.getState ? camera.getState() : null;
        return result;
      }})()
    """
    result = eval_js(client, script)
    if "exceptionDetails" in result:
        raise RuntimeError(f"Hidden capture control JS failed: {json.dumps(result['exceptionDetails'])}")
    value = result.get("result", {}).get("value")
    if value is None:
        raise RuntimeError(f"Hidden capture control JS returned no value: {json.dumps(result)}")
    return value


def launch_chrome(port: int, replay_url: str, width: int, height: int):
    user_data_dir = Path(f"/tmp/chrome-mesa-replay-{int(time.time() * 1000)}-{port}")
    user_data_dir.mkdir(parents=True, exist_ok=True)
    proc = subprocess.Popen(
        [
            str(CHROME),
            "--headless=new",
            f"--remote-debugging-port={port}",
            "--remote-allow-origins=*",
            "--no-sandbox",
            "--disable-dev-shm-usage",
            "--disable-background-timer-throttling",
            "--disable-backgrounding-occluded-windows",
            "--disable-renderer-backgrounding",
            "--disable-frame-rate-limit",
            "--run-all-compositor-stages-before-draw",
            "--enable-webgl",
            "--ignore-gpu-blocklist",
            "--use-angle=swiftshader",
            "--enable-unsafe-swiftshader",
            f"--window-size={width},{height}",
            f"--user-data-dir={user_data_dir}",
            "about:blank",
        ],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    return proc, user_data_dir


def choose_free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


def wait_for_devtools(port: int, timeout_s: float = 30.0):
    deadline = time.time() + timeout_s
    while time.time() < deadline:
        try:
            get_json(f"http://127.0.0.1:{port}/json/list")
            return
        except Exception:
            time.sleep(0.1)
    raise RuntimeError(f"DevTools did not come up within {timeout_s:.1f}s")


def open_tab(port: int, replay_url: str):
    encoded = urllib.parse.quote(replay_url, safe="/:?=&")
    tab = get_json(f"http://127.0.0.1:{port}/json/new?{encoded}", method="PUT")
    return CdpClient(tab["webSocketDebuggerUrl"])


def get_page_state(client):
    script = """
      (async () => {
        const replayResponse = await fetch('/replay/info', { cache: 'no-store' });
        if (!replayResponse.ok) throw new Error('/replay/info HTTP ' + replayResponse.status);
        const replayInfo = await replayResponse.json();
        return {
          replayInfo,
          probe: window.__mesaReplayCaptureProbe ?? null,
          camera: window.__clawbloxInternal?.get?.('camera')?.getState?.() ?? null,
        };
      })()
    """
    result = eval_js(client, script, await_promise=True, timeout_s=30)
    value = result.get("result", {}).get("value")
    if value is None:
        raise RuntimeError(f"Page state probe returned no value: {json.dumps(result)}")
    return value


def summarize_state(state):
    replay_info = (state or {}).get("replayInfo") or {}
    probe = (state or {}).get("probe") or {}
    camera = (state or {}).get("camera") or {}
    summary = {
        "replay_tick": replay_info.get("current_tick"),
        "replay_paused": replay_info.get("paused"),
        "replay_speed": replay_info.get("speed"),
        "selected_player_id": replay_info.get("selected_player_id"),
        "probe_tick": probe.get("lastTick"),
        "probe_message_count": probe.get("messageCount"),
        "probe_player_id": probe.get("lastSelectedPlayerId"),
        "probe_player_name": probe.get("lastSelectedPlayerName"),
        "probe_player_pos": probe.get("lastSelectedPlayerPos"),
        "camera": camera,
    }
    return summary


def write_json(path: Path, payload):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")


def write_csv(path: Path, fieldnames: list[str], rows: list[dict]):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fieldnames)
        writer.writeheader()
        writer.writerows(rows)


def tick_to_ms(tick: float, replay_info: dict) -> float:
    tick_rate = float(replay_info.get("tick_rate") or 60)
    first_tick = float(replay_info.get("first_tick") or 0)
    if not math.isfinite(tick_rate) or tick_rate <= 0:
        return 0.0
    return max(0.0, ((float(tick) - first_tick) / tick_rate) * 1000.0)


def build_frame_ticks(start_tick: int, end_tick: int, tick_rate: int, output_fps: int) -> list[float]:
    step = float(tick_rate) / float(output_fps)
    ticks = []
    index = 0
    current = float(start_tick)
    while current < float(end_tick) - 1e-9:
        ticks.append(round(current, 6))
        index += 1
        current = float(start_tick) + index * step
    if not ticks or abs(ticks[-1] - float(end_tick)) > 1e-9:
        ticks.append(float(end_tick))
    return ticks


def comparable_observation(state: dict | None):
    if not isinstance(state, dict):
        return state
    comparable = dict(state)
    comparable.pop("tick", None)
    return comparable


def resolve_selected_player(state: dict, requested_player=None):
    players = (state or {}).get("players") or []
    if requested_player is not None:
        requested = str(requested_player)
        for player in players:
            player_id = player.get("id")
            player_name = player.get("name")
            if (player_id is not None and str(player_id) == requested) or (
                player_name is not None and str(player_name) == requested
            ):
                return player
        return None
    if len(players) == 1:
        return players[0]
    return None


def frame_position_record(index: int, tick: float, player: dict | None):
    record = {
        "frame_index": index,
        "tick": tick,
        "source_from_tick": None,
        "source_to_tick": None,
        "interpolation_alpha": None,
        "player_id": None,
        "player_name": None,
        "x": None,
        "y": None,
        "z": None,
    }
    if not player:
        return record
    position = player.get("position") or []
    if len(position) >= 3:
        record["x"] = float(position[0])
        record["y"] = float(position[1])
        record["z"] = float(position[2])
    record["player_id"] = player.get("id")
    record["player_name"] = player.get("name")
    return record


def interpolate_player_state(player_a: dict | None, player_b: dict | None, alpha: float):
    alpha = max(0.0, min(1.0, float(alpha)))
    if player_a is None:
        return player_b
    if player_b is None:
        return player_a
    position_a = player_a.get("position") or []
    position_b = player_b.get("position") or []
    if len(position_a) < 3 or len(position_b) < 3:
        return player_b if alpha >= 0.5 else player_a
    blended = {
        **player_b,
        "id": player_b.get("id") if player_b.get("id") is not None else player_a.get("id"),
        "name": player_b.get("name") if player_b.get("name") is not None else player_a.get("name"),
        "position": [
            float(position_a[0]) + (float(position_b[0]) - float(position_a[0])) * alpha,
            float(position_a[1]) + (float(position_b[1]) - float(position_a[1])) * alpha,
            float(position_a[2]) + (float(position_b[2]) - float(position_a[2])) * alpha,
        ],
    }
    return blended


def interpolated_selected_player(
    state_a: dict | None,
    tick_a: int,
    state_b: dict | None,
    tick_b: int,
    frame_tick: float,
    requested_player=None,
):
    player_a = resolve_selected_player(state_a or {}, requested_player)
    player_b = resolve_selected_player(state_b or {}, requested_player)
    if tick_b <= tick_a:
        return player_a or player_b, 0.0
    alpha = (float(frame_tick) - float(tick_a)) / float(tick_b - tick_a)
    alpha = max(0.0, min(1.0, alpha))
    return interpolate_player_state(player_a, player_b, alpha), alpha


def position_delta(previous: dict, current: dict):
    if any(previous.get(axis) is None or current.get(axis) is None for axis in ("x", "y", "z")):
        return None
    dx = float(current["x"]) - float(previous["x"])
    dy = float(current["y"]) - float(previous["y"])
    dz = float(current["z"]) - float(previous["z"])
    return {
        "dx": dx,
        "dy": dy,
        "dz": dz,
        "distance": math.sqrt(dx * dx + dy * dy + dz * dz),
    }


def build_motion_report(frame_records: list[dict], output_fps: int):
    deltas = []
    csv_rows = []
    previous = None
    for record in frame_records:
        row = dict(record)
        row["tick_delta"] = None if previous is None else float(record["tick"]) - float(previous["tick"])
        delta = None if previous is None else position_delta(previous, record)
        if delta is not None:
            row["dx"] = delta["dx"]
            row["dy"] = delta["dy"]
            row["dz"] = delta["dz"]
            row["distance"] = delta["distance"]
            deltas.append(
                {
                    "frame_index": record["frame_index"],
                    "tick": record["tick"],
                    "tick_delta": row["tick_delta"],
                    **delta,
                }
            )
        else:
            row["dx"] = None
            row["dy"] = None
            row["dz"] = None
            row["distance"] = None
        csv_rows.append(row)
        previous = record

    report = {
        "output_fps": output_fps,
        "frame_count": len(frame_records),
        "frames_with_selected_player": sum(1 for record in frame_records if record["player_id"] is not None),
        "frames_without_selected_player": sum(1 for record in frame_records if record["player_id"] is None),
        "max_position_delta": max((delta["distance"] for delta in deltas), default=None),
        "min_position_delta": min((delta["distance"] for delta in deltas), default=None),
        "max_tick_delta": max((delta["tick_delta"] for delta in deltas), default=None),
        "min_tick_delta": min((delta["tick_delta"] for delta in deltas), default=None),
        "zero_motion_transitions": sum(1 for delta in deltas if delta["distance"] == 0.0),
        "largest_position_deltas": sorted(
            deltas,
            key=lambda delta: (-delta["distance"], delta["frame_index"]),
        )[:10],
    }
    return report, csv_rows


def write_frame_position_sidecars(output_mp4: Path, frame_records: list[dict], output_fps: int):
    report, csv_rows = build_motion_report(frame_records, output_fps)
    json_path = output_mp4.with_suffix(".frames.json")
    csv_path = output_mp4.with_suffix(".frames.csv")
    report_path = output_mp4.with_suffix(".motion_report.json")
    write_json(
        json_path,
        {
            "video_path": str(output_mp4),
            "output_fps": output_fps,
            "frames": frame_records,
        },
    )
    write_csv(
        csv_path,
        [
            "frame_index",
            "tick",
            "source_from_tick",
            "source_to_tick",
            "interpolation_alpha",
            "player_id",
            "player_name",
            "x",
            "y",
            "z",
            "tick_delta",
            "dx",
            "dy",
            "dz",
            "distance",
        ],
        csv_rows,
    )
    write_json(report_path, report)


def capture_screenshot(client, path: Path):
    result = client.send(
        "Page.captureScreenshot",
        {
            "format": "jpeg",
            "quality": 90,
            "captureBeyondViewport": False,
        },
    )
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(base64.b64decode(result["data"]))


def capture_canvas_frame(client, path: Path):
    script = f"""
      (async () => {{
        const canvas = document.querySelector('#stage');
        if (!canvas) throw new Error('stage canvas missing');
        await new Promise((resolve) => requestAnimationFrame(() => resolve()));
        const gl = canvas.__mesaCaptureGlContext || window.__mesaCaptureGlContext || null;
        if (gl?.flush) gl.flush();
        if (gl?.finish) gl.finish();
        const dataUrl = canvas.toDataURL('image/jpeg', 0.92);
        const comma = dataUrl.indexOf(',');
        if (comma < 0) throw new Error('canvas.toDataURL returned unexpected payload');
        return {{
          base64: dataUrl.slice(comma + 1),
          size: dataUrl.length,
          width: canvas.width,
          height: canvas.height,
        }};
      }})()
    """
    result = eval_js(client, script, await_promise=True, timeout_s=60)
    value = result.get("result", {}).get("value")
    if value is None or "base64" not in value:
        raise RuntimeError(f"Canvas frame capture returned no frame payload: {json.dumps(result)}")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(base64.b64decode(value["base64"]))
    return value


def record_debug_snapshot(client, debug_dir: Path | None, name: str, state):
    if debug_dir is None:
        return
    write_json(debug_dir / f"{name}.json", summarize_state(state))
    capture_canvas_frame(client, debug_dir / f"{name}.jpg")


def reset_probe(client):
    result = eval_js(
        client,
        """
          (() => {
            const probe = window.__mesaReplayCaptureProbe;
            if (!probe || typeof probe.reset !== 'function') throw new Error('probe unavailable');
            probe.reset();
            return true;
          })()
        """,
    )
    if "exceptionDetails" in result:
        raise RuntimeError(f"Probe reset failed: {json.dumps(result['exceptionDetails'])}")


def post_replay_in_page(client, path: str, refresh_info: bool = False):
    script = f"""
      (async () => {{
        const response = await fetch({json.dumps(path)}, {{ method: 'POST' }});
        if (!response.ok) throw new Error({json.dumps(path)} + ' HTTP ' + response.status);
        if (!{str(refresh_info).lower()}) return null;
        const infoResponse = await fetch('/replay/info', {{ cache: 'no-store' }});
        if (!infoResponse.ok) throw new Error('/replay/info HTTP ' + infoResponse.status);
        const info = await infoResponse.json();
        const replay = window.__clawbloxInternal?.get?.('replay');
        if (replay?.rerender) replay.rerender();
        return info;
      }})()
    """
    result = eval_js(client, script, await_promise=True, timeout_s=30)
    if "exceptionDetails" in result:
        raise RuntimeError(f"Replay command failed for {path}: {json.dumps(result['exceptionDetails'])}")
    return result.get("result", {}).get("value")


def fetch_replay_state_at(client, tick: int):
    script = f"""
      (async () => {{
        const response = await fetch('/replay/state?tick=' + encodeURIComponent(String({int(tick)})), {{
          cache: 'no-store',
        }});
        if (!response.ok) throw new Error('/replay/state HTTP ' + response.status);
        return await response.json();
      }})()
    """
    result = eval_js(client, script, await_promise=True, timeout_s=30)
    if "exceptionDetails" in result:
        raise RuntimeError(f"Replay state fetch failed at tick {tick}: {json.dumps(result['exceptionDetails'])}")
    value = result.get("result", {}).get("value")
    if value is None:
        raise RuntimeError(f"Replay state fetch returned no value at tick {tick}: {json.dumps(result)}")
    return value


def render_replay_state(client, state, info=None, select_player=None, options=None):
    state_json = json.dumps(state)
    info_json = "null" if info is None else json.dumps(info)
    select_player_json = "null" if select_player is None else json.dumps(str(select_player))
    options_json = "null" if options is None else json.dumps(options)
    script = f"""
      (() => {{
        const replay = window.__clawbloxInternal?.get?.('replay');
        if (!replay?.renderState) throw new Error('hidden replay renderState api unavailable');
        const selectedPlayer = {select_player_json};
        if (selectedPlayer !== null && replay.selectPlayer) replay.selectPlayer(selectedPlayer);
        const rendered = replay.renderState({state_json}, {info_json}, {options_json});
        return rendered;
      }})()
    """
    result = eval_js(client, script, timeout_s=30)
    if "exceptionDetails" in result:
        raise RuntimeError(f"Replay renderState failed: {json.dumps(result['exceptionDetails'])}")
    return result.get("result", {}).get("value")


def render_tick_to_frame(client, tick: int, replay_info: dict, select_player=None):
    info = {
        "first_tick": replay_info.get("first_tick"),
        "current_tick": int(tick),
        "total_ticks": replay_info.get("total_ticks"),
        "tick_rate": replay_info.get("tick_rate"),
        "duration_ms": replay_info.get("duration_ms"),
        "speed": 0,
        "paused": True,
    }
    select_player_json = "null" if select_player is None else json.dumps(str(select_player))
    script = f"""
      (async () => {{
        const response = await fetch('/replay/state?tick=' + encodeURIComponent(String({int(tick)})), {{
          cache: 'no-store',
        }});
        if (!response.ok) throw new Error('/replay/state HTTP ' + response.status);
        const state = await response.json();
        const replay = window.__clawbloxInternal?.get?.('replay');
        if (!replay?.renderState) throw new Error('hidden replay renderState api unavailable');
        replay.renderState(state, {json.dumps(info)});
        const selectedPlayer = {select_player_json};
        if (selectedPlayer !== null && replay.selectPlayer) {{
          replay.selectPlayer(selectedPlayer);
          if (replay.rerender) replay.rerender();
        }}
        await new Promise((resolve) => requestAnimationFrame(() => resolve()));
        const canvas = document.querySelector('#stage');
        if (!canvas) throw new Error('stage canvas missing');
        const gl = canvas.__mesaCaptureGlContext || window.__mesaCaptureGlContext || null;
        if (gl?.flush) gl.flush();
        if (gl?.finish) gl.finish();
        const dataUrl = canvas.toDataURL('image/jpeg', 0.92);
        const comma = dataUrl.indexOf(',');
        if (comma < 0) throw new Error('canvas.toDataURL returned unexpected payload');
        return {{
          base64: dataUrl.slice(comma + 1),
          size: dataUrl.length,
          width: canvas.width,
          height: canvas.height,
          state,
        }};
      }})()
    """
    result = eval_js(client, script, await_promise=True, timeout_s=60)
    if "exceptionDetails" in result:
        raise RuntimeError(f"Render tick {tick} failed: {json.dumps(result['exceptionDetails'])}")
    value = result.get("result", {}).get("value")
    if value is None or "base64" not in value:
        raise RuntimeError(f"Render tick {tick} returned no frame payload: {json.dumps(result)}")
    return value


def wait_for_seek_tick(client, target_tick: int, timeout_s: float):
    deadline = time.time() + timeout_s
    last_state = None
    while time.time() < deadline:
        last_state = get_page_state(client)
        replay_tick = (((last_state or {}).get("replayInfo") or {}).get("current_tick"))
        probe_tick = (((last_state or {}).get("probe") or {}).get("lastTick"))
        if probe_tick is not None and abs(int(probe_tick) - int(target_tick)) <= 1:
            return last_state
        if replay_tick is not None and abs(int(replay_tick) - int(target_tick)) <= 1:
            return last_state
        time.sleep(0.1)
    raise RuntimeError(f"Replay page did not settle at tick {target_tick}: {json.dumps(last_state)}")


def wait_for_end_tick(
    client,
    replay_url: str,
    start_tick: int,
    target_tick: int,
    timeout_s: float,
    debug_dir: Path | None = None,
    debug_sample_ms: int = 500,
):
    deadline = time.time() + timeout_s
    last_state = None
    paused_at_end = False
    last_debug_sample = 0.0
    debug_samples = []
    while time.time() < deadline:
        last_state = get_page_state(client)
        replay_info = (last_state or {}).get("replayInfo") or {}
        replay_tick = replay_info.get("current_tick")
        probe_tick = (((last_state or {}).get("probe") or {}).get("lastTick"))
        now = time.time()
        if (
            debug_dir is not None
            and now - last_debug_sample >= max(0.1, debug_sample_ms / 1000.0)
        ):
            sample = summarize_state(last_state)
            sample["wall_time_s"] = round(now, 3)
            debug_samples.append(sample)
            last_debug_sample = now
        if (
            not paused_at_end and
            replay_tick is not None and
            int(replay_tick) >= int(target_tick)
        ):
            post_replay_in_page(client, "/replay/pause", refresh_info=True)
            paused_at_end = True
        if probe_tick is not None and int(probe_tick) >= int(target_tick):
            if debug_dir is not None:
                write_json(debug_dir / "playback_samples.json", debug_samples)
            return last_state
        time.sleep(0.1)
    if debug_dir is not None:
        write_json(debug_dir / "playback_samples.json", debug_samples)
    raise RuntimeError(f"Replay page never reached tick {target_tick}: {json.dumps(last_state)}")


def encode_image_sequence(frame_dir: Path, output_mp4: Path, output_fps: int):
    subprocess.run(
        [
            str(FFMPEG),
            "-y",
            "-framerate",
            str(output_fps),
            "-i",
            str(frame_dir / "frame_%06d.jpg"),
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            str(output_mp4),
        ],
        check=True,
    )


def extract_video_frames(input_video: Path, frame_dir: Path):
    frame_dir.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        [
            str(FFMPEG),
            "-y",
            "-fflags",
            "+genpts",
            "-i",
            str(input_video),
            # Reassign timestamps by decode order so sparse or non-monotonic WebM PTS
            # from headless Chrome do not break image sequence extraction.
            "-vf",
            "setpts=N/TB",
            str(frame_dir / "frame_%06d.jpg"),
        ],
        check=True,
    )
    return sorted(frame_dir.glob("frame_*.jpg"))


def start_recording(client, fps: int, bitrate: int):
    script = f"""
      (() => {{
        const canvas = document.querySelector('#stage');
        if (!canvas) throw new Error('stage canvas missing');
        const stream = canvas.captureStream({fps});
        const preferred = [
          'video/webm;codecs=vp9',
          'video/webm;codecs=vp8',
          'video/webm'
        ];
        const mimeType = preferred.find((x) => MediaRecorder.isTypeSupported(x)) || '';
        const chunks = [];
        const recorder = new MediaRecorder(stream, {{
          mimeType,
          videoBitsPerSecond: {bitrate},
        }});
        const done = new Promise((resolve, reject) => {{
          recorder.ondataavailable = (event) => {{
            if (event.data && event.data.size > 0) chunks.push(event.data);
          }};
          recorder.onerror = (event) => reject(String(event.error || event.message || 'MediaRecorderError'));
          recorder.onstop = async () => {{
            try {{
              const blob = new Blob(chunks, {{ type: mimeType || 'video/webm' }});
              const buffer = await blob.arrayBuffer();
              let binary = '';
              const bytes = new Uint8Array(buffer);
              const chunkSize = 0x8000;
              for (let i = 0; i < bytes.length; i += chunkSize) {{
                binary += String.fromCharCode(...bytes.subarray(i, i + chunkSize));
              }}
              resolve({{
                mimeType: mimeType || 'video/webm',
                base64: btoa(binary),
                size: blob.size,
              }});
            }} catch (err) {{
              reject(String(err));
            }}
          }};
        }});
        recorder.start(250);
        window.__mesaReplayRecorder = {{ recorder, done, startedAt: performance.now() }};
        return {{
          started: true,
          mimeType: mimeType || 'video/webm',
          width: canvas.width,
          height: canvas.height,
        }};
      }})()
    """
    return eval_js(client, script)


def stop_recording(client):
    script = """
      (() => {
        if (!window.__mesaReplayRecorder) throw new Error('recorder not initialized');
        const { recorder, done } = window.__mesaReplayRecorder;
        if (recorder.state !== 'inactive') recorder.stop();
        return done;
      })()
    """
    return eval_js(client, script, await_promise=True, timeout_s=300)


def capture_clip_realtime(client, args, output_mp4: Path, webm_path: Path, debug_dir: Path | None):
    post_replay_in_page(client, f"/replay/speed?value={urllib.parse.quote(str(args.speed), safe='')}", refresh_info=True)
    time.sleep(args.warmup_ms / 1000.0)
    record_debug_snapshot(client, debug_dir, "before_recording", get_page_state(client))

    started = start_recording(client, fps=args.fps, bitrate=args.video_bitrate)
    started_value = started.get("result", {}).get("value")
    print(json.dumps(started_value))
    if debug_dir is not None:
        write_json(debug_dir / "recorder_started.json", started_value)

    post_replay_in_page(client, "/replay/play", refresh_info=True)
    time.sleep(0.1)
    record_debug_snapshot(client, debug_dir, "after_play", get_page_state(client))
    end_state = wait_for_end_tick(
        client,
        args.replay_url,
        args.start_tick,
        args.end_tick,
        max(30.0, args.probe_timeout_ms / 1000.0),
        debug_dir=debug_dir,
        debug_sample_ms=args.debug_sample_ms,
    )
    print(f"end_seen={json.dumps(end_state)}")
    record_debug_snapshot(client, debug_dir, "end_seen", end_state)
    time.sleep(args.tail_ms / 1000.0)
    record_debug_snapshot(client, debug_dir, "before_stop", get_page_state(client))

    result = stop_recording(client)["result"]["value"]
    webm_path.write_bytes(base64.b64decode(result["base64"]))
    print(f"webm_size={result['size']}")
    print(webm_path)
    if debug_dir is not None:
        write_json(debug_dir / "recorder_result.json", result)
        ffprobe_webm = subprocess.run(
            [
                str(FFMPEG),
                "-i",
                str(webm_path),
                "-f",
                "null",
                "-",
            ],
            capture_output=True,
            text=True,
            check=False,
        )
        write_json(
            debug_dir / "ffmpeg_decode_webm.json",
            {
                "returncode": ffprobe_webm.returncode,
                "stdout": ffprobe_webm.stdout,
                "stderr": ffprobe_webm.stderr,
            },
        )

    frame_dir = (debug_dir / "raw_frames") if debug_dir is not None else Path(
        f"/tmp/mesa-replay-raw-frames-{int(time.time() * 1000)}"
    )
    frames = extract_video_frames(webm_path, frame_dir)
    if not frames:
        raise RuntimeError(f"No frames decoded from {webm_path}")
    if debug_dir is not None:
        write_json(
            debug_dir / "decoded_frame_summary.json",
            {
                "frame_count": len(frames),
                "first_frame": str(frames[0]),
                "last_frame": str(frames[-1]),
            },
        )
    encode_image_sequence(frame_dir, output_mp4, args.output_fps)
    if debug_dir is None:
        shutil.rmtree(frame_dir, ignore_errors=True)


def capture_clip_stepped(client, args, output_mp4: Path, debug_dir: Path | None):
    initial_state = get_page_state(client)
    replay_info = (initial_state.get("replayInfo") or {})
    tick_rate = int(replay_info.get("tick_rate") or 60)
    interpolation_delay_ms = (
        int(args.capture_interpolation_delay_ms)
        if args.capture_interpolation_delay_ms is not None
        else 100
    )
    frame_ticks = build_frame_ticks(args.start_tick, args.end_tick, tick_rate, args.output_fps)
    if debug_dir is not None:
        write_json(
            debug_dir / "stepped_capture_config.json",
            {
                "tick_rate": tick_rate,
                "start_tick": args.start_tick,
                "end_tick": args.end_tick,
                "output_fps": args.output_fps,
                "interpolation_delay_ms": interpolation_delay_ms,
                "frame_count": len(frame_ticks),
                "frame_ticks_preview": frame_ticks[:10],
            },
        )

    frame_dir = (debug_dir / "frames") if debug_dir is not None else Path(
        f"/tmp/mesa-replay-frames-{int(time.time() * 1000)}"
    )
    frame_dir.mkdir(parents=True, exist_ok=True)
    captured_frames = []
    frame_positions = []
    settled = initial_state
    selected_player = args.focus_player
    state_cache = {}
    first_push = True
    last_pushed_index = -1

    def get_cached_state(tick: int):
        cached = state_cache.get(tick)
        if cached is not None:
            return cached
        base_url = args.replay_url.rstrip("/")
        state = get_json(f"{base_url}/replay/state?tick={tick}")
        state_cache[tick] = state
        return state

    def replay_info_for_tick(current_tick: float):
        return {
            "first_tick": replay_info.get("first_tick"),
            "current_tick": int(round(current_tick)),
            "total_ticks": replay_info.get("total_ticks"),
            "tick_rate": replay_info.get("tick_rate"),
            "duration_ms": replay_info.get("duration_ms"),
            "speed": 0,
            "paused": True,
        }

    timeline_ticks = []
    previous_state = None
    for tick in range(args.start_tick, args.end_tick + 1):
        state = get_cached_state(tick)
        comparable = comparable_observation(state)
        if previous_state is None or comparable != previous_state:
            timeline_ticks.append(tick)
            previous_state = comparable
    if not timeline_ticks:
        timeline_ticks.append(args.start_tick)
    if timeline_ticks[-1] != args.end_tick:
        timeline_ticks.append(args.end_tick)

    if debug_dir is not None:
        write_json(
            debug_dir / "state_change_timeline.json",
            {
                "timeline_ticks": timeline_ticks,
                "count": len(timeline_ticks),
            },
        )

    for index, frame_tick in enumerate(frame_ticks, start=1):
        floor_index = bisect.bisect_right(timeline_ticks, frame_tick) - 1
        floor_index = max(0, min(floor_index, len(timeline_ticks) - 1))
        ceil_index = bisect.bisect_left(timeline_ticks, frame_tick)
        ceil_index = max(0, min(ceil_index, len(timeline_ticks) - 1))
        source_from_tick = timeline_ticks[floor_index]
        source_to_tick = timeline_ticks[ceil_index]

        for push_index in range(last_pushed_index + 1, ceil_index + 1):
            push_tick = timeline_ticks[push_index]
            push_state = get_cached_state(push_tick)
            push_player = resolve_selected_player(push_state, selected_player)
            if selected_player is None and push_player is not None:
                player_id = push_player.get("id")
                if player_id is not None:
                    selected_player = str(player_id)
            render_replay_state(
                client,
                push_state,
                info=replay_info_for_tick(push_tick),
                select_player=selected_player,
                options={
                    "resetBuffer": first_push,
                    "pushNowMs": tick_to_ms(push_tick, replay_info),
                    "renderNowMs": tick_to_ms(push_tick, replay_info) + interpolation_delay_ms,
                    "dt": 1.0 / max(1, tick_rate),
                },
            )
            first_push = False
            last_pushed_index = push_index

        state_from = get_cached_state(source_from_tick)
        state_to = get_cached_state(source_to_tick)
        selected_player_state, alpha = interpolated_selected_player(
            state_from,
            source_from_tick,
            state_to,
            source_to_tick,
            frame_tick,
            requested_player=selected_player,
        )
        render_state = state_to if source_to_tick >= source_from_tick else state_from
        render_replay_state(
            client,
            render_state,
            info=replay_info_for_tick(frame_tick),
            select_player=selected_player,
            options={
                "push": False,
                "renderNowMs": tick_to_ms(frame_tick, replay_info) + interpolation_delay_ms,
                "dt": 1.0 / max(1, args.output_fps),
            },
        )
        frame_state_summary = None
        if index == 1 or index == len(frame_ticks) or debug_dir is not None:
            settled = get_page_state(client)
            frame_state_summary = summarize_state(settled)
        frame_path = frame_dir / f"frame_{index:06d}.jpg"
        capture_canvas_frame(client, frame_path)
        frame_record = {
            "index": index,
            "tick": frame_tick,
            "frame_path": str(frame_path),
            "state": frame_state_summary,
        }
        captured_frames.append(frame_record)
        position_record = frame_position_record(index, frame_tick, selected_player_state)
        position_record["source_from_tick"] = source_from_tick
        position_record["source_to_tick"] = source_to_tick
        position_record["interpolation_alpha"] = round(alpha, 6)
        frame_positions.append(position_record)
        if index == 1:
            print(f"first_frame={json.dumps(frame_record['state'])}")
        elif index == len(frame_ticks):
            print(f"last_frame={json.dumps(frame_record['state'])}")

    if debug_dir is not None:
        write_json(debug_dir / "stepped_frames.json", captured_frames)

    encode_image_sequence(frame_dir, output_mp4, args.output_fps)
    write_frame_position_sidecars(output_mp4, frame_positions, args.output_fps)
    if debug_dir is None:
        shutil.rmtree(frame_dir, ignore_errors=True)


def capture_clip(args):
    ensure_tools()
    output_mp4 = Path(args.output_mp4).resolve()
    output_mp4.parent.mkdir(parents=True, exist_ok=True)
    webm_path = output_mp4.with_suffix(".webm")
    debug_dir = Path(args.debug_dir).resolve() if args.debug_dir else None
    if debug_dir is not None:
        debug_dir.mkdir(parents=True, exist_ok=True)

    chrome_proc = None
    chrome_profile = None
    client = None
    port = choose_free_port()

    try:
        chrome_proc, chrome_profile = launch_chrome(port, args.replay_url, args.width, args.height)
        wait_for_devtools(port)
        client = open_tab(port, args.replay_url)
        client.send("Runtime.enable")
        client.send("Page.enable")
        client.send(
            "Page.addScriptToEvaluateOnNewDocument",
            {"source": build_preload_script(hide_ui=not args.show_ui, capture_config=build_capture_config(args))},
        )
        client.send(
            "Emulation.setDeviceMetricsOverride",
            {
                "width": args.width,
                "height": args.height,
                "deviceScaleFactor": 1,
                "mobile": False,
            },
        )
        client.send("Page.navigate", {"url": args.replay_url})
        client.send("Page.bringToFront")
        time.sleep(args.load_wait_ms / 1000.0)

        post_replay_in_page(client, "/replay/pause", refresh_info=True)
        controls = apply_hidden_capture_controls(client, args)
        if controls is not None:
            print(json.dumps(controls))
            if debug_dir is not None:
                write_json(debug_dir / "initial_controls.json", controls)

        reset_probe(client)
        post_replay_in_page(
            client,
            f"/replay/seek?tick={urllib.parse.quote(str(args.start_tick), safe='')}",
            refresh_info=True,
        )
        settled = wait_for_seek_tick(client, args.start_tick, args.probe_timeout_ms / 1000.0)
        print(f"seek_settled={json.dumps(settled)}")
        record_debug_snapshot(client, debug_dir, "after_seek", settled)

        controls = apply_hidden_capture_controls(client, args)
        if controls is not None:
            print(json.dumps(controls))
            if debug_dir is not None:
                write_json(debug_dir / "post_seek_controls.json", controls)

        if args.capture_mode == "stepped":
            capture_clip_stepped(client, args, output_mp4, debug_dir)
        else:
            capture_clip_realtime(client, args, output_mp4, webm_path, debug_dir)
        print(output_mp4)
        if debug_dir is not None:
            mp4_probe = subprocess.run(
                [
                    "ffprobe",
                    "-v",
                    "error",
                    "-show_entries",
                    "stream=avg_frame_rate,nb_frames,duration,width,height",
                    "-of",
                    "json",
                    str(output_mp4),
                ],
                capture_output=True,
                text=True,
                check=False,
            )
            write_json(
                debug_dir / "ffprobe_mp4.json",
                {
                    "returncode": mp4_probe.returncode,
                    "stdout": mp4_probe.stdout,
                    "stderr": mp4_probe.stderr,
                },
            )
    finally:
        if client is not None:
            try:
                client.close()
            except Exception:
                pass
        if chrome_proc is not None:
            try:
                os.kill(chrome_proc.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
        if chrome_profile is not None and chrome_profile.exists():
            shutil.rmtree(chrome_profile, ignore_errors=True)
        if args.capture_mode == "realtime" and not args.keep_webm:
            try:
                webm_path.unlink()
            except FileNotFoundError:
                pass


def main():
    capture_clip(parse_args())


if __name__ == "__main__":
    main()
