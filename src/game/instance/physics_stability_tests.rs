//! Physics stability regression tests.
//!
//! Each test asserts a physical invariant the engine must uphold:
//!
//! - Water drag is dissipative: it may slow relative motion but never amplify
//!   it. The explicit drag step diverges when
//!   (1/density) * (LINEAR_WATER_DRAG + QUADRATIC_WATER_DRAG * speed) * dt > 2
//!   (~20 studs/s for a density-0.22 part at 60 Hz), which turns into a
//!   tick-to-tick velocity sign-flipping limit cycle at the waterline.
//! - A welded assembly is one persistent rigid body: it keeps its physics
//!   body while moving, and a free tumbling assembly conserves linear
//!   momentum. Rebuilding the body from quantized, f32 round-tripped world
//!   poses re-seeds the COM velocity from the root part's point velocity
//!   (v_com + w x r_root), injecting momentum every rebuilt tick.
//! - AlignPosition/AlignOrientation holds settle. Each constraint applies a
//!   full-assembly-mass / full-assembly-inertia critically damped response,
//!   so N constraints on one body stack their velocity feedback to
//!   N * 2 * responsiveness * dt, approaching the explicit stability limit
//!   of 2, and the MaxVelocity/MaxForce/MaxTorque saturations can sustain a
//!   limit cycle instead of damping.
//! - Constraint energy stored against static geometry (a grip pressing a part
//!   into the floor) must not discharge as a launch when the constraint is
//!   removed.

use uuid::Uuid;

use super::GameInstance;

/// Typical manipulation-grip parameters (stiff hand-like hold).
const GRIP_RESPONSIVENESS: f32 = 25.0;
const GRIP_MAX_FORCE: f32 = 6000.0;
const GRIP_MAX_VELOCITY: f32 = 40.0;
const GRIP_MAX_TORQUE: f32 = 3000.0;

fn new_instance(script: &str) -> GameInstance {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);
    instance.load_script(script);
    instance
}

fn eval_f64(instance: &GameInstance, code: &str) -> f64 {
    instance
        .lua_runtime
        .as_ref()
        .unwrap()
        .lua()
        .load(code)
        .eval()
        .unwrap()
}

fn eval_vec3(instance: &GameInstance, expr: &str) -> [f64; 3] {
    let (x, y, z): (f64, f64, f64) = instance
        .lua_runtime
        .as_ref()
        .unwrap()
        .lua()
        .load(&format!("local v = {expr} return v.X, v.Y, v.Z"))
        .eval()
        .unwrap();
    [x, y, z]
}

fn find_part_id(instance: &GameInstance, name: &str) -> u64 {
    let runtime = instance.lua_runtime.as_ref().unwrap();
    for inst in runtime.workspace().get_descendants() {
        let data = inst.data.lock().unwrap();
        if data.name == name && data.part_data.is_some() {
            return data.id.0;
        }
    }
    panic!("part {name} not found in workspace");
}

// ---------------------------------------------------------------------------
// Water drag stability
// ---------------------------------------------------------------------------

/// A fully submerged part given a fast initial push must shed speed through
/// drag; drag may never amplify motion. With density 0.22 the explicit drag
/// step is divergent above ~20 studs/s, so a 30 studs/s push grows into a
/// high-speed oscillation instead of decaying.
#[test]
fn submerged_fast_part_speed_decays() {
    let mut instance = new_instance(
        r#"
        Workspace.Gravity = 10
        Workspace.Terrain:FillBlock(CFrame.new(0, -10, 0), Vector3.new(96, 20, 96), Enum.Material.Water)

        local p = Instance.new("Part")
        p.Name = "FastPart"
        p.Size = Vector3.new(12, 1, 1.5)
        p.CustomPhysicalProperties = PhysicalProperties.new(0.22, 0.75, 0.08, 1, 1)
        p.Position = Vector3.new(0, -8, 0)
        p.CanCollide = false
        p.AssemblyLinearVelocity = Vector3.new(30, 0, 0)
        p.Parent = Workspace
        _G.p = p
    "#,
    );

    let mut max_speed: f64 = 0.0;
    for _ in 0..240 {
        instance.tick();
        let speed = eval_f64(&instance, "return _G.p.AssemblyLinearVelocity.Magnitude");
        max_speed = max_speed.max(speed);
    }
    let final_speed = eval_f64(&instance, "return _G.p.AssemblyLinearVelocity.Magnitude");

    assert!(
        max_speed < 45.0,
        "water drag must never amplify motion: initial 30 u/s push peaked at {max_speed:.1} u/s"
    );
    assert!(
        final_speed < 10.0,
        "submerged part should have shed most speed after 4s, still moving at {final_speed:.1} u/s"
    );
}

/// A floating part disturbed past the drag stability threshold must return to
/// rest at the surface instead of oscillating indefinitely at the waterline.
#[test]
fn disturbed_floating_part_settles() {
    let mut instance = new_instance(
        r#"
        Workspace.Gravity = 10
        Workspace.Terrain:FillBlock(CFrame.new(0, -8, 0), Vector3.new(96, 16, 96), Enum.Material.Water)

        local p = Instance.new("Part")
        p.Name = "Floater"
        p.Size = Vector3.new(12, 1, 1.5)
        p.CustomPhysicalProperties = PhysicalProperties.new(0.22, 0.75, 0.08, 1, 1)
        p.Position = Vector3.new(0, -0.2, 0)
        p.CanCollide = false
        p.Parent = Workspace
        _G.p = p
    "#,
    );

    // Let it find float equilibrium first.
    for _ in 0..300 {
        instance.tick();
    }
    let rest_y = eval_f64(&instance, "return _G.p.Position.Y");

    // Disturb it past the explicit-drag stability threshold (~20 studs/s at
    // density 0.22), as a manipulation constraint routinely does.
    instance
        .lua_runtime
        .as_ref()
        .unwrap()
        .lua()
        .load("_G.p.AssemblyLinearVelocity = Vector3.new(0, -30, 0)")
        .exec()
        .unwrap();

    for _ in 0..600 {
        instance.tick();
    }

    let speed = eval_f64(&instance, "return _G.p.AssemblyLinearVelocity.Magnitude");
    let y = eval_f64(&instance, "return _G.p.Position.Y");
    assert!(
        speed < 2.0,
        "disturbed floater should settle within 10s, still moving at {speed:.1} u/s"
    );
    assert!(
        (y - rest_y).abs() < 1.0,
        "disturbed floater should return to float height {rest_y:.2}, ended at {y:.2}"
    );
}

// ---------------------------------------------------------------------------
// Welded assembly persistence
// ---------------------------------------------------------------------------

const TUMBLING_ASSEMBLY_SCRIPT: &str = r#"
    Workspace.Gravity = 0

    local function plank(name, z)
        local p = Instance.new("Part")
        p.Name = name
        p.Size = Vector3.new(5.25, 0.35, 0.9)
        p.CustomPhysicalProperties = PhysicalProperties.new(0.35, 0.75, 0.08, 1, 1)
        -- Far from the origin, where f32 round-trip noise on world poses is
        -- largest relative to the assembly-signature quantization.
        p.Position = Vector3.new(-70, 20, -3 + z)
        p.CanCollide = true
        p.Parent = Workspace
        return p
    end

    local a = plank("PlankA", 0)
    local b = plank("PlankB", 0.9)

    local weld = Instance.new("WeldConstraint")
    weld.Part0 = a
    weld.Part1 = b
    weld.Parent = a

    -- Multi-axis tumble, no linear motion.
    a.AssemblyAngularVelocity = Vector3.new(1.5, 2.0, 0.9)
    _G.a = a
    _G.b = b
"#;

/// A welded assembly must keep its physics body while it moves: rebuilding it
/// resets sleep state, contact warm-starts, and (see next test) its momentum.
/// The assembly signature derives weld-local frames from f32 round-tripped
/// world poses quantized at 1e-5, so a tumbling assembly's signature can flip
/// every tick, tearing down and recreating the body.
#[test]
fn welded_assembly_body_handle_stable_while_tumbling() {
    let mut instance = new_instance(TUMBLING_ASSEMBLY_SCRIPT);

    instance.tick();
    let part_id = find_part_id(&instance, "PlankA");
    let initial_handle = instance
        .physics
        .get_handle(part_id)
        .expect("welded plank should have a physics body");

    let mut rebuilds = 0;
    for _ in 0..120 {
        instance.tick();
        let handle = instance
            .physics
            .get_handle(part_id)
            .expect("welded plank should keep a physics body");
        if handle != initial_handle {
            rebuilds += 1;
        }
    }

    assert_eq!(
        rebuilds, 0,
        "tumbling welded assembly was rebuilt on {rebuilds}/120 ticks (sleep, contacts and momentum are reset on every rebuild)"
    );
}

/// A freely tumbling welded assembly in zero gravity must conserve linear
/// momentum. On every rebuild the new body's COM velocity is seeded from the
/// root part's point velocity (v_com + w x r_root), so a tumbling assembly
/// that rebuilds each tick gains ~|w x r_root| of COM speed per tick — a
/// steady phantom thrust on a free body.
#[test]
fn tumbling_welded_assembly_conserves_linear_momentum() {
    let mut instance = new_instance(TUMBLING_ASSEMBLY_SCRIPT);

    for _ in 0..300 {
        instance.tick();
    }

    // The two planks have equal mass, so their midpoint is the assembly COM:
    // individual part velocities carry an w x r tumble component, the
    // midpoint does not.
    let speed = eval_f64(
        &instance,
        "return ((_G.a.AssemblyLinearVelocity + _G.b.AssemblyLinearVelocity) / 2).Magnitude",
    );
    let com = eval_vec3(&instance, "(_G.a.Position + _G.b.Position) / 2");
    let drift =
        ((com[0] + 70.0).powi(2) + (com[1] - 20.0).powi(2) + (com[2] + 2.55).powi(2)).sqrt();

    assert!(
        speed < 0.5,
        "tumbling welded assembly gained {speed:.1} u/s of COM speed from nothing (momentum injected on assembly rebuilds)"
    );
    assert!(
        drift < 1.0,
        "tumbling welded assembly's COM drifted {drift:.2} studs from its spawn with no external force"
    );
}

// ---------------------------------------------------------------------------
// Grip (AlignPosition/AlignOrientation) hold stability
// ---------------------------------------------------------------------------

/// Builds a held-in-the-air grip scenario and returns the instance.
///
/// `parts`: how many 5.25 x 0.35 x 0.9 planks to weld into the held assembly
/// (1 = a lone part, no weld). `plank_step`: (dx, dz) offset between
/// consecutive planks — (0, 0.9) welds them side by side into a deck,
/// (5.3, 0) end to end into a long beam. `grip_offsets`: x offsets of the
/// hand-style grips (AlignPosition + AlignOrientation each) from the
/// assembly center. Targets sit 2 studs above the spawn pose so the hold has
/// to do real work against gravity.
fn grip_scenario(parts: usize, plank_step: (f32, f32), grip_offsets: &[f32]) -> GameInstance {
    let offsets = grip_offsets
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let script = format!(
        r#"
        Workspace.Gravity = 10

        local ground = Instance.new("Part")
        ground.Name = "Ground"
        ground.Size = Vector3.new(120, 1, 60)
        ground.Position = Vector3.new(0, -0.5, 0)
        ground.Anchored = true
        ground.Parent = Workspace

        local parts = {parts}
        local stepX, stepZ = {step_x}, {step_z}

        local function plank(name, x, z)
            local p = Instance.new("Part")
            p.Name = name
            p.Size = Vector3.new(5.25, 0.35, 0.9)
            p.CustomPhysicalProperties = PhysicalProperties.new(0.35, 0.75, 0.08, 1, 1)
            p.Position = Vector3.new(x, 0.175, z)
            p.CanCollide = true
            p.Parent = Workspace
            return p
        end

        local a = plank("Held1", 0, 0)
        for i = 2, parts do
            local b = plank("Held" .. i, (i - 1) * stepX, (i - 1) * stepZ)
            local weld = Instance.new("WeldConstraint")
            weld.Part0 = a
            weld.Part1 = b
            weld.Parent = a
        end
        _G.a = a

        -- Assembly center in world space (planks spawn axis-aligned at x=0).
        local centerX = (parts - 1) * stepX * 0.5
        local centerZ = (parts - 1) * stepZ * 0.5

        for _, dx in ipairs({{ {offsets} }}) do
            local x = centerX + dx
            -- Anchored stand-in for a kinematic hand marker part.
            local hand = Instance.new("Part")
            hand.Name = "Hand" .. tostring(dx)
            hand.Size = Vector3.new(0.7, 0.7, 0.7)
            hand.Anchored = true
            hand.CanCollide = false
            -- Hold target: 2 studs above the grab point.
            hand.Position = Vector3.new(x, 2.175, centerZ)
            hand.Parent = Workspace

            local handAtt = Instance.new("Attachment")
            handAtt.Parent = hand

            local gripAtt = Instance.new("Attachment")
            -- Local to the root plank, which spawns at the world origin.
            gripAtt.CFrame = CFrame.new(x, 0, centerZ)
            gripAtt.Parent = a

            local ap = Instance.new("AlignPosition")
            ap.Attachment0 = gripAtt
            ap.Attachment1 = handAtt
            ap.Responsiveness = {resp}
            ap.MaxForce = {max_force}
            ap.MaxVelocity = {max_velocity}
            ap.RigidityEnabled = false
            ap.Parent = Workspace

            local ao = Instance.new("AlignOrientation")
            ao.Attachment0 = gripAtt
            ao.Attachment1 = handAtt
            ao.Responsiveness = {resp}
            ao.MaxTorque = Vector3.new({max_torque}, {max_torque}, {max_torque})
            ao.RigidityEnabled = false
            ao.Parent = Workspace
        end
    "#,
        parts = parts,
        step_x = plank_step.0,
        step_z = plank_step.1,
        offsets = offsets,
        resp = GRIP_RESPONSIVENESS,
        max_force = GRIP_MAX_FORCE,
        max_velocity = GRIP_MAX_VELOCITY,
        max_torque = GRIP_MAX_TORQUE,
    );
    new_instance(&script)
}

/// Runs a grip scenario for 10 simulated seconds and returns
/// (worst speed over the last second, final distance from the hold height).
fn run_grip_scenario(parts: usize, plank_step: (f32, f32), grip_offsets: &[f32]) -> (f64, f64) {
    let mut instance = grip_scenario(parts, plank_step, grip_offsets);
    for _ in 0..540 {
        instance.tick();
    }
    let mut late_speed: f64 = 0.0;
    for _ in 0..60 {
        instance.tick();
        let speed = eval_f64(&instance, "return _G.a.AssemblyLinearVelocity.Magnitude");
        late_speed = late_speed.max(speed);
    }
    let y = eval_f64(&instance, "return _G.a.Position.Y");
    (late_speed, (y - 2.0).abs())
}

fn assert_hold_settles(label: &str, parts: usize, plank_step: (f32, f32), grip_offsets: &[f32]) {
    let (late_speed, height_error) = run_grip_scenario(parts, plank_step, grip_offsets);
    assert!(
        late_speed < 2.0,
        "{label}: hold should be calm after 9s, still moving at {late_speed:.1} u/s"
    );
    assert!(
        height_error < 1.0,
        "{label}: hold should track the target height, off by {height_error:.2} studs"
    );
}

/// Control: a single grip holding a single (un-welded) part settles. A lone
/// part is a one-part weld component whose assembly signature is trivially
/// stable, so its body is never rebuilt while held.
#[test]
fn single_grip_single_part_hold_settles() {
    assert_hold_settles("single part, centered grip", 1, (0.0, 0.9), &[0.0]);
}

/// A single grip holding a welded two-part assembly must settle exactly like
/// the single part does. The held assembly's pose changes every tick, which
/// flips the quantized assembly signature and rebuilds the body mid-hold;
/// each rebuild re-seeds the COM velocity from the root part's point
/// velocity, feeding the grip controller corrupted state every tick — the
/// held assembly visibly wobbles and never comes to rest.
#[test]
fn single_grip_welded_assembly_hold_settles() {
    assert_hold_settles("welded pair, centered grip", 2, (0.0, 0.9), &[0.0]);
}

/// Off-center single grip on a welded assembly: the lever arm couples the
/// position constraint into the rotation mode on top of the rebuild noise.
#[test]
fn single_offcenter_grip_welded_assembly_hold_settles() {
    assert_hold_settles("welded pair, off-center grip", 2, (0.0, 0.9), &[-1.5]);
}

/// Two symmetric grips on the same welded assembly must settle. Each grip
/// independently applies a full-assembly-mass / full-assembly-inertia
/// response, so grips stack their discrete velocity feedback toward the
/// explicit stability limit.
#[test]
fn dual_grip_welded_assembly_hold_settles() {
    assert_hold_settles(
        "welded pair, symmetric dual grip",
        2,
        (0.0, 0.9),
        &[-1.5, 1.5],
    );
}

/// A long welded beam held by two closely spaced central grips must settle.
/// The grips have a short baseline relative to the beam's inertia, so the
/// pitch mode is weakly controlled while the constraint gains stay at
/// full-assembly scale ("long beam carried by two hands seesaws and never
/// damps").
#[test]
fn dual_central_grip_long_beam_hold_settles() {
    // Four planks welded end to end into a ~21-stud beam; grips 2 apart at
    // the middle, so the pitch mode has a short control baseline relative to
    // the beam's inertia.
    assert_hold_settles("long beam, central dual grip", 4, (5.3, 0.0), &[-1.0, 1.0]);
}

/// Two grips commanding irreconcilable targets — a tug-of-war, the
/// multi-character carry conflict case — must reach a bounded compromise:
/// constraint disagreement may store bounded strain but never velocity
/// growth, oscillation, or a catapult.
#[test]
fn conflicting_dual_grip_tug_of_war_stays_bounded() {
    let mut instance = new_instance(&format!(
        r#"
        Workspace.Gravity = 10

        local ground = Instance.new("Part")
        ground.Name = "Ground"
        ground.Size = Vector3.new(60, 1, 60)
        ground.Position = Vector3.new(0, -0.5, 0)
        ground.Anchored = true
        ground.Parent = Workspace

        local a = Instance.new("Part")
        a.Name = "Contested"
        a.Size = Vector3.new(5.25, 0.35, 0.9)
        a.CustomPhysicalProperties = PhysicalProperties.new(0.35, 0.75, 0.08, 1, 1)
        a.Position = Vector3.new(0, 0.175, 0)
        a.CanCollide = true
        a.Parent = Workspace
        _G.a = a

        -- Grips 3 studs apart on the part; hand targets 8 studs apart, so
        -- the commanded stretch is impossible to satisfy.
        for _, grip in ipairs({{ {{ -1.5, -4.0 }}, {{ 1.5, 4.0 }} }}) do
            local gripX, handX = grip[1], grip[2]

            local hand = Instance.new("Part")
            hand.Name = "Hand" .. tostring(handX)
            hand.Size = Vector3.new(0.7, 0.7, 0.7)
            hand.Anchored = true
            hand.CanCollide = false
            hand.Position = Vector3.new(handX, 2.175, 0)
            hand.Parent = Workspace

            local handAtt = Instance.new("Attachment")
            handAtt.Parent = hand

            local gripAtt = Instance.new("Attachment")
            gripAtt.CFrame = CFrame.new(gripX, 0, 0)
            gripAtt.Parent = a

            local ap = Instance.new("AlignPosition")
            ap.Attachment0 = gripAtt
            ap.Attachment1 = handAtt
            ap.Responsiveness = {resp}
            ap.MaxForce = {max_force}
            ap.MaxVelocity = {max_velocity}
            ap.RigidityEnabled = false
            ap.Parent = Workspace
        end
    "#,
        resp = GRIP_RESPONSIVENESS,
        max_force = GRIP_MAX_FORCE,
        max_velocity = GRIP_MAX_VELOCITY,
    ));

    for _ in 0..540 {
        instance.tick();
    }
    let mut late_speed: f64 = 0.0;
    for _ in 0..60 {
        instance.tick();
        let speed = eval_f64(&instance, "return _G.a.AssemblyLinearVelocity.Magnitude");
        late_speed = late_speed.max(speed);
    }
    let pos = eval_vec3(&instance, "_G.a.Position");

    assert!(
        late_speed < 2.0,
        "tug-of-war should reach a static stalemate, still moving at {late_speed:.1} u/s"
    );
    // The opposing saturated motors balance anywhere along the contested
    // axis (neutral equilibrium), so only escape counts as failure.
    assert!(
        pos[0].abs() < 3.0 && pos[2].abs() < 2.0 && (0.0..6.0).contains(&pos[1]),
        "contested part should stay between the two grips, ended at ({:.1}, {:.1}, {:.1})",
        pos[0],
        pos[1],
        pos[2]
    );
}

// ---------------------------------------------------------------------------
// Character vs loose parts
// ---------------------------------------------------------------------------

const LOOSE_WALL_SCRIPT: &str = r#"
    local floor = Instance.new("Part")
    floor.Name = "Floor"
    floor.Size = Vector3.new(120, 1, 120)
    floor.Position = Vector3.new(0, 0, 0)
    floor.Anchored = true
    floor.Parent = Workspace

    -- A light wall lying across the walking path: too tall to autostep,
    -- too wide to slide around within the test arena.
    local wall = Instance.new("Part")
    wall.Name = "LooseWall"
    wall.Size = Vector3.new(1.4, 2.6, 12)
    wall.CustomPhysicalProperties = PhysicalProperties.new(0.15, 0.7, 0.05, 1, 1)
    wall.Position = Vector3.new(3, 1.9, 0)
    wall.Parent = Workspace
    _G.wall = wall

    local Players = game:GetService("Players")
    Players.PlayerAdded:Connect(function(player)
        _G.player = player
    end)
"#;

fn move_player_into_loose_wall(instance: &mut GameInstance) -> (f64, f64, f64) {
    assert!(instance.add_player(Uuid::new_v4(), "Walker"));
    // Land, then let the wall settle and fall asleep.
    for _ in 0..240 {
        instance.tick();
    }
    let wall_x0 = eval_f64(instance, "return _G.wall.Position.X");

    instance
        .lua_runtime
        .as_ref()
        .unwrap()
        .lua()
        .load(
            r#"
            local humanoid = _G.player.Character:FindFirstChild("Humanoid")
            humanoid:MoveTo(Vector3.new(14, 0, 0))
        "#,
        )
        .exec()
        .unwrap();

    for _ in 0..300 {
        instance.tick();
    }

    let wall_x = eval_f64(instance, "return _G.wall.Position.X");
    let player_x = eval_f64(
        instance,
        "return _G.player.Character.HumanoidRootPart.Position.X",
    );
    (wall_x0, wall_x, player_x)
}

/// Walking into a light, sleeping loose part must wake it and shove it
/// aside. The kinematic controller must not treat dynamic bodies as static
/// walls: a loose part at rest against the capsule sleeps and never moves on
/// its own, so without push impulses it blocks the character forever (the
/// "freeze" the manipulation agents kept hitting).
#[test]
fn character_pushes_loose_parts_instead_of_freezing() {
    let mut instance = new_instance(LOOSE_WALL_SCRIPT);
    let (wall_x0, wall_x, player_x) = move_player_into_loose_wall(&mut instance);
    assert!(
        wall_x > wall_x0 + 1.0,
        "walking into a light sleeping part should wake and push it; wall moved {:.2} studs",
        wall_x - wall_x0
    );
    assert!(
        player_x > wall_x0,
        "character should make progress through the pushed wall, reached x={player_x:.2}"
    );
}

/// Compatibility mode keeps the mesa-island-era behavior: loose dynamic parts
/// still block character movement, but characters do not explicitly push them.
#[test]
fn player_object_physics_disabled_keeps_loose_parts_blocking() {
    let mut instance = new_instance(LOOSE_WALL_SCRIPT);
    instance.set_player_object_physics(false);

    let (wall_x0, wall_x, player_x) = move_player_into_loose_wall(&mut instance);

    assert!(
        wall_x < wall_x0 + 0.35,
        "compatibility mode should not shove the loose wall; moved {:.2} studs",
        wall_x - wall_x0
    );
    assert!(
        player_x < wall_x0,
        "loose wall should still block character movement; reached x={player_x:.2}, wall started at x={wall_x0:.2}"
    );
}

// ---------------------------------------------------------------------------
// Release catapult
// ---------------------------------------------------------------------------

/// Removing a constraint that has pressed a part into static geometry must
/// not launch the part. The constraint is far stronger than gravity, so an
/// unreachable target below the floor stores contact penetration that can
/// discharge as a catapult the moment the constraint is destroyed.
#[test]
fn pressed_release_does_not_catapult() {
    let mut instance = new_instance(&format!(
        r#"
        Workspace.Gravity = 10

        local ground = Instance.new("Part")
        ground.Name = "Ground"
        ground.Size = Vector3.new(60, 1, 60)
        ground.Position = Vector3.new(0, -0.5, 0)
        ground.Anchored = true
        ground.Parent = Workspace

        local p = Instance.new("Part")
        p.Name = "Pressed"
        p.Size = Vector3.new(5.25, 0.35, 0.9)
        p.CustomPhysicalProperties = PhysicalProperties.new(0.35, 0.75, 0.08, 1, 1)
        p.Position = Vector3.new(0, 0.175, 0)
        p.CanCollide = true
        p.Parent = Workspace
        _G.p = p

        local hand = Instance.new("Part")
        hand.Name = "Hand"
        hand.Size = Vector3.new(0.7, 0.7, 0.7)
        hand.Anchored = true
        hand.CanCollide = false
        -- Commanded 0.5 studs below the part's rest pose: an unreachable
        -- target that presses the part into the floor.
        hand.Position = Vector3.new(0, -0.325, 0)
        hand.Parent = Workspace

        local handAtt = Instance.new("Attachment")
        handAtt.Parent = hand

        local gripAtt = Instance.new("Attachment")
        gripAtt.Parent = p

        local ap = Instance.new("AlignPosition")
        ap.Attachment0 = gripAtt
        ap.Attachment1 = handAtt
        ap.Responsiveness = {resp}
        ap.MaxForce = {max_force}
        ap.MaxVelocity = {max_velocity}
        ap.RigidityEnabled = false
        ap.Parent = Workspace
        _G.ap = ap
    "#,
        resp = GRIP_RESPONSIVENESS,
        max_force = GRIP_MAX_FORCE,
        max_velocity = GRIP_MAX_VELOCITY,
    ));

    // Press for 2 simulated seconds.
    for _ in 0..120 {
        instance.tick();
    }

    // Release.
    instance
        .lua_runtime
        .as_ref()
        .unwrap()
        .lua()
        .load("_G.ap:Destroy()")
        .exec()
        .unwrap();

    let mut max_speed: f64 = 0.0;
    for _ in 0..60 {
        instance.tick();
        let speed = eval_f64(&instance, "return _G.p.AssemblyLinearVelocity.Magnitude");
        max_speed = max_speed.max(speed);
    }
    let pos = eval_vec3(&instance, "_G.p.Position");
    let horizontal = (pos[0].powi(2) + pos[2].powi(2)).sqrt();

    assert!(
        max_speed < 8.0,
        "releasing a pressed part launched it at {max_speed:.1} u/s"
    );
    assert!(
        horizontal < 3.0,
        "released part flew {horizontal:.1} studs sideways"
    );
}

// ---------------------------------------------------------------------------
// Walking across multi-part ground
// ---------------------------------------------------------------------------
//
// Large floors are commonly assembled from several anchored boxes. The
// character sweep collides with each collider independently and has no
// internal-edge filtering, so at a box boundary the neighbor's full-height
// vertical face terminates exactly at the walking plane and a sweep can
// treat the flat floor as a wall. The character must be able to walk across
// such a boundary from any rest position, including standing directly on it.

/// Two abutting layouts share the main box (x in [-50, 46], top y = 0); the
/// western box is supplied by `lobe`. The character is parked with its
/// capsule straddling x = `straddle`, then must cross to x < -53 with a
/// single MoveTo.
fn ground_boundary_cross_from_rest(lobe: &str, straddles: &[f64]) -> Vec<(f64, f64)> {
    let mut instance = new_instance(&format!(
        r#"
        Workspace.Gravity = 10

        local terrain = PhysicalProperties.new(2.0, 1.4, 0.0, 1, 1)

        local center = Instance.new("Part")
        center.Name = "GroundCenter"
        center.Size = Vector3.new(96, 10, 100)
        center.Position = Vector3.new(-2, -5, 0)
        center.Anchored = true
        center.CustomPhysicalProperties = terrain
        center.Parent = Workspace

        {lobe}

        local Players = game:GetService("Players")
        Players.PlayerAdded:Connect(function(player)
            _G.player = player
        end)
    "#
    ));

    assert!(instance.add_player(Uuid::new_v4(), "BoundaryWalker"));
    for _ in 0..120 {
        instance.tick(); // land and settle
    }

    let mut wedged = Vec::new();
    for &straddle in straddles {
        // Park the capsule straddling the boundary, fully at rest.
        instance
            .lua_runtime
            .as_ref()
            .unwrap()
            .lua()
            .load(&format!(
                "_G.player.Character.HumanoidRootPart.Position = Vector3.new({straddle}, 3.2, -3)"
            ))
            .exec()
            .unwrap();
        for _ in 0..90 {
            instance.tick();
        }
        let p = eval_vec3(&instance, "_G.player.Character.HumanoidRootPart.Position");
        assert!(
            (p[0] - straddle).abs() < 0.4 && p[1] > 1.0,
            "staging teleport to x={straddle} did not hold (at {p:?})"
        );

        // One plain MoveTo across the boundary; 360 ticks = 6 s for ~4 studs.
        instance
            .lua_runtime
            .as_ref()
            .unwrap()
            .lua()
            .load(
                r#"
                local humanoid = _G.player.Character:FindFirstChild("Humanoid")
                humanoid:MoveTo(Vector3.new(-54, 0, -3))
            "#,
            )
            .exec()
            .unwrap();
        let mut crossed = false;
        for _ in 0..360 {
            instance.tick();
            let p = eval_vec3(&instance, "_G.player.Character.HumanoidRootPart.Position");
            if p[0] < -53.0 {
                crossed = true;
                break;
            }
        }
        if !crossed {
            let p = eval_vec3(&instance, "_G.player.Character.HumanoidRootPart.Position");
            wedged.push((straddle, p[0]));
        }
    }
    wedged
}

/// Boundary positions a resting capsule (radius 1.0) can straddle.
const BOUNDARY_STRADDLES: &[f64] = &[
    -50.8, -50.6, -50.4, -50.2, -50.1, -50.0, -49.9, -49.8, -49.6, -49.4, -49.2,
];

/// Exact tiling: the boxes share the face plane x = -50 with coplanar tops.
#[test]
fn character_resting_on_exact_tiled_ground_boundary_crosses_it() {
    let tiled = r#"
        local lobe = Instance.new("Part")
        lobe.Name = "GroundW"
        lobe.Size = Vector3.new(13, 10, 40)
        lobe.Position = Vector3.new(-56.5, -5, -3)
        lobe.Anchored = true
        lobe.CustomPhysicalProperties = terrain
        lobe.Parent = Workspace
    "#;
    let wedged = ground_boundary_cross_from_rest(tiled, BOUNDARY_STRADDLES);
    assert!(
        wedged.is_empty(),
        "walker wedged on the exact-tiled boundary from rest at: {wedged:?}"
    );
}

/// Overlap + offset: the boxes overlap by 2 studs and the western top sits
/// 0.02 below the main top, so no box edge coincides with the walking plane.
/// The 0.02 riser is far below the controller's contact offset (0.05) and
/// autostep limit. This is the recommended layout for multi-part floors.
#[test]
fn character_resting_on_overlapped_offset_ground_boundary_crosses_it() {
    let overlapped = r#"
        local lobe = Instance.new("Part")
        lobe.Name = "GroundW"
        lobe.Size = Vector3.new(14, 10, 40)
        lobe.Position = Vector3.new(-56, -5.02, -3)
        lobe.Anchored = true
        lobe.CustomPhysicalProperties = terrain
        lobe.Parent = Workspace
    "#;
    let wedged = ground_boundary_cross_from_rest(overlapped, BOUNDARY_STRADDLES);
    assert!(
        wedged.is_empty(),
        "walker wedged on the overlapped/offset boundary from rest at: {wedged:?}"
    );
}

// ---------------------------------------------------------------------------
// Character push impulses
// ---------------------------------------------------------------------------
//
// The kinematic character shoves dynamic bodies out of its walking path with
// per-tick impulses. The impulse repeats every tick the capsule still
// overlaps the obstacle, with no cumulative cap, so an obstacle that cannot
// clear the path (heavy, wedged against static geometry, or held by
// friction) can be pumped far beyond the character's own speed. A walking
// character must never accelerate anything past ~1.5x its walk speed.

/// Walking into a welded assembly trapped against an anchored wall: the
/// character may be blocked or may climb, but the assembly must not be
/// launched.
#[test]
fn character_pushing_welded_assembly_into_wall_does_not_launch_it() {
    let mut instance = new_instance(
        r#"
        Workspace.Gravity = 10

        local floor = Instance.new("Part")
        floor.Name = "Floor"
        floor.Size = Vector3.new(80, 1, 80)
        floor.Position = Vector3.new(0, -0.5, 0)
        floor.Anchored = true
        floor.CustomPhysicalProperties = PhysicalProperties.new(2.0, 1.4, 0.0, 1, 1)
        floor.Parent = Workspace

        local wall = Instance.new("Part")
        wall.Name = "Wall"
        wall.Size = Vector3.new(1, 4, 12)
        wall.Position = Vector3.new(6.75, 2, 0)
        wall.Anchored = true
        wall.Parent = Workspace

        local prev = nil
        for i = 0, 5 do
            local b = Instance.new("Part")
            b.Name = "Slab" .. i
            b.Size = Vector3.new(0.75, 0.75, 0.75)
            b.CustomPhysicalProperties = PhysicalProperties.new(0.7, 0.7, 0.05, 1, 1)
            b.Position = Vector3.new(2.125 + i * 0.75, 0.375, 0)
            b.Parent = Workspace
            if prev then
                local w = Instance.new("WeldConstraint")
                w.Part0 = prev
                w.Part1 = b
                w.Parent = prev
            end
            if i == 0 then _G.slab = b end
            prev = b
        end

        local Players = game:GetService("Players")
        Players.PlayerAdded:Connect(function(player)
            _G.player = player
        end)
    "#,
    );

    assert!(instance.add_player(Uuid::new_v4(), "Pusher"));
    for _ in 0..120 {
        instance.tick();
    }

    instance
        .lua_runtime
        .as_ref()
        .unwrap()
        .lua()
        .load(
            r#"
            local humanoid = _G.player.Character:FindFirstChild("Humanoid")
            humanoid:MoveTo(Vector3.new(10, 0, 0))
        "#,
        )
        .exec()
        .unwrap();

    let start = eval_vec3(&instance, "_G.slab.Position");
    let mut max_speed: f64 = 0.0;
    let mut max_disp: f64 = 0.0;
    for _ in 0..600 {
        instance.tick();
        let speed = eval_f64(&instance, "return _G.slab.AssemblyLinearVelocity.Magnitude");
        max_speed = max_speed.max(speed);
        let p = eval_vec3(&instance, "_G.slab.Position");
        let disp =
            ((p[0] - start[0]).powi(2) + (p[1] - start[1]).powi(2) + (p[2] - start[2]).powi(2))
                .sqrt();
        max_disp = max_disp.max(disp);
    }
    // The capped shove may still pop the assembly over the wall with a
    // ballistic hop (part-level readings add spin and post-escape gravity to
    // the governed body speed), but launch-grade energy must be gone: the
    // ungoverned pathology reached 65-130 u/s and carried assemblies tens of
    // studs.
    assert!(
        max_speed < 45.0,
        "walking into a wall-trapped welded assembly pumped it to {max_speed:.1} u/s"
    );
    assert!(
        max_disp < 30.0,
        "wall-trapped assembly was carried {max_disp:.1} studs by character pushes"
    );
}

/// Walking over a loose plank lying in the path: it may be kicked aside, but
/// never launched.
#[test]
fn character_walking_over_loose_plank_does_not_launch_it() {
    let mut instance = new_instance(
        r#"
        Workspace.Gravity = 10

        local floor = Instance.new("Part")
        floor.Name = "Floor"
        floor.Size = Vector3.new(80, 1, 80)
        floor.Position = Vector3.new(0, -0.5, 0)
        floor.Anchored = true
        floor.CustomPhysicalProperties = PhysicalProperties.new(2.0, 1.4, 0.0, 1, 1)
        floor.Parent = Workspace

        local plank = Instance.new("Part")
        plank.Name = "Plank"
        plank.Size = Vector3.new(0.9, 0.35, 5.25)
        plank.CustomPhysicalProperties = PhysicalProperties.new(0.35, 0.75, 0.08, 1, 1)
        plank.Position = Vector3.new(2, 0.175, 0)
        plank.Parent = Workspace
        _G.plank = plank

        local Players = game:GetService("Players")
        Players.PlayerAdded:Connect(function(player)
            _G.player = player
        end)
    "#,
    );

    assert!(instance.add_player(Uuid::new_v4(), "Walker"));
    for _ in 0..120 {
        instance.tick();
    }

    instance
        .lua_runtime
        .as_ref()
        .unwrap()
        .lua()
        .load(
            r#"
            local humanoid = _G.player.Character:FindFirstChild("Humanoid")
            humanoid:MoveTo(Vector3.new(8, 0, 0))
        "#,
        )
        .exec()
        .unwrap();

    let mut max_speed: f64 = 0.0;
    for _ in 0..360 {
        instance.tick();
        let speed = eval_f64(
            &instance,
            "return _G.plank.AssemblyLinearVelocity.Magnitude",
        );
        max_speed = max_speed.max(speed);
    }
    assert!(
        max_speed < 24.0,
        "walking over a loose plank launched it at {max_speed:.1} u/s"
    );
}

// ---------------------------------------------------------------------------
// Character standing on a floating assembly
// ---------------------------------------------------------------------------
//
// A character standing on a buoyant assembly couples three mechanisms: the
// support load pressing the assembly down, platform carry feeding the
// assembly's velocity back into the capsule, and one-way contacts (forces
// can never push a dynamic body through the kinematic capsule). If the pair
// starts descending, buoyancy cannot arrest it through the capsule and the
// stack can ratchet downward indefinitely. A raft with ample reserve
// buoyancy must keep both itself and its rider at the surface.

#[test]
fn character_standing_on_floating_raft_stays_at_surface() {
    let mut instance = new_instance(
        r#"
        Workspace.Gravity = 10
        Workspace.Terrain:FillBlock(CFrame.new(0, -8, 0), Vector3.new(96, 16, 96), Enum.Material.Water)

        -- 54 volume at density 0.25: reserve buoyancy ~40 mass units.
        local prev = nil
        for i = -1, 1 do
            local p = Instance.new("Part")
            p.Name = "RaftPlank" .. i
            p.Size = Vector3.new(12, 1, 1.5)
            p.CustomPhysicalProperties = PhysicalProperties.new(0.25, 0.75, 0.08, 1, 1)
            p.Position = Vector3.new(-4.5, -0.25, i * 1.5)
            p.CanCollide = true
            p.Parent = Workspace
            if prev then
                local w = Instance.new("WeldConstraint")
                w.Part0 = prev
                w.Part1 = p
                w.Parent = prev
            end
            if i == 0 then _G.raft = p end
            prev = p
        end

        local Players = game:GetService("Players")
        Players.PlayerAdded:Connect(function(player)
            _G.player = player
        end)
    "#,
    );

    // Let the raft find its float height before the rider arrives.
    for _ in 0..240 {
        instance.tick();
    }
    // First player spawns at (-4.5, 6, 0): directly above the raft center.
    assert!(instance.add_player(Uuid::new_v4(), "Rider"));
    for _ in 0..240 {
        instance.tick();
    }

    // Walk to one end of the deck and back, then ride for 30 s. Deck motion
    // (reaction drift, heel) is expected; sinking is not.
    let mut min_raft_y = f64::INFINITY;
    let mut min_player_y = f64::INFINITY;
    let track = |instance: &GameInstance, min_raft_y: &mut f64, min_player_y: &mut f64| {
        let raft_y = eval_f64(instance, "return _G.raft.Position.Y");
        let player_y = eval_f64(
            instance,
            "return _G.player.Character.HumanoidRootPart.Position.Y",
        );
        *min_raft_y = min_raft_y.min(raft_y);
        *min_player_y = min_player_y.min(player_y);
    };
    for target in [
        "Vector3.new(0, 0, 0)",
        "Vector3.new(-9, 0, 0)",
        "Vector3.new(-4.5, 0, 0)",
    ] {
        instance
            .lua_runtime
            .as_ref()
            .unwrap()
            .lua()
            .load(&format!(
                r#"
                local humanoid = _G.player.Character:FindFirstChild("Humanoid")
                humanoid:MoveTo({target})
            "#
            ))
            .exec()
            .unwrap();
        for _ in 0..240 {
            instance.tick();
            track(&instance, &mut min_raft_y, &mut min_player_y);
        }
    }
    for _ in 0..1800 {
        instance.tick();
        track(&instance, &mut min_raft_y, &mut min_player_y);
    }

    assert!(
        min_raft_y > -2.0,
        "raft was driven under by its rider (min raft y = {min_raft_y:.2})"
    );
    assert!(
        min_player_y > 0.0,
        "rider sank with the raft (min root y = {min_player_y:.2})"
    );
}

// ---------------------------------------------------------------------------
// Floor-wedged character recovery
// ---------------------------------------------------------------------------

/// A capsule can come to rest with its bottom marginally below the
/// controller's contact offset (landing and ground-snap both target the
/// offset exactly; integration rounding can finish a hair low). From that
/// pose every horizontal sweep returns an immediate floor hit whose
/// up-facing normal defeats the slide projection, and the controller burns
/// its iteration budget making no progress: the character wedges permanently
/// on open flat ground. The controller must recover and keep walking.
#[test]
fn character_resting_below_contact_offset_still_walks() {
    let mut instance = new_instance(
        r#"
        Workspace.Gravity = 10

        local floor = Instance.new("Part")
        floor.Name = "Floor"
        floor.Size = Vector3.new(80, 1, 80)
        floor.Position = Vector3.new(0, -0.5, 0)
        floor.Anchored = true
        floor.CustomPhysicalProperties = PhysicalProperties.new(2.0, 1.4, 0.0, 1, 1)
        floor.Parent = Workspace

        local Players = game:GetService("Players")
        Players.PlayerAdded:Connect(function(player)
            _G.player = player
        end)
    "#,
    );

    let agent = Uuid::new_v4();
    assert!(instance.add_player(agent, "LowWalker"));
    for _ in 0..120 {
        instance.tick();
    }
    let hrp = *instance.player_hrp_ids.get(&agent).unwrap();

    // Park the capsule bottom 0.0014 below the 0.05 contact offset — a pose
    // observed after ordinary walking (floor top y=0, capsule half extent 2.5).
    instance
        .physics
        .set_character_position(hrp, [0.0, 2.5486, 0.0]);
    for _ in 0..30 {
        instance.tick();
    }

    instance
        .lua_runtime
        .as_ref()
        .unwrap()
        .lua()
        .load(
            r#"
            local humanoid = _G.player.Character:FindFirstChild("Humanoid")
            humanoid:MoveTo(Vector3.new(12, 0, 0))
        "#,
        )
        .exec()
        .unwrap();
    for _ in 0..240 {
        instance.tick();
    }

    let p = instance.physics.get_character_position(hrp).unwrap();
    assert!(
        p[0] > 6.0,
        "capsule wedged on flat ground from clearance below offset (x={:.3}, y={:.4})",
        p[0],
        p[1]
    );
}

/// A character capsule overlapping a floating welded assembly (stepped off
/// the deck into the water; the hull now sits against the capsule) must not
/// drag it under. Capsule contacts are one-way — the assembly can never push
/// the kinematic capsule up — so any controller descent grinds the assembly
/// down with infinite authority and buoyancy never wins; conversely,
/// platform-carry from a body the capsule itself displaces pumps the pair
/// upward without bound. The capsule must hold height and the assembly must
/// keep floating.
#[test]
fn character_overlapping_floating_assembly_does_not_drag_it_under() {
    let mut instance = new_instance(
        r#"
        Workspace.Gravity = 10
        Workspace.Terrain:FillBlock(CFrame.new(0, -8.25, 0), Vector3.new(96, 15.5, 96), Enum.Material.Water)

        local prev = nil
        for i = -1, 1 do
            local p = Instance.new("Part")
            p.Name = "Hull" .. i
            p.Size = Vector3.new(12, 1, 1.5)
            p.CustomPhysicalProperties = PhysicalProperties.new(0.25, 0.75, 0.08, 1, 1)
            p.Position = Vector3.new(0, -0.3, i * 1.5)
            p.CanCollide = true
            p.Parent = Workspace
            if prev then
                local w = Instance.new("WeldConstraint")
                w.Part0 = prev; w.Part1 = p; w.Parent = prev
            end
            if i == 0 then _G.hull = p end
            prev = p
        end

        local Players = game:GetService("Players")
        Players.PlayerAdded:Connect(function(player) _G.player = player end)
    "#,
    );

    let agent = Uuid::new_v4();
    assert!(instance.add_player(agent, "Overboard"));
    let hrp = *instance.player_hrp_ids.get(&agent).unwrap();
    for _ in 0..240 {
        instance.tick(); // land on the deck, raft settles
    }

    // Capsule shoved down into the water so the hull sits against it.
    instance
        .physics
        .set_character_position(hrp, [0.0, -0.30, 0.0]);

    // The hull may transiently squeeze below the capsule tip (~-3.4) while
    // working itself free; what must never happen is a runaway in either
    // direction or a failure to return to the surface. The broken behaviors
    // this guards against ratcheted the hull below -10 (and to the void in
    // longer runs) or pumped the pair above +8 within this window.
    // Only the assembly's fate is asserted: the capsule may legitimately
    // work itself free of the hull and sink in open water (characters have
    // no water interaction; resets are the world layer's responsibility).
    let mut min_hull_y = f64::INFINITY;
    let mut max_hull_y = f64::NEG_INFINITY;
    for _ in 0..1800 {
        instance.tick();
        let hull_y = eval_f64(&instance, "return _G.hull.Position.Y");
        min_hull_y = min_hull_y.min(hull_y);
        max_hull_y = max_hull_y.max(hull_y);
    }
    let final_hull_y = eval_f64(&instance, "return _G.hull.Position.Y");

    assert!(
        min_hull_y > -6.0,
        "floating assembly was ratcheted under by the overlapping capsule (min hull y = {min_hull_y:.2})"
    );
    assert!(
        max_hull_y < 5.0,
        "capsule/assembly pair pumped itself upward (max hull y = {max_hull_y:.2})"
    );
    // The hull either floats free near the surface or rests pinned against
    // the capsule's underside (capsule bottom is at -2.8 in this setup) —
    // both are stable held states. Anything deeper means it kept sinking.
    assert!(
        final_hull_y > -3.5,
        "assembly did not stay at the surface or against the capsule (final hull y = {final_hull_y:.2})"
    );
}
