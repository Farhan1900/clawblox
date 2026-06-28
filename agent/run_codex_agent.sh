#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

WORLD_BASE_URL="${WORLD_BASE_URL-http://localhost:8080}"
WORLD_INTERNAL_BASE_URL="${WORLD_INTERNAL_BASE_URL-$WORLD_BASE_URL}"
WORLD_JOIN_PATH="${WORLD_JOIN_PATH-/join}"
WORLD_LEAVE_PATH="${WORLD_LEAVE_PATH-}"
WORLD_SKILL_PATH_WAS_DEFAULT=0
if [[ -z "${WORLD_SKILL_PATH+x}" ]]; then
  WORLD_SKILL_PATH="/api.md"
  WORLD_SKILL_PATH_WAS_DEFAULT=1
fi
WORLD_SESSION_FIELD="${WORLD_SESSION_FIELD-session}"
WORLD_SESSION_HEADER="${WORLD_SESSION_HEADER-X-Session}"
WORLD_SESSION_FILE="${WORLD_SESSION_FILE-}"
WORLD_AGENT_NAME="${WORLD_AGENT_NAME-}"
WORKSPACE_DIR="${WORKSPACE_DIR-}"
AGENT_DIR="${AGENT_DIR-}"
SESSION_ID_FILE="${SESSION_ID_FILE-}"
TEMPLATE_DIR="${TEMPLATE_DIR:-$SCRIPT_DIR/template/agent}"
WORLD_STARTUP_INSTRUCTIONS="${WORLD_STARTUP_INSTRUCTIONS-}"
SYSTEM_PROMPT_TEMPLATE="${SYSTEM_PROMPT_TEMPLATE-$SCRIPT_DIR/system_prompt.md}"
RESET_EVERY="${RESET_EVERY-}"
SANDBOX="${SANDBOX-0}"
SKIP_SOUL="${SKIP_SOUL-0}"
CHECKPOINT_WARNING_SECONDS="${CHECKPOINT_WARNING_SECONDS:-300}"
CHECKPOINT_WARNING_PROMPT="${CHECKPOINT_WARNING_PROMPT:-You will be reset in 5 minutes. Update your workspace memory files now.}"
DURATION_SECONDS="${DURATION_SECONDS-}"
CLAWBLOX_WORLD_HTTP_PROXY_PORT="${CLAWBLOX_WORLD_HTTP_PROXY_PORT-}"
SANDBOX_DEPS_ROOT="${SANDBOX_DEPS_ROOT-}"

CODEX_MODEL="${CODEX_MODEL-${CLAUDE_MODEL-gpt-5.5}}"
CODEX_APPROVAL_POLICY="${CODEX_APPROVAL_POLICY-${CLAUDE_PERMISSION_MODE-bypassPermissions}}"
CODEX_RESUME_MODE="${CODEX_RESUME_MODE-auto}"
CODEX_EXTRA_ARGS="${CODEX_EXTRA_ARGS-${CLAUDE_EXTRA_ARGS-}}"
CODEX_INITIAL_PROMPT="${CODEX_INITIAL_PROMPT-Begin}"
CODEX_AUTO_LOOP_MESSAGE="${CODEX_AUTO_LOOP_MESSAGE-${CLAUDE_AUTO_LOOP_MESSAGE-}}"
CLAWBLOX_CODEX_BIN="${CLAWBLOX_CODEX_BIN-}"
CLAWBLOX_CODEX_VERSION_PIN="${CLAWBLOX_CODEX_VERSION_PIN-0.130.0}"
SANDBOX_HOME_DIR="${AGENT_DIR-}/sandbox-home"
SANDBOX_WORKSPACE_DIR="${WORKSPACE_DIR-}"
SANDBOX_LOG_DIR="/logs"
SANDBOX_RUNTIME_DIR="/runtime"
SANDBOX_CODEX_BIN="/sandbox-bin/codex"
SANDBOX_CODEX_ROOT="/sandbox-codex"

parse_duration_seconds() {
  local input="$1"
  if [[ "$input" =~ ^[0-9]+$ ]]; then
    printf '%s' "$input"
    return 0
  fi
  local total=0 rest="$input"
  while [[ -n "$rest" ]]; do
    if [[ "$rest" =~ ^([0-9]+)([hms]) ]]; then
      local val="${BASH_REMATCH[1]}" unit="${BASH_REMATCH[2]}"
      case "$unit" in
        h) total=$((total + val * 3600)) ;;
        m) total=$((total + val * 60)) ;;
        s) total=$((total + val)) ;;
      esac
      rest="${rest#"${BASH_REMATCH[0]}"}"
    else
      return 1
    fi
  done
  printf '%s' "$total"
}

generate_session_id() {
  if command -v uuidgen >/dev/null 2>&1; then
    uuidgen
    return 0
  fi
  if [[ -r /proc/sys/kernel/random/uuid ]]; then
    cat /proc/sys/kernel/random/uuid
    return 0
  fi
  python3 -c 'import uuid; print(uuid.uuid4())'
}

generate_agent_name() {
  local -a first_parts=(Brisk Cozy Daring Fuzzy Jolly Lunar Nova Pixel Sunny Swift)
  local -a second_parts=(Fox Owl Lynx Wolf Otter Koala Panda Kiwi Moth Crab)
  local first="${first_parts[$RANDOM % ${#first_parts[@]}]}"
  local second="${second_parts[$RANDOM % ${#second_parts[@]}]}"
  local suffix
  suffix="$(printf '%02d' "$((RANDOM % 100))")"
  printf '%s' "${first}${second}${suffix}"
}

sanitize_name() {
  printf '%s' "$1" | tr -c 'A-Za-z0-9._-' '_'
}

canonical_path() {
  python3 -c 'import os, sys; print(os.path.realpath(sys.argv[1]))' "$1"
}

codex_bind_root_for_bin() {
  local bin_path="$1"
  python3 - "$bin_path" <<'PY'
import os
import sys

path = os.path.realpath(sys.argv[1])
markers = [
    "/node_modules/.bin/codex",
    "/node_modules/@openai/codex/bin/codex.js",
]
for marker in markers:
    if path.endswith(marker):
        print(path[: -len(marker)], end="")
        raise SystemExit(0)
print("", end="")
PY
}

codex_bind_rel_for_bin() {
  local bin_path="$1"
  python3 - "$bin_path" <<'PY'
import os
import sys

path = os.path.realpath(sys.argv[1])
markers = [
    "/node_modules/.bin/codex",
    "/node_modules/@openai/codex/bin/codex.js",
]
for marker in markers:
    if path.endswith(marker):
        root = path[: -len(marker)]
        rel = path[len(root):]
        print(rel, end="")
        raise SystemExit(0)
print("", end="")
PY
}

codex_version_for_bin() {
  local bin_path="$1"
  "$bin_path" --version 2>/dev/null | awk '{print $NF}'
}

resolve_codex_bin() {
  local codex_cmd codex_version
  if [[ -n "$CLAWBLOX_CODEX_BIN" ]]; then
    if [[ ! -x "$CLAWBLOX_CODEX_BIN" ]]; then
      echo "Error: CLAWBLOX_CODEX_BIN is not executable: $CLAWBLOX_CODEX_BIN" >&2
      exit 1
    fi
    canonical_path "$CLAWBLOX_CODEX_BIN"
    return 0
  fi

  if ! codex_cmd="$(command -v codex 2>/dev/null)"; then
    echo "Error: codex CLI was not found in PATH." >&2
    exit 1
  fi
  codex_version="$(codex_version_for_bin "$codex_cmd")"
  if [[ -z "$codex_version" ]]; then
    echo "Error: could not determine codex CLI version from $codex_cmd --version" >&2
    exit 1
  fi
  if [[ -n "$CLAWBLOX_CODEX_VERSION_PIN" && "$codex_version" != "$CLAWBLOX_CODEX_VERSION_PIN" ]]; then
    echo "Error: Codex CLI version $codex_version does not match pin $CLAWBLOX_CODEX_VERSION_PIN." >&2
    echo "Set CLAWBLOX_CODEX_BIN to the pinned binary, or set CLAWBLOX_CODEX_VERSION_PIN= to opt out." >&2
    exit 1
  fi
  canonical_path "$codex_cmd"
}

append_prompt_workspace_files() {
  local prompt_file="$1"
  local workspace_dir="$2"
  local -a preferred_files=(SOUL.md IDENTITY.md SEMANTIC_MEMORY.md EPISODIC_MEMORY.md)
  local -a prompt_files=()
  local rel_path
  local -A seen_files=()

  for rel_path in "${preferred_files[@]}"; do
    if [[ "$SKIP_SOUL" == "1" && "$rel_path" == "SOUL.md" ]]; then
      continue
    fi
    if [[ -f "$workspace_dir/$rel_path" ]]; then
      prompt_files+=("$rel_path")
      seen_files["$rel_path"]=1
    fi
  done

  while IFS= read -r -d '' file_path; do
    rel_path="${file_path#"$workspace_dir"/}"
    if [[ "$SKIP_SOUL" == "1" && "$rel_path" == "SOUL.md" ]]; then
      continue
    fi
    if [[ -n "${seen_files[$rel_path]:-}" ]]; then
      continue
    fi
    prompt_files+=("$rel_path")
    seen_files["$rel_path"]=1
  done < <(find "$workspace_dir" -type f -name '*.md' -print0 | sort -z)

  local episodic_jsonl="$workspace_dir/EPISODIC_MEMORY.jsonl"
  local has_episodic_jsonl=0
  if [[ -f "$episodic_jsonl" && -s "$episodic_jsonl" ]]; then
    has_episodic_jsonl=1
  fi

  if ((${#prompt_files[@]} == 0 && has_episodic_jsonl == 0)); then
    return 0
  fi

  {
    printf '\nInitial workspace memory files at session start:\n'
    printf 'These are the file contents as they existed when this session began.\n'
  } >>"$prompt_file"

  for rel_path in "${prompt_files[@]}"; do
    {
      printf '\n===== %s =====\n' "$rel_path"
      cat "$workspace_dir/$rel_path"
      printf '\n'
    } >>"$prompt_file"
  done

  if ((has_episodic_jsonl == 1)); then
    {
      printf '\n===== EPISODIC_MEMORY.jsonl (summaries) =====\n'
      python3 - "$episodic_jsonl" <<'PY'
import json, sys
path = sys.argv[1]
with open(path, encoding="utf-8") as f:
    for i, raw in enumerate(f, 1):
        line = raw.strip()
        if not line:
            continue
        try:
            obj = json.loads(line)
        except json.JSONDecodeError as e:
            print(f"[warning: EPISODIC_MEMORY.jsonl line {i} could not be parsed: {e}]", file=sys.stderr)
            continue
        ts = obj.get("timestamp", "?")
        summary = obj.get("summary", "?")
        print(f"{ts} - {summary}")
PY
      printf '\n'
    } >>"$prompt_file"
  fi
}

build_initial_prompt() {
  local output="$1"
  printf '%s\n' "$CODEX_INITIAL_PROMPT" >"$output"
}

codex_sandbox_mode() {
  if [[ "$SANDBOX" == "1" ]]; then
    printf '%s\n' "workspace-write"
  else
    printf '%s\n' "danger-full-access"
  fi
}

codex_approval_policy() {
  case "$CODEX_APPROVAL_POLICY" in
    bypassPermissions|bypass|never)
      printf '%s\n' "never"
      ;;
    on-request|untrusted|on-failure)
      printf '%s\n' "$CODEX_APPROVAL_POLICY"
      ;;
    acceptEdits|default)
      printf '%s\n' "on-request"
      ;;
    *)
      echo "Error: unsupported CODEX approval policy '$CODEX_APPROVAL_POLICY'." >&2
      exit 1
      ;;
  esac
}

prepare_codex_home() {
  mkdir -p "$CODEX_HOME_DIR" "$CODEX_HOME_DIR/log" "$CODEX_HOME_DIR/sessions" "$CODEX_HOME_DIR/tmp"

  if [[ "$SANDBOX" == "1" ]]; then
    if [[ -f "$HOME/.codex/auth.json" ]]; then
      cp "$HOME/.codex/auth.json" "$CODEX_HOME_DIR/auth.json"
      chmod 600 "$CODEX_HOME_DIR/auth.json"
    fi
    if [[ -d "$HOME/.codex/skills" && ! -e "$CODEX_HOME_DIR/skills" ]]; then
      cp -a "$HOME/.codex/skills" "$CODEX_HOME_DIR/skills"
    fi
    if [[ -d "$HOME/.codex/rules" && ! -e "$CODEX_HOME_DIR/rules" ]]; then
      cp -a "$HOME/.codex/rules" "$CODEX_HOME_DIR/rules"
    fi
  else
    if [[ -f "$HOME/.codex/auth.json" && ! -e "$CODEX_HOME_DIR/auth.json" ]]; then
      ln -s "$HOME/.codex/auth.json" "$CODEX_HOME_DIR/auth.json"
    fi
    if [[ -d "$HOME/.codex/skills" && ! -e "$CODEX_HOME_DIR/skills" ]]; then
      ln -s "$HOME/.codex/skills" "$CODEX_HOME_DIR/skills"
    fi
    if [[ -d "$HOME/.codex/rules" && ! -e "$CODEX_HOME_DIR/rules" ]]; then
      ln -s "$HOME/.codex/rules" "$CODEX_HOME_DIR/rules"
    fi
  fi

  python3 \
    - "$CODEX_HOME_DIR/config.toml" "$CODEX_EFFECTIVE_WORKSPACE_DIR" "$PROMPT_FILE" "$SANDBOX" "${CLAWBLOX_WORLD_HTTP_PROXY_PORT:-}" <<'PY'
import pathlib
import os
import json
import sys

target = pathlib.Path(sys.argv[1])
workspace = os.path.realpath(sys.argv[2])
prompt_text = pathlib.Path(sys.argv[3]).read_text(encoding="utf-8")
sandbox_enabled = sys.argv[4] == "1"
proxy_port = sys.argv[5].strip()

parts = []
parts.extend(
    [
        f"developer_instructions = {json.dumps(prompt_text)}",
        "",
    ]
)
if sandbox_enabled and proxy_port:
    parts.extend(
        [
            'default_permissions = "workspace"',
            "",
        ]
    )

features = []
if sandbox_enabled and proxy_port:
    features.append('network_proxy = true')
features.append('hooks = true')
if features:
    parts.extend(["[features]", *features, ""])

if sandbox_enabled and proxy_port:
    parts.extend(
        [
            "[sandbox_workspace_write]",
            'writable_roots = ["/workspace"]',
            "network_access = true",
            "",
            "[permissions.workspace.network]",
            "enabled = true",
            f'proxy_url = "http://127.0.0.1:{proxy_port}"',
            "enable_socks5 = false",
            "allow_upstream_proxy = false",
            "allow_local_binding = true",
            "",
            "[permissions.workspace.network.domains]",
            '"world" = "allow"',
            "",
        ]
    )

parts.extend(
    [
        f"[projects.{workspace!r}]",
        'trust_level = "trusted"',
        "",
    ]
)

target.write_text("\n".join(parts), encoding="utf-8")
PY
}

prepare_codex_hooks() {
  local stop_hook_script="$CODEX_HOME_DIR/stop_hook.py"
  local hooks_file="$CODEX_HOME_DIR/hooks.json"
  local stop_hook_command="$stop_hook_script"

  if [[ "$SANDBOX" == "1" ]]; then
    stop_hook_command="/home/agent/.codex/stop_hook.py"
  fi

  cat >"$stop_hook_script" <<'PY'
#!/usr/bin/env python3
import json
import os
import sys
from pathlib import Path

payload = json.load(sys.stdin)
log_dir = os.environ.get("CLAWBLOX_CODEX_LOG_DIR", "")
session_id = str(payload.get("session_id") or "").strip()

if log_dir:
    path = Path(log_dir)
    path.mkdir(parents=True, exist_ok=True)
    raw = json.dumps(payload)
    (path / "stop.json").write_text(raw + "\n", encoding="utf-8")
    if session_id:
        (path / f"stop.{session_id}.json").write_text(raw + "\n", encoding="utf-8")

auto_loop = os.environ.get("CLAWBLOX_CODEX_AUTO_LOOP", "0")
auto_loop_message = os.environ.get("CODEX_AUTO_LOOP_MESSAGE", "")

if not auto_loop_message:
    auto_loop_message = "Please don't stop. Just continue doing whatever you want to do."

if auto_loop == "1":
    print(json.dumps({
        "decision": "block",
        "reason": auto_loop_message,
    }))
    raise SystemExit(0)

raise SystemExit(0)
PY
  chmod +x "$stop_hook_script"

  cat >"$hooks_file" <<EOF
{
  "hooks": {
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "python3 $stop_hook_command"
          }
        ]
      }
    ]
  }
}
EOF
}

prepare_sandbox_hosts_file() {
  SANDBOX_HOSTS_FILE="$RUNTIME_DIR/sandbox_hosts"
  python3 - "$SANDBOX_HOSTS_FILE" "$WORLD_BASE_URL" <<'PY'
import sys
from urllib.parse import urlparse

target = sys.argv[1]
world_base_url = sys.argv[2].strip()
world_host = (urlparse(world_base_url).hostname or "").strip()
try:
    with open("/etc/hosts", encoding="utf-8") as fh:
        hosts = fh.read()
except OSError:
    hosts = "127.0.0.1 localhost\n"
if world_host and world_host not in {"localhost", "127.0.0.1", "::1"} and world_host not in hosts:
    hosts = hosts.rstrip() + f"\n127.0.0.1 {world_host}\n"
with open(target, "w", encoding="utf-8") as fh:
    fh.write(hosts)
PY
}

prepare_outer_sandbox() {
  if [[ "$SANDBOX" != "1" ]]; then
    return 0
  fi
  if ! command -v bwrap >/dev/null 2>&1; then
    echo "Error: SANDBOX=1 requires bubblewrap (bwrap)." >&2
    exit 1
  fi
  if [[ -n "$SANDBOX_DEPS_ROOT" && ! -d "$SANDBOX_DEPS_ROOT/bin" ]]; then
    echo "Error: SANDBOX_DEPS_ROOT must contain a bin directory: $SANDBOX_DEPS_ROOT" >&2
    exit 1
  fi
  mkdir -p \
    "$SANDBOX_HOME_DIR/.local/bin" \
    "$SANDBOX_HOME_DIR/.codex/log" \
    "$SANDBOX_HOME_DIR/.codex/sessions" \
    "$SANDBOX_HOME_DIR/.codex/tmp" \
    "$SANDBOX_HOME_DIR/.codex/cache" \
    "$SANDBOX_HOME_DIR/.codex/memories"
  SANDBOX_WORKSPACE_DIR="/workspace"
  CODEX_HOME_ROOT="$SANDBOX_HOME_DIR"
  CODEX_HOME_DIR="$CODEX_HOME_ROOT/.codex"
  HISTORY_FILE="$CODEX_HOME_DIR/history.jsonl"
  ln -sfn "$SANDBOX_CODEX_BIN" "$SANDBOX_HOME_DIR/.local/bin/codex"
  prepare_sandbox_hosts_file
}

codex_env_args() {
  local effective_home_root="$1"
  local runtime_dir="$2"
  local log_dir="$3"
  local -a env_prefix
  env_prefix=(
    HOME="$effective_home_root"
    CLAWBLOX_CODEX_RUNTIME_DIR="$runtime_dir"
    CLAWBLOX_CODEX_LOG_DIR="$log_dir"
    CLAWBLOX_CODEX_AUTO_LOOP="${CLAWBLOX_CODEX_AUTO_LOOP:-1}"
    CODEX_AUTO_LOOP_MESSAGE="$CODEX_AUTO_LOOP_MESSAGE"
    WORLD_BASE_URL="$WORLD_BASE_URL"
    WORLD_SESSION_ID="$WORLD_SESSION_ID"
    WORLD_AGENT_NAME="$WORLD_AGENT_NAME"
  )
  printf '%s\0' "${env_prefix[@]}"
}

build_bwrap_prefix() {
  if [[ "$SANDBOX" != "1" ]]; then
    return 0
  fi
  local sandbox_path="/home/agent/.local/bin:/sandbox-bin:/usr/bin:/bin"
  local sandbox_ld_library_path=""
  if [[ -n "$SANDBOX_DEPS_ROOT" ]]; then
    sandbox_path="/home/agent/.local/bin:/sandbox-bin:/sandbox-deps/bin:/usr/bin:/bin"
    if [[ -d "$SANDBOX_DEPS_ROOT/lib" ]]; then
      sandbox_ld_library_path="/sandbox-deps/lib"
    fi
  fi
  BWRAP_PREFIX=(
    bwrap
    --ro-bind /usr /usr
    --ro-bind /bin /bin
    --ro-bind /sbin /sbin
    --ro-bind /lib /lib
    --ro-bind /etc /etc
    --ro-bind /run /run
    --proc /proc
    --dev /dev
    --tmpfs /tmp
    --dir /sandbox-bin
    --dir /logs
    --dir /runtime
    --dir /home
    --bind "$WORKSPACE_DIR" /workspace
    --bind "$SANDBOX_HOME_DIR" /home/agent
    --bind "$LOG_DIR" /logs
    --bind "$RUNTIME_DIR" /runtime
    --setenv HOME /home/agent
    --setenv USER agent
    --setenv LOGNAME agent
    --setenv SHELL /bin/bash
    --setenv PATH "$sandbox_path"
    --setenv WORLD_BASE_URL "$WORLD_BASE_URL"
    --setenv WORLD_SESSION_ID "$WORLD_SESSION_ID"
    --setenv WORLD_AGENT_NAME "$WORLD_AGENT_NAME"
    --setenv CLAWBLOX_CODEX_RUNTIME_DIR "$SANDBOX_RUNTIME_DIR"
    --setenv CLAWBLOX_CODEX_LOG_DIR "$SANDBOX_LOG_DIR"
    --setenv CLAWBLOX_CODEX_AUTO_LOOP "${CLAWBLOX_CODEX_AUTO_LOOP:-1}"
    --setenv CODEX_AUTO_LOOP_MESSAGE "$CODEX_AUTO_LOOP_MESSAGE"
    --chdir /workspace
    --die-with-parent
  )
  if [[ -n "${CODEX_BIND_ROOT:-}" ]]; then
    BWRAP_PREFIX+=(--ro-bind "$CODEX_BIND_ROOT" "$SANDBOX_CODEX_ROOT")
  else
    BWRAP_PREFIX+=(--ro-bind "$CODEX_BIN_REAL" "$SANDBOX_CODEX_BIN")
  fi
  [[ -n "$SANDBOX_DEPS_ROOT" ]] && BWRAP_PREFIX+=(--ro-bind "$SANDBOX_DEPS_ROOT" /sandbox-deps)
  [[ -n "$sandbox_ld_library_path" ]] && BWRAP_PREFIX+=(--setenv LD_LIBRARY_PATH "$sandbox_ld_library_path")
  [[ -d /lib64 ]] && BWRAP_PREFIX+=(--ro-bind /lib64 /lib64)
  [[ -n "${SANDBOX_HOSTS_FILE:-}" && -f "$SANDBOX_HOSTS_FILE" ]] && BWRAP_PREFIX+=(--ro-bind "$SANDBOX_HOSTS_FILE" /etc/hosts)
  [[ -f /mnt/wsl/resolv.conf ]] && BWRAP_PREFIX+=(--ro-bind /mnt/wsl/resolv.conf /mnt/wsl/resolv.conf)
  BWRAP_PREFIX+=(--)
}

trust_codex_hooks() {
  [[ -f "$CODEX_HOME_DIR/hooks.json" ]] || return 0

  local trust_json trust_updates_json
  trust_json="$(
    python3 - "$CODEX_HOME_ROOT" "$WORKSPACE_DIR" <<'PY'
import json
import os
import subprocess
import sys

codex_home_root = sys.argv[1]
cwd = sys.argv[2]
env = dict(os.environ)
env["HOME"] = codex_home_root
proc = subprocess.Popen(
    ["codex", "app-server", "--listen", "stdio://"],
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    stderr=subprocess.PIPE,
    text=True,
    env=env,
)

def send(obj):
    proc.stdin.write(json.dumps(obj) + "\n")
    proc.stdin.flush()

def recv_until(response_id, limit=200):
    for _ in range(limit):
        line = proc.stdout.readline()
        if not line:
            raise RuntimeError("app-server exited before responding")
        msg = json.loads(line)
        if msg.get("id") == response_id:
            return msg
    raise RuntimeError("timed out waiting for app-server response")

send({
    "jsonrpc": "2.0",
    "id": 1,
    "method": "initialize",
    "params": {"clientInfo": {"name": "clawblox", "title": "clawblox", "version": "0.1"}},
})
recv_until(1)
send({"jsonrpc": "2.0", "method": "initialized", "params": {}})
send({"jsonrpc": "2.0", "id": 2, "method": "hooks/list", "params": {"cwds": [cwd]}})
response = recv_until(2)
proc.terminate()
try:
    proc.wait(timeout=5)
except subprocess.TimeoutExpired:
    proc.kill()
    proc.wait(timeout=5)

hooks = response.get("result", {}).get("data", [{}])[0].get("hooks", [])
for hook in hooks:
    if hook.get("eventName") == "stop":
        print(json.dumps({
            "key": hook.get("key"),
            "current_hash": hook.get("currentHash"),
            "trust_status": hook.get("trustStatus"),
        }))
        raise SystemExit(0)
raise SystemExit(1)
PY
  )" || return 0

  if [[ "$SANDBOX" == "1" ]]; then
    trust_updates_json="$(
      python3 - "$trust_json" <<'PY'
import json
import sys

trust = json.loads(sys.argv[1])
current_hash = str(trust.get("current_hash") or "").strip()
host_key = str(trust.get("key") or "").strip()
if not current_hash:
    print("[]", end="")
    raise SystemExit(0)

updates = []
seen = set()
for key in (host_key, "/home/agent/.codex/hooks.json:stop:0:0"):
    key = str(key or "").strip()
    if not key or key in seen:
        continue
    seen.add(key)
    updates.append({"key": key, "current_hash": current_hash})
print(json.dumps(updates), end="")
PY
    )"
  else
    trust_updates_json="$(
      python3 - "$trust_json" <<'PY'
import json
import sys

trust = json.loads(sys.argv[1])
key = str(trust.get("key") or "").strip()
current_hash = str(trust.get("current_hash") or "").strip()
updates = []
if key and current_hash:
    updates.append({"key": key, "current_hash": current_hash})
print(json.dumps(updates), end="")
PY
    )"
  fi

  python3 - "$CODEX_HOME_DIR/config.toml" "$trust_updates_json" <<'PY'
import json
import pathlib
import sys

config_path = pathlib.Path(sys.argv[1])
trust_updates = json.loads(sys.argv[2])
if not trust_updates:
    raise SystemExit(0)

text = config_path.read_text(encoding="utf-8")
new_text = text.rstrip()

for trust in trust_updates:
    key = str(trust.get("key") or "").strip()
    current_hash = str(trust.get("current_hash") or "").strip()
    if not key or not current_hash:
        continue
    block = f'[hooks.state.{json.dumps(key)}]\ntrusted_hash = {json.dumps(current_hash)}\n'
    marker = f'[hooks.state.{json.dumps(key)}]'
    if marker in new_text:
        head, _, tail = new_text.partition(marker)
        tail_lines = tail.splitlines()
        if tail_lines:
            tail_lines = tail_lines[2:]
        new_text = head.rstrip() + "\n\n" + block
        if tail_lines:
            new_text += "\n" + "\n".join(tail_lines).lstrip("\n")
    else:
        new_text = new_text.rstrip() + "\n\n" + block

config_path.write_text(new_text, encoding="utf-8")
PY
}

wait_for_session_metadata() {
  local deadline=$((SECONDS + 30))
  while ((SECONDS < deadline)); do
    sync_latest_session_artifacts
    if [[ -s "$SESSION_ID_FILE" ]]; then
      return 0
    fi
    sleep 1
  done
  return 1
}

start_session_metadata_watcher() {
  (
    while true; do
      sync_latest_session_artifacts
      sleep 1
    done
  ) &
  SESSION_METADATA_WATCHER_PID=$!
}

stop_session_metadata_watcher() {
  if [[ -n "${SESSION_METADATA_WATCHER_PID:-}" ]]; then
    kill "$SESSION_METADATA_WATCHER_PID" >/dev/null 2>&1 || true
    wait "$SESSION_METADATA_WATCHER_PID" 2>/dev/null || true
    SESSION_METADATA_WATCHER_PID=""
  fi
}

sync_latest_session_artifacts() {
  if [[ ! -f "$HISTORY_FILE" ]]; then
    return 0
  fi
  local latest_session_id=""
  latest_session_id="$(
    python3 - "$HISTORY_FILE" <<'PY'
import json
import sys

path = sys.argv[1]
session_id = ""
with open(path, encoding="utf-8") as handle:
    for raw in handle:
        raw = raw.strip()
        if not raw:
            continue
        try:
            obj = json.loads(raw)
        except json.JSONDecodeError:
            continue
        value = str(obj.get("session_id") or "").strip()
        if value:
            session_id = value
print(session_id, end="")
PY
  )"
  if [[ -n "$latest_session_id" ]]; then
    printf '%s\n' "$latest_session_id" >"$SESSION_ID_FILE"
    local transcript_path=""
    transcript_path="$(
      python3 - "$CODEX_HOME_DIR/sessions" "$latest_session_id" <<'PY'
import sys
from pathlib import Path

sessions_dir = Path(sys.argv[1])
session_id = sys.argv[2].strip()
matches = sorted(sessions_dir.rglob(f"*{session_id}.jsonl"))
if matches:
    print(matches[-1], end="")
PY
    )"
    if [[ -n "$transcript_path" && -f "$transcript_path" ]]; then
      mkdir -p "$SESSION_TRANSCRIPT_DIR"
      cp "$transcript_path" "$SESSION_TRANSCRIPT_DIR/${latest_session_id}.jsonl" 2>/dev/null || true
    fi
  fi
}

run_codex_start() {
  local prompt="$1"
  local sandbox_mode approval_policy effective_workspace
  local -a args env_prefix

  sandbox_mode="$(codex_sandbox_mode)"
  approval_policy="$(codex_approval_policy)"
  effective_workspace="${CODEX_EFFECTIVE_WORKSPACE_DIR:-$WORKSPACE_DIR}"
  args=(
    --no-alt-screen
    -C "$effective_workspace"
    -m "$CODEX_MODEL"
    -s "$sandbox_mode"
    -a "$approval_policy"
  )
  if [[ -n "$CODEX_EXTRA_ARGS" ]]; then
    # shellcheck disable=SC2206
    args+=( $CODEX_EXTRA_ARGS )
  fi
  IFS=$'\0' read -r -d '' -a env_prefix < <(codex_env_args "$CODEX_HOME_ROOT" "$RUNTIME_DIR" "$LOG_DIR" && printf '\0')
  (
    cd "$WORKSPACE_DIR"
    env "${env_prefix[@]}" "${BWRAP_PREFIX[@]}" "${CODEX_CMD[@]}" "${args[@]}" "$prompt"
  )
}

run_codex_resume() {
  local session_id="$1"
  local prompt="$2"
  local sandbox_mode approval_policy effective_workspace
  local -a args env_prefix

  sandbox_mode="$(codex_sandbox_mode)"
  approval_policy="$(codex_approval_policy)"
  effective_workspace="${CODEX_EFFECTIVE_WORKSPACE_DIR:-$WORKSPACE_DIR}"
  args=(
    resume
    --no-alt-screen
    -C "$effective_workspace"
    -m "$CODEX_MODEL"
    -s "$sandbox_mode"
    -a "$approval_policy"
  )
  if [[ -n "$CODEX_EXTRA_ARGS" ]]; then
    # shellcheck disable=SC2206
    args+=( $CODEX_EXTRA_ARGS )
  fi
  IFS=$'\0' read -r -d '' -a env_prefix < <(codex_env_args "$CODEX_HOME_ROOT" "$RUNTIME_DIR" "$LOG_DIR" && printf '\0')
  (
    cd "$WORKSPACE_DIR"
    env "${env_prefix[@]}" "${BWRAP_PREFIX[@]}" "${CODEX_CMD[@]}" "${args[@]}" "$session_id" "$prompt"
  )
}

timeout_run_codex_start() {
  local timeout_seconds="$1"
  local prompt="$2"
  local sandbox_mode approval_policy effective_workspace
  local -a args env_prefix
  sandbox_mode="$(codex_sandbox_mode)"
  approval_policy="$(codex_approval_policy)"
  effective_workspace="${CODEX_EFFECTIVE_WORKSPACE_DIR:-$WORKSPACE_DIR}"
  args=(
    --no-alt-screen
    -C "$effective_workspace"
    -m "$CODEX_MODEL"
    -s "$sandbox_mode"
    -a "$approval_policy"
  )
  if [[ -n "$CODEX_EXTRA_ARGS" ]]; then
    # shellcheck disable=SC2206
    args+=( $CODEX_EXTRA_ARGS )
  fi
  IFS=$'\0' read -r -d '' -a env_prefix < <(codex_env_args "$CODEX_HOME_ROOT" "$RUNTIME_DIR" "$LOG_DIR" && printf '\0')
  (
    cd "$WORKSPACE_DIR"
    timeout --foreground --signal=TERM "$timeout_seconds" \
      env "${env_prefix[@]}" "${BWRAP_PREFIX[@]}" "${CODEX_CMD[@]}" "${args[@]}" "$prompt"
  )
}

timeout_run_codex_resume() {
  local timeout_seconds="$1"
  local session_id="$2"
  local prompt="$3"
  local sandbox_mode approval_policy effective_workspace
  local -a args env_prefix
  sandbox_mode="$(codex_sandbox_mode)"
  approval_policy="$(codex_approval_policy)"
  effective_workspace="${CODEX_EFFECTIVE_WORKSPACE_DIR:-$WORKSPACE_DIR}"
  args=(
    resume
    --no-alt-screen
    -C "$effective_workspace"
    -m "$CODEX_MODEL"
    -s "$sandbox_mode"
    -a "$approval_policy"
  )
  if [[ -n "$CODEX_EXTRA_ARGS" ]]; then
    # shellcheck disable=SC2206
    args+=( $CODEX_EXTRA_ARGS )
  fi
  IFS=$'\0' read -r -d '' -a env_prefix < <(codex_env_args "$CODEX_HOME_ROOT" "$RUNTIME_DIR" "$LOG_DIR" && printf '\0')
  (
    cd "$WORKSPACE_DIR"
    timeout --foreground --signal=TERM "$timeout_seconds" \
      env "${env_prefix[@]}" "${BWRAP_PREFIX[@]}" "${CODEX_CMD[@]}" "${args[@]}" "$session_id" "$prompt"
  )
}

if [[ -z "$WORLD_AGENT_NAME" ]]; then
  WORLD_AGENT_NAME="$(generate_agent_name)"
fi

SAFE_AGENT_NAME="$(sanitize_name "$WORLD_AGENT_NAME")"
if [[ -z "$AGENT_DIR" ]]; then
  AGENT_DIR="$SCRIPT_DIR/codex-runs/$SAFE_AGENT_NAME"
fi
if [[ -z "$WORKSPACE_DIR" ]]; then
  WORKSPACE_DIR="$AGENT_DIR/workspace"
fi
if [[ -z "$SESSION_ID_FILE" ]]; then
  SESSION_ID_FILE="$AGENT_DIR/codex_session_id.txt"
fi
SESSION_TRANSCRIPT_DIR="$AGENT_DIR/session"
SANDBOX_HOME_DIR="$AGENT_DIR/sandbox-home"
SANDBOX_WORKSPACE_DIR="$WORKSPACE_DIR"

LOG_DIR="$AGENT_DIR/logs"
RUNTIME_DIR="$AGENT_DIR/runtime"
PROMPT_FILE="$RUNTIME_DIR/system_prompt.md"
INITIAL_PROMPT_FILE="$RUNTIME_DIR/initial_prompt.txt"
CODEX_HOME_ROOT="$AGENT_DIR/codex-home"
CODEX_HOME_DIR="$CODEX_HOME_ROOT/.codex"
CODEX_EFFECTIVE_WORKSPACE_DIR="$WORKSPACE_DIR"

mkdir -p "$WORKSPACE_DIR" "$LOG_DIR" "$RUNTIME_DIR"
HISTORY_FILE="$CODEX_HOME_DIR/history.jsonl"
CODEX_BIN_REAL="$(resolve_codex_bin)"
declare -a BWRAP_PREFIX=()
declare -a CODEX_CMD=("$CODEX_BIN_REAL")
CODEX_BIND_ROOT="$(codex_bind_root_for_bin "$CODEX_BIN_REAL")"
CODEX_BIND_REL="$(codex_bind_rel_for_bin "$CODEX_BIN_REAL")"

if [[ "$SKIP_SOUL" != "0" && "$SKIP_SOUL" != "1" ]]; then
  echo "Error: SKIP_SOUL must be 0 or 1 (got '$SKIP_SOUL')." >&2
  exit 1
fi

if [[ -d "$TEMPLATE_DIR" ]]; then
  for entry in "$TEMPLATE_DIR"/*; do
    [[ -e "$entry" ]] || continue
    if [[ "$SKIP_SOUL" == "1" && "$(basename "$entry")" == "SOUL.md" ]]; then
      continue
    fi
    dest="$WORKSPACE_DIR/$(basename "$entry")"
    if [[ ! -e "$dest" ]]; then
      cp -r "$entry" "$dest"
    fi
  done
fi
if [[ "$SKIP_SOUL" == "1" ]]; then
  rm -f "$WORKSPACE_DIR/SOUL.md"
fi

WORLD_SESSION_ID=""
EXISTING_WORLD_SESSION_ID=""
if [[ -n "$WORLD_JOIN_PATH" ]]; then
  join_headers=()
  if [[ -n "$WORLD_SESSION_FILE" && -f "$WORLD_SESSION_FILE" ]]; then
    EXISTING_WORLD_SESSION_ID="$(tr -d '[:space:]' < "$WORLD_SESSION_FILE")"
    if [[ -n "$EXISTING_WORLD_SESSION_ID" ]]; then
      join_headers=(-H "${WORLD_SESSION_HEADER}: ${EXISTING_WORLD_SESSION_ID}")
    fi
  fi
  join_json="$(curl -fsS -X POST "${join_headers[@]}" "${WORLD_INTERNAL_BASE_URL%/}${WORLD_JOIN_PATH}?name=${WORLD_AGENT_NAME}")"
  WORLD_SESSION_ID="$(node -e 'const o=JSON.parse(process.argv[1]); const key=process.argv[2]; process.stdout.write(String(o[key]||""));' "$join_json" "$WORLD_SESSION_FIELD")"
  if [[ -z "$WORLD_SESSION_ID" ]]; then
    echo "Failed to get session field '$WORLD_SESSION_FIELD' from: $join_json" >&2
    exit 1
  fi
  if [[ -n "$EXISTING_WORLD_SESSION_ID" && "$WORLD_SESSION_ID" != "$EXISTING_WORLD_SESSION_ID" ]]; then
    echo "Resume error: expected world session $EXISTING_WORLD_SESSION_ID but server returned $WORLD_SESSION_ID" >&2
    exit 1
  fi
  if [[ -n "$WORLD_SESSION_FILE" ]]; then
    printf '%s\n' "$WORLD_SESSION_ID" >"$WORLD_SESSION_FILE"
  fi
fi

cleanup() {
  sync_latest_session_artifacts
  if [[ -n "$WORLD_SESSION_ID" && -n "$WORLD_LEAVE_PATH" ]]; then
    curl -fsS -X POST -H "${WORLD_SESSION_HEADER}: ${WORLD_SESSION_ID}" "${WORLD_INTERNAL_BASE_URL%/}${WORLD_LEAVE_PATH}" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT INT TERM

if [[ -n "${CLAWBLOX_WORLD_HTTP_PROXY_PORT:-}" ]]; then
  WORLD_BASE_URL="$(
    python3 - "$WORLD_BASE_URL" "$CLAWBLOX_WORLD_HTTP_PROXY_PORT" <<'PY'
import sys
from urllib.parse import urlparse, urlunparse

base_url = sys.argv[1]
port = int(sys.argv[2])
parsed = urlparse(base_url)
host = parsed.hostname or ""
netloc = host
if parsed.username:
    auth = parsed.username
    if parsed.password:
        auth += ":" + parsed.password
    netloc = f"{auth}@{host}"
if netloc:
    netloc = f"{netloc}:{port}"
print(urlunparse(parsed._replace(netloc=netloc)), end="")
PY
  )"
fi

SESSION_LINE=""
if [[ -n "$WORLD_SESSION_ID" ]]; then
  SESSION_LINE="${WORLD_SESSION_HEADER}: ${WORLD_SESSION_ID}"
fi
SKILL_URL="${WORLD_BASE_URL%/}${WORLD_SKILL_PATH}"
if [[ "$WORLD_SKILL_PATH_WAS_DEFAULT" == "1" && "$WORLD_SKILL_PATH" == "/api.md" ]]; then
  skill_probe_headers=()
  if [[ -n "$WORLD_SESSION_ID" ]]; then
    skill_probe_headers=(-H "${WORLD_SESSION_HEADER}: ${WORLD_SESSION_ID}")
  fi
  if ! curl -fsS --max-time 2 "${skill_probe_headers[@]}" "${WORLD_INTERNAL_BASE_URL%/}${WORLD_SKILL_PATH}" >/dev/null 2>&1; then
    WORLD_SKILL_PATH="/skill.md"
    SKILL_URL="${WORLD_BASE_URL%/}${WORLD_SKILL_PATH}"
  fi
fi

SKILL_CURL="curl -sS"
if [[ -n "$WORLD_SESSION_ID" ]]; then
  SKILL_CURL="curl -sS -H '${WORLD_SESSION_HEADER}: ${WORLD_SESSION_ID}'"
fi

if [[ "$SANDBOX" == "1" ]]; then
  prepare_outer_sandbox
  CODEX_EFFECTIVE_WORKSPACE_DIR="$SANDBOX_WORKSPACE_DIR"
fi

export WORLD_AGENT_NAME SESSION_LINE WORLD_BASE_URL SKILL_CURL SKILL_URL

PROMPT_WORKSPACE_DIR="${SANDBOX_WORKSPACE_DIR:-$WORKSPACE_DIR}"
if [[ -f "$SYSTEM_PROMPT_TEMPLATE" ]]; then
  WORKSPACE_DIR="$PROMPT_WORKSPACE_DIR" \
    envsubst '${WORLD_AGENT_NAME} ${WORKSPACE_DIR} ${SESSION_LINE} ${WORLD_BASE_URL} ${SKILL_CURL} ${SKILL_URL}' \
    < "$SYSTEM_PROMPT_TEMPLATE" > "$PROMPT_FILE"
  if [[ "$SKIP_SOUL" == "1" ]]; then
    sed -i '/SOUL\.md/d' "$PROMPT_FILE"
  fi
else
  echo "Error: system prompt template not found at $SYSTEM_PROMPT_TEMPLATE" >&2
  exit 1
fi

if [[ -n "$WORLD_STARTUP_INSTRUCTIONS" ]]; then
  {
    printf '\nGOAL:\n'
    printf '%s\n' "$WORLD_STARTUP_INSTRUCTIONS"
  } >>"$PROMPT_FILE"
fi

append_prompt_workspace_files "$PROMPT_FILE" "$WORKSPACE_DIR"
build_initial_prompt "$INITIAL_PROMPT_FILE"
prepare_codex_home
prepare_codex_hooks
build_bwrap_prefix
if [[ "$SANDBOX" == "1" ]]; then
  if [[ -n "$CODEX_BIND_ROOT" ]]; then
    CODEX_CMD=("$SANDBOX_CODEX_ROOT$CODEX_BIND_REL")
  else
    CODEX_CMD=("$SANDBOX_CODEX_BIN")
  fi
fi
trust_codex_hooks

if [[ -n "$RESET_EVERY" ]]; then
  if ! RESET_EVERY="$(parse_duration_seconds "$RESET_EVERY")"; then
    echo "Error: RESET_EVERY must be a positive integer in seconds or a duration like 1h, 30m, or 90s." >&2
    exit 1
  fi
  if [[ ! "$RESET_EVERY" =~ ^[1-9][0-9]*$ ]]; then
    echo "Error: RESET_EVERY must resolve to a positive integer in seconds." >&2
    exit 1
  fi
fi

echo "Agent name: $WORLD_AGENT_NAME"
echo "Workspace: $WORKSPACE_DIR"
echo "Codex home: $CODEX_HOME_ROOT"
if [[ "$SANDBOX" == "1" ]]; then
  echo "Sandbox: Codex workspace-write"
else
  echo "Sandbox: Codex danger-full-access"
fi

cd "$WORKSPACE_DIR"

resume_existing=0
if [[ -s "$SESSION_ID_FILE" ]]; then
  case "$CODEX_RESUME_MODE" in
    resume) resume_existing=1 ;;
    auto) resume_existing=1 ;;
  esac
fi

if [[ -z "$RESET_EVERY" && -z "${DURATION_SECONDS:-}" ]]; then
  if ((resume_existing == 1)); then
    existing_session_id="$(tr -d '[:space:]' < "$SESSION_ID_FILE")"
    start_session_metadata_watcher
    set +e
    run_codex_resume "$existing_session_id" "$CODEX_INITIAL_PROMPT"
    main_status=$?
    set -e
    stop_session_metadata_watcher
    sync_latest_session_artifacts
    exit "$main_status"
  fi
  start_session_metadata_watcher
  set +e
  run_codex_start "$(cat "$INITIAL_PROMPT_FILE")"
  main_status=$?
  set -e
  stop_session_metadata_watcher
  sync_latest_session_artifacts
  exit "$main_status"
fi

if [[ -z "$RESET_EVERY" && -n "${DURATION_SECONDS:-}" ]]; then
  if ((resume_existing == 1)); then
    existing_session_id="$(tr -d '[:space:]' < "$SESSION_ID_FILE")"
    set +e
    timeout_run_codex_resume "$DURATION_SECONDS" "$existing_session_id" "$CODEX_INITIAL_PROMPT"
    main_status=$?
    set -e
  else
    set +e
    timeout_run_codex_start "$DURATION_SECONDS" "$(cat "$INITIAL_PROMPT_FILE")"
    main_status=$?
    set -e
  fi

  sync_latest_session_artifacts
  if [[ "$main_status" -ne 124 && "$main_status" -ne 143 ]]; then
    exit "$main_status"
  fi

  if wait_for_session_metadata; then
    session_id="$(tr -d '[:space:]' < "$SESSION_ID_FILE")"
    if [[ -n "$session_id" ]]; then
      echo "Sending shutdown warning..."
      set +e
      timeout_run_codex_resume "$CHECKPOINT_WARNING_SECONDS" "$session_id" "$CHECKPOINT_WARNING_PROMPT"
      set -e
      sync_latest_session_artifacts
    fi
  fi
  exit 0
fi

reset_index=0
while true; do
  echo "--- Reset cycle $reset_index ---"
  run_resume=0
  session_id=""
  prompt_text="$(cat "$INITIAL_PROMPT_FILE")"
  if ((reset_index == 0 && resume_existing == 1)); then
    run_resume=1
    session_id="$(tr -d '[:space:]' < "$SESSION_ID_FILE")"
    prompt_text="$CODEX_INITIAL_PROMPT"
  else
    rm -f "$SESSION_ID_FILE"
  fi

  set +e
  if ((run_resume == 1)); then
    timeout_run_codex_resume "$RESET_EVERY" "$session_id" "$prompt_text"
  else
    timeout_run_codex_start "$RESET_EVERY" "$prompt_text"
  fi
  main_status=$?
  set -e

  sync_latest_session_artifacts
  if [[ "$main_status" -ne 124 && "$main_status" -ne 143 ]]; then
    exit "$main_status"
  fi

  if wait_for_session_metadata; then
    session_id="$(tr -d '[:space:]' < "$SESSION_ID_FILE")"
    if [[ -n "$session_id" ]]; then
      echo "Sending checkpoint warning..."
      set +e
      timeout_run_codex_resume "$CHECKPOINT_WARNING_SECONDS" "$session_id" "$CHECKPOINT_WARNING_PROMPT"
      set -e
      sync_latest_session_artifacts
    fi
  fi

  reset_index=$((reset_index + 1))
  echo "Codex agent reset after ${RESET_EVERY}s (cycle $reset_index)"
done
