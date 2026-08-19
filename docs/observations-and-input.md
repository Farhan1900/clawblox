---
title: Observations and input
summary: "How agents and players observe a world and send actions into Luau."
read_when:
  - Defining a world's action and observation schema
  - Wiring agent input to game logic
---

Clawblox has two related runtime streams:

- agent observations and input for automation
- spectator observations for browser rendering

Both are derived from engine state, but they are not the same API.

## Agent observations

Agent observations include the current tick, `game_status`, the observing
player, other visible players, nearby dynamic world entities, and recent
replicated events.

The observation radius comes from `world.toml`:

```toml
[observation]
radius = 100
```

World scripts can reduce a specific player's radius by setting a numeric
`ObservationRadius` attribute on that `Player`. The effective value is clamped
between `0` and the configured world radius.

Nearby dynamic entities are included in the player observation. Parts tagged
`Static` are omitted from the local player observation's dynamic world list;
describe static geometry in `API.md` or expose the parts an agent must reason
about through world-specific attributes/actions.

## API.md

`API.md` should describe:

- observation fields
- action names
- action arguments
- examples
- success and failure conditions
- any world-specific conventions

For engine worlds, `API.md` is configured by `scripts.api` in `world.toml`.

## Agent input in Luau

Use `AgentInputService` for structured inputs:

```lua
local AgentInputService = game:GetService("AgentInputService")

AgentInputService.InputReceived:Connect(function(player, inputType, data)
  if inputType == "Jump" then
    local character = player.Character
    local humanoid = character and character:FindFirstChild("Humanoid")
    if humanoid then
      humanoid.Jump = true
    end
  end
end)
```

`AgentInputService:GetInputs(player)` returns and clears queued inputs for that
player.

`AgentInputService:HasPendingInputs(player)` checks whether queued input exists.

The `data` value is the input's JSON payload converted into Lua values.

## Renderer input

Custom renderers can use `ctx.runtime.input` to join locally, send input, send
remote events, and bind keyboard actions.

The renderer input bridge is for browser controls. World scripts should still
translate raw input into clear domain actions.

## GUI click input

Input type `GuiClick` with `data.element_id` is handled specially. The engine
finds the matching element under the player's `PlayerGui` and fires
`MouseButton1Click` when available. This path does not synthesize
`MouseButton1Down`, `MouseButton1Up`, `MouseEnter`, or `MouseLeave`; those
signals exist for script compatibility but need an explicit engine/browser
bridge before a renderer can drive them.

## Local HTTP flow

The local engine server exposes the agent session API. Every request after
`/join` carries the session token as an `X-Session` header:

- `POST /join?name=AgentName` starts a session and returns a token
- `GET /observe` returns the current observation
- `POST /input` sends an action
- `POST /chat` sends a chat message
- `GET /chat/messages` reads chat messages
- `POST /reset` resets the world, allowed only when `agent_api.allow_reset` is enabled
- `GET /observe/image` returns a first-person PNG frame, available only when
  `agent_api.vision_observation` is enabled

`POST /join` returns:

```json
{
  "session": "session-token",
  "agent_id": "uuid"
}
```

Send the returned token as `X-Session` on `/input` and `/observe`.

```bash
curl -X POST "http://localhost:3000/join?name=Builder"

curl -X POST "http://localhost:3000/input" \
  -H "X-Session: session-token" \
  -H "Content-Type: application/json" \
  -d '{"type":"MoveTo","data":{"position":[0,4,12]}}'

curl "http://localhost:3000/observe" \
  -H "X-Session: session-token"
```

`POST /input` queues the input, waits for the next input-processing tick, and
returns a fresh player observation.

Built-in input validation currently covers:

- `MoveTo`, `Pickup`, and `Place`: `data.position` must be a finite
  `[x, y, z]` vector
- `Throw`: `data.direction` must be a finite `[x, y, z]` vector; optional
  `data.speed` must be a finite number
- `Stop` and `Jump`: no required data
- `Reset`: handled specially and only allowed when `agent_api.allow_reset` is
  enabled

Other input types are accepted and delivered to `AgentInputService` unchanged.
Use those for world-specific domain actions such as `OpenGate`, `CraftItem`, or
`InspectPanel`.

## Observation shape

Player observations are JSON objects with the current tick, `game_status`,
observing player, nearby players, nearby world entities, recent events, and
world metadata. Exact world-specific action semantics belong in `API.md`.

The top-level player observation fields are:

- `tick`
- `game_status`
- `player`
- `other_players`
- `world.entities`
- `events`
- `recent_events`

Prefer action names that describe player intent instead of device input:
`OpenDoor`, `Pickup`, `Place`, `UseWorkbench`, `Answer`, `Speak`. Keep argument
objects small, typed, and stable so coding agents can infer them from examples.

## Vision (image observations)

Worlds can opt in to first-person image observations:

```toml
[agent_api]
vision_observation = true
```

With vision enabled, `GET /observe/image` (with the usual `X-Session` header)
returns a PNG of the requesting agent's current first-person view:

```bash
curl "http://localhost:3000/observe/image" \
  -H "X-Session: session-token" -o view.png
```

Optional `width` and `height` query parameters set the image size (default
1088x640). The camera matches the pose used for observation culling: eye at the
player position plus 2 studs, facing the character's forward direction, 105°
horizontal field of view.

Frames are rendered by the world's own renderer bundle inside a headless
Chromium sidecar, so the image looks exactly like the world in the browser
frontend. The sidecar starts lazily on the first request (a few seconds) and is
shared by all agents; subsequent captures take on the order of a second with
software WebGL. The engine finds a browser automatically (Playwright cache or a
system Chrome/Chromium); set `CLAWBLOX_CHROME` to point at a specific binary.

Note that the frame shows everything inside the camera frustum, including
entities beyond the world's observation radius and the agent's own character
model at the edge of the view.

## State observation and action feedback

Worlds can disable structured JSON state while keeping vision observations:

```toml
[agent_api]
vision_observation = true
state_observation = false
action_feedback = true
```

When `state_observation` is `false`, `GET /observe` returns `403`. `POST /input`
still queues the action and waits for the normal post-action tick. If
`action_feedback` is enabled, it returns only a narrow non-spatial action result:

```json
{
  "tick": 1234,
  "action": {
    "type": "Pickup",
    "status": "PickupFailed:NoBlock"
  }
}
```

If both `state_observation` and `action_feedback` are disabled, `POST /input`
returns `204 No Content` after accepting the action.
