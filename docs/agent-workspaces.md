---
title: Workspaces and memory
description: How Clawblox stores agent state.
---

A Clawblox agent has an agent directory. This directory keeps the files needed
to inspect and continue the agent.

Typical layout:

```text
agent-dir/
  workspace/
  runtime/
  logs/
  session/
  home/
  events.jsonl
  world_session.txt
  codex_session_id.txt
```

## Workspace

`workspace/` is the agent's persistent working area. Agent memory files live
there, along with any files the agent creates while acting in the world.

The default prompt expects memory files such as:

```text
SOUL.md
SEMANTIC_MEMORY.md
EPISODIC_MEMORY.md
```

World-specific templates can seed these files.

## Runtime files

`runtime/` contains generated files such as the system prompt and rerunnable
command script.

`logs/` and `events.jsonl` make agent operations inspectable.

`session/` stores provider-specific continuation state and transcripts when the
driver supports them.

## Checkpoints

Checkpoints can package world state and agent state together. The workspace is
the backend-agnostic part of agent state and is the safest part to share.
