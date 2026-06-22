# Manifest

Clawblox writes `.clawblox/manifest.json` as the strict metadata contract for
replay and app tooling. Tools should read this file instead of probing backend
processes or inferring paths from naming conventions.

## Agent Identity

Each entry in `agents` describes one logical agent and its recorded artifacts.

```json
{
  "id": "world-agent-uuid",
  "name": "Rocky",
  "run_name": "Rocky-rgenexp-claude-20260620T173017Z-g001-w0-a0",
  "dir": "agents/Rocky-rgenexp-claude-20260620T173017Z-g001-w0-a0"
}
```

Fields:

| Field | Meaning |
|---|---|
| `id` | Stable world/session agent id when the world provides one. If no world id exists, this may be the run name. |
| `name` | Logical display name used in UI, prompts, chat, and agent selectors. This should be short and human meaningful, such as `Rocky` or `Agent1`. |
| `run_name` | Unique run/process/artifact name. This is the value used for tmux windows, filesystem-safe run directories, and disambiguating repeated uses of the same logical `name`. |
| `dir` | Agent artifact directory relative to the manifest state root when possible. |
| `world_agent_id` | Compatibility copy of the world-provided id. New code should prefer `id`. |

Manifest writers should merge agent metadata by `id` when the id is known, or by
`dir` when different subsystems record artifacts for the same agent before the
world id is available. The manifest should not contain separate agent entries for
the same logical agent's transcript, workspace recording, and world connection.

## Recording Metadata

Combined replay requires static `recordings[].info` metadata.

```json
{
  "path": "recordings/20260620_173019.clawrec",
  "format": "clawrec",
  "info": {
    "first_tick": 0,
    "total_ticks": 443388,
    "tick_rate": 60,
    "duration_ms": 7242370
  }
}
```

All four `info` fields are required for combined replay. Missing recording
metadata is a protocol error; app tooling must not launch replay backends to infer
segment duration.
