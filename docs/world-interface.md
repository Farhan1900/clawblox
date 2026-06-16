# World Interface

A world is a directory with a `world.toml`.

```python
from clawblox import World

world = World(dir="worlds/mesa-small-world-3")
world.start(port=8085, record=True, record_dir="runs/g000/recordings")
```

`World(dir=...)` identifies the world. `world.start(...)` launches one running
instance of it.

## Before Start

`World(dir=...)` exposes everything known from `world.toml`:

```text
config          parsed world.toml as a plain dict
name            config["name"]
description     config.get("description")
scripts         config.get("scripts", {})
renderer        config.get("renderer", {})
runtime         config.get("runtime", {})
run             config.get("run", {})
run_command     config.get("run", {}).get("command")
api             agent API metadata known before launch
api_doc_path    API doc file path, currently scripts.skill or API.md
start_command   template command from world.toml, or the default Clawblox runner
start_env       template env for delegated start commands
```

This is static metadata. It should not require starting a server.

## After Start

After `start`, a world has:

```text
url             local URL for this running instance
port            local port for this running instance
command_file    rerunnable launch command
log_file        world process log
```

`world.start(...)` returns the same runtime details as data:

```python
started = world.start(port=8085, record=True, record_dir="runs/g000/recordings")

started.command         # concrete outer command that was run
started.start_command   # static/template command exposed by World(dir=...)
started.delegated       # true when world.toml [run].command is used
started.run_command     # command from world.toml, if any
started.env             # concrete env known for delegated commands
started.url
started.command_file
started.log_file
```

## Agent Access

Connect an agent to a running world:

```python
api_access = world.connect(agent=agent)
api_path = world.api_path(path="/api.md", access=api_access)
```

`api_access` is ordinary data: headers/session information that scripts can put
into prompts however they want.

The agent does not need to know what a world is. It only receives prompts and
paths.

## Schema Direction

The existing schema stays valid.

Today, `world.toml` can configure the built-in Clawblox engine or delegate
startup with:

```toml
[run]
command = ["bash", "./run-stack.sh"]
```

That is enough to launch external worlds because `clawblox run` starts the
command with a small environment contract:

```text
WORLD_HOST
WORLD_PORT
WORLD_BASE_PATH
WORLD_RECORD
WORLD_RECORD_DIR
WORLD_RESUME_PATH
WORLD_OPERATOR_TOKEN
CLAWBLOX_BIN
```

External runtimes should treat `WORLD_*` values as defaults. Explicit command
line flags should still override environment defaults. `CLAWBLOX_BIN` is
namespaced because it points back to the Clawblox executable rather than to a
world runtime setting.

If we add schema, it should be justified by engine-agnostic needs that the
current schema cannot express clearly:

```text
how to start one instance
how to check readiness
where the agent API docs are
how an agent obtains API access
optional recording and renderer commands
```

The reason to add this is not aesthetics. It lets MuJoCo, Minecraft, external
servers, and built-in Clawblox worlds use the same `World` interface without
pretending they all use the Clawblox Lua engine.

## Save / Load

A world supports save/load by implementing exactly two things:

```text
1. GET /snapshot      Return a complete, restorable state document for the
                      running instance. The format is world-owned and opaque
                      to Clawblox. Gate the endpoint: accept requests whose
                      X-Operator-Token header matches WORLD_OPERATOR_TOKEN.
                      Never document it in the agent-facing API.

2. WORLD_RESUME_PATH  If set at launch, restore full state from that file
                      before serving. The file contains a previous GET
                      /snapshot response body. (--resume as a CLI flag
                      should override the env value, like other WORLD_*.)
```

Vocabulary: a *snapshot* is the state document a world can produce and
consume; a *checkpoint* is a saved snapshot registered in the run manifest,
possibly bundled with agent state (see `docs/checkpoints.md`).

Everything else is generic and lives in Clawblox:

```python
ckpt = world.save(dir=run_dir / "checkpoints")   # GET /snapshot -> file
                                                 # + manifest "checkpoints" entry
world.start(port=8085, resume=ckpt["path"])      # sets WORLD_RESUME_PATH
```

Semantic requirements for a conforming world:

- Restore is exact. A restored instance is observationally identical to the
  instance at snapshot time: physics state, sim time, sessions (the same
  `X-Session` ids keep working without re-join), and chat history.
- If the snapshot is JSON with top-level `format` and `time` fields, Clawblox
  records them in the manifest; otherwise the checkpoint is registered with
  format `world-snapshot`.
- Snapshots should identify their world content (for example a scene hash)
  and refuse to restore into a mismatched world.
- Agent-visible payloads expose simulation time only, never host wall-clock
  time. Wall time both leaks the host and makes restores observable; the
  save/load design keeps checkpoints invisible to agents.

Recordings answer "what happened"; snapshots answer "where were we". Keep
history in the recording protocol and state in the snapshot — a snapshot
should not embed event history.

Large-state worlds: the v1 protocol returns state inline from `GET
/snapshot`. If a world's state is too large for that, a future revision may
let the endpoint return a file reference instead; do not invent a private
variant.

## Debugging Contract

World start failures should include:

```text
world directory
command_file
log_file
url or port
```

Research scripts should keep protocol choices visible: ports, recording paths,
prompt construction, agent access, timing, and validation should stay in the
script unless they are generic world mechanics.
