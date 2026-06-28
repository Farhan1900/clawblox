#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import subprocess
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Extract text-only transcript content from a session directory or session .jsonl file."
    )
    parser.add_argument(
        "input_path",
        type=Path,
        help="Path to a session directory containing reset-*/*.jsonl files, or a single session .jsonl file",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=None,
        help="Exact output file path. Overrides --output-dir.",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=None,
        help="Directory for the output text file (default: <script_dir>/results/<script_stem>)",
    )
    return parser.parse_args()


def collect_jsonl_files(input_path: Path) -> list[Path]:
    if input_path.is_file():
        if input_path.suffix != ".jsonl":
            raise ValueError(f"Expected a .jsonl file, got: {input_path}")
        return [input_path]

    if input_path.is_dir():
        files = sorted(input_path.glob("reset-*/*.jsonl"))
        if not files:
            raise ValueError(f"No session .jsonl files found under: {input_path}")
        return files

    raise ValueError(f"Input path does not exist: {input_path}")


def derive_source_label(input_path: Path) -> str:
    if input_path.is_file():
        return input_path.stem
    if input_path.name == "session" and input_path.parent.name:
        return input_path.parent.name
    return input_path.name


def derive_run_name(input_path: Path) -> str:
    parts = input_path.resolve().parts
    if "results" in parts:
        idx = parts.index("results")
        if idx + 1 < len(parts):
            return parts[idx + 1]
    return derive_source_label(input_path)


def default_output_path(script_path: Path, input_path: Path) -> Path:
    results_dir = script_path.parent / "results" / script_path.stem
    filename = f"{derive_source_label(input_path)}_messages_text_only.txt"
    return results_dir / filename


def default_output_dir(script_path: Path) -> Path:
    return script_path.parent / "results" / script_path.stem


def summary_output_dir(base_output_dir: Path, input_path: Path) -> Path:
    return base_output_dir / derive_run_name(input_path) / "summaries"


def load_anthropic_api_key(repo_root: Path) -> tuple[str, str]:
    dotenv_path = find_dotenv(repo_root)
    dotenv_values = read_dotenv(dotenv_path) if dotenv_path is not None else {}
    if "ANTHROPIC_API_KEY" in os.environ:
        return os.environ["ANTHROPIC_API_KEY"], "environment"
    if "ANTHROPIC_API_KEY" in dotenv_values:
        assert dotenv_path is not None
        return dotenv_values["ANTHROPIC_API_KEY"], str(dotenv_path)
    raise SystemExit("ANTHROPIC_API_KEY not found in environment or any .env file from repo root upward.")


def read_dotenv(dotenv_path: Path) -> dict[str, str]:
    values: dict[str, str] = {}
    if not dotenv_path.exists():
        return values

    for raw_line in dotenv_path.read_text(encoding="utf-8").splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        key = key.strip()
        value = value.strip()
        if value and len(value) >= 2 and value[0] == value[-1] and value[0] in {"'", '"'}:
            value = value[1:-1]
        if key:
            values[key] = value
    return values


def find_dotenv(start: Path) -> Path | None:
    for candidate_dir in [start, *start.parents]:
        candidate = candidate_dir / ".env"
        if candidate.exists():
            return candidate
    return None


def iter_text_messages(jsonl_path: Path):
    with jsonl_path.open(encoding="utf-8") as handle:
        for raw_line in handle:
            raw_line = raw_line.strip()
            if not raw_line:
                continue

            try:
                record = json.loads(raw_line)
            except json.JSONDecodeError:
                continue

            if record.get("type") != "message":
                continue

            message = record.get("message") or {}
            role = message.get("role")
            if role == "toolResult":
                continue

            text_parts = []
            for item in message.get("content") or []:
                if item.get("type") == "text":
                    text = item.get("text", "").strip()
                    if text:
                        text_parts.append(text)

            if not text_parts:
                continue

            yield record.get("timestamp", ""), role, text_parts


def build_output(jsonl_files: list[Path]) -> str:
    lines: list[str] = []

    for jsonl_path in jsonl_files:
        lines.append(f"## {jsonl_path.parent.name} / {jsonl_path.name}")
        for timestamp, role, text_parts in iter_text_messages(jsonl_path):
            lines.append(f"[{timestamp}] {role}")
            lines.extend(text_parts)
            lines.append("")
        lines.append("")

    return "\n".join(lines).rstrip() + "\n"


def build_single_session_output(jsonl_path: Path) -> str:
    lines: list[str] = [f"## {jsonl_path.parent.name} / {jsonl_path.name}"]

    for timestamp, role, text_parts in iter_text_messages(jsonl_path):
        lines.append(f"[{timestamp}] {role}")
        lines.extend(text_parts)
        lines.append("")

    return "\n".join(lines).rstrip() + "\n"


def chunk_text(text: str, max_chars: int) -> list[str]:
    if len(text) <= max_chars:
        return [text]

    chunks: list[str] = []
    start = 0
    text_len = len(text)
    while start < text_len:
        end = min(start + max_chars, text_len)
        if end < text_len:
            split = text.rfind("\n\n", start, end)
            if split == -1:
                split = text.rfind("\n", start, end)
            if split != -1 and split > start + max_chars // 2:
                end = split
        chunk = text[start:end].strip()
        if chunk:
            chunks.append(chunk)
        start = end
    return chunks


def pi_ai_complete(
    helper_path: Path,
    helper_workdir: Path,
    api_key: str,
    model: str,
    max_tokens: int,
    system_prompt: str,
    user_text: str,
) -> str:
    module_dir = helper_workdir / "node_modules" / "@mariozechner" / "pi-ai"
    if not module_dir.exists():
        raise RuntimeError(
            "Missing Node dependency @mariozechner/pi-ai. "
            f"Run `cd {helper_workdir} && npm install` first."
        )

    payload = {
        "model": model,
        "maxTokens": max_tokens,
        "systemPrompt": system_prompt,
        "userText": user_text,
    }
    env = os.environ.copy()
    env["ANTHROPIC_API_KEY"] = api_key
    result = subprocess.run(
        ["node", str(helper_path)],
        input=json.dumps(payload),
        text=True,
        capture_output=True,
        check=False,
        cwd=helper_workdir,
        env=env,
    )
    if result.returncode != 0:
        stderr = result.stderr.strip() or result.stdout.strip() or "pi-ai helper failed"
        raise RuntimeError(stderr)
    return result.stdout.strip()


def summarize_session_text(
    helper_path: Path,
    helper_workdir: Path,
    api_key: str,
    model: str,
    max_tokens: int,
    chunk_chars: int,
    session_text: str,
) -> str:
    system_prompt = (
        "You summarize agent session transcripts. "
        "Write a short, concrete summary of what the agent did, what changed, and where it got stuck. "
        "Do not mention transcript formatting."
    )
    chunk_prompt_template = (
        "Summarize this session transcript chunk in 3 to 6 bullet points.\n"
        "Focus on actions, discoveries, failures, and outcome.\n\n"
        "Transcript chunk:\n\n{chunk}"
    )
    final_prompt_template = (
        "These are chunk summaries for one full session transcript.\n"
        "Merge them into one short summary in 5 to 8 bullet points.\n"
        "Focus on what the agent tried across the session, what it learned, and the result.\n\n"
        "Chunk summaries:\n\n{summaries}"
    )

    chunks = chunk_text(session_text, chunk_chars)
    if len(chunks) == 1:
        return pi_ai_complete(
            helper_path=helper_path,
            helper_workdir=helper_workdir,
            api_key=api_key,
            model=model,
            max_tokens=max_tokens,
            system_prompt=system_prompt,
            user_text=chunk_prompt_template.format(chunk=chunks[0]),
        )

    chunk_summaries: list[str] = []
    for index, chunk in enumerate(chunks, start=1):
        summary = pi_ai_complete(
            helper_path=helper_path,
            helper_workdir=helper_workdir,
            api_key=api_key,
            model=model,
            max_tokens=max_tokens,
            system_prompt=system_prompt,
            user_text=(
                f"Chunk {index} of {len(chunks)}\n\n"
                + chunk_prompt_template.format(chunk=chunk)
            ),
        )
        chunk_summaries.append(f"### Chunk {index}\n{summary}")

    return pi_ai_complete(
        helper_path=helper_path,
        helper_workdir=helper_workdir,
        api_key=api_key,
        model=model,
        max_tokens=max_tokens,
        system_prompt=system_prompt,
        user_text=final_prompt_template.format(summaries="\n\n".join(chunk_summaries)),
    )


def main() -> int:
    args = parse_args()
    input_path = args.input_path.resolve()
    script_path = Path(__file__).resolve()
    repo_root = script_path.parent.parent
    anthropic_model = "claude-haiku-4-5"
    summary_max_tokens = 500
    summary_chunk_chars = 24000
    pi_helper_path = script_path.parent / "pi_ai_summarize.mjs"
    pi_helper_workdir = script_path.parent

    jsonl_files = collect_jsonl_files(input_path)

    if input_path.is_file():
        if args.output is not None:
            output_path = args.output.resolve()
        elif args.output_dir is not None:
            output_path = args.output_dir.resolve() / f"{derive_source_label(input_path)}_messages_text_only.txt"
        else:
            output_path = default_output_path(script_path, input_path)

        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_text(build_single_session_output(jsonl_files[0]), encoding="utf-8")
        print(output_path)

        api_key, api_key_source = load_anthropic_api_key(repo_root)

        base_output_dir = args.output_dir.resolve() if args.output_dir is not None else default_output_dir(script_path)
        summaries_dir = summary_output_dir(base_output_dir, input_path)
        summaries_dir.mkdir(parents=True, exist_ok=True)

        try:
            summary = summarize_session_text(
                helper_path=pi_helper_path,
                helper_workdir=pi_helper_workdir,
                api_key=api_key,
                model=anthropic_model,
                max_tokens=summary_max_tokens,
                chunk_chars=summary_chunk_chars,
                session_text=output_path.read_text(encoding="utf-8"),
            )
        except RuntimeError as exc:
            raise SystemExit(
                f"pi-ai summary failed for {output_path.name} using key from {api_key_source}: {exc}"
            ) from exc

        summary_path = summaries_dir / output_path.name.replace("_messages_text_only.txt", "_summary.md")
        summary_path.write_text(summary.rstrip() + "\n", encoding="utf-8")
        print(summary_path)
    else:
        base_output_dir = args.output_dir.resolve() if args.output_dir is not None else default_output_dir(script_path)
        output_dir = base_output_dir / derive_run_name(input_path) / "sessions"
        output_dir.mkdir(parents=True, exist_ok=True)

        for jsonl_path in jsonl_files:
            output_path = output_dir / f"{jsonl_path.stem}_messages_text_only.txt"
            output_path.write_text(build_single_session_output(jsonl_path), encoding="utf-8")
            print(output_path)

        api_key, api_key_source = load_anthropic_api_key(repo_root)

        summaries_dir = summary_output_dir(base_output_dir, input_path)
        summaries_dir.mkdir(parents=True, exist_ok=True)

        for session_file in sorted(output_dir.glob("*.txt")):
            session_text = session_file.read_text(encoding="utf-8")
            try:
                summary = summarize_session_text(
                    helper_path=pi_helper_path,
                    helper_workdir=pi_helper_workdir,
                    api_key=api_key,
                    model=anthropic_model,
                    max_tokens=summary_max_tokens,
                    chunk_chars=summary_chunk_chars,
                    session_text=session_text,
                )
            except RuntimeError as exc:
                raise SystemExit(
                    f"pi-ai summary failed for {session_file.name} using key from {api_key_source}: {exc}"
                ) from exc

            summary_path = summaries_dir / session_file.name.replace(
                "_messages_text_only.txt", "_summary.md"
            )
            summary_path.write_text(summary.rstrip() + "\n", encoding="utf-8")
            print(summary_path)

    print(f"{len(jsonl_files)} session files processed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
