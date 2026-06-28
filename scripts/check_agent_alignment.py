#!/usr/bin/env python3
import argparse
import json
import sys
import urllib.error
import urllib.parse
import urllib.request


def parse_args():
    parser = argparse.ArgumentParser(
        description="Check that replay agent-message timing aligns with replay start."
    )
    parser.add_argument(
        "--base-url",
        default="http://127.0.0.1:8084",
        help="Clawblox app base URL, e.g. http://127.0.0.1:8084",
    )
    parser.add_argument(
        "--recording-path",
        required=True,
        help="Recording path relative to the app project root.",
    )
    parser.add_argument(
        "--tolerance-ticks",
        type=float,
        default=1.0,
        help="Allowed absolute tick difference between replay start and first agent message.",
    )
    parser.add_argument(
        "--sample-tick",
        type=int,
        default=None,
        help="Optional extra tick to probe via /replay/agent?tick=<N> for debugging.",
    )
    return parser.parse_args()


def http_json(url, method="GET", payload=None):
    data = None
    headers = {}
    if payload is not None:
        data = json.dumps(payload).encode("utf-8")
        headers["Content-Type"] = "application/json"
    req = urllib.request.Request(url, data=data, headers=headers, method=method)
    with urllib.request.urlopen(req, timeout=15) as resp:
        return json.load(resp)


def open_replay(base_url, recording_path):
    return http_json(
        f"{base_url}/api/open-replay",
        method="POST",
        payload={"path": recording_path},
    )


def fail(message):
    print(f"FAIL: {message}", file=sys.stderr)
    return 1


def main():
    args = parse_args()
    base_url = args.base_url.rstrip("/")
    try:
        opened = open_replay(base_url, args.recording_path)
        session_id = opened["session_id"]
        replay_info = http_json(f"{base_url}/sessions/{session_id}/replay/info")
        agent_payload = http_json(f"{base_url}/sessions/{session_id}/replay/agent")
    except urllib.error.HTTPError as exc:
        body = exc.read().decode("utf-8", errors="replace")
        return fail(f"{exc.code} {exc.reason} from {exc.url}: {body}")
    except urllib.error.URLError as exc:
        return fail(f"could not reach app server: {exc}")

    first_tick = float(replay_info["first_tick"])
    current_tick = float(replay_info["current_tick"])
    messages = agent_payload.get("messages", [])
    enabled = bool(agent_payload.get("enabled"))
    total_messages = len(messages)

    print(f"session_id={session_id}")
    print(f"recording_path={opened['recording_path']}")
    print(f"first_tick={first_tick:g}")
    print(f"current_tick={current_tick:g}")
    print(f"agent_enabled={enabled}")
    print(f"loaded_messages={total_messages}")

    if not enabled:
        return fail("agent replay payload is disabled for this recording")
    if not messages:
        return fail("agent replay payload has no messages")

    first_message = messages[0]
    first_message_tick = float(first_message["tick"])
    tick_delta = first_message_tick - first_tick
    any_before_start = any(
        float(msg.get("tick", 0.0)) < (first_tick - args.tolerance_ticks)
        for msg in messages
    )

    print(f"first_message_tick={first_message_tick:.6f}")
    print(f"first_message_delta={tick_delta:.6f}")
    print(f"first_message_role={first_message.get('role', '')}")
    first_block = (first_message.get("blocks") or [{}])[0]
    print(f"first_message_preview={json.dumps(first_block, ensure_ascii=True)}")
    print(f"any_before_start={str(any_before_start).lower()}")

    if args.sample_tick is not None:
        sampled = http_json(
            f"{base_url}/sessions/{session_id}/replay/agent?{urllib.parse.urlencode({'tick': args.sample_tick})}"
        )
        print(f"sample_tick={args.sample_tick}")
        print(f"sample_message_count={len(sampled.get('messages', []))}")

    if abs(tick_delta) > args.tolerance_ticks:
        return fail(
            f"first agent message is misaligned by {tick_delta:.3f} ticks "
            f"(tolerance {args.tolerance_ticks:.3f})"
        )
    if any_before_start:
        return fail("found agent messages before replay start tick")

    print("PASS: first agent message aligns with replay start")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
