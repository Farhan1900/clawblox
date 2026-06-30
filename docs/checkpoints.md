---
title: Checkpoints
summary: "Save and resume a whole run, world plus agent state, as one file."
read_when:
  - Resuming or sharing a run
  - Running generational or crash-recoverable experiments
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
- **workspace**: agent-authored files. They do not depend on which coding agent
  produced them, they are what carries over from one agent generation to the
  next, and they are the safe thing to share. Reconstructable artifacts
  (`.venv`, caches) are excluded.
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
- Worlds expose sim time only (never the host machine's real clock) in
  agent-visible payloads. One leftover way an agent could still notice: it can
  compare its own `date` reading against what it remembers from before the
  crash. Closing that gap completely would require virtualizing time at the VM
  level, which is out of scope here.
