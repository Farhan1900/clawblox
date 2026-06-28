---
title: Checkpoints
description: Save and load a run as a single relocatable file of world and agent state.
---

A checkpoint is a single relocatable `.ckpt` file (a zip) that captures a run
at a moment in time: world state plus agent state. Save and load follow the
PyTorch idiom: construct first, then load state into it.

```python
from clawblox import World, save_checkpoint, load_checkpoint

# save: world snapshot + agent archives -> one file
snap = world.save(dir=run_dir / "checkpoints")
ckpt = save_checkpoint(
    run_dir / "checkpoints" / "gen001-t000900.ckpt",
    world_snapshot=snap["path"],
    agents={agent.name: agent_dir},
    backend="claude",
    metadata={"generation": 1, "elapsed_seconds": 900},
)

# load: materialize, then resume through the normal APIs
loaded = load_checkpoint(ckpt, new_run_dir)
world.start(port=8085, resume=loaded["world_snapshot"])
agent = Agent(agent="claude", name=name, dir=loaded["agents"][name], ...)
world.connect(agent=agent)        # rejoins the recorded world session id
agent.send(prompt=AUTO_CONTINUE)  # claude --continue resumes the transcript
```

## Archive layout

```text
gen001-t000900.ckpt
├── metadata.json                 schema_version, created_at, session_format,
│                                 agents, plus caller metadata (generation, ...)
├── world.snapshot                opaque document from the world's GET /snapshot
└── agents/<name>/
    ├── workspace/...             workspace tier
    └── session/...               session tier
```

All paths inside the archive are relative; a checkpoint can be copied to
another machine and loaded there, given the same world directory and
clawblox version (state travels, code does not, like a `state_dict`
without the model class).

## Tiers

- **world**: the snapshot document. Exact restore is the world's
  responsibility (see `world-interface.md`).
- **workspace**: agent-authored files. Backend-agnostic, the unit of
  generational inheritance, and the shareable artifact. Reconstructable
  artifacts (`.venv`, caches) are excluded.
- **session**: provider continuation state (conversation transcripts),
  selected by a per-backend **allowlist** (`claude-code-v1`, `codex-v1`).
  Needed only to resume the exact conversation. `workspace_only=True`
  omits it. That is the safe default for sharing.

## Safety model

State loads, policy doesn't:

- Credentials and capability config (`.credentials.json`, `.claude.json`,
  `settings.json`, permission files) are never written to an archive (the
  session tier is an allowlist, so unknown files stay out) and never
  extracted from one, even if a malicious archive contains them.
- A secret scan runs over every file at save time and refuses to produce an
  archive containing credential-shaped content (`CheckpointSecretError`).
- Extraction rejects absolute paths and `..` traversal.
- Loading a foreign session tier still means adopting a conversation you did
  not audit. Prefer `workspace_only=True` for checkpoints you did not create.

## Invisibility

Checkpoints must be invisible to agents. Telling an agent it is being
checkpointed is like telling a network it is being `torch.save`d:

- Nothing is sent to the agent at save time; agent state is archived from
  disk, preferring an idle moment (transcripts are append-only, so a
  mid-turn snapshot resumes from the last completed message).
- On resume, the world restores the same session ids, the agent's original
  system prompt still applies, and the resume nudge uses the standard
  auto-continue prompt, indistinguishable from a routine idle nudge.
- Worlds expose sim time only (never host wall time) in agent-visible
  payloads. Known residual channel: the agent can compare its own `date`
  with pre-crash memory; closing that fully requires VM-level time
  virtualization and is out of scope here.

## Generation chains

`run_agent_generations.py` (research harness) integrates checkpoints:

- `CHECKPOINT_EVERY` (seconds, default 900, `0` disables) checkpoints each
  running generation in `runs/<run_id>/checkpoints/`, with the latest pointer
  in the experiment's `checkpoint.json`.
- `--resume-from <ckpt>` resumes a generation that crashed mid-flight: the
  remaining generation duration is computed from the checkpoint's
  `elapsed_seconds`, and the chain continues into subsequent generations
  normally. The chain CSV records `template_in` as `checkpoint:<path>`.
- Failed run directories that contain checkpoints are preserved (renamed
  `*.failed-<stamp>`), never deleted by the retry loop.
