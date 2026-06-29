# Sandbox setup

The `--sandbox` flag wraps each Claude agent in a bubblewrap (`bwrap`) jail so
only the agent workspace is writable. Per-world sandbox settings live in
`world.toml`:

```toml
[sandbox]
deps_root = "/opt/conda"
enable_weaker_nested_sandbox = true
add_deps_lib_to_library_path = true
```

`deps_root` is optional. When set, Clawblox mounts that host prefix read-only at
`/sandbox-deps` inside the agent sandbox and adds `/sandbox-deps/bin` to `PATH`.
Use it for world runtimes installed outside system paths, such as a Docker image
with Python, MuJoCo, ROS, or DDS dependencies under `/opt/conda`.
Absolute dependency prefixes are also mounted read-only at their original path
so console scripts with absolute shebangs, such as Conda-generated entrypoints,
continue to work.
By default Clawblox also adds `/sandbox-deps/lib` to `LD_LIBRARY_PATH` for
compatibility with dependency prefixes that need it. Set
`add_deps_lib_to_library_path = false` when the prefix is self-contained enough
without a global library path, or when its libraries conflict with system
commands.

`enable_weaker_nested_sandbox` is off by default. Enable it only when running
Clawblox inside an unprivileged Docker or WSL2 container where standard
`bubblewrap` cannot create its namespace. This mode is weaker than the standard
sandbox and relies on the outer container as part of the isolation boundary.
The agent process still runs as a non-root user inside the nested sandbox.

The non-root mapping is intentional. Claude Code's autonomous permission mode
uses `--dangerously-skip-permissions`, and Claude refuses to run that mode as
root. Docker containers commonly run as root, so the nested sandbox maps the
agent to uid/gid `1000` inside the bubblewrap namespace. That keeps the desired
UX, where agents can run commands without approval prompts, while Clawblox's
outer sandbox remains the boundary.

On machines where you don't have root, install `bwrap` (and optionally `socat`)
via conda-forge:

```bash
conda create -y -n sandbox-deps -c conda-forge bubblewrap socat
conda activate sandbox-deps
```

Then launch with the env active so `bwrap` is on PATH:

```bash
conda activate sandbox-deps
clawblox run worlds/mesa-world --agent claude --sandbox
```

For new worlds, prefer `world.toml` over environment variables for sandbox
policy. `SANDBOX_DEPS_ROOT` remains supported as a fallback when
`sandbox.deps_root` is not set.

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

Before removing or changing the pin, run a sandboxed Claude agent with the
candidate Claude Code binary and verify multiple Bash tool calls inside the same
session:

```bash
CLAWBLOX_CLAUDE_BIN=/path/to/candidate/claude \
  clawblox run worlds/mesa-world --agent claude --sandbox
```

Expected outcomes:

- `PASS`: repeated Bash calls complete; the candidate is not showing the known
  fd-backed Bash failure.
- `FAIL`: the candidate reproduces `/proc/self/fd/3: Permission denied`; keep
  the pin.
- `UNKNOWN`: inspect the agent log; the run did not reach the
  known pass/fail markers.

Only unpin sandboxed TUI runs after the candidate returns `PASS` and a normal
one-agent sandboxed world run can use `curl http://world/...` through the world
capability proxy.
