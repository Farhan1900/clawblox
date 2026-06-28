---
title: Running worlds
summary: "Run a world locally, from GitHub, with recording, or resumed from a snapshot."
read_when:
  - Running a world
  - You need run flags like port, record, or resume
---

Every world runs the same way: point `clawblox run` at a folder that contains a
[`world.toml`](/worlds). This page covers the flags you add on top of that. To
start an agent alongside the world, see [Agents](/agents).

## Single world

```bash
clawblox run <world>
```

`<world>` can be a local folder or a GitHub folder URL. The folder must contain
`world.toml`. Run it from inside the folder to use the current directory:

```bash
cd my-world
clawblox run
```

Either way, this starts a local server on `http://localhost:8080`.

Common flags:

```bash
clawblox run <world> --port 8280
clawblox run <world> --agent codex          # also start an agent
clawblox run <world> --record --record-dir runs/r001/recordings
clawblox run <world> --resume runs/r001/resume/latest.json
```

`--agent` starts a managed agent in the world; see [Agents](/agents)
for coding agents, sandboxing, and multi-agent runs. `--resume` restores a saved
world snapshot; see [Save, load, and resume](/save-load-resume) for what a
snapshot captures.

## Remote worlds

GitHub folder URLs are downloaded into the Clawblox home directory, then run as
ordinary local worlds.

Supported URL shape:

```text
https://github.com/<owner>/<repo>/tree/<ref>/<path-to-world>
```

Example:

```bash
clawblox run https://github.com/nacloos/clawblox/tree/main/worlds/mesa-small-world-3
```

The downloaded folder is cached under `~/.clawblox/worlds/`. Set `CLAWBLOX_HOME`
to use another location:

```bash
CLAWBLOX_HOME=/tmp/clawblox clawblox run https://github.com/nacloos/clawblox/tree/main/worlds/mesa-small-world-3
```

## Resume

Resume is strict: there is no fallback to a fresh run.

```bash
clawblox run <world> --resume <snapshot.json>
```

Relative paths are resolved from the world directory. Replay and resume are
different: replay reads a recording, resume restores a live world snapshot. See
[Save, load, and resume](/save-load-resume) for snapshot contents, and
[Checkpoints](/checkpoints) for resuming a world together with its agents.

## Notes

- Restart the server after editing world files.
- Use `clawblox start` to run in the background and `clawblox stop` to stop it.
- For multi-agent runs and agent sandboxing, see [Agents](/agents).
