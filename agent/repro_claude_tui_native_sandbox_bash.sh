#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CLAUDE_BIN="${CLAWBLOX_CLAUDE_BIN:-$(command -v claude 2>/dev/null || true)}"
MODEL="${CLAUDE_MODEL:-claude-opus-4-7}"
TIMEOUT_SECONDS="${TIMEOUT_SECONDS:-90}"
REPRO_ID="${REPRO_ID:-$(date +%Y%m%dT%H%M%S)}"
TMP_ROOT="${TMP_ROOT:-/tmp/clawblox-claude-tui-native-sandbox-repro-$REPRO_ID}"

if [[ -z "$CLAUDE_BIN" || ! -x "$CLAUDE_BIN" ]]; then
  echo "Error: set CLAWBLOX_CLAUDE_BIN to an executable Claude Code binary, or put claude on PATH." >&2
  exit 1
fi
if ! command -v bwrap >/dev/null 2>&1; then
  echo "Error: bwrap is required." >&2
  exit 1
fi
if ! command -v script >/dev/null 2>&1; then
  echo "Error: script is required." >&2
  exit 1
fi
if ! command -v python3 >/dev/null 2>&1; then
  echo "Error: python3 is required." >&2
  exit 1
fi

log_contains() {
  local pattern="$1"
  local file="$2"
  if command -v rg >/dev/null 2>&1; then
    rg -q "$pattern" "$file"
  else
    grep -Eq "$pattern" "$file"
  fi
}

if [[ -f "$ROOT_DIR/.env" ]]; then
  set -a
  # shellcheck disable=SC1091
  source "$ROOT_DIR/.env"
  set +a
fi

mkdir -p "$TMP_ROOT/home/.claude/projects" "$TMP_ROOT/home/.local/bin" "$TMP_ROOT/workspace" "$TMP_ROOT/runtime"
printf '%s\n' '{"sandbox":{"enabled":true,"failIfUnavailable":true,"allowUnsandboxedCommands":false,"autoAllowBashIfSandboxed":true,"network":{"allowedDomains":[],"deniedDomains":["localhost","127.0.0.1"]}}}' \
  >"$TMP_ROOT/runtime/settings.json"

if [[ -f "$HOME/.claude.json" ]]; then
  cp "$HOME/.claude.json" "$TMP_ROOT/home/.claude.json"
else
  printf '%s\n' '{}' >"$TMP_ROOT/home/.claude.json"
fi

python3 - "$TMP_ROOT/home/.claude.json" <<'PY'
import json, sys
path = sys.argv[1]
with open(path) as fh:
    data = json.load(fh)
data["hasCompletedOnboarding"] = True
data["lastOnboardingVersion"] = data.get("lastOnboardingVersion") or "2.1.116"
workspace = data.setdefault("projects", {}).setdefault("/workspace", {})
workspace["hasTrustDialogAccepted"] = True
workspace.setdefault("allowedTools", [])
workspace.setdefault("mcpContextUris", [])
workspace.setdefault("mcpServers", {})
workspace.setdefault("enabledMcpjsonServers", [])
workspace.setdefault("disabledMcpjsonServers", [])
with open(path, "w") as fh:
    json.dump(data, fh)
PY

ln -sfn /sandbox-bin/claude "$TMP_ROOT/home/.local/bin/claude"

declare -a bwrap_args=(
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
  --dir /home
  --dir /runtime
  --bind "$TMP_ROOT/home" /home/agent
  --bind "$TMP_ROOT/workspace" /workspace
  --bind "$TMP_ROOT/runtime" /runtime
  --ro-bind "$CLAUDE_BIN" /sandbox-bin/claude
  --setenv HOME /home/agent
  --setenv USER agent
  --setenv LOGNAME agent
  --setenv SHELL /bin/bash
  --setenv PATH /home/agent/.local/bin:/sandbox-bin:/usr/bin:/bin
  --unsetenv CLAUDE_CONFIG_DIR
  --unsetenv CLAUDE_CODE_EXECPATH
  --unsetenv CLAUDE_CODE_ENTRYPOINT
  --chdir /workspace
  --die-with-parent
)
[[ -d /lib64 ]] && bwrap_args+=(--ro-bind /lib64 /lib64)
[[ -f /mnt/wsl/resolv.conf ]] && bwrap_args+=(--ro-bind /mnt/wsl/resolv.conf /mnt/wsl/resolv.conf)
bwrap_args+=(--)

prompt="Use the Bash tool exactly twice, sequentially. First run: echo FIRST. Second run: echo SECOND. Then answer DONE and do nothing else."
log="$TMP_ROOT/typescript.log"
cmd_file="$TMP_ROOT/run_repro.sh"

{
  printf '#!/usr/bin/env bash\n'
  printf 'exec timeout --foreground %q ' "$TIMEOUT_SECONDS"
  printf '%q ' "${bwrap_args[@]}"
  printf '%q ' /sandbox-bin/claude
  printf '%q ' --settings /runtime/settings.json --model "$MODEL" --session-id 12121212-1212-4212-8212-121212121117 "$prompt"
  printf '\n'
} >"$cmd_file"
chmod +x "$cmd_file"

echo "Claude binary: $CLAUDE_BIN"
"$CLAUDE_BIN" --version || true
echo "Repro log: $log"

set +e
script -qfec "bash $(printf '%q' "$cmd_file")" "$log"
status=$?
set -e

if log_contains '/proc/self/fd/3: Permission denied' "$log"; then
  echo "FAIL: reproduced /proc/self/fd/3 native-sandbox Bash failure."
  exit 2
fi
if log_contains 'SECOND' "$log"; then
  echo "PASS: second Bash call completed without the known fd failure."
  exit 0
fi

echo "UNKNOWN: did not find the known fd failure or a completed SECOND output. Inspect $log."
exit "$status"
