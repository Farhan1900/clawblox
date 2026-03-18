#!/usr/bin/env python3
import argparse
import json
import re
from pathlib import Path
from typing import Any


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Convert agent session JSONL logs into Markdown files in one folder."
    )
    parser.add_argument(
        "results_dir",
        type=Path,
        help="Results directory containing runs/ with agent session JSONL files.",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=None,
        help="Output directory for Markdown files (default: <results_dir>/agent_sessions_md).",
    )
    return parser.parse_args()


def sanitize_filename(value: str) -> str:
    return re.sub(r"[^A-Za-z0-9._-]+", "_", value).strip("._") or "session"


def text_from_parts(parts: list[dict[str, Any]]) -> str:
    chunks: list[str] = []
    for part in parts:
        if part.get("type") == "text":
            text = str(part.get("text", "")).strip()
            if text:
                chunks.append(text)
    return "\n\n".join(chunks).strip()


def render_message_event(event: dict[str, Any]) -> str:
    message = event.get("message", {})
    role = message.get("role", "unknown")
    text = text_from_parts(message.get("content", []))
    if not text:
        return ""
    return f"**{role.title()}:** {text}\n"


def render_event(event: dict[str, Any], session_info: dict[str, Any]) -> str:
    event_type = event.get("type")
    if event_type == "session":
        session_info.update(event)
        return ""
    if event_type == "message":
        if event.get("message", {}).get("role") not in {"user", "assistant"}:
            return ""
        return render_message_event(event)
    if event_type == "model_change":
        return ""
    if event_type == "thinking_level_change":
        return ""
    return ""


def iter_session_files(results_dir: Path) -> list[Path]:
    runs_dir = results_dir / "runs"
    search_root = runs_dir if runs_dir.is_dir() else results_dir
    return sorted(search_root.glob("*/worlds/world-0/agents/*/session/*.jsonl"))


def output_name(results_dir: Path, session_path: Path) -> str:
    rel = session_path.relative_to(results_dir)
    parts = rel.parts
    run_id = "unknown-run"
    agent_name = "unknown-agent"
    if len(parts) >= 2 and parts[0] == "runs":
        run_id = parts[1]
    if "agents" in parts:
        agent_index = parts.index("agents")
        if agent_index + 1 < len(parts):
            agent_name = parts[agent_index + 1]
    stem = session_path.stem
    return (
        f"{sanitize_filename(run_id)}__"
        f"{sanitize_filename(agent_name)}__"
        f"{sanitize_filename(stem)}.md"
    )


def write_session_markdown(results_dir: Path, session_path: Path, output_dir: Path) -> Path:
    session_info: dict[str, Any] = {}
    rendered_events: list[str] = []
    with session_path.open() as handle:
        for line in handle:
            line = line.strip()
            if not line:
                continue
            event = json.loads(line)
            block = render_event(event, session_info)
            if block:
                rendered_events.append(block)

    output_path = output_dir / output_name(results_dir, session_path)
    body = "".join(rendered_events).strip()
    output_path.write_text(f"# {output_path.stem}\n\n{body}\n")
    return output_path


def write_index(results_dir: Path, output_dir: Path, output_paths: list[Path]) -> None:
    lines = [
        "# Agent Sessions",
        "",
        f"- Source results dir: `{results_dir}`",
        f"- Session count: `{len(output_paths)}`",
        "",
    ]
    for path in output_paths:
        lines.append(f"- [{path.name}]({path.name})")
    lines.append("")
    (output_dir / "index.md").write_text("\n".join(lines))


def main() -> None:
    args = parse_args()
    results_dir = args.results_dir.resolve()
    if not results_dir.is_dir():
        raise SystemExit(f"Results directory not found: {results_dir}")
    output_dir = (args.output_dir or (results_dir / "agent_sessions_md")).resolve()
    output_dir.mkdir(parents=True, exist_ok=True)

    session_files = iter_session_files(results_dir)
    if not session_files:
        raise SystemExit(f"No agent session JSONL files found under: {results_dir}")

    output_paths = [
        write_session_markdown(results_dir, session_path, output_dir)
        for session_path in session_files
    ]
    write_index(results_dir, output_dir, output_paths)
    print(f"[done] wrote {len(output_paths)} markdown files to: {output_dir}")


if __name__ == "__main__":
    main()
