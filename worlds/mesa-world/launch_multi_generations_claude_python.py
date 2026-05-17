#!/usr/bin/env python3
"""
Research-facing sketch for replacing:

  bash worlds/mesa-world/launch_multi_generations_claude.sh \
    --world-dir worlds/mesa-small-world-3 \
    --generations 30 \
    --generation-duration 1h \
    --base-port 8085 \
    --tmux-prefix mesa-small-world-3-0 \
    --system-prompt worlds/mesa-world/system_prompt.md \
    --model claude-opus-4-7 \
    --sandbox

This file intentionally keeps all model-facing prompt construction visible.
The imported clawblox Python API stays thin: it starts/stops agents and worlds,
while this script owns the research protocol.
"""

import json
import os
import re
import shlex
import shutil
import stat
import subprocess
import time
from argparse import ArgumentParser
from datetime import datetime, timezone
from pathlib import Path
from string import Template

from clawblox import Agent, World


WORLD_DIR = Path("worlds/mesa-small-world-3")
SYSTEM_PROMPT_TEMPLATE = Path("worlds/mesa-world/system_prompt.md")
DEFAULT_AGENT_TEMPLATE_DIR = Path("agent/template/agent")

GENERATIONS = 30
GENERATION_DURATION_SECONDS = 60 * 60
BASE_PORT = 8085
RUN_NAME = "mesa-small-world-3-0"
EXPERIMENT_ID = ""
RUN_PREFIX = ""
TMUX_PREFIX = "clawblox-gen-claude"
AGENT_NAME_PREFIX = "agent"

AGENT = "claude"
MODEL = "claude-opus-4-7"
PERMISSION_MODE = "bypassPermissions"
SANDBOX = True
BARE = False
EXTRA_ARGS = []
# Pinned for reproducibility. Do not replace these with whatever happens to be
# installed locally; if the exact version is unavailable, the run should fail.
CLAUDE_CODE_VERSION_PIN = "2.1.116"
CODEX_VERSION_PIN = "0.130.0"
AGENT_BINARY = ""

RECORD = True
HEALTH_TIMEOUT_SECONDS = 30
PORT_WAIT_SECONDS = 120
STOP_GRACE_SECONDS = 10
SAVE_GRACE_SECONDS = 300
MAX_GENERATION_RETRIES = 3
RETRY_DELAY_SECONDS = 15
SKIP_SOUL = False
KEEP_WORLD = False
CLAWBLOX_ENV_FILE = ""

INITIAL_PROMPT = "Begin"
SAVE_PROMPT = "You will be reset in 5 minutes. Update your workspace memory files now."
AUTO_CONTINUE_PROMPT = "Please don't stop. Just continue doing whatever you want to do."
GOAL = ""


def utc_now_compact() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def utc_now_iso() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def sanitize_name(value: str) -> str:
    return re.sub(r"[^A-Za-z0-9._-]", "_", value)


def agent_display_name_for_index(index: int) -> str:
    names = ["Eko", "Moa", "Rua", "Tavi", "Oni", "Zev", "Ika", "Pala", "Sori", "Nyx"]
    if index < len(names):
        return names[index]
    return f"{AGENT_NAME_PREFIX}-a{index}"


def resolve_root_path(value: str | Path) -> Path:
    path = Path(value)
    if path.is_absolute():
        return path
    return Path.cwd() / path


def write_shell_env(path: Path, values: dict[str, object]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    lines = [f"{key}={shlex.quote(str(value))}\n" for key, value in values.items()]
    path.write_text("".join(lines), encoding="utf-8")


def csv_escape(value: object) -> str:
    return str(value).replace('"', '""')


def append_generation_chain_row(
    path: Path,
    *,
    generation: int,
    agent_index: int,
    agent_name: str,
    run_id: str,
    tmux_session: str,
    template_in: Path,
    workspace_out: Path,
    start_utc: str,
    end_utc: str,
) -> None:
    row = [
        generation,
        agent_index,
        agent_name,
        run_id,
        tmux_session,
        template_in,
        workspace_out,
        start_utc,
        end_utc,
        GENERATION_DURATION_SECONDS,
    ]
    with path.open("a", encoding="utf-8") as handle:
        handle.write(",".join(f'"{csv_escape(value)}"' for value in row))
        handle.write("\n")


def load_env_file(path: Path) -> dict[str, str]:
    env: dict[str, str] = {}
    if not path.is_file():
        raise RuntimeError(f"env file not found: {path}")
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if line.startswith("export "):
            line = line[len("export ") :].strip()
        if "=" not in line:
            continue
        name, value = line.split("=", 1)
        value = value.strip()
        if "#" in value:
            value = value.split("#", 1)[0].strip()
        if (value.startswith('"') and value.endswith('"')) or (
            value.startswith("'") and value.endswith("'")
        ):
            value = value[1:-1]
        env[name.strip()] = value
    return env


def parse_version(text: str) -> tuple[int, ...]:
    match = re.search(r"(\d+(?:\.\d+)+)", text)
    if not match:
        raise RuntimeError(f"could not parse version from: {text!r}")
    return tuple(int(part) for part in match.group(1).split("."))


def command_output(args: list[str]) -> str:
    return subprocess.check_output(args, text=True, stderr=subprocess.STDOUT).strip()


def command_path(name: str) -> Path:
    resolved = shutil.which(name)
    if resolved is None:
        raise RuntimeError(f"required command not found in PATH: {name}")
    return Path(resolved)


def require_exact_version(binary: Path, expected: str) -> None:
    actual_text = command_output([str(binary), "--version"])
    actual = parse_version(actual_text)
    required = parse_version(expected)
    if actual != required:
        raise RuntimeError(
            f"{binary} is version {actual_text!r}, but this run pins {expected}"
        )


def write_codex_version_wrapper(run_root: Path, version: str) -> Path:
    wrapper = run_root / "bin" / f"codex-{version}"
    wrapper.parent.mkdir(parents=True, exist_ok=True)
    wrapper.write_text(
        "#!/usr/bin/env bash\n"
        "set -euo pipefail\n"
        f"exec npx -y @openai/codex@{shlex.quote(version)} \"$@\"\n",
        encoding="utf-8",
    )
    wrapper.chmod(wrapper.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    return wrapper


def resolve_agent_binary(run_root: Path) -> Path | None:
    if AGENT_BINARY:
        binary = Path(AGENT_BINARY)
        if not binary.is_file():
            raise RuntimeError(f"configured AGENT_BINARY does not exist: {binary}")
        return binary

    if AGENT == "claude" and CLAUDE_CODE_VERSION_PIN:
        version_bin = Path.home() / ".local/share/claude/versions" / CLAUDE_CODE_VERSION_PIN
        if version_bin.is_file():
            require_exact_version(version_bin, CLAUDE_CODE_VERSION_PIN)
            return version_bin
        binary = command_path("claude")
        require_exact_version(binary, CLAUDE_CODE_VERSION_PIN)
        return binary

    if AGENT == "codex" and CODEX_VERSION_PIN:
        binary = command_path("codex")
        try:
            require_exact_version(binary, CODEX_VERSION_PIN)
            return binary
        except RuntimeError:
            if shutil.which("npx") is None:
                raise
            return write_codex_version_wrapper(run_root, CODEX_VERSION_PIN)

    return None


def render_system_prompt_template(
    template_path: Path,
    *,
    world_agent_name: str,
    workspace_dir: Path,
    session_line: str,
    world_base_url: str,
    skill_curl: str,
    skill_url: str,
) -> str:
    template = Template(template_path.read_text(encoding="utf-8"))
    return template.safe_substitute(
        WORLD_AGENT_NAME=world_agent_name,
        WORKSPACE_DIR=str(workspace_dir),
        SESSION_LINE=session_line,
        WORLD_BASE_URL=world_base_url,
        SKILL_CURL=skill_curl,
        SKILL_URL=skill_url,
    )


def api_headers_text(api_access: dict) -> str:
    return "\n".join(
        f"{name}: {value}" for name, value in api_access["headers"].items()
    )


def api_curl(api_access: dict) -> str:
    headers = " ".join(
        f"-H {shlex.quote(f'{name}: {value}')}"
        for name, value in api_access["headers"].items()
    )
    if headers:
        return f"curl -sS {headers}"
    return "curl -sS"


def append_workspace_memory(prompt: str, workspace_dir: Path) -> str:
    preferred_files = [
        "SOUL.md",
        "IDENTITY.md",
        "SEMANTIC_MEMORY.md",
        "EPISODIC_MEMORY.md",
    ]

    prompt_files = []
    seen = set()

    for rel_path in preferred_files:
        if SKIP_SOUL and rel_path == "SOUL.md":
            continue
        path = workspace_dir / rel_path
        if path.is_file():
            prompt_files.append(path)
            seen.add(path.resolve())

    for path in sorted(workspace_dir.rglob("*.md")):
        if SKIP_SOUL and path.name == "SOUL.md":
            continue
        resolved = path.resolve()
        if resolved in seen:
            continue
        prompt_files.append(path)
        seen.add(resolved)

    episodic_jsonl = workspace_dir / "EPISODIC_MEMORY.jsonl"
    has_episodic_jsonl = episodic_jsonl.is_file() and episodic_jsonl.stat().st_size > 0

    if not prompt_files and not has_episodic_jsonl:
        return prompt

    parts = [prompt]
    parts.append("\nInitial workspace memory files at session start:\n")
    parts.append("These are the file contents as they existed when this session began.\n")

    for path in prompt_files:
        rel_path = path.relative_to(workspace_dir)
        parts.append(f"\n===== {rel_path} =====\n")
        parts.append(path.read_text(encoding="utf-8"))
        parts.append("\n")

    if has_episodic_jsonl:
        parts.append("\n===== EPISODIC_MEMORY.jsonl (summaries) =====\n")
        with episodic_jsonl.open(encoding="utf-8") as handle:
            for line_number, raw in enumerate(handle, 1):
                line = raw.strip()
                if not line:
                    continue
                try:
                    obj = json.loads(line)
                except json.JSONDecodeError as err:
                    print(
                        "[warning: EPISODIC_MEMORY.jsonl line "
                        f"{line_number} could not be parsed: {err}]"
                    )
                    continue
                ts = obj.get("timestamp", "?")
                summary = obj.get("summary", "?")
                parts.append(f"{ts} - {summary}\n")
        parts.append("\n")

    return "".join(parts)


def copy_template_workspace(template_dir: Path, workspace_dir: Path) -> None:
    if workspace_dir.exists():
        shutil.rmtree(workspace_dir)

    shutil.copytree(template_dir, workspace_dir)

    if SKIP_SOUL:
        (workspace_dir / "SOUL.md").unlink(missing_ok=True)


def build_system_prompt(
    *,
    world: World,
    agent: Agent,
    api_access: dict,
    skill_path: str,
) -> str:
    prompt = render_system_prompt_template(
        SYSTEM_PROMPT_TEMPLATE,
        world_agent_name=agent.name,
        workspace_dir=agent.visible_workspace_dir,
        session_line=api_headers_text(api_access),
        world_base_url=world.url,
        skill_curl=api_curl(api_access),
        skill_url=f"{world.url}{skill_path}",
    )

    if SKIP_SOUL:
        prompt = "\n".join(line for line in prompt.splitlines() if "SOUL.md" not in line)

    if GOAL:
        prompt += "\nGOAL:\n"
        prompt += GOAL
        prompt += "\n"

    return append_workspace_memory(prompt, agent.workspace_dir)


def validate_generation(agent_dir: Path) -> None:
    session_file_name = {
        "claude": "claude_session_id.txt",
        "codex": "codex_session_id.txt",
    }[AGENT]
    required_files = [
        agent_dir / "logs" / "agent.log",
        agent_dir / "runtime" / "system_prompt.md",
        agent_dir / session_file_name,
        agent_dir / "world_session.txt",
    ]

    for path in required_files:
        if not path.is_file() or path.stat().st_size == 0:
            raise RuntimeError(f"missing or empty required artifact: {path}")

    workspace_dir = agent_dir / "workspace"
    if not workspace_dir.is_dir():
        raise RuntimeError(f"missing workspace: {workspace_dir}")

    if not any(workspace_dir.iterdir()):
        raise RuntimeError(f"empty workspace: {workspace_dir}")


def run_generation(
    *,
    generation: int,
    template_dir: Path,
    experiment_runs_dir: Path,
    chain_csv: Path,
    run_prefix: str,
    tmux_prefix: str,
) -> Path:
    port = BASE_PORT
    run_id = sanitize_name(f"{run_prefix}-g{generation:03d}")
    tmux_session = sanitize_name(f"{tmux_prefix}-g{generation:03d}")
    run_dir = experiment_runs_dir / run_id
    agent_binary = resolve_agent_binary(experiment_runs_dir.parent)

    if run_dir.exists():
        raise RuntimeError(f"run directory already exists: {run_dir}")

    agent_display_name = agent_display_name_for_index(0)
    agent_name = sanitize_name(f"{agent_display_name}-r{run_id}-w0-a0")
    agent_dir = run_dir / "worlds" / "world-0" / "agents" / agent_name
    world_root = run_dir / "worlds" / "world-0"
    world_log = world_root / "logs" / "world.log"
    world_command = world_root / "runtime" / "world.sh"
    start_utc = utc_now_iso()

    world = World(dir=WORLD_DIR)

    agent = Agent(
        agent=AGENT,
        name=agent_name,
        dir=agent_dir,
        model=MODEL,
        binary=agent_binary,
        permission_mode=PERMISSION_MODE,
        auto_continue_prompt=AUTO_CONTINUE_PROMPT,
        sandbox=SANDBOX,
        bare=BARE,
        extra_args=EXTRA_ARGS,
        tmux=tmux_session,
    )

    success = False
    try:
        copy_template_workspace(template_dir, agent.workspace_dir)

        world_start = world.start(
            port=port,
            record=RECORD,
            record_dir=run_dir / "recordings",
            health_timeout=HEALTH_TIMEOUT_SECONDS,
            port_wait=PORT_WAIT_SECONDS,
            command_file=world_command,
            log_file=world_log,
        )
        print(f"World start command: {shlex.join(world_start.command)}")

        api_access = world.connect(agent=agent)
        skill_path = world.api_path(path="/api.md", access=api_access)
        system_prompt = build_system_prompt(
            world=world,
            agent=agent,
            api_access=api_access,
            skill_path=skill_path,
        )

        save_seconds = 0
        active_seconds = GENERATION_DURATION_SECONDS
        if SAVE_GRACE_SECONDS > 0 and GENERATION_DURATION_SECONDS > 1:
            save_seconds = min(SAVE_GRACE_SECONDS, GENERATION_DURATION_SECONDS - 1)
            active_seconds = GENERATION_DURATION_SECONDS - save_seconds

        agent.start(
            initial_prompt=INITIAL_PROMPT,
            system_prompt=system_prompt,
        )

        time.sleep(active_seconds)

        agent.stop(grace_seconds=STOP_GRACE_SECONDS)

        if save_seconds > 0:
            agent.send(SAVE_PROMPT)
            time.sleep(save_seconds)
            agent.stop(grace_seconds=STOP_GRACE_SECONDS)

        validate_generation(agent_dir)
        end_utc = utc_now_iso()
        append_generation_chain_row(
            chain_csv,
            generation=generation,
            agent_index=0,
            agent_name=agent_name,
            run_id=run_id,
            tmux_session=tmux_session,
            template_in=template_dir,
            workspace_out=agent.workspace_dir,
            start_utc=start_utc,
            end_utc=end_utc,
        )

        if not KEEP_WORLD:
            world.stop()
        success = True
        return agent.workspace_dir
    finally:
        if not success:
            try:
                agent.stop(grace_seconds=STOP_GRACE_SECONDS)
            except Exception:
                pass
            try:
                world.stop()
            except Exception:
                pass
            if run_dir.exists():
                shutil.rmtree(run_dir)


def configure_from_args() -> tuple[Path, Path, Path, str, str]:
    global WORLD_DIR
    global SYSTEM_PROMPT_TEMPLATE
    global GENERATIONS
    global GENERATION_DURATION_SECONDS
    global BASE_PORT
    global MODEL
    global PERMISSION_MODE
    global SANDBOX
    global RECORD
    global HEALTH_TIMEOUT_SECONDS
    global PORT_WAIT_SECONDS
    global STOP_GRACE_SECONDS
    global SAVE_GRACE_SECONDS
    global MAX_GENERATION_RETRIES
    global RETRY_DELAY_SECONDS
    global GOAL
    global CLAWBLOX_ENV_FILE
    global SKIP_SOUL
    global AGENT_NAME_PREFIX
    global EXTRA_ARGS
    global AUTO_CONTINUE_PROMPT

    parser = ArgumentParser()
    parser.add_argument("--world-dir", default=str(WORLD_DIR))
    parser.add_argument("--generations", type=int, default=GENERATIONS)
    parser.add_argument("--generation-duration", default=f"{GENERATION_DURATION_SECONDS}s")
    parser.add_argument("--base-port", type=int, default=BASE_PORT)
    parser.add_argument("--tmux-prefix", default=TMUX_PREFIX)
    parser.add_argument("--run-prefix", default=RUN_PREFIX)
    parser.add_argument("--experiment-id", default=EXPERIMENT_ID)
    parser.add_argument("--system-prompt", default=str(SYSTEM_PROMPT_TEMPLATE))
    parser.add_argument("--model", default=MODEL)
    parser.add_argument("--permission-mode", default=PERMISSION_MODE)
    parser.add_argument("--record", default=str(RECORD).lower())
    parser.add_argument("--health-timeout", type=int, default=HEALTH_TIMEOUT_SECONDS)
    parser.add_argument("--port-wait-seconds", type=int, default=PORT_WAIT_SECONDS)
    parser.add_argument("--stop-grace-seconds", type=int, default=STOP_GRACE_SECONDS)
    parser.add_argument("--save-grace-seconds", type=int, default=SAVE_GRACE_SECONDS)
    parser.add_argument("--generation-retries", type=int, default=MAX_GENERATION_RETRIES)
    parser.add_argument("--retry-delay-seconds", type=int, default=RETRY_DELAY_SECONDS)
    parser.add_argument("--goal", default=GOAL)
    parser.add_argument("--env-file", default=CLAWBLOX_ENV_FILE)
    parser.add_argument("--template", default=str(DEFAULT_AGENT_TEMPLATE_DIR))
    parser.add_argument("--agent-name-prefix", default=AGENT_NAME_PREFIX)
    parser.add_argument("--agents-per-world", type=int, default=1)
    parser.add_argument("--claude-extra-args", default="")
    parser.add_argument("--auto-loop-message", default=AUTO_CONTINUE_PROMPT)
    parser.add_argument("--checkpoint-interval", default="1800")
    parser.add_argument("--clawblox-bin", default="")
    parser.add_argument("--sandbox", action="store_true", default=SANDBOX)
    parser.add_argument("--skip-soul", "--no-soul", action="store_true", default=SKIP_SOUL)
    parser.add_argument("--use-env-auth", action="store_true", default=False)
    parser.add_argument("--keep-world", "--persistent-world", action="store_true", default=False)
    parser.add_argument("--resume", action="store_true", default=False)
    parser.add_argument("--resume-prune-partial", action="store_true", default=False)
    args = parser.parse_args()

    if args.agents_per_world != 1:
        raise RuntimeError("this Python parity script currently supports --agents-per-world 1 only")
    if args.keep_world:
        raise RuntimeError("--keep-world parity is not implemented in this Python script yet")
    if args.resume or args.resume_prune_partial:
        raise RuntimeError("--resume parity is not implemented in this Python script yet")
    if args.generations <= 0:
        raise RuntimeError("--generations must be a positive integer")
    if args.base_port <= 0 or args.base_port > 65535:
        raise RuntimeError("--base-port must be in [1, 65535]")
    if args.health_timeout <= 0:
        raise RuntimeError("--health-timeout must be positive")
    if args.port_wait_seconds < 0 or args.stop_grace_seconds < 0 or args.save_grace_seconds < 0:
        raise RuntimeError("wait/grace durations must be non-negative")
    if args.generation_retries <= 0:
        raise RuntimeError("--generation-retries must be positive")
    if args.retry_delay_seconds < 0:
        raise RuntimeError("--retry-delay-seconds must be non-negative")
    if args.record.lower() not in {"true", "false"}:
        raise RuntimeError("--record must be true or false")

    WORLD_DIR = resolve_root_path(args.world_dir)
    SYSTEM_PROMPT_TEMPLATE = resolve_root_path(args.system_prompt)
    GENERATIONS = args.generations
    GENERATION_DURATION_SECONDS = parse_duration_seconds(args.generation_duration)
    BASE_PORT = args.base_port
    MODEL = args.model
    PERMISSION_MODE = args.permission_mode
    SANDBOX = args.sandbox
    RECORD = args.record.lower() == "true"
    HEALTH_TIMEOUT_SECONDS = args.health_timeout
    PORT_WAIT_SECONDS = args.port_wait_seconds
    STOP_GRACE_SECONDS = args.stop_grace_seconds
    SAVE_GRACE_SECONDS = args.save_grace_seconds
    MAX_GENERATION_RETRIES = args.generation_retries
    RETRY_DELAY_SECONDS = args.retry_delay_seconds
    GOAL = args.goal
    CLAWBLOX_ENV_FILE = args.env_file
    SKIP_SOUL = args.skip_soul
    AGENT_NAME_PREFIX = args.agent_name_prefix
    EXTRA_ARGS = shlex.split(args.claude_extra_args) if args.claude_extra_args else []
    AUTO_CONTINUE_PROMPT = args.auto_loop_message

    if args.clawblox_bin:
        os.environ["CLAWBLOX_BIN"] = args.clawblox_bin

    if args.env_file:
        env_file = resolve_root_path(args.env_file)
        os.environ.update(load_env_file(env_file))
    elif args.use_env_auth:
        env_file = resolve_root_path(".env")
        if env_file.is_file():
            os.environ.update(load_env_file(env_file))

    if not WORLD_DIR.is_dir():
        raise RuntimeError(f"world directory not found: {WORLD_DIR}")
    if not SYSTEM_PROMPT_TEMPLATE.is_file():
        raise RuntimeError(f"system prompt file not found: {SYSTEM_PROMPT_TEMPLATE}")
    template_dir = resolve_root_path(args.template)
    if not template_dir.is_dir():
        raise RuntimeError(f"template directory not found: {template_dir}")

    experiment_id = args.experiment_id or f"genexp-claude-{utc_now_compact()}"
    experiment_safe_id = sanitize_name(experiment_id)
    experiment_dir = WORLD_DIR / "results" / experiment_safe_id
    experiment_runs_dir = experiment_dir / "runs"
    chain_csv = experiment_dir / "generation_chain.csv"
    run_prefix = args.run_prefix or experiment_id
    tmux_prefix = args.tmux_prefix

    if experiment_dir.exists():
        raise RuntimeError(
            f"experiment directory already exists: {experiment_dir}; choose --experiment-id or resume manually"
        )

    experiment_runs_dir.mkdir(parents=True)
    chain_csv.write_text(
        "generation,agent_index,agent_name,run_id,tmux_session,template_in,"
        "workspace_out,start_utc,end_utc,duration_seconds\n",
        encoding="utf-8",
    )
    write_shell_env(
        experiment_dir / "run.env",
        {
            "RUN_ID": experiment_id,
            "RUN_SAFE_ID": experiment_safe_id,
            "RUN_DIR": experiment_dir,
            "WORLDS_ROOT": experiment_runs_dir,
            "WORLD_ABS_DIR": WORLD_DIR,
            "NUM_WORLDS": 1,
            "AGENTS_PER_WORLD": 1,
            "BASE_PORT": BASE_PORT,
            "RECORD": str(RECORD).lower(),
            "RESET_EVERY": "",
            "TMUX_PREFIX": tmux_prefix,
        },
    )
    return template_dir, experiment_runs_dir, chain_csv, run_prefix, tmux_prefix


def parse_duration_seconds(value: str) -> int:
    match = re.fullmatch(r"([0-9]+)([smh]?)", value)
    if not match:
        raise RuntimeError(f"invalid duration '{value}'. Use 3600, 60m, or 1h.")
    amount = int(match.group(1))
    unit = match.group(2)
    if unit == "m":
        return amount * 60
    if unit == "h":
        return amount * 3600
    return amount


def main() -> None:
    template_dir, experiment_runs_dir, chain_csv, run_prefix, tmux_prefix = configure_from_args()

    for generation in range(1, GENERATIONS + 1):
        for attempt in range(1, MAX_GENERATION_RETRIES + 1):
            try:
                template_dir = run_generation(
                    generation=generation,
                    template_dir=template_dir,
                    experiment_runs_dir=experiment_runs_dir,
                    chain_csv=chain_csv,
                    run_prefix=run_prefix,
                    tmux_prefix=tmux_prefix,
                )
                break
            except Exception as err:
                if attempt == MAX_GENERATION_RETRIES:
                    raise
                print(f"Generation {generation} attempt {attempt} failed: {err}")
                time.sleep(RETRY_DELAY_SECONDS)


if __name__ == "__main__":
    main()
