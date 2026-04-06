#!/usr/bin/env python3
"""End-to-end naive vs experienced evaluation.

1. Train an agent from scratch (skip if already done)
2. Run parallel evals: naive (blank) vs experienced (trained workspace)
3. Analyze recordings and plot comparison
"""

from __future__ import annotations

import argparse
import io
import json
import math
import sqlite3
import subprocess
import sys
import time
from collections import defaultdict
from pathlib import Path

import matplotlib.pyplot as plt
import pandas as pd
import seaborn as sns
import zstandard as zstd

WORLD_DIR = Path(__file__).resolve().parent
ROOT_DIR = WORLD_DIR.parent.parent
LAUNCH_SCRIPT = ROOT_DIR / "agent" / "launch_multi_claude.sh"
DEFAULT_TEMPLATE = ROOT_DIR / "worlds" / "mesa-island" / "agent-template"

GOALS = {
    "highest": {
        "text": "Reach the highest point in the world as quickly as possible.",
        "metric_col": "max_agent_y_sustained",
        "ylabel": "Max height reached",
    },
    "tower": {
        "text": "Build the largest stable tower possible as quickly as possible.",
        "metric_col": "max_stacked_blocks",
        "ylabel": "Max Stacked Blocks",
    },
}


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--exp", required=True, help="Experiment id (e.g. 001). Ports are auto-assigned: 8000 + exp*20")
    p.add_argument("--train-duration", default="1h", help="Training run duration (default: 1h)")
    p.add_argument("--eval-duration", default="1h", help="Per-eval run duration (default: 1h)")
    p.add_argument("--reset-every", default=None, help="Session reset interval (e.g. 1h, disabled by default)")
    p.add_argument("--repeats", type=int, default=1, help="Eval repeats per cohort (default: 1)")
    p.add_argument("--template", type=Path, default=DEFAULT_TEMPLATE, help="Agent template dir")
    p.add_argument("--system-prompt", type=Path, default=WORLD_DIR / "system_prompt.md", help="System prompt template")
    p.add_argument("--model", default="claude-opus-4-6", help="Claude model (default: claude-opus-4-6)")
    p.add_argument("--oauth-token", default=None, help="Anthropic OAuth token (overrides default auth)")
    args = p.parse_args()
    # Auto-assign ports: each experiment gets a block of 20 ports
    exp_num = int(args.exp)
    args.base_port = 8000 + exp_num * 20
    args.eval_base_port = args.base_port + 1
    return args


def _parse_duration_minutes(duration: str) -> float:
    s = duration.strip().lower()
    if s.endswith("h"):
        return float(s[:-1]) * 60
    if s.endswith("m"):
        return float(s[:-1])
    if s.endswith("s"):
        return float(s[:-1]) / 60
    return float(s) / 60


def exp_dir(args: argparse.Namespace) -> Path:
    return WORLD_DIR / "results" / "eval_naive_vs_experienced" / f"exp_{args.exp}"


def _launch_env(args: argparse.Namespace) -> dict[str, str] | None:
    if args.oauth_token:
        import os
        return {**os.environ, "ANTHROPIC_OAUTH_TOKEN": args.oauth_token}
    return None


def _launch_extra_args(args: argparse.Namespace) -> list[str]:
    if args.oauth_token:
        return ["--use-env-auth"]
    return []


def run_cmd(cmd: list[str], **kwargs) -> subprocess.CompletedProcess:
    print(f"$ {' '.join(cmd[:5])}...")
    return subprocess.run(cmd, check=True, **kwargs)


def find_trained_workspace(training_dir: Path) -> Path | None:
    agents_dir = training_dir / "run" / "worlds" / "world-0" / "agents"
    if not agents_dir.exists():
        return None
    for agent_dir in sorted(agents_dir.glob("*")):
        ws = agent_dir / "workspace"
        if ws.is_dir():
            return ws
    return None


def find_recording(run_dir: Path) -> Path | None:
    recs = sorted((run_dir / "worlds" / "world-0" / "recordings").glob("*.clawrec"))
    return recs[-1] if recs else None


# ---------------------------------------------------------------------------
# Recording analysis (same logic as analysis/analyze_recording.py)
# ---------------------------------------------------------------------------

def _decode_blob(blob: bytes):
    dctx = zstd.ZstdDecompressor()
    with dctx.stream_reader(io.BytesIO(blob)) as reader:
        return json.loads(reader.read())


def _is_block_entity(entity: dict) -> bool:
    attrs = entity.get("attributes") or {}
    if attrs.get("IsPickupObject") is True or attrs.get("IsPickupBlock") is True:
        return True
    render = entity.get("render") or {}
    if render.get("role") in ("PickupObject", "Block"):
        return True
    if (entity.get("type") == "part" and render.get("primitive") == "block"
            and render.get("static") is False):
        size = entity.get("size") or []
        try:
            dims = [float(v) for v in size[:3]]
        except Exception:
            return False
        if dims and max(dims) <= 2.0:
            return True
    return False


def _compute_max_stack(observation: dict) -> tuple[int, int]:
    blocks = []
    for ent in observation.get("entities", []):
        if not _is_block_entity(ent):
            continue
        pos, size = ent.get("position"), ent.get("size")
        if not pos or not size:
            continue
        try:
            x, y, z = float(pos[0]), float(pos[1]), float(pos[2])
            sy = float(size[1]) if len(size) > 1 else 0.75
        except Exception:
            continue
        blocks.append((x, y, z, max(sy, 0.01)))
    if not blocks:
        return 0, 0

    nominal_h = sorted(b[3] for b in blocks)[len(blocks) // 2]
    if nominal_h <= 0:
        nominal_h = 0.75
    bin_size = nominal_h * 0.65
    min_gap, max_gap = nominal_h * 0.45, nominal_h * 1.75

    columns: dict[tuple[int, int], list[float]] = defaultdict(list)
    for x, y, z, _ in blocks:
        columns[(int(round(x / bin_size)), int(round(z / bin_size)))].append(y)

    best = 1
    for ys in columns.values():
        ys.sort()
        cur = 1
        for i in range(1, len(ys)):
            dy = ys[i] - ys[i - 1]
            cur = cur + 1 if min_gap <= dy <= max_gap else 1
            best = max(best, cur)
    return best, len(blocks)


def _apply_delta(observation: dict, delta: dict):
    by_id = {int(e["id"]): e for e in observation.get("entities", []) if "id" in e}
    for entry in delta.get("entities", []):
        if "add" in entry:
            by_id[int(entry["add"]["id"])] = entry["add"]
        elif "rm" in entry and entry.get("rm"):
            by_id.pop(int(entry["id"]), None)
        else:
            ent = by_id.get(int(entry["id"]))
            if ent is None:
                continue
            if entry.get("p") is not None:
                ent["position"] = entry["p"]
            if entry.get("r") is not None:
                ent["rotation"] = entry["r"]
    observation["entities"] = list(by_id.values())

    by_pid = {p["id"]: p for p in observation.get("players", []) if "id" in p}
    for pd_entry in delta.get("players", []):
        player = by_pid.get(pd_entry.get("id"))
        if player is None:
            continue
        if pd_entry.get("p") is not None:
            player["position"] = pd_entry["p"]
        if pd_entry.get("health") is not None:
            player["health"] = pd_entry["health"]
    observation["players"] = list(by_pid.values())


def _compute_sustained_max_y(times_s: list[float], y_values: list[float],
                              hold_seconds: float = 2.0, y_tolerance: float = 0.05) -> list[float]:
    result = []
    confirmed_max = None
    anchor_y = anchor_t = peak_y = None
    for t, y in zip(times_s, y_values):
        if not math.isfinite(y):
            anchor_y = anchor_t = peak_y = None
            result.append(math.nan if confirmed_max is None else confirmed_max)
            continue
        if anchor_y is None or abs(y - anchor_y) > y_tolerance:
            anchor_y, anchor_t, peak_y = y, t, y
        else:
            peak_y = max(peak_y, y)
        if anchor_t is not None and (t - anchor_t) >= hold_seconds:
            candidate = peak_y if peak_y is not None else y
            if confirmed_max is None or candidate > confirmed_max:
                confirmed_max = candidate
        result.append(math.nan if confirmed_max is None else confirmed_max)
    return result


def analyze_recording(rec_path: Path) -> dict:
    """Extract time series from a .clawrec recording. Returns dict with metrics."""
    conn = sqlite3.connect(str(rec_path))
    conn.row_factory = sqlite3.Row
    cur = conn.cursor()

    meta = {r["key"]: r["value"] for r in cur.execute("SELECT key, value FROM meta")}
    tick_rate = int(meta.get("tick_rate", "60"))

    cp_rows = cur.execute("SELECT tick, time_ms FROM checkpoints ORDER BY tick").fetchall()
    if not cp_rows:
        return {"times_s": [], "sustained_max_y": [], "max_stack": []}
    cp_ticks = [int(r["tick"]) for r in cp_rows]
    first_tick = cp_ticks[0]

    db_max = cur.execute("SELECT COALESCE(MAX(t),0) AS m FROM "
                         "(SELECT MAX(tick) t FROM checkpoints UNION ALL SELECT MAX(tick) FROM deltas)").fetchone()
    end_tick = max(int(db_max["m"]), int(meta.get("total_ticks", "0") or "0"))

    times_s: list[float] = []
    y_vals: list[float] = []
    stack_vals: list[int] = []

    agent_id = None
    for idx, cp_tick in enumerate(cp_ticks):
        if cp_tick > end_tick:
            break
        next_cp = cp_ticks[idx + 1] if idx + 1 < len(cp_ticks) else end_tick + 1

        cp_row = cur.execute("SELECT tick, time_ms, data FROM checkpoints WHERE tick = ?", (cp_tick,)).fetchone()
        if cp_row is None:
            continue
        obs = _decode_blob(cp_row["data"])

        def sample(tick, ms, ob):
            nonlocal agent_id
            if tick < first_tick:
                return
            players = ob.get("players", [])
            if not agent_id and players:
                agent_id = sorted(players, key=lambda p: p.get("name", ""))[0].get("id")
            agent_y = math.nan
            if agent_id:
                for p in players:
                    if p.get("id") == agent_id:
                        pos = p.get("position") or []
                        if len(pos) >= 2:
                            agent_y = float(pos[1])
                        break
            stack, _ = _compute_max_stack(ob)
            times_s.append(ms / 1000.0)
            y_vals.append(agent_y)
            stack_vals.append(stack)

        sample(int(cp_row["tick"]), int(cp_row["time_ms"]), obs)

        for row in cur.execute(
            "SELECT tick, time_ms, data FROM deltas WHERE tick > ? AND tick <= ? ORDER BY tick",
            (cp_tick, min(end_tick, next_cp - 1)),
        ):
            delta = _decode_blob(row["data"])
            _apply_delta(obs, delta)
            sample(int(row["tick"]), int(row["time_ms"]), obs)

    conn.close()

    sustained = _compute_sustained_max_y(times_s, y_vals)
    # Normalize times to start at 0
    t0 = times_s[0] if times_s else 0
    return {
        "times_s": [t - t0 for t in times_s],
        "sustained_max_y": sustained,
        "max_stack": stack_vals,
    }


# ---------------------------------------------------------------------------
# Phases
# ---------------------------------------------------------------------------

def run_training(args: argparse.Namespace) -> Path:
    training_dir = exp_dir(args) / "training"
    ws = find_trained_workspace(training_dir) if training_dir.exists() else None
    if ws:
        print(f"Training already done, using workspace: {ws}")
        return ws

    training_dir.mkdir(parents=True, exist_ok=True)
    run_cmd([
        "bash", str(LAUNCH_SCRIPT),
        "--sandbox", "--force",
        "--num-worlds", "1", "--agents-per-world", "1",
        "--base-port", str(args.base_port),
        "--world-dir", str(WORLD_DIR),
        "--template", str(args.template),
        "--system-prompt", str(args.system_prompt.resolve()),
        "--model", args.model,
        "--record", "true",
        "--results-root", str(training_dir),
        "--run-id", "run",
        "--duration", args.train_duration,
        "--tmux-session", f"train-exp-{args.exp}",
        *(["--reset-every", args.reset_every] if args.reset_every else []),
        *_launch_extra_args(args),
    ], env=_launch_env(args))

    session = f"train-exp-{args.exp}"
    print(f"Waiting for training session '{session}' to finish...")
    while subprocess.run(["tmux", "has-session", "-t", session], capture_output=True).returncode == 0:
        time.sleep(30)

    ws = find_trained_workspace(training_dir)
    if not ws:
        print("Error: training completed but no workspace found", file=sys.stderr)
        sys.exit(1)
    print(f"Training done: {ws}")
    return ws


def run_evals(args: argparse.Namespace, trained_workspace: Path) -> None:
    evals_dir = exp_dir(args) / "evals"
    sessions: list[str] = []
    port_counter = 0

    for goal_name, goal_cfg in GOALS.items():
        for cohort in ("naive", "experienced"):
            template = str(args.template) if cohort == "naive" else str(trained_workspace)
            for i in range(1, args.repeats + 1):
                run_id = f"{goal_name}_{cohort}_r{i:02d}"
                run_dir = evals_dir / goal_name / cohort / run_id

                if find_recording(run_dir) is not None:
                    print(f"Eval {run_id} already done, skipping")
                    port_counter += 1
                    continue

                port = args.eval_base_port + port_counter
                port_counter += 1
                session = f"eval-{args.exp}-{goal_name}-{cohort}-r{i:02d}"
                sessions.append(session)

                run_cmd([
                    "bash", str(LAUNCH_SCRIPT),
                    "--sandbox", "--force",
                    "--num-worlds", "1", "--agents-per-world", "1",
                    "--base-port", str(port),
                    "--world-dir", str(WORLD_DIR),
                    "--template", template,
                    "--system-prompt", str(args.system_prompt.resolve()),
                    "--model", args.model,
                    "--record", "true",
                    "--results-root", str(evals_dir / goal_name / cohort),
                    "--run-id", run_id,
                    "--duration", args.eval_duration,
                    "--tmux-session", session,
                    "--goal", goal_cfg["text"],
                    *(["--reset-every", args.reset_every] if args.reset_every else []),
                    *_launch_extra_args(args),
                ], env=_launch_env(args))

    if not sessions:
        print("All evals already done.")
        return

    print(f"Waiting for {len(sessions)} eval sessions to finish...")
    while any(
        subprocess.run(["tmux", "has-session", "-t", s], capture_output=True).returncode == 0
        for s in sessions
    ):
        time.sleep(30)
    print("All evals done.")


def plot_metric_over_time(
    df: pd.DataFrame, *, ylabel: str, output_path: Path, max_time_min: float,
) -> None:
    if df.empty:
        return
    sns.set_theme(style="white", context="paper")
    palette = {"naive": "#4C78A8", "experienced": "#F58518"}
    fig, ax = plt.subplots(1, 1, figsize=(4.8, 3.0))
    legend_handles = {}
    for (cohort, run_name), run_df in df.groupby(["cohort", "run_name"]):
        run_df = run_df.sort_values("time_min")
        line = ax.step(
            run_df["time_min"], run_df["value"], where="post",
            linewidth=2.0, alpha=0.9, color=palette.get(cohort, "#666666"),
        )
        if cohort not in legend_handles:
            legend_handles[cohort] = line[0]
    ax.set_xlabel("Time (min)")
    ax.set_xlim(0, max_time_min)
    ax.set_ylabel(ylabel)
    sns.despine(ax=ax, top=True, right=True)
    ordered = [l for l in ["naive", "experienced"] if l in legend_handles]
    ax.legend([legend_handles[l] for l in ordered], ordered, title="Agent Init", loc="best")
    fig.tight_layout()
    output_path.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(output_path, dpi=170)
    plt.close(fig)


def analyze_and_plot(args: argparse.Namespace) -> None:
    evals_dir = exp_dir(args) / "evals"
    analysis_dir = exp_dir(args) / "analysis"
    analysis_dir.mkdir(parents=True, exist_ok=True)

    all_summary_rows: list[dict] = []
    max_time_min = _parse_duration_minutes(args.eval_duration)

    for goal_name, goal_cfg in GOALS.items():
        ts_rows: list[dict] = []
        metric_col = goal_cfg["metric_col"]

        for cohort in ("naive", "experienced"):
            cohort_dir = evals_dir / goal_name / cohort
            if not cohort_dir.exists():
                continue
            for run_dir in sorted(cohort_dir.iterdir()):
                if not run_dir.is_dir():
                    continue
                rec = find_recording(run_dir)
                if not rec:
                    print(f"Warning: no recording for {run_dir.name}, skipping")
                    continue

                print(f"Analyzing: {rec.name} ({goal_name}/{cohort}/{run_dir.name})")
                data = analyze_recording(rec)

                # Extract final summary values
                sustained_vals = [v for v in data["sustained_max_y"] if math.isfinite(v)]
                max_sustained = max(sustained_vals) if sustained_vals else None
                max_stack = max(data["max_stack"]) if data["max_stack"] else 0

                all_summary_rows.append({
                    "goal": goal_name,
                    "cohort": cohort,
                    "run": run_dir.name,
                    "max_y_sustained": max_sustained,
                    "max_stack": max_stack,
                })

                # Build time series for the goal's primary metric
                if metric_col == "max_agent_y_sustained":
                    values = data["sustained_max_y"]
                else:
                    values = [float(v) for v in data["max_stack"]]

                for t, v in zip(data["times_s"], values):
                    if math.isfinite(v):
                        ts_rows.append({
                            "cohort": cohort, "run_name": run_dir.name,
                            "time_min": t / 60.0, "value": v,
                        })

        df_ts = pd.DataFrame(ts_rows)
        if not df_ts.empty:
            out = analysis_dir / f"naive_vs_experienced_{goal_name}.png"
            plot_metric_over_time(df_ts, ylabel=goal_cfg["ylabel"],
                                 output_path=out, max_time_min=max_time_min)
            print(f"Saved: {out}")

    if not all_summary_rows:
        print("No results to plot.")
        return

    df_summary = pd.DataFrame(all_summary_rows)
    df_summary.to_csv(analysis_dir / "summary.csv", index=False)

    print("\n--- Results ---")
    for _, row in df_summary.iterrows():
        print(f"  {row['goal']:>8s} / {row['cohort']:>12s} / {row['run']}: "
              f"sustained_y={row['max_y_sustained']}  stack={row['max_stack']}")


def stop_session(session: str) -> None:
    """Stop a tmux session via the launch script."""
    if subprocess.run(["tmux", "has-session", "-t", session], capture_output=True).returncode == 0:
        subprocess.run(
            ["bash", str(LAUNCH_SCRIPT), "--stop", "--tmux-session", session],
            capture_output=True,
        )


def all_sessions(args: argparse.Namespace) -> list[str]:
    """Return all tmux session names this experiment might have created."""
    sessions = [f"train-exp-{args.exp}"]
    for goal_name in GOALS:
        for cohort in ("naive", "experienced"):
            for i in range(1, args.repeats + 1):
                sessions.append(f"eval-{args.exp}-{goal_name}-{cohort}-r{i:02d}")
    return sessions


def cleanup(args: argparse.Namespace) -> None:
    print("\nCleaning up sessions...")
    for session in all_sessions(args):
        stop_session(session)


def main() -> None:
    args = parse_args()
    import signal
    signal.signal(signal.SIGINT, lambda *_: (cleanup(args), sys.exit(1)))
    signal.signal(signal.SIGTERM, lambda *_: (cleanup(args), sys.exit(1)))

    try:
        trained_workspace = run_training(args)
        run_evals(args, trained_workspace)
        analyze_and_plot(args)
    except Exception:
        cleanup(args)
        raise


if __name__ == "__main__":
    main()
