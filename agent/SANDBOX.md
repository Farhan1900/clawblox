# Sandbox setup

The `--sandbox` flag wraps each Claude agent in a bubblewrap (`bwrap`) jail so
only the agent workspace is writable. On machines where you don't have root,
install `bwrap` (and optionally `socat`) via conda-forge:

```bash
conda create -y -n sandbox-deps -c conda-forge bubblewrap socat
conda activate sandbox-deps
```

Then launch with the env active so `bwrap` is on PATH:

```bash
conda activate sandbox-deps
bash agent/launch_multi_claude.sh --world-dir worlds/mesa-world --sandbox
```

## Claude Code version pin

Sandboxed Claude TUI runs currently pin Claude Code to `2.1.116` by default:

```bash
export CLAWBLOX_CLAUDE_CODE_VERSION_PIN=2.1.116
```

This is intentional. Claude Code `2.1.117` has a verified regression in the
interactive TUI path when Claude's native Linux sandbox is enabled. In that
configuration, the first Bash tool call succeeds, but the second Bash tool call
fails before the user command runs:

```text
/bin/bash: line 4: /proc/self/fd/3: Permission denied
```

The failure was reproduced with:

- Claude Code `2.1.117`, interactive TUI entrypoint, native sandbox enabled:
  fails on the second Bash call.
- The same `2.1.117` TUI setup without the world HTTP proxy: still fails.
- The same `2.1.117` TUI setup with `CLAUDE_BASH_NO_LOGIN=true`: still fails.
- The same `2.1.117` TUI setup with `CLAUDE_CODE_SHELL_PREFIX='bash -lc'`:
  still fails.
- Claude Code `2.1.116`, interactive TUI entrypoint, native sandbox enabled:
  both Bash calls succeed.
- Claude Code `2.1.117`, non-interactive `--print` entrypoint, native sandbox
  enabled: both Bash calls succeed.

That rules out the world proxy, curl, OAuth/setup-token auth, and project hooks
as the root cause. The operational root cause is a `2.1.117` interactive TUI
Bash-runner/native-sandbox regression around the fd-backed runner path. The pin
should be removed only after a newer Claude Code version passes the same
two-Bash-call TUI repro under native sandboxing.

For portability, the launcher does not assume every machine has Anthropic's
native version cache. Resolution order is:

1. `CLAWBLOX_CLAUDE_BIN`, if set, must point to an executable Claude Code binary.
2. `$HOME/.local/share/claude/versions/$CLAWBLOX_CLAUDE_CODE_VERSION_PIN`, if it
   exists.
3. `claude` on `PATH`.

For sandboxed native TUI runs, the resolved binary must report the pinned
version. On machines where `2.1.116` is installed somewhere else, set:

```bash
export CLAWBLOX_CLAUDE_BIN=/path/to/claude-2.1.116
```

To intentionally test a newer Claude Code build, clear or change the pin:

```bash
export CLAWBLOX_CLAUDE_CODE_VERSION_PIN=
```

Do this only for verification runs until the `2.1.117`-class Bash failure is
confirmed fixed.

### Retest before unpinning

Before removing or changing the pin, run the dedicated TUI/native-sandbox repro
against the candidate Claude Code binary:

```bash
CLAWBLOX_CLAUDE_BIN=/path/to/candidate/claude \
  bash agent/repro_claude_tui_native_sandbox_bash.sh
```

Expected outcomes:

- `PASS`: the second Bash call completed; the candidate is not showing the known
  fd-backed Bash failure.
- `FAIL`: the candidate reproduced `/proc/self/fd/3: Permission denied`; keep
  the pin.
- `UNKNOWN`: inspect the printed `typescript.log`; the run did not reach the
  known pass/fail markers.

Only unpin sandboxed TUI runs after the candidate returns `PASS` and a normal
one-agent sandboxed world run can use `curl http://world/...` through the world
capability proxy.
