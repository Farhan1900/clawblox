//! Re-simulate a recorded agent session: load a world script into a fresh
//! GameInstance, inject a recorded input schedule at the original ticks, and
//! print the player trajectory plus dynamic-part anomalies (speed spikes,
//! parts below a floor threshold). Used to reproduce live physics failures
//! deterministically so they can be instrumented in-process.
//!
//! Usage:
//!   cargo run --release --example replay_inputs -- \
//!     <world_script.lua> <inputs.json> <until_tick> [trace_from] [trace_every]
//!
//! inputs.json: [{"tick": N, "type": "MoveTo", "data": {...}}, ...]

use std::collections::HashMap;
use std::env;
use std::fs;

use clawblox::game::instance::GameInstance;
use uuid::Uuid;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        eprintln!(
            "usage: replay_inputs <world_script.lua> <inputs.json> <until_tick> [trace_from] [trace_every]"
        );
        std::process::exit(2);
    }
    let script = fs::read_to_string(&args[1]).expect("read world script");
    let inputs_raw = fs::read_to_string(&args[2]).expect("read inputs json");
    let until_tick: u64 = args[3].parse().expect("until_tick");
    let trace_from: u64 = args.get(4).map(|s| s.parse().unwrap()).unwrap_or(0);
    let trace_every: u64 = args.get(5).map(|s| s.parse().unwrap()).unwrap_or(60);

    #[derive(serde::Deserialize)]
    struct In {
        tick: u64,
        #[serde(rename = "type")]
        input_type: String,
        data: serde_json::Value,
    }
    let inputs: Vec<In> = serde_json::from_str(&inputs_raw).expect("parse inputs");
    let mut by_tick: HashMap<u64, Vec<(String, serde_json::Value)>> = HashMap::new();
    for input in inputs {
        if input.tick <= until_tick {
            by_tick
                .entry(input.tick)
                .or_default()
                .push((input.input_type, input.data));
        }
    }

    let mut instance = GameInstance::new(Uuid::new_v4(), None);
    instance.load_script(&script);

    let agent_id = Uuid::new_v4();
    assert!(instance.add_player(agent_id, "ReplayAgent"));
    let user_id = *instance.players.get(&agent_id).expect("player user id");
    let hrp_id = *instance
        .player_hrp_ids
        .get(&agent_id)
        .expect("player hrp id");

    let mut prev_speeds: HashMap<u64, [f32; 3]> = HashMap::new();
    for t in 1..=until_tick {
        if let Some(queued) = by_tick.remove(&t) {
            for (input_type, data) in queued {
                instance.queue_agent_input(user_id, input_type, data);
            }
        }
        instance.tick();

        if t >= trace_from && t % trace_every == 0 {
            if let Some(p) = instance.physics.get_character_position(hrp_id) {
                println!("TRACE {t} player {:.3} {:.3} {:.3}", p[0], p[1], p[2]);
            }
        }

        // Anomaly scan at 1 Hz: dynamic parts moving fast or fallen below the map.
        if t % 60 == 0 {
            let runtime = instance.lua_runtime.as_ref().unwrap();
            for inst in runtime.workspace().get_descendants() {
                let (id, name, anchored) = {
                    let data = inst.data.lock().unwrap();
                    let Some(part) = data.part_data.as_ref() else {
                        continue;
                    };
                    (data.id.0, data.name.clone(), part.anchored)
                };
                if anchored || !instance.physics.has_part(id) {
                    continue;
                }
                let Some(pos) = instance.physics.get_part_position(id) else {
                    continue;
                };
                let prev = prev_speeds.insert(id, pos);
                if let Some(prev) = prev {
                    let d = ((pos[0] - prev[0]).powi(2)
                        + (pos[1] - prev[1]).powi(2)
                        + (pos[2] - prev[2]).powi(2))
                    .sqrt();
                    if d > 8.0 {
                        println!(
                            "ANOMALY {t} part {id} {name} moved {d:.1} in 60 ticks -> ({:.1},{:.1},{:.1})",
                            pos[0], pos[1], pos[2]
                        );
                    }
                }
                if pos[1] < -2.0 {
                    println!(
                        "ANOMALY {t} part {id} {name} below water shell at ({:.1},{:.2},{:.1})",
                        pos[0], pos[1], pos[2]
                    );
                }
            }
        }
    }
    let p = instance
        .physics
        .get_character_position(hrp_id)
        .unwrap_or([f32::NAN; 3]);
    println!(
        "FINAL {until_tick} player {:.3} {:.3} {:.3}",
        p[0], p[1], p[2]
    );
}
