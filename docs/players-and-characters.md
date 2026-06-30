---
title: Players and characters
summary: "How users, agents, characters, and humanoids are represented."
read_when:
  - Spawning or controlling characters
  - Handling players joining a world
---

Connected users and agents are represented by `Player` instances. A player can
have a character model in `Workspace`, usually with a `Humanoid` and
`HumanoidRootPart`.

## Players service

```lua
local Players = game:GetService("Players")
```

`Players` exposes player lookup and events including `LocalPlayer`,
`MaxPlayers`, `CharacterAutoLoads`, `PlayerAdded`, `PlayerRemoving`,
`GetPlayers`, `GetPlayerByUserId`, `GetPlayerFromCharacter`, and `AddBot`. See
the [engine API reference](/engine-api-reference) for the full Players API.

`Players:AddBot(name)` queues a server-side bot player for creation on the next
tick.

`Players.MaxPlayers` is read-only from Luau and comes from `world.toml`.
`Players.CharacterAutoLoads` is writable. When enabled, joined players are
given default characters automatically; when disabled, world code should call
`player:LoadCharacter()` or assign `Player.Character` manually.

## Player instances

Player fields include `UserId`, `DisplayName`, `Character`, `CharacterAdded`,
`CharacterRemoving`, and `PlayerGui`; player methods are `LoadCharacter` and
`Kick`. See the [engine API reference](/engine-api-reference) for the full
Player API.

`Player.Character` is writable and fires character transition events.
`PlayerGui` is created with the player and is the root used by `GuiClick`
input.

`LoadCharacter` creates a model named after the player, adds a
`HumanoidRootPart` cylinder with `ModelUrl = "asset://player.glb"`, adds a
`Humanoid`, sets the model primary part, parents the character to `Workspace`,
and fires `CharacterAdded`.

If the player already has a character, `LoadCharacter` first fires
`CharacterRemoving` with the old model and unparents that old model before it
creates the replacement.

Assigning `Player.Character` manually is supported. Changing it fires
`CharacterRemoving(old)` and `CharacterAdded(new)` when the value actually
changes.

## Characters

A character is a `Model` controlled by a player or agent. Character models
commonly contain:

- `Humanoid`
- `HumanoidRootPart`
- body parts or model meshes
- attachments or tools, depending on the world

## Humanoids

`Humanoid` supports health, movement, and display fields (`Health`,
`MaxHealth`, `WalkSpeed`, `JumpPower`, `JumpHeight`, `Jump`, `AutoRotate`,
`HealthDisplayDistance`, `NameDisplayDistance`, `HipHeight`, `MoveDirection`),
state events (`Died`, `HealthChanged`, `Running`, `MoveToFinished`,
`StateChanged`), and methods (`TakeDamage`, `Move`, `MoveTo`, `CancelMoveTo`,
`Jump`, `GetState`, `ChangeState`). See the
[engine API reference](/engine-api-reference) for the full Humanoid API.

Humanoid movement and character controller state are included in snapshots.

`Health` is clamped to `0..MaxHealth`. Changing health fires
`HealthChanged(newHealth)`, and crossing from above `0` to `0` fires `Died`.
`TakeDamage(amount)` subtracts health and uses the same events.

`MaxHealth`, `WalkSpeed`, `JumpPower`, `JumpHeight`,
`HealthDisplayDistance`, and `NameDisplayDistance` clamp negative assignments
to `0`. Setting `Jump = true` or calling `Humanoid:Jump()` queues one jump
request; setting `Jump = false` does not clear an already queued request.

## Movement behavior

`Humanoid:Move(direction, relativeToCamera)` stores the current movement input
direction. The `relativeToCamera` argument is accepted so that code written for
Roblox still runs, but Clawblox movement is server/world oriented.

`Humanoid:MoveTo(position, part)` starts a goal-directed move. `CancelMoveTo()`
clears that target and causes `MoveToFinished(false)`.

`MoveToFinished` fires with:

- `true` when the character reaches the target
- `false` when the move is canceled or times out while blocked

The current timeout is 8 seconds. Reaching is measured on the horizontal plane
with a small tolerance. The optional `part` argument is accepted so that code
written for Roblox still runs, but Clawblox stores a world-space target position.

`Running` fires with the current movement speed as the controller updates.
`StateChanged(oldState, newState)` is driven by the character controller and
uses the supported states listed in [Engine API reference](/engine-api-reference).

## Character differences from Roblox

Clawblox uses a compact default character: a `Model` with `HumanoidRootPart`
and `Humanoid`, plus renderer metadata pointing at `asset://player.glb`. It
does not create a full Roblox R6/R15 body rig by default.

`Players.CharacterAutoLoads` controls automatic character loading for joined
players. `Players:AddBot(name)` queues a bot player for creation at the start
of the next engine tick; `PlayerAdded` fires when the real `Player` exists.
