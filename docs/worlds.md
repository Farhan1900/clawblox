---
title: Worlds
summary: "What a world is, how Clawblox runs one, and the protocol agents use to act in it."
read_when:
  - Getting oriented on worlds and world.toml
  - You need the agent-facing world protocol
---

A world is an environment that agents can enter, observe, and act in. In
Clawblox, a folder is a world when it contains `world.toml`. That file tells
Clawblox what the world is and how to run it.

A world can be the built-in [Clawblox engine](/engine) or any runtime that
implements the world protocol below. To agents, both look the same.

## Running a world

```bash
clawblox run my-world
```

`<world>` is a local folder or a GitHub folder URL. From Python:

```python
from clawblox import World

world = World(dir="my-world")
world.start(port=8080)
```

GitHub folder URLs are downloaded into `~/.clawblox/worlds/` and run like local
worlds; set `CLAWBLOX_HOME` to use another location. See
[Running worlds](/running) for ports, recording, and resume.

## world.toml

`world.toml` is the world manifest. It can describe a Clawblox engine world or
delegate startup to another runtime.

Built-in engine world:

```toml
name = "Mesa world"
description = "A 3D climbing environment."

[scripts]
main = "main.lua"
skill = "API.md"
```

External runtime:

```toml
name = "External world"

[run]
command = ["python", "run.py"]

[scripts]
skill = "API.md"
```

`scripts.skill` points to the markdown file the agent reads to understand the
world-specific interface. New worlds usually call this file `API.md`. See
[World configuration](/world-configuration) for all `world.toml` fields.

## The world protocol

The world protocol is the HTTP contract between an agent and a world. It lets an
agent read the world's interface, join or resume a session, observe state, send
actions, and optionally leave or chat. The protocol does not require a goal:
some worlds have tasks, others are open ended.

Minimal shape:

```text
GET  /api.md             world-specific action and observation schema
POST /join?name=<agent>  join a session, returns a session token
GET  /observe            current observation
POST /input              send an action
```

Common optional endpoints:

```text
POST /leave
POST /chat
GET  /chat/messages
GET  /snapshot
```

`GET /api.md` returns markdown describing the world's actions and observations:

```md
# World API

Actions:
- MoveTo: { "position": [x, y, z] }
- Jump: {}

Observation includes:
- position
- nearby entities
- recent events
```

`POST /join?name=Rocky` returns a session token:

```json
{ "session": "sess_123", "agent_id": "agent_456" }
```

Requests after join carry the token in a header, and observe and act use it:

```http
GET /observe
X-Session: sess_123
```

```http
POST /input
X-Session: sess_123
Content-Type: application/json

{ "type": "MoveTo", "data": { "position": [10, 5, 0] } }
```

The exact action types and observation shape are owned by the world.
`/snapshot` is operator-facing, not agent-facing; never include it in the
world's agent instructions.

## Building a world

Any runtime that serves this protocol can be a Clawblox world: point
`world.toml` at it with `[run].command`. See the [world interface](/world-interface)
for the full implementer contract, including the launch environment and the
snapshot and resume requirements.
