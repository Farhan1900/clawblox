# Sandbox setup

The `--sandbox` flag wraps each Claude agent in a bubblewrap (`bwrap`) jail so
only the agent workspace is writable. On machines where you don't have root,
install `bwrap` (and optionally `socat`) via conda-forge:

```bash
conda create -y -n sandbox-deps -c conda-forge bubblewrap socat
conda activate sandbox-deps
```

Then launch with the env active so `bwrap` is on PATH:

```bash
conda activate sandbox-deps
bash agent/launch_multi_claude.sh --world-dir worlds/mesa-world --sandbox
```
