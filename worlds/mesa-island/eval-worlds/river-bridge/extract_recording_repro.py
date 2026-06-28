#!/usr/bin/env python3

import argparse
import json
import sqlite3
from pathlib import Path


def parse_args():
    parser = argparse.ArgumentParser(
        description=(
            "Extract the exact recorded input sequence after the last Reset before a target "
            "tick/time and emit a runnable repro script."
        )
    )
    parser.add_argument("recording", type=Path, help="Path to .clawrec recording")
    parser.add_argument(
        "--until-tick",
        type=int,
        default=None,
        help="Include actions up to and including this tick",
    )
    parser.add_argument(
        "--until-time-s",
        type=float,
        default=None,
        help="Include actions up to and including this recording time in seconds",
    )
    parser.add_argument(
        "--output",
        type=Path,
        required=True,
        help="Path to write the generated repro script",
    )
    parser.add_argument(
        "--base-url",
        default="http://localhost:8099",
        help="Default base URL baked into the generated script",
    )
    parser.add_argument(
        "--session-name",
        default="river-bridge-recording-repro",
        help="Default session name baked into the generated script",
    )
    args = parser.parse_args()
    if args.until_tick is None and args.until_time_s is None:
        parser.error("pass --until-tick or --until-time-s")
    return args


def choose_cutoff_ms(cur: sqlite3.Cursor, until_tick: int | None, until_time_s: float | None) -> tuple[int, int]:
    if until_tick is not None:
        row = cur.execute(
            "SELECT tick, time_ms FROM inputs WHERE tick <= ? ORDER BY tick DESC LIMIT 1",
            (until_tick,),
        ).fetchone()
        if row is None:
            raise RuntimeError(f"no input found at or before tick {until_tick}")
        return int(row[0]), int(row[1])

    until_ms = int(round(until_time_s * 1000.0))
    row = cur.execute(
        "SELECT tick, time_ms FROM inputs WHERE time_ms <= ? ORDER BY time_ms DESC LIMIT 1",
        (until_ms,),
    ).fetchone()
    if row is None:
        raise RuntimeError(f"no input found at or before {until_time_s:.3f}s")
    return int(row[0]), int(row[1])


def extract_actions(cur: sqlite3.Cursor, cutoff_ms: int) -> tuple[sqlite3.Row | None, list[sqlite3.Row]]:
    reset_row = cur.execute(
        """
        SELECT tick, time_ms
        FROM inputs
        WHERE input_type = 'Reset' AND time_ms <= ?
        ORDER BY time_ms DESC
        LIMIT 1
        """,
        (cutoff_ms,),
    ).fetchone()
    start_ms = int(reset_row["time_ms"]) if reset_row is not None else -1
    actions = cur.execute(
        """
        SELECT tick, time_ms, input_type, data
        FROM inputs
        WHERE time_ms > ? AND time_ms <= ?
        ORDER BY time_ms
        """,
        (start_ms, cutoff_ms),
    ).fetchall()
    return reset_row, actions


def build_action_payloads(actions: list[sqlite3.Row]) -> list[dict]:
    payloads = []
    prev_ms = None
    for row in actions:
        time_ms = int(row["time_ms"])
        payloads.append(
            {
                "tick": int(row["tick"]),
                "time_ms": time_ms,
                "delay_s": 0.0 if prev_ms is None else round((time_ms - prev_ms) / 1000.0, 3),
                "type": row["input_type"],
                "data": json.loads(row["data"]),
            }
        )
        prev_ms = time_ms
    return payloads


def render_script(
    recording: Path,
    reset_row: sqlite3.Row | None,
    cutoff_tick: int,
    cutoff_ms: int,
    actions: list[dict],
    base_url: str,
    session_name: str,
) -> str:
    reset_comment = (
        f"tick {int(reset_row['tick'])}, {int(reset_row['time_ms'])/1000.0:.3f}s"
        if reset_row is not None
        else "recording start"
    )
    actions_literal = json.dumps(actions, indent=4, sort_keys=False)
    return f"""#!/usr/bin/env python3

import json
import time
import urllib.request


BASE = {base_url!r}
SESSION_NAME = {session_name!r}
SOURCE_RECORDING = {str(recording)!r}
SOURCE_RESET = {reset_comment!r}
SOURCE_CUTOFF_TICK = {cutoff_tick}
SOURCE_CUTOFF_TIME_S = {cutoff_ms / 1000.0:.3f}

ACTIONS = {actions_literal}


def reqj(session, path, method="GET", data=None, timeout=5):
    headers = {{"X-Session": session}}
    body = None
    if data is not None:
        body = json.dumps(data).encode()
        headers["Content-Type"] = "application/json"
    req = urllib.request.Request(BASE + path, data=body, headers=headers, method=method)
    with urllib.request.urlopen(req, timeout=timeout) as response:
        return json.load(response)


def join(name=SESSION_NAME):
    with urllib.request.urlopen(BASE + f"/join?name={{name}}", data=b"", timeout=5) as response:
        return json.load(response)["session"]


def action(session, kind, data):
    obs = reqj(session, "/input", method="POST", data={{"type": kind, "data": data}})
    return obs["player"]["attributes"].get("LastActionResult")


def observe(session):
    return reqj(session, "/observe")


def main():
    session = join()
    print("session", session)
    print("recording", SOURCE_RECORDING)
    print("sequence_start", SOURCE_RESET)
    print("sequence_end", {{"tick": SOURCE_CUTOFF_TICK, "time_s": SOURCE_CUTOFF_TIME_S}})
    print("actions", len(ACTIONS))

    for index, item in enumerate(ACTIONS, start=1):
        delay_s = item["delay_s"]
        if delay_s > 0:
            time.sleep(delay_s)
        result = action(session, item["type"], item["data"])
        print(
            "step",
            index,
            {{
                "tick": item["tick"],
                "time_s": round(item["time_ms"] / 1000.0, 3),
                "type": item["type"],
                "result": result,
            }},
        )

    final_obs = observe(session)
    print("final_player", final_obs["player"]["position"])
    print("final_action", final_obs["player"]["attributes"].get("LastActionResult"))


if __name__ == "__main__":
    main()
"""


def main():
    args = parse_args()
    recording = args.recording.resolve()
    conn = sqlite3.connect(str(recording))
    conn.row_factory = sqlite3.Row
    cur = conn.cursor()

    cutoff_tick, cutoff_ms = choose_cutoff_ms(cur, args.until_tick, args.until_time_s)
    reset_row, action_rows = extract_actions(cur, cutoff_ms)
    if not action_rows:
        raise RuntimeError("no actions found after the selected reset boundary")

    actions = build_action_payloads(action_rows)
    script = render_script(
        recording=recording,
        reset_row=reset_row,
        cutoff_tick=cutoff_tick,
        cutoff_ms=cutoff_ms,
        actions=actions,
        base_url=args.base_url,
        session_name=args.session_name,
    )

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(script)
    args.output.chmod(0o755)

    start_label = (
        f"tick {int(reset_row['tick'])} @ {int(reset_row['time_ms']) / 1000.0:.3f}s"
        if reset_row is not None
        else "recording start"
    )
    print(f"[done] Wrote repro script: {args.output}")
    print(f"[done] Reset boundary: {start_label}")
    print(f"[done] Final action bound: tick {cutoff_tick} @ {cutoff_ms / 1000.0:.3f}s")
    print(f"[done] Actions emitted: {len(actions)}")


if __name__ == "__main__":
    main()
