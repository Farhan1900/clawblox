use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use clawblox::game::instance::GameInstance;
use serde_json::Value;
use uuid::Uuid;

fn read_f64(instance: &GameInstance, expr: &str) -> f64 {
    instance
        .lua_runtime
        .as_ref()
        .expect("missing runtime")
        .lua()
        .load(expr)
        .eval()
        .expect("failed to read f64")
}

fn default_machine_playing_paper_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("clawblox should have a parent directory")
        .join("machine-playing-paper")
}

fn machine_playing_paper_root() -> PathBuf {
    std::env::var_os("MACHINE_PLAYING_PAPER_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(default_machine_playing_paper_root)
}

fn construction_world_dir() -> PathBuf {
    machine_playing_paper_root()
        .join("worlds")
        .join("construction-world")
}

fn construction_recording_path() -> PathBuf {
    std::env::var_os("CONSTRUCTION_WORLD_REPRO_RECORDING")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            construction_world_dir()
                .join("results")
                .join("genexp-claude-20260604T002926Z")
                .join("runs")
                .join("genexp-claude-20260604T002926Z-g001")
                .join("recordings")
                .join("20260604_002927.clawrec")
        })
}

/// Regression for the construction-world floating-block root cause.
///
/// Roblox-style CFrame writes to a welded assembly member should move the
/// whole assembly while preserving weld offsets. Before the fix, an anchored
/// welded assembly can become Lua-visible as a split state: the written member
/// moves to hand height while connected members stay on the ground.
#[test]
#[ignore = "focused construction-world regression; run manually when touching welded assembly CFrame sync"]
fn anchored_welded_member_cframe_write_moves_whole_lua_assembly() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);
    instance.load_script(
        r#"
        Workspace.Gravity = 0

        local low = Instance.new("Part")
        low.Name = "Low"
        low.Size = Vector3.new(0.75, 0.75, 0.75)
        low.Position = Vector3.new(-1.5, 0.375, -13.5)
        low.Anchored = true
        low.CanCollide = false
        low.Parent = Workspace

        local high = Instance.new("Part")
        high.Name = "High"
        high.Size = Vector3.new(0.75, 0.75, 0.75)
        high.Position = Vector3.new(-0.75, 0.375, -13.5)
        high.Anchored = true
        high.CanCollide = false
        high.Parent = Workspace

        local weld = Instance.new("WeldConstraint")
        weld.Name = "ConstructionBond"
        weld.Part0 = low
        weld.Part1 = high
        weld.Parent = low

        _G.low = low
        _G.high = high
        "#,
    );

    for _ in 0..5 {
        instance.tick();
    }

    instance
        .lua_runtime
        .as_ref()
        .unwrap()
        .lua()
        .load("_G.high.CFrame = CFrame.new(-1.5, 6.05, -13.5)")
        .exec()
        .expect("failed to write member CFrame");

    for _ in 0..5 {
        instance.tick();
    }

    let low_x = read_f64(&instance, "return _G.low.Position.X");
    let low_y = read_f64(&instance, "return _G.low.Position.Y");
    let high_x = read_f64(&instance, "return _G.high.Position.X");
    let high_y = read_f64(&instance, "return _G.high.Position.Y");
    let connected: i64 = instance
        .lua_runtime
        .as_ref()
        .unwrap()
        .lua()
        .load("return #_G.high:GetConnectedParts(true)")
        .eval()
        .expect("failed to read connected part count");

    assert_eq!(connected, 1);
    assert!(
        (high_x + 1.5).abs() < 0.01 && (high_y - 6.05).abs() < 0.01,
        "written member should reach target, high=({high_x}, {high_y})"
    );
    assert!(
        (low_x + 2.25).abs() < 0.01 && (low_y - 6.05).abs() < 0.01,
        "connected member should preserve offset and move with assembly, low=({low_x}, {low_y})"
    );
}

/// Slow end-to-end regression for the June 4 construction-world recording.
///
/// This replays inputs through tick 14030 and checks that the local construction
/// cluster does not reproduce the floating-block/fly-away failure. It is ignored
/// because it takes several minutes and depends on the machine-playing-paper
/// fixture recording being present.
#[test]
#[ignore = "slow fixture replay; run manually when validating the construction-world fix"]
fn construction_world_recording_prefix_does_not_float_or_fly_away() {
    let world_dir = construction_world_dir();
    let recording = construction_recording_path();
    let source = fs::read_to_string(world_dir.join("main.lua")).unwrap_or_else(|err| {
        panic!(
            "failed to read construction world at '{}': {err}",
            world_dir.display()
        )
    });
    let inputs = read_inputs(&recording);

    let mut instance = GameInstance::new(Uuid::new_v4(), None);
    instance.load_script(&source);
    let agent_id = Uuid::parse_str("c0c709ef-0bee-43ba-8455-78ffd6bb76dd").unwrap();
    assert!(instance.add_player(agent_id, "Eko-rgenexp-claude-20260604T002926Z-g001-w0-a0"));
    let user_id = *instance.players.get(&agent_id).expect("missing player");

    let mut next = 0usize;
    for tick in 0..=14_030usize {
        while next < inputs.len() && inputs[next].0 == tick {
            let (_, kind, data) = &inputs[next];
            instance.queue_agent_input(user_id, kind.clone(), data.clone());
            next += 1;
        }
        instance.tick();
    }

    let cluster = read_cluster_positions(&instance);
    let max_y = cluster
        .iter()
        .map(|(_, _, y, _)| *y)
        .fold(f64::NEG_INFINITY, f64::max);
    let max_distance_from_build = cluster
        .iter()
        .map(|(_, x, _, z)| ((x + 1.5).powi(2) + (z + 12.75).powi(2)).sqrt())
        .fold(0.0, f64::max);

    assert!(
        max_y < 8.0,
        "cluster should not launch upward after replay; cluster={cluster:?}"
    );
    assert!(
        max_distance_from_build < 4.0,
        "cluster should stay near the build site after replay; cluster={cluster:?}"
    );
}

fn read_inputs(recording: &Path) -> Vec<(usize, String, Value)> {
    let out = Command::new("sqlite3")
        .arg(recording)
        .arg("select tick || '|' || input_type || '|' || data from inputs where tick <= 14030 order by tick;")
        .output()
        .unwrap_or_else(|err| panic!("failed to query recording '{}': {err}", recording.display()));
    assert!(
        out.status.success(),
        "sqlite3 failed for '{}': {}",
        recording.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|line| {
            let mut parts = line.splitn(3, '|');
            let tick = parts.next().unwrap().parse::<usize>().unwrap();
            let kind = parts.next().unwrap().to_string();
            let data = serde_json::from_str(parts.next().unwrap()).unwrap();
            (tick, kind, data)
        })
        .collect()
}

fn read_cluster_positions(instance: &GameInstance) -> Vec<(String, f64, f64, f64)> {
    let runtime = instance.lua_runtime.as_ref().expect("missing runtime");
    let raw: String = runtime
        .lua()
        .load(
            r#"
            local names = {
                "ConstructionBlock_1",
                "ConstructionBlock_2",
                "ConstructionBlock_11",
                "ConstructionBlock_51",
                "ConstructionBlock_52",
                "ConstructionBlock_61",
            }
            local out = {}
            for _, name in ipairs(names) do
                local b = Workspace[name]
                table.insert(out, string.format("%s|%.6f|%.6f|%.6f", name, b.Position.X, b.Position.Y, b.Position.Z))
            end
            return table.concat(out, "\n")
        "#,
        )
        .eval()
        .expect("failed to read cluster");

    raw.lines()
        .map(|line| {
            let mut parts = line.split('|');
            let name = parts.next().unwrap().to_string();
            let x = parts.next().unwrap().parse().unwrap();
            let y = parts.next().unwrap().parse().unwrap();
            let z = parts.next().unwrap().parse().unwrap();
            (name, x, y, z)
        })
        .collect()
}
