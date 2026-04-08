#!/usr/bin/env python3
"""Plot eval summaries for naive/scratch vs experienced runs."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import shutil
import subprocess
import sys

import matplotlib.pyplot as plt
import pandas as pd
import seaborn as sns


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Aggregate eval runs under a single eval directory and plot "
            "naive (scratch) vs experienced for max agent Y and max stacked blocks."
        )
    )
    parser.add_argument(
        "evals_dir",
        type=Path,
        help=(
            "Path to an eval directory that directly contains objective folders "
            "such as highest/ and tower/ (e.g., results/<exp>/evals/single-peak-2xblocks)."
        ),
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=None,
        help="Output directory for plots/CSV (default: <evals_dir>/analysis).",
    )
    parser.add_argument(
        "--reanalyze",
        action="store_true",
        help="Re-run analyze_recording.py for each run before plotting.",
    )
    parser.add_argument(
        "--analyzer-script",
        type=Path,
        default=None,
        help="Path to analysis/analyze_recording.py (default: sibling file).",
    )
    parser.add_argument(
        "--max-time-min",
        type=float,
        default=60.0,
        help="Truncate over-time plots at this many minutes (default: 60).",
    )
    return parser.parse_args()


def canonical_cohort(name: str) -> str:
    lower = name.strip().lower()
    if lower in {"scratch", "naive"}:
        return "naive"
    if lower in {"experienced", "expert"}:
        return "experienced"
    return lower


def latest_recording(run_dir: Path) -> Path:
    matches = sorted(run_dir.glob("worlds/world-0/recordings/*.clawrec"))
    if not matches:
        raise FileNotFoundError(f"No .clawrec found under run dir: {run_dir}")
    return matches[-1]


def latest_summary(run_dir: Path) -> Path:
    matches = sorted(run_dir.glob("worlds/world-0/recordings/*_analysis/summary.json"))
    if not matches:
        raise FileNotFoundError(f"No summary.json found under run dir: {run_dir}")
    return matches[-1]


def run_recording_analysis(analyzer_script: Path, recording: Path) -> Path:
    output_dir = recording.with_name(f"{recording.stem}_analysis")
    uv = shutil.which("uv")
    if uv:
        cmd = [
            uv,
            "run",
            "python",
            str(analyzer_script),
            str(recording),
            "--output-dir",
            str(output_dir),
        ]
    else:
        cmd = [
            sys.executable,
            str(analyzer_script),
            str(recording),
            "--output-dir",
            str(output_dir),
        ]
    subprocess.run(cmd, check=True, cwd=str(analyzer_script.parent.parent))
    summary = output_dir / "summary.json"
    if not summary.exists():
        raise FileNotFoundError(f"Analyzer did not produce summary.json at: {summary}")
    return summary


def discover_runs(evals_dir: Path) -> list[tuple[str, str, Path]]:
    discovered: list[tuple[str, str, Path]] = []
    for objective_dir in sorted(p for p in evals_dir.iterdir() if p.is_dir() and p.name in {"highest", "tower"}):
        objective = objective_dir.name
        for cohort_dir in sorted(p for p in objective_dir.iterdir() if p.is_dir()):
            cohort = canonical_cohort(cohort_dir.name)
            for run_dir in sorted(p for p in cohort_dir.iterdir() if p.is_dir()):
                discovered.append((objective, cohort, run_dir))
    return discovered


def load_summary(summary_json: Path) -> dict:
    with summary_json.open("r", encoding="utf-8") as f:
        return json.load(f)


def plot_metric(
    df: pd.DataFrame,
    *,
    metric: str,
    ylabel: str,
    title: str,
    output_path: Path,
) -> None:
    sns.set_theme(style="whitegrid", context="paper")
    plt.figure(figsize=(7.0, 4.2))
    hue_order = ["naive", "experienced"]
    sns.barplot(
        data=df,
        x="objective",
        y=metric,
        hue="cohort",
        hue_order=hue_order,
        errorbar=None,
        palette={"naive": "#4C78A8", "experienced": "#F58518"},
    )
    sns.stripplot(
        data=df,
        x="objective",
        y=metric,
        hue="cohort",
        hue_order=hue_order,
        dodge=True,
        marker="o",
        alpha=0.85,
        size=6,
        linewidth=0.5,
        edgecolor="white",
        palette={"naive": "#4C78A8", "experienced": "#F58518"},
    )
    plt.xlabel("Eval Objective")
    plt.ylabel(ylabel)

    handles, labels = plt.gca().get_legend_handles_labels()
    # stripplot adds duplicate legend entries; keep first occurrence in order
    unique: dict[str, object] = {}
    for h, l in zip(handles, labels):
        if l not in unique and l in {"naive", "experienced"}:
            unique[l] = h
    plt.legend(unique.values(), unique.keys(), title="Agent Init")

    plt.tight_layout()
    output_path.parent.mkdir(parents=True, exist_ok=True)
    plt.savefig(output_path, dpi=170)
    plt.close()


def load_time_series_rows(
    *,
    csv_path: Path,
    time_col: str,
    value_col: str,
    objective: str,
    cohort: str,
    run_name: str,
) -> list[dict]:
    if not csv_path.exists():
        return []
    df = pd.read_csv(csv_path, usecols=[time_col, value_col])
    time_s = pd.to_numeric(df[time_col], errors="coerce")
    values = pd.to_numeric(df[value_col], errors="coerce")
    mask = time_s.notna() & values.notna()
    if not mask.any():
        return []
    time_s = time_s[mask].reset_index(drop=True)
    values = values[mask].reset_index(drop=True)
    time_min = (time_s - float(time_s.iloc[0])) / 60.0
    return [
        {
            "objective": objective,
            "cohort": cohort,
            "run_name": run_name,
            "time_min": float(t),
            "value": float(v),
        }
        for t, v in zip(time_min, values)
    ]


def plot_metric_over_time(
    df: pd.DataFrame,
    *,
    ylabel: str,
    output_path: Path,
    max_time_min: float,
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
            run_df["time_min"],
            run_df["value"],
            where="post",
            linewidth=2.0,
            alpha=0.9,
            color=palette.get(cohort, "#666666"),
            label=cohort,
        )
        if cohort not in legend_handles:
            legend_handles[cohort] = line[0]

    ax.set_xlabel("Time (min)")
    ax.set_xlim(0, max_time_min)
    sns.despine(ax=ax, top=True, right=True)
    ax.set_ylabel(ylabel)

    ordered_labels = [l for l in ["naive", "experienced"] if l in legend_handles]
    ax.legend(
        [legend_handles[l] for l in ordered_labels],
        ordered_labels,
        title="Agent Init",
        loc="best",
    )
    fig.tight_layout()
    output_path.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(output_path, dpi=170)
    plt.close(fig)


def main() -> None:
    args = parse_args()
    evals_dir = args.evals_dir.resolve()
    if not evals_dir.exists():
        raise FileNotFoundError(f"evals_dir does not exist: {evals_dir}")

    script_dir = Path(__file__).resolve().parent
    analyzer_script = (
        args.analyzer_script.resolve()
        if args.analyzer_script is not None
        else (script_dir / "analyze_recording.py")
    )
    if not analyzer_script.exists():
        raise FileNotFoundError(f"Analyzer script not found: {analyzer_script}")

    output_dir = (
        args.output_dir.resolve()
        if args.output_dir is not None
        else (evals_dir / "analysis")
    )
    output_dir.mkdir(parents=True, exist_ok=True)

    runs = discover_runs(evals_dir)
    if not runs:
        raise RuntimeError(
            f"No runs discovered under: {evals_dir}. "
            "Pass a directory that directly contains objective folders like highest/ and tower/."
        )

    height_time_rows: list[dict] = []
    stack_time_rows: list[dict] = []
    for objective, cohort, run_dir in runs:
        try:
            if args.reanalyze:
                summary_json = run_recording_analysis(analyzer_script, latest_recording(run_dir))
            else:
                summary_json = latest_summary(run_dir)
        except FileNotFoundError:
            try:
                summary_json = run_recording_analysis(analyzer_script, latest_recording(run_dir))
            except FileNotFoundError:
                print(f"[skip] Incomplete run (missing recording/summary): {run_dir}")
                continue

        summary = load_summary(summary_json)
        outputs = summary.get("outputs", {})
        sustained_csv = Path(outputs.get("agent_max_y_sustained_csv", ""))
        stack_csv = Path(outputs.get("stack_csv", ""))

        height_time_rows.extend(
            load_time_series_rows(
                csv_path=sustained_csv,
                time_col="time_s",
                value_col="max_agent_y_sustained_2s",
                objective=objective,
                cohort=cohort,
                run_name=run_dir.name,
            )
        )
        stack_time_rows.extend(
            load_time_series_rows(
                csv_path=stack_csv,
                time_col="time_s",
                value_col="max_stacked_blocks",
                objective=objective,
                cohort=cohort,
                run_name=run_dir.name,
            )
        )

    # Keep the analysis folder focused: remove older outputs from prior script versions.
    for old in output_dir.glob("naive_vs_experienced_*"):
        if old.is_file():
            old.unlink()

    df_height_time = pd.DataFrame(height_time_rows)
    if not df_height_time.empty:
        df_height_time = df_height_time[
            df_height_time["cohort"].isin(["naive", "experienced"])
            & (df_height_time["objective"] == "highest")
        ].copy()
    df_stack_time = pd.DataFrame(stack_time_rows)
    if not df_stack_time.empty:
        df_stack_time = df_stack_time[
            df_stack_time["cohort"].isin(["naive", "experienced"])
            & (df_stack_time["objective"] == "tower")
        ].copy()

    wrote_any = False

    if not df_height_time.empty:
        max_y_over_time_plot = output_dir / "naive_vs_experienced_max_height_over_time_highest.png"
        plot_metric_over_time(
            df_height_time,
            ylabel="Max height reached",
            output_path=max_y_over_time_plot,
            max_time_min=args.max_time_min,
        )
        print(f"[done] Wrote plot: {max_y_over_time_plot}")
        wrote_any = True
    else:
        print(f"[skip] No highest-objective naive/experienced time-series rows found under: {evals_dir}")

    if not df_stack_time.empty:
        stack_over_time_plot = output_dir / "naive_vs_experienced_max_stacked_blocks_over_time_tower.png"
        plot_metric_over_time(
            df_stack_time,
            ylabel="Max Stacked Blocks",
            output_path=stack_over_time_plot,
            max_time_min=args.max_time_min,
        )
        print(f"[done] Wrote plot: {stack_over_time_plot}")
        wrote_any = True
    else:
        print(f"[skip] No tower-objective naive/experienced time-series rows found under: {evals_dir}")

    if not wrote_any:
        raise RuntimeError(
            f"No naive/experienced time-series rows found for any supported objective under: {evals_dir}"
        )


if __name__ == "__main__":
    main()
