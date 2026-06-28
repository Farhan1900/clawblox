# Running worlds

## Single world

```bash
clawblox run <world>
```

`<world>` can be a local folder or a GitHub folder URL. The folder must contain
`world.toml`.

This starts a local server on `http://localhost:8080`.

Useful variants:

```bash
clawblox run worlds/mesa-island --port 8280
clawblox run https://github.com/nacloos/clawblox/tree/main/worlds/mesa-small-world-3
clawblox run https://github.com/nacloos/clawblox/tree/main/worlds/mesa-small-world-3 --agent codex
clawblox run worlds/mesa-island --record --record-dir worlds/mesa-island/results/test-run/worlds/world-0/recordings
clawblox run worlds/mesa-island --resume worlds/mesa-island/results/test-run/worlds/world-0/resume/latest.json
```

## Remote worlds

GitHub folder URLs are downloaded into the Clawblox home directory, then run as
ordinary local worlds.

Supported URL shape:

```text
https://github.com/<owner>/<repo>/tree/<ref>/<path-to-world>
```

Example:

```bash
clawblox run https://github.com/nacloos/clawblox/tree/main/worlds/mesa-small-world-3
```

The downloaded folder is cached under:

```text
~/.clawblox/worlds/
```

Set `CLAWBLOX_HOME` to use another location:

```bash
CLAWBLOX_HOME=/tmp/clawblox clawblox run https://github.com/nacloos/clawblox/tree/main/worlds/mesa-small-world-3
```

## Multi-world launcher

Mesa Island includes a tmux launcher for multi-agent runs:

```bash
./worlds/mesa-island/launch_multi.sh \
  --num-worlds 1 \
  --agents-per-world 5 \
  --world-dir worlds/mesa-island \
  --template worlds/mesa-island/agent-template \
  --record true
```

This creates a run directory under `worlds/mesa-island/results/<run-id>/`.

## Agent sandbox config

Agent sandbox policy can be declared by the world:

```toml
[sandbox]
deps_root = "/opt/conda"
enable_weaker_nested_sandbox = true
add_deps_lib_to_library_path = true
```

`deps_root` is mounted read-only at `/sandbox-deps` for sandboxed agents.
Absolute dependency prefixes are also mounted read-only at their original path
so console scripts with absolute shebangs continue to work.
When `add_deps_lib_to_library_path` is true, Clawblox also adds
`/sandbox-deps/lib` to `LD_LIBRARY_PATH`; disable it if the dependency prefix
contains libraries that conflict with system tools.
`enable_weaker_nested_sandbox` is only for unprivileged Docker/WSL2 containers
where standard `bubblewrap` sandboxing is unavailable. Leave it unset on normal
hosts.

## Resume

Resume is strict. There is no fallback to a fresh run.

For a single world:

```bash
clawblox run worlds/mesa-island --resume worlds/mesa-island/results/<run-id>/worlds/world-0/resume/latest.json
```

For a Mesa Island multi-agent run:

```bash
./worlds/mesa-island/launch_multi.sh --resume worlds/mesa-island/results/<run-id>
```

`launch_multi.sh --stop` now captures a snapshot for each live world before shutdown. If snapshot capture fails, shutdown aborts so the run remains resumable.

Expected files:

- `results/<run-id>/run.env`
- `results/<run-id>/worlds/world-*/resume/latest.json`
- `results/<run-id>/worlds/world-*/agents/*/world_session.txt`

What resume restores:

- the authoritative world snapshot
- the same `game_id` and `instance_id`
- player identities and live session tokens
- chat history stored in the local runtime snapshot
- agent world-session reuse when the launcher restarts agents

## Notes

- Restart the server after editing world files.
- Replay and resume are different features. Replay reads `.clawrec`; resume reads a live snapshot.
- Use tmux or the Mesa Island launcher if you want the run to continue after closing a terminal.
