#!/usr/bin/env python3
"""Plot cumulative best metric over chained generations for multiple conditions."""

from __future__ import annotations

import argparse
from pathlib import Path

import matplotlib.pyplot as plt
import pandas as pd
import seaborn as sns


METRIC_SPECS = {
    "height": {
        "analysis_csv": "agent_vertical.csv",
        "time_col": "time_s",
        "value_col": "agent_y",
        "ylabel": "Max height reached so far",
        "default_output_name": "cumulative_best_height_conditions.png",
    },
    "stacked_blocks": {
        "analysis_csv": "max_stack.csv",
        "time_col": "time_s",
        "value_col": "max_stacked_blocks",
        "ylabel": "Max stacked blocks reached so far",
        "default_output_name": "cumulative_best_stacked_blocks_conditions.png",
    },
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Compare cumulative best-so-far agent height across chained generation runs "
            "for multiple conditions."
        )
    )
    parser.add_argument(
        "--condition",
        nargs=2,
        action="append",
        metavar=("LABEL", "RUNS_DIR"),
        required=True,
        help=(
            "Condition label and runs directory. Repeat for each condition, e.g. "
            "--condition normal /path/to/runs --condition randomized_labels /path/to/runs"
        ),
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=None,
        help="Output PNG path (default: <cwd>/cumulative_best_height_conditions.png).",
    )
    parser.add_argument(
        "--csv-output",
        type=Path,
        default=None,
        help="Output CSV path for the stitched plotting dataframe.",
    )
    parser.add_argument(
        "--max-hours",
        type=float,
        default=10.0,
        help="Maximum x-axis extent in hours (default: 10).",
    )
    parser.add_argument(
        "--figsize",
        nargs=2,
        type=float,
        default=(6.5, 3.5),
        metavar=("W", "H"),
        help="Figure size in inches (default: 6.5 3.5).",
    )
    parser.add_argument(
        "--metric",
        choices=sorted(METRIC_SPECS),
        default="height",
        help="Metric to aggregate and plot (default: height).",
    )
    return parser.parse_args()


def resolve_experiment_root(runs_dir: Path) -> Path:
    runs_dir = runs_dir.resolve()
    if runs_dir.name == "runs" and (runs_dir.parent / "generation_chain.csv").exists():
        return runs_dir.parent
    if (runs_dir / "generation_chain.csv").exists():
        return runs_dir
    raise FileNotFoundError(
        f"Could not find generation_chain.csv for runs dir or experiment dir: {runs_dir}"
    )


def latest_metric_csv(run_dir: Path, analysis_csv: str) -> Path:
    matches = sorted(run_dir.glob(f"worlds/world-0/recordings/*_analysis/{analysis_csv}"))
    if not matches:
        raise FileNotFoundError(f"No {analysis_csv} found under run dir: {run_dir}")
    return matches[-1]


def session_trace(csv_path: Path, time_col: str, value_col: str) -> pd.DataFrame:
    df = pd.read_csv(csv_path, usecols=[time_col, value_col])
    df[time_col] = pd.to_numeric(df[time_col], errors="coerce")
    df[value_col] = pd.to_numeric(df[value_col], errors="coerce")
    df = df.dropna(subset=[time_col, value_col]).copy()
    if df.empty:
        return df
    df["local_time_s"] = df[time_col]
    df["metric_value"] = df[value_col]
    df = df[df["local_time_s"] >= 0.0].copy()
    return df


def build_condition_frame(label: str, runs_dir: Path, metric: str) -> pd.DataFrame:
    metric_spec = METRIC_SPECS[metric]
    experiment_root = resolve_experiment_root(runs_dir)
    chain_csv = experiment_root / "generation_chain.csv"
    chain = pd.read_csv(chain_csv)
    chain["generation"] = pd.to_numeric(chain["generation"], errors="raise").astype(int)
    chain["duration_seconds"] = pd.to_numeric(chain["duration_seconds"], errors="raise")
    chain = chain.sort_values("generation").reset_index(drop=True)

    rows: list[dict[str, float | int | str]] = []
    current_best = 0.0

    for _, row in chain.iterrows():
        run_id = str(row["run_id"])
        generation = int(row["generation"])
        run_dir = experiment_root / "runs" / run_id
        duration_s = float(row["duration_seconds"])
        offset_s = float(chain.loc[chain["generation"] < generation, "duration_seconds"].sum())

        rows.append(
            {
                "condition": label,
                "generation": generation,
                "run_id": run_id,
                "global_time_h": offset_s / 3600.0,
                "best_so_far": current_best,
            }
        )

        metric_csv = latest_metric_csv(run_dir, metric_spec["analysis_csv"])
        trace = session_trace(
            metric_csv,
            time_col=metric_spec["time_col"],
            value_col=metric_spec["value_col"],
        )
        if not trace.empty:
            running = trace["metric_value"].cummax()
            running = running.where(running > current_best, current_best)
            for local_time_s, best_height in zip(trace["local_time_s"], running):
                rows.append(
                    {
                        "condition": label,
                        "generation": generation,
                        "run_id": run_id,
                        "global_time_h": (offset_s + float(local_time_s)) / 3600.0,
                        "best_so_far": float(best_height),
                    }
                )
            current_best = max(current_best, float(running.iloc[-1]))

        rows.append(
            {
                "condition": label,
                "generation": generation,
                "run_id": run_id,
                "global_time_h": (offset_s + duration_s) / 3600.0,
                "best_so_far": current_best,
            }
        )

    return pd.DataFrame(rows).sort_values(["condition", "global_time_h"]).reset_index(drop=True)


def compress_step_frame(plot_df: pd.DataFrame) -> pd.DataFrame:
    keep_parts: list[pd.DataFrame] = []
    for _, frame in plot_df.groupby("condition", sort=False):
        frame = frame.sort_values("global_time_h").reset_index(drop=True)
        best_change = frame["best_so_far"].ne(frame["best_so_far"].shift(fill_value=-1.0))
        run_change_prev = frame["run_id"].ne(frame["run_id"].shift())
        run_change_next = frame["run_id"].ne(frame["run_id"].shift(-1))
        keep = best_change | run_change_prev | run_change_next
        keep.iloc[0] = True
        keep.iloc[-1] = True
        keep_parts.append(frame.loc[keep].copy())
    return pd.concat(keep_parts, ignore_index=True)


def main() -> None:
    args = parse_args()
    sns.set_theme(style="white", context="paper")

    condition_frames = [
        build_condition_frame(label=label, runs_dir=Path(runs_dir), metric=args.metric)
        for label, runs_dir in args.condition
    ]
    plot_df = pd.concat(condition_frames, ignore_index=True)
    plot_df = plot_df[plot_df["global_time_h"] <= args.max_hours].copy()
    export_df = compress_step_frame(plot_df)
    metric_spec = METRIC_SPECS[args.metric]

    output_path = (
        args.output.resolve()
        if args.output is not None
        else (Path.cwd() / metric_spec["default_output_name"])
    )
    csv_output_path = (
        args.csv_output.resolve()
        if args.csv_output is not None
        else output_path.with_suffix(".csv")
    )

    plt.figure(figsize=tuple(args.figsize))
    palette = {
        "normal": "#4C78A8",
        "randomized_labels": "#F58518",
    }
    ax = plt.gca()
    for label, frame in export_df.groupby("condition", sort=False):
        color = palette.get(label, None)
        ax.step(
            frame["global_time_h"],
            frame["best_so_far"],
            where="post",
            linewidth=2.0,
            label=label,
            color=color,
        )

    ax.set_xlim(0, args.max_hours)
    ax.set_xlabel("Total time (hours)")
    ax.set_ylabel(metric_spec["ylabel"])
    sns.despine(ax=ax, top=True, right=True)
    ax.legend(frameon=False)
    plt.tight_layout()

    output_path.parent.mkdir(parents=True, exist_ok=True)
    plt.savefig(output_path, dpi=180)
    plt.close()

    export_df.to_csv(csv_output_path, index=False)
    print(f"[done] Wrote figure: {output_path}")
    print(f"[done] Wrote csv: {csv_output_path}")


if __name__ == "__main__":
    main()
