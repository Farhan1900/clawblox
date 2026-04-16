# Claude Agent Prototype

This directory is an isolated proof-of-concept for running a mesa-island agent
with Claude Code as a separate backend from the existing `pi` launcher.

The goal is to validate the assumptions that matter for a real launcher:

- caller-controlled `session_id`
- per-agent workspace as Claude's cwd
- project-local plugin loading with `--plugin-dir`
- hook access to `session_id`, `transcript_path`, `cwd`, and failure details
- hook ability to persist environment variables for later Bash calls

The prototype is intentionally small. It focuses on lifecycle wiring and
observability, not full autonomy yet.

## Run

Example local probe without joining a world:

```bash
WORLD_JOIN_PATH='' \
AGENT_DIR="$PWD/claude-prototype/test-run" \
CLAUDE_EXTRA_ARGS='-p --output-format json --dangerously-skip-permissions' \
CLAUDE_INITIAL_PROMPT='Reply exactly with OK' \
./claude-prototype/run_claude_agent.sh
```

Artifacts appear under the chosen `AGENT_DIR`:

- `claude_session_id.txt`: caller-owned Claude session UUID
- `runtime/hook_session.env`: hook-captured session metadata
- `logs/session_start.json`: raw SessionStart hook payload
- `logs/user_prompt_submit.json`: raw UserPromptSubmit hook payload
- `logs/stop_failure.json`: raw StopFailure hook payload

For a real multi-world tmux launch, use:

```bash
./launch_multi_claude.sh \
  --tmux-session clawblox-claude \
  --base-port 8097 \
  --num-worlds 1 \
  --agents-per-world 1 \
  --world-dir worlds/mesa-island \
  --template worlds/mesa-island/agent-template
```

By default the launcher clears `ANTHROPIC_API_KEY` and `ANTHROPIC_OAUTH_TOKEN`
from the tmux agent panes so Claude uses the interactive account login already
stored in `~/.claude/`. Pass `--use-env-auth` only if you explicitly want env
credentials to win.

## Findings

Validated locally on April 4, 2026:

- `--session-id` creates a Claude transcript with the exact UUID requested.
- Subsequent runs must use `--resume <session-id>` instead of combining
  `--resume` with `--session-id`.
- `SessionStart`, `UserPromptSubmit`, and `StopFailure` hooks all fire in
  `claude -p` mode.
- `SessionStart` exposes `session_id`, `transcript_path`, `cwd`, and `source`.
- A resumed run reports `source=resume`.
- `--bare` is not suitable for a Max-account-backed launcher because it bypasses
  the normal Claude.ai credential path and behaved as unauthenticated here.

Still unverified in this prototype:

- whether hook-persisted environment variables are visible to later Bash tool
  calls in a real model turn
- whether channels are the best mechanism for world chat and checkpoint nudges

Updated finding:

- `Stop` hooks can block termination and force another Claude turn, but the
  follow-up behavior is only as good as the feedback text. A vague block reason
  produces an open-ended loop. A production launcher should prefer an outer
  supervisor loop over a generic always-continue stop hook.
