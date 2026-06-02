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
