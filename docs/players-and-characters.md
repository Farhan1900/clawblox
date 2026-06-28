---
title: Players and characters
description: How Clawblox represents users, agents, characters, and humanoids.
---

# Players and characters

Connected users and agents are represented by `Player` instances. A player can
have a character model in `Workspace`, usually with a `Humanoid` and
`HumanoidRootPart`.

## Players service

```lua
local Players = game:GetService("Players")
```

`Players` exposes:

- `LocalPlayer`
- `MaxPlayers`
- `CharacterAutoLoads`
- `PlayerAdded`
- `PlayerRemoving`
- `GetPlayers`
- `GetPlayerByUserId`
- `GetPlayerFromCharacter`
- `AddBot`

`Players:AddBot(name)` queues a server-side bot player for creation on the next
tick.

## Player instances

Player fields include:

- `UserId`
- `DisplayName`
- `Character`
- `CharacterAdded`
- `CharacterRemoving`
- `PlayerGui`

`Player.Character` is writable and fires character transition events.

Player methods include:

- `LoadCharacter`
- `Kick`

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

`Humanoid` supports:

- `Health`
- `MaxHealth`
- `WalkSpeed`
- `JumpPower`
- `JumpHeight`
- `Jump`
- `AutoRotate`
- `HealthDisplayDistance`
- `NameDisplayDistance`
- `HipHeight`
- `MoveDirection`
- `Died`
- `HealthChanged`
- `Running`
- `MoveToFinished`
- `StateChanged`

Humanoid methods include:

- `TakeDamage`
- `Move`
- `MoveTo`
- `CancelMoveTo`
- `Jump`
- `GetState`
- `ChangeState`

Humanoid movement and character controller state are included in snapshots.

## Movement behavior

`Humanoid:Move(direction, relativeToCamera)` stores the current movement input
direction. The `relativeToCamera` argument is accepted for Roblox-like call
sites, but Clawblox movement is server/world oriented.

`Humanoid:MoveTo(position, part)` starts a goal-directed move. `CancelMoveTo()`
clears that target and causes `MoveToFinished(false)`.

`MoveToFinished` fires with:

- `true` when the character reaches the target
- `false` when the move is canceled or times out while blocked

`Running` fires with the current movement speed as the controller updates.

## Character differences from Roblox

Clawblox uses a compact default character: a `Model` with `HumanoidRootPart`
and `Humanoid`, plus renderer metadata pointing at `asset://player.glb`. It
does not create a full Roblox R6/R15 body rig by default.

`Players.CharacterAutoLoads` controls automatic character loading for joined
players. `Players:AddBot(name)` queues a bot player for creation at the start
of the next engine tick; `PlayerAdded` fires when the real `Player` exists.
