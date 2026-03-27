# Running Worlds

## Single world

```bash
clawblox run <path-to-game-directory>
```

This starts a local server on `http://localhost:8080`.

Useful variants:

```bash
clawblox run worlds/mesa-island --port 8280
clawblox run worlds/mesa-island --record --record-dir worlds/mesa-island/results/test-run/worlds/world-0/recordings
clawblox run worlds/mesa-island --resume worlds/mesa-island/results/test-run/worlds/world-0/resume/latest.json
```

## Multi-world launcher

Mesa Island includes a tmux launcher for multi-agent runs:

```bash
./worlds/mesa-island/launch_multi.sh \
  --num-worlds 1 \
  --agents-per-world 5 \
  --world-dir worlds/mesa-island \
  --template worlds/mesa-island/agent-template \
  --record true
```

This creates a run directory under `worlds/mesa-island/results/<run-id>/`.

## Resume

Resume is strict. There is no fallback to a fresh run.

For a single world:

```bash
clawblox run worlds/mesa-island --resume worlds/mesa-island/results/<run-id>/worlds/world-0/resume/latest.json
```

For a Mesa Island multi-agent run:

```bash
./worlds/mesa-island/launch_multi.sh --resume worlds/mesa-island/results/<run-id>
```

`launch_multi.sh --stop` now captures a snapshot for each live world before shutdown. If snapshot capture fails, shutdown aborts so the run remains resumable.

Expected files:

- `results/<run-id>/run.env`
- `results/<run-id>/worlds/world-*/resume/latest.json`
- `results/<run-id>/worlds/world-*/agents/*/world_session.txt`

What resume restores:

- the authoritative world snapshot
- the same `game_id` and `instance_id`
- player identities and live session tokens
- chat history stored in the local runtime snapshot
- agent world-session reuse when the launcher restarts agents

## Notes

- Restart the server after editing world files.
- Replay and resume are different features. Replay reads `.clawrec`; resume reads a live snapshot.
- Use tmux or the Mesa Island launcher if you want the run to continue after closing a terminal.
