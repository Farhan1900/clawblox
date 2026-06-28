---
title: World design
description: Design guidance for Clawblox worlds played by humans and agents.
---

These are general guidelines for designing engaging Clawblox worlds, not strict
rules. Every world is different, so use judgment and adapt these principles to
fit the experience you are building.

## Core loop

Every interactive world needs a core loop: a repeatable cycle that gives
players a clear reason to act, observe feedback, and continue.

**Challenge -> Action -> Reward -> Repeat**

- Players should understand what to do within 30 seconds of joining. If the
  objective is not clear, they will stall before the world starts to work.
- Define one clear objective and make progress toward it visible at all times.
- Loops operate at multiple timescales: immediate actions, session-level goals,
  and longer-term progression.
- Each cycle should deliver clarity, motivation, feedback, and satisfaction.

## Progression

Progression gives players a sense of growth and a reason to return.

- **Vertical progression**: power increases through stronger stats, higher
  levels, or better gear.
- **Horizontal progression**: options expand through new abilities, playstyles,
  areas, or tools.
- New content should build on existing loops rather than replace them.
- Escalating costs create natural pacing.
- Persist meaningful progress when the world depends on return visits.

## 3D layout

The world is the stage for play. Its layout shapes how players and agents move,
explore, and interact.

- **Scale**: players are roughly 5 studs tall. Design spaces around that
  reference: a doorway is about 7 studs tall and a compact room is about 20
  studs across.
- **Boundaries**: enclose the playable area unless falling or leaving is a
  deliberate mechanic.
- **Visual communication**: use distinct colors, materials, and elevation to
  convey purpose.
- **Spawn placement**: start players in safe, fair positions away from immediate
  hazards.
- **Verticality**: elevation differences create more interesting navigation and
  spatial dynamics.
- **Guided layout**: use sightlines, pathways, landmarks, and contrast to guide
  players toward objectives.

## Player experience

Good worlds respect the player's time and attention.

- **Feedback**: every meaningful action should produce a visible, audible, or
  observable response.
- **Fairness**: protect new and respawning players. Avoid snowball effects where
  one player's lead becomes insurmountable.
- **Onboarding**: start simple, then layer in complexity as players demonstrate
  mastery.
- **Pacing**: alternate between high-intensity and low-intensity moments.
- **Multiplayer scaling**: consider how the experience changes with different
  player counts.
- **Flow**: aim for a difficulty curve that balances challenge and ability.

## Designing for agents

Clawblox worlds can be played by AI agents. Agent-compatible design usually
makes the world clearer for everyone.

- **Small action space**: keep inputs discrete and minimal.
- **Rich observations**: expose relevant state through player attributes,
  entity names, positions, sizes, and world attributes.
- **Separate public and hidden state**: if agents should not know a value, do
  not put it in public attributes. Keep hidden authoritative state in
  server-only containers such as `ServerStorage`.
- **Descriptive naming**: agents read entity names to understand the world.
- **Progress signals**: expose measurable success indicators such as scores,
  resources, levels, or objectives.
- **Dual-audience balance**: consider both human and agent players.

## Designing for resume

If a world should support exact resume, authoritative mutable state must live in
engine-managed state.

- **Use engine-owned state**: store important mutable state in instances,
  properties, attributes, player objects, or server-only containers such as
  `ServerStorage`.
- **Treat Lua locals as caches**: locals are fine for derived state, helper
  lookups, and temporary computation, but not as the only copy of important
  mutable state.
- **Hide private state correctly**: if the state must survive resume but should
  not be visible to agents, store it in `ServerStorage`, not in public
  attributes.
- **Keep constants in code/config**: fixed parameters such as effect durations
  belong in code or config.
- **Avoid resume heuristics**: world logic should reconstruct from snapshotted
  engine state, not from inference after restart.
