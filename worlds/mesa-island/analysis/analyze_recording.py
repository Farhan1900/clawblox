#!/usr/bin/env python3
import argparse
import csv
import io
import json
import math
import sqlite3
from collections import defaultdict
from pathlib import Path
from statistics import median

import matplotlib.pyplot as plt
from matplotlib.colors import Normalize
from matplotlib.colors import LinearSegmentedColormap
import zstandard as zstd


def parse_duration_seconds(value: str) -> float:
    text = value.strip().lower()
    if not text:
        raise argparse.ArgumentTypeError("Duration cannot be empty.")

    unit_multiplier = 1.0
    if text[-1] in {"s", "m", "h"}:
        unit_multiplier = {"s": 1.0, "m": 60.0, "h": 3600.0}[text[-1]]
        text = text[:-1].strip()

    try:
        amount = float(text)
    except ValueError as exc:
        raise argparse.ArgumentTypeError(
            "Invalid duration. Use seconds (e.g. 3600) or suffix with s/m/h (e.g. 1h)."
        ) from exc

    if amount < 0:
        raise argparse.ArgumentTypeError("Duration must be >= 0.")

    return amount * unit_multiplier


def parse_args():
    parser = argparse.ArgumentParser(
        description="Analyze a .clawrec recording and plot agent vertical position + block stack height over time."
    )
    parser.add_argument("recording", type=Path, help="Path to .clawrec file")
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=None,
        help="Output directory for CSV/PNG files (default: <recording_stem>_analysis)",
    )
    parser.add_argument(
        "--agent-name",
        type=str,
        default=None,
        help="Track this exact agent/player name (default: first seen player)",
    )
    parser.add_argument(
        "--agent-id",
        type=str,
        default=None,
        help="Track this exact agent UUID (default: first seen player)",
    )
    parser.add_argument(
        "--tick-step",
        type=int,
        default=1,
        help="Sample every N ticks in output (default: 1)",
    )
    parser.add_argument(
        "--max-tick",
        type=int,
        default=None,
        help="Stop analysis at this tick",
    )
    parser.add_argument(
        "--max-duration",
        type=parse_duration_seconds,
        default=None,
        help="Stop analysis after this many seconds from recording start (supports suffix s/m/h, e.g. 1h)",
    )
    parser.add_argument(
        "--stack-bin-size",
        type=float,
        default=None,
        help="XZ bin size for stack grouping (default: auto from block size)",
    )
    return parser.parse_args()


def decode_blob(blob: bytes):
    # Some frames omit uncompressed size in the header; stream_reader handles both cases.
    dctx = zstd.ZstdDecompressor()
    with dctx.stream_reader(io.BytesIO(blob)) as reader:
        decompressed = reader.read()
    return json.loads(decompressed)


def is_block_entity(entity: dict) -> bool:
    attrs = entity.get("attributes") or {}
    if attrs.get("IsPickupObject") is True:
        return True
    if attrs.get("IsPickupBlock") is True:
        return True
    render = entity.get("render") or {}
    if render.get("role") == "PickupObject":
        return True
    if render.get("role") == "Block":
        return True
    return False


def is_visible_world_part(entity: dict) -> bool:
    if entity.get("type") != "part":
        return False
    pos = entity.get("position")
    size = entity.get("size")
    render = entity.get("render") or {}
    if not pos or len(pos) < 3 or not size or len(size) < 3:
        return False
    if render.get("kind") != "primitive":
        return False
    if render.get("visible") is False:
        return False
    if render.get("role") in {"Ocean", "Sky", "Cloud"}:
        return False
    try:
        sx = float(size[0])
        sz = float(size[2])
    except Exception:
        return False
    return sx > 0 and sz > 0


def entity_xz_bounds(entity: dict) -> tuple[float, float, float, float] | None:
    pos = entity.get("position")
    size = entity.get("size")
    if not pos or len(pos) < 3 or not size or len(size) < 3:
        return None
    try:
        x = float(pos[0])
        z = float(pos[2])
        sx = float(size[0])
        sz = float(size[2])
    except Exception:
        return None
    half_x = sx * 0.5
    half_z = sz * 0.5
    return (x - half_x, x + half_x, z - half_z, z + half_z)


def entity_ground_height(entity: dict) -> float | None:
    pos = entity.get("position")
    size = entity.get("size")
    if not pos or len(pos) < 2 or not size or len(size) < 2:
        return None
    try:
        y = float(pos[1])
        sy = float(size[1])
    except Exception:
        return None
    return y + sy * 0.5


def compute_max_stack(observation: dict, explicit_bin_size: float | None) -> tuple[int, int]:
    blocks = []
    for ent in observation.get("entities", []):
        if not is_block_entity(ent):
            continue
        pos = ent.get("position")
        size = ent.get("size")
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

    heights = sorted(b[3] for b in blocks)
    nominal_h = heights[len(heights) // 2] if heights else 0.75
    if nominal_h <= 0:
        nominal_h = 0.75

    bin_size = explicit_bin_size if explicit_bin_size and explicit_bin_size > 0 else nominal_h * 0.65
    min_gap = nominal_h * 0.45
    max_gap = nominal_h * 1.75

    columns = defaultdict(list)
    for x, y, z, _ in blocks:
        key = (int(round(x / bin_size)), int(round(z / bin_size)))
        columns[key].append(y)

    best = 1
    for ys in columns.values():
        ys.sort()
        cur = 1
        col_best = 1
        for i in range(1, len(ys)):
            dy = ys[i] - ys[i - 1]
            if min_gap <= dy <= max_gap:
                cur += 1
            else:
                cur = 1
            if cur > col_best:
                col_best = cur
        if col_best > best:
            best = col_best

    return best, len(blocks)


def apply_delta(observation: dict, delta: dict):
    entities = observation.get("entities", [])
    by_entity_id = {int(e["id"]): e for e in entities if "id" in e}

    for entry in delta.get("entities", []):
        if "add" in entry:
            add_ent = entry["add"]
            by_entity_id[int(add_ent["id"])] = add_ent
        elif "rm" in entry and entry.get("rm"):
            by_entity_id.pop(int(entry["id"]), None)
        else:
            ent = by_entity_id.get(int(entry["id"]))
            if ent is None:
                continue
            if "p" in entry and entry["p"] is not None:
                ent["position"] = entry["p"]
            if "r" in entry and entry["r"] is not None:
                ent["rotation"] = entry["r"]

    observation["entities"] = list(by_entity_id.values())

    players = observation.get("players", [])
    by_player_id = {p["id"]: p for p in players if "id" in p}
    for pd in delta.get("players", []):
        player = by_player_id.get(pd.get("id"))
        if player is None:
            continue
        if pd.get("p") is not None:
            player["position"] = pd["p"]
        if pd.get("health") is not None:
            player["health"] = pd["health"]

    observation["players"] = list(by_player_id.values())
    observation["sounds"] = delta.get("sounds", [])
    observation["recent_events"] = delta.get("recent_events", [])


def choose_agent(players: list[dict], explicit_name: str | None, explicit_id: str | None):
    if explicit_id:
        for p in players:
            if p.get("id") == explicit_id:
                return explicit_id, p.get("name", explicit_id)
        return None, None
    if explicit_name:
        for p in players:
            if p.get("name") == explicit_name:
                return p.get("id"), explicit_name
        return None, None
    if not players:
        return None, None
    players_sorted = sorted(players, key=lambda p: p.get("name", ""))
    chosen = players_sorted[0]
    return chosen.get("id"), chosen.get("name", chosen.get("id"))


def get_agent_position(player: dict, entities: list[dict]) -> tuple[float, float, float]:
    player_id = player.get("id", "unknown")
    root_part_id = player.get("root_part_id")
    if not isinstance(root_part_id, int):
        raise RuntimeError(f"Player {player_id} is missing integer root_part_id: {root_part_id!r}")

    root_entity = next((ent for ent in entities if ent.get("id") == root_part_id), None)
    if root_entity is None:
        raise RuntimeError(f"Player {player_id} root_part_id={root_part_id} not found in entities")

    pos = root_entity.get("position")
    if not isinstance(pos, list) or len(pos) < 3:
        raise RuntimeError(
            f"Player {player_id} root entity {root_part_id} has invalid position payload: {pos!r}"
        )

    x = float(pos[0])
    y = float(pos[1])
    z = float(pos[2])
    if not (math.isfinite(x) and math.isfinite(y) and math.isfinite(z)):
        raise RuntimeError(
            f"Player {player_id} root entity {root_part_id} position is non-finite: {pos!r}"
        )
    return x, y, z


def plot_series(
    x_values: list[float],
    y_values: list[float],
    ylabel: str,
    title: str,
    output_png: Path,
):
    plt.figure(figsize=(12, 4))
    plt.plot(x_values, y_values, linewidth=1.2)
    plt.xlabel("Time (s)")
    plt.ylabel(ylabel)
    plt.title(title)
    plt.grid(True, alpha=0.3)
    plt.tight_layout()
    plt.savefig(output_png, dpi=150)
    plt.close()


def interpolate_series(values: list[float]) -> list[float]:
    if not values:
        return []

    out = list(values)
    finite_indices = [i for i, value in enumerate(values) if math.isfinite(value)]
    if len(finite_indices) < 2:
        return out

    for left_idx, right_idx in zip(finite_indices, finite_indices[1:]):
        left_value = values[left_idx]
        right_value = values[right_idx]
        gap = right_idx - left_idx
        if gap <= 1:
            continue
        step = (right_value - left_value) / gap
        for offset in range(1, gap):
            out[left_idx + offset] = left_value + step * offset

    return out


def plot_top_down_trajectory(
    x_values: list[float],
    z_values: list[float],
    y_values: list[float],
    map_parts: list[dict],
    title: str,
    output_png: Path,
):
    fig, ax = plt.subplots(figsize=(9, 9))

    map_min_x = None
    map_max_x = None
    map_min_z = None
    map_max_z = None
    part_heights = [h for h in (entity_ground_height(part) for part in map_parts) if h is not None]
    ground_norm = Normalize(
        vmin=min(part_heights) if part_heights else 0.0,
        vmax=max(part_heights) if part_heights and max(part_heights) > min(part_heights) else (min(part_heights) + 1e-6 if part_heights else 1.0),
    )
    ground_cmap = LinearSegmentedColormap.from_list(
        "earthy_gray",
        ["#d2c6b8", "#bda88f", "#a2896f", "#89705a"],
    )

    for part in map_parts:
        bounds = entity_xz_bounds(part)
        if bounds is None:
            continue
        min_x, max_x, min_z, max_z = bounds
        height = entity_ground_height(part)
        facecolor = ground_cmap(ground_norm(height if height is not None else ground_norm.vmin))
        rect = plt.Rectangle(
            (min_x, min_z),
            max_x - min_x,
            max_z - min_z,
            facecolor=facecolor,
            edgecolor="none",
            linewidth=0,
            alpha=1.0,
            zorder=1,
        )
        ax.add_patch(rect)
        map_min_x = min_x if map_min_x is None else min(map_min_x, min_x)
        map_max_x = max_x if map_max_x is None else max(map_max_x, max_x)
        map_min_z = min_z if map_min_z is None else min(map_min_z, min_z)
        map_max_z = max_z if map_max_z is None else max(map_max_z, max_z)

    finite_samples = [
        (x, z, y)
        for x, z, y in zip(x_values, z_values, y_values)
        if math.isfinite(x) and math.isfinite(z) and math.isfinite(y)
    ]
    if len(finite_samples) < 2:
        raise ValueError("Need at least two finite X/Z/Y samples to plot a trajectory gradient.")

    xs = [sample[0] for sample in finite_samples]
    zs = [sample[1] for sample in finite_samples]
    ys = [sample[2] for sample in finite_samples]

    ax.plot(xs, zs, color="#111111", linewidth=1.8, alpha=0.95, zorder=3)
    ax.scatter([xs[0]], [zs[0]], color="#2ca02c", s=40, zorder=4)
    ax.scatter([xs[-1]], [zs[-1]], color="#1f77b4", s=40, zorder=4)
    path_min_x = min(xs)
    path_max_x = max(xs)
    path_min_z = min(zs)
    path_max_z = max(zs)

    min_x = path_min_x if map_min_x is None else min(path_min_x, map_min_x)
    max_x = path_max_x if map_max_x is None else max(path_max_x, map_max_x)
    min_z = path_min_z if map_min_z is None else min(path_min_z, map_min_z)
    max_z = path_max_z if map_max_z is None else max(path_max_z, map_max_z)

    span_x = max(max_x - min_x, 1.0)
    span_z = max(max_z - min_z, 1.0)
    pad_x = span_x * 0.05
    pad_z = span_z * 0.05
    ax.set_xlim(min_x - pad_x, max_x + pad_x)
    ax.set_ylim(min_z - pad_z, max_z + pad_z)
    ax.set_aspect("equal", adjustable="box")
    ax.set_title("")
    ax.grid(False)
    ax.set_axis_off()
    fig.tight_layout()
    fig.savefig(output_png, dpi=180)
    plt.close(fig)


def compute_sustained_max_y_series(
    times_s: list[float],
    y_values: list[float],
    hold_seconds: float = 2.0,
    y_tolerance: float = 0.05,
) -> list[float]:
    sustained_max_values = []
    confirmed_max_y = None
    run_anchor_y = None
    run_start_t = None
    run_peak_y = None

    for t_s, y in zip(times_s, y_values):
        if not math.isfinite(y):
            run_anchor_y = None
            run_start_t = None
            run_peak_y = None
            sustained_max_values.append(math.nan if confirmed_max_y is None else confirmed_max_y)
            continue

        if run_anchor_y is None or abs(y - run_anchor_y) > y_tolerance:
            run_anchor_y = y
            run_start_t = t_s
            run_peak_y = y
        else:
            run_peak_y = max(run_peak_y, y)

        if run_start_t is not None and (t_s - run_start_t) >= hold_seconds:
            candidate_max = run_peak_y if run_peak_y is not None else y
            if confirmed_max_y is None or candidate_max > confirmed_max_y:
                confirmed_max_y = candidate_max

        sustained_max_values.append(math.nan if confirmed_max_y is None else confirmed_max_y)

    return sustained_max_values


def main():
    args = parse_args()
    recording = args.recording.resolve()
    if not recording.exists():
        raise FileNotFoundError(f"Recording not found: {recording}")

    out_dir = args.output_dir or recording.with_name(f"{recording.stem}_analysis")
    out_dir.mkdir(parents=True, exist_ok=True)

    conn = sqlite3.connect(str(recording))
    conn.row_factory = sqlite3.Row
    cur = conn.cursor()

    meta = {
        row["key"]: row["value"]
        for row in cur.execute("SELECT key, value FROM meta").fetchall()
    }
    tick_rate = int(meta.get("tick_rate", "60"))

    cp_rows = cur.execute("SELECT tick, time_ms FROM checkpoints ORDER BY tick").fetchall()
    if not cp_rows:
        raise RuntimeError("No checkpoints in recording.")
    cp_ticks = [int(r["tick"]) for r in cp_rows]
    first_tick = cp_ticks[0]
    first_time_ms = int(cp_rows[0]["time_ms"])

    db_max_tick_row = cur.execute(
        """
        SELECT COALESCE(MAX(t), 0) AS max_tick
        FROM (
          SELECT MAX(tick) AS t FROM checkpoints
          UNION ALL
          SELECT MAX(tick) AS t FROM deltas
        )
        """
    ).fetchone()
    db_max_tick = int(db_max_tick_row["max_tick"]) if db_max_tick_row else first_tick
    meta_total_ticks = int(meta.get("total_ticks", "0") or "0")
    end_tick = max(db_max_tick, meta_total_ticks)
    if args.max_tick is not None:
        end_tick = min(end_tick, args.max_tick)

    max_end_time_ms = None
    if args.max_duration is not None:
        max_end_time_ms = first_time_ms + int(round(args.max_duration * 1000.0))
        print(f"[info] Limiting analysis to first {args.max_duration:.3f}s of recording time.")

    agent_csv = out_dir / "agent_vertical.csv"
    stack_csv = out_dir / "max_stack.csv"
    y_plot_png = out_dir / "agent_vertical.png"
    y_sustained_csv = out_dir / "agent_max_y_sustained.csv"
    y_sustained_plot_png = out_dir / "agent_max_y_sustained.png"
    stack_plot_png = out_dir / "max_stack.png"
    trajectory_csv = out_dir / "agent_trajectory.csv"
    trajectory_plot_png = out_dir / "agent_trajectory_map.png"
    summary_json = out_dir / "summary.json"

    sustained_hold_s = 1.0
    y_times = []
    y_vals = []
    x_vals = []
    z_vals = []
    agent_sample_ticks = []
    agent_sample_times = []
    agent_sample_y = []
    agent_sample_x = []
    agent_sample_z = []
    stack_times = []
    stack_vals = []
    static_map_parts = []
    map_part_ids = set()

    selected_agent_id = args.agent_id
    selected_agent_name = args.agent_name
    auto_select_announced = False

    max_stack_seen = 0
    max_stack_tick = None
    max_y_seen = None
    max_y_tick = None

    with (
        agent_csv.open("w", newline="") as f_agent,
        stack_csv.open("w", newline="") as f_stack,
        trajectory_csv.open("w", newline="") as f_traj,
    ):
        agent_writer = csv.writer(f_agent)
        stack_writer = csv.writer(f_stack)
        traj_writer = csv.writer(f_traj)
        agent_writer.writerow(["tick", "time_s", "agent_id", "agent_name", "agent_y"])
        stack_writer.writerow(["tick", "time_s", "max_stacked_blocks", "num_blocks"])
        traj_writer.writerow(["tick", "time_s", "agent_id", "agent_name", "agent_x", "agent_z", "agent_y"])

        for idx, cp_tick in enumerate(cp_ticks):
            if cp_tick > end_tick:
                break
            next_cp = cp_ticks[idx + 1] if idx + 1 < len(cp_ticks) else end_tick + 1
            segment_end = min(end_tick, next_cp - 1)
            if segment_end < cp_tick:
                continue

            cp_row = cur.execute(
                "SELECT tick, time_ms, data FROM checkpoints WHERE tick = ?",
                (cp_tick,),
            ).fetchone()
            if cp_row is None:
                continue

            time_ms = int(cp_row["time_ms"])
            if max_end_time_ms is not None and time_ms > max_end_time_ms:
                break
            observation = decode_blob(cp_row["data"])
            for ent in observation.get("entities", []):
                ent_id = ent.get("id")
                if ent_id in map_part_ids:
                    continue
                if is_visible_world_part(ent):
                    static_map_parts.append(ent)
                    map_part_ids.add(ent_id)

            def sample_now(tick: int, ms: int, obs: dict):
                nonlocal selected_agent_id, selected_agent_name
                nonlocal auto_select_announced, max_stack_seen, max_stack_tick, max_y_seen, max_y_tick

                if tick < first_tick:
                    return
                if max_end_time_ms is not None and ms > max_end_time_ms:
                    return
                if tick != end_tick and ((tick - first_tick) % args.tick_step != 0):
                    return

                players = obs.get("players", [])
                if not selected_agent_id:
                    aid, aname = choose_agent(players, args.agent_name, args.agent_id)
                    if aid:
                        selected_agent_id = aid
                        selected_agent_name = aname
                        if args.agent_name is None and args.agent_id is None and not auto_select_announced:
                            print(f"[info] Auto-selected agent: {selected_agent_name} ({selected_agent_id})")
                            auto_select_announced = True

                agent_y = None
                agent_x = None
                agent_z = None
                if selected_agent_id:
                    for p in players:
                        if p.get("id") == selected_agent_id:
                            agent_x, agent_y, agent_z = get_agent_position(
                                p,
                                obs.get("entities", []),
                            )
                            break

                max_stack, num_blocks = compute_max_stack(obs, args.stack_bin_size)
                t_s = ms / 1000.0
                agent_sample_ticks.append(tick)
                agent_sample_times.append(t_s)
                agent_sample_y.append(math.nan if agent_y is None else agent_y)
                agent_sample_x.append(math.nan if agent_x is None else agent_x)
                agent_sample_z.append(math.nan if agent_z is None else agent_z)

                agent_writer.writerow(
                    [
                        tick,
                        f"{t_s:.6f}",
                        selected_agent_id or "",
                        selected_agent_name or "",
                        "" if agent_y is None else f"{agent_y:.6f}",
                    ]
                )
                traj_writer.writerow(
                    [
                        tick,
                        f"{t_s:.6f}",
                        selected_agent_id or "",
                        selected_agent_name or "",
                        "" if agent_x is None else f"{agent_x:.6f}",
                        "" if agent_z is None else f"{agent_z:.6f}",
                        "" if agent_y is None else f"{agent_y:.6f}",
                    ]
                )
                stack_writer.writerow([tick, f"{t_s:.6f}", max_stack, num_blocks])

                if agent_y is not None:
                    y_times.append(t_s)
                    y_vals.append(agent_y)
                    if max_y_seen is None or agent_y > max_y_seen:
                        max_y_seen = agent_y
                        max_y_tick = tick
                if agent_x is not None and agent_z is not None:
                    x_vals.append(agent_x)
                    z_vals.append(agent_z)

                stack_times.append(t_s)
                stack_vals.append(float(max_stack))
                if max_stack > max_stack_seen:
                    max_stack_seen = max_stack
                    max_stack_tick = tick

            sample_now(int(cp_row["tick"]), time_ms, observation)

            delta_query = """
                SELECT tick, time_ms, data
                FROM deltas
                WHERE tick > ? AND tick <= ?
            """
            delta_params = [cp_tick, segment_end]
            if max_end_time_ms is not None:
                delta_query += " AND time_ms <= ?"
                delta_params.append(max_end_time_ms)
            delta_query += "\n                ORDER BY tick"
            delta_rows = cur.execute(delta_query, tuple(delta_params)).fetchall()

            for row in delta_rows:
                tick = int(row["tick"])
                time_ms = int(row["time_ms"])
                delta = decode_blob(row["data"])
                apply_delta(observation, delta)
                observation["tick"] = tick
                observation["server_time_ms"] = time_ms
                sample_now(tick, time_ms, observation)

    sustained_max_values = compute_sustained_max_y_series(
        agent_sample_times,
        agent_sample_y,
        hold_seconds=sustained_hold_s,
    )
    interpolated_agent_x = interpolate_series(agent_sample_x)
    interpolated_agent_y = interpolate_series(agent_sample_y)
    interpolated_agent_z = interpolate_series(agent_sample_z)
    with y_sustained_csv.open("w", newline="") as f_sustained:
        sustained_writer = csv.writer(f_sustained)
        sustained_writer.writerow(["tick", "time_s", "max_agent_y_sustained_2s"])
        for tick, t_s, max_y in zip(agent_sample_ticks, agent_sample_times, sustained_max_values):
            sustained_writer.writerow([tick, f"{t_s:.6f}", "" if not math.isfinite(max_y) else f"{max_y:.6f}"])

    if y_times and y_vals:
        plot_series(
            y_times,
            y_vals,
            ylabel="Agent Y Position",
            title=f"Agent Vertical Position Over Time ({selected_agent_name or 'agent'})",
            output_png=y_plot_png,
        )
    else:
        print("[warn] No agent Y samples found; agent_vertical.png not generated.")

    has_sustained_max = any(math.isfinite(v) for v in sustained_max_values)
    if has_sustained_max:
        plot_series(
            agent_sample_times,
            sustained_max_values,
            ylabel=f"Max Agent Y (held >= {sustained_hold_s:.0f}s)",
            title=f"Sustained Max Agent Y Over Time ({selected_agent_name or 'agent'})",
            output_png=y_sustained_plot_png,
        )
    else:
        print("[warn] No sustained max agent Y samples found; agent_max_y_sustained.png not generated.")

    if stack_times and stack_vals:
        plot_series(
            stack_times,
            stack_vals,
            ylabel="Max Stacked Blocks",
            title="Max Number of Blocks Stacked Over Time",
            output_png=stack_plot_png,
        )
    else:
        print("[warn] No stack samples found; max_stack.png not generated.")

    if x_vals and z_vals:
        plot_top_down_trajectory(
            interpolated_agent_x,
            interpolated_agent_z,
            interpolated_agent_y,
            map_parts=static_map_parts,
            title=f"Top-Down Agent Trajectory ({selected_agent_name or 'agent'})",
            output_png=trajectory_plot_png,
        )
    else:
        print("[warn] No agent X/Z samples found; agent_trajectory_map.png not generated.")

    max_sustained_y = None
    max_sustained_y_tick = None
    for tick, value in zip(agent_sample_ticks, sustained_max_values):
        if not math.isfinite(value):
            continue
        if max_sustained_y is None or value > max_sustained_y:
            max_sustained_y = value
            max_sustained_y_tick = tick

    summary = {
        "recording": str(recording),
        "tick_rate": tick_rate,
        "first_tick": first_tick,
        "first_time_ms": first_time_ms,
        "end_tick": end_tick,
        "max_duration_s": args.max_duration,
        "agent_id": selected_agent_id,
        "agent_name": selected_agent_name,
        "trajectory_samples": len(x_vals),
        "map_part_count": len(static_map_parts),
        "max_agent_y": max_y_seen,
        "max_agent_y_tick": max_y_tick,
        "max_agent_y_sustained_hold_s": sustained_hold_s,
        "max_agent_y_sustained": max_sustained_y,
        "max_agent_y_sustained_tick": max_sustained_y_tick,
        "max_stack_seen": max_stack_seen,
        "max_stack_tick": max_stack_tick,
        "outputs": {
            "agent_csv": str(agent_csv),
            "agent_max_y_sustained_csv": str(y_sustained_csv),
            "stack_csv": str(stack_csv),
            "agent_plot_png": str(y_plot_png) if y_times else None,
            "agent_max_y_sustained_plot_png": str(y_sustained_plot_png) if has_sustained_max else None,
            "stack_plot_png": str(stack_plot_png) if stack_times else None,
        },
    }
    summary_json.write_text(json.dumps(summary, indent=2))

    print(f"[done] Wrote analysis to: {out_dir}")
    print(f"[done] Agent CSV: {agent_csv}")
    print(f"[done] Agent sustained max CSV: {y_sustained_csv}")
    print(f"[done] Stack CSV: {stack_csv}")
    if y_times:
        print(f"[done] Agent plot: {y_plot_png}")
    if has_sustained_max:
        print(f"[done] Agent sustained max plot: {y_sustained_plot_png}")
    if stack_times:
        print(f"[done] Stack plot: {stack_plot_png}")
    print(f"[done] Summary: {summary_json}")


if __name__ == "__main__":
    main()
