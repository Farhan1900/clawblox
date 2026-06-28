use clawblox::game::instance::GameInstance;
use uuid::Uuid;

fn read_f64(instance: &GameInstance, expr: &str) -> f64 {
    instance
        .lua_runtime
        .as_ref()
        .unwrap()
        .lua()
        .load(expr)
        .eval()
        .unwrap()
}

fn read_u64(instance: &GameInstance, expr: &str) -> u64 {
    instance
        .lua_runtime
        .as_ref()
        .unwrap()
        .lua()
        .load(expr)
        .eval()
        .unwrap()
}

fn physics_body_is_sleeping(instance: &GameInstance, lua_id: u64) -> bool {
    let handle = instance.physics.get_handle(lua_id).unwrap();
    instance
        .physics
        .rigid_body_set
        .get(handle)
        .unwrap()
        .is_sleeping()
}

fn lua_debug_id_to_u64_script(expr: &str) -> String {
    format!(
        r#"
        local debugId = {expr}:GetDebugId()
        return tonumber(string.sub(debugId, 4), 16)
    "#
    )
}

/// Tests whether a WeldConstraint between an anchored and a dynamic block
/// is stable over many ticks. With the current clear-and-rebuild-every-tick
/// approach, warmstarting is lost each frame — this test measures how much
/// the dynamic block drifts from its expected position over 10 seconds.
#[test]
fn weld_joint_drift_over_600_ticks() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    instance.load_script(
        r#"
        Workspace.Gravity = 30

        local anchored = Instance.new("Part")
        anchored.Name = "Anchor"
        anchored.Size = Vector3.new(0.75, 0.75, 0.75)
        anchored.Position = Vector3.new(0, 5, 0)
        anchored.Anchored = true
        anchored.CanCollide = true
        anchored.Parent = Workspace

        local dynamic = Instance.new("Part")
        dynamic.Name = "Dynamic"
        dynamic.Size = Vector3.new(0.75, 0.75, 0.75)
        dynamic.Position = Vector3.new(0.75, 5, 0)
        dynamic.Anchored = false
        dynamic.CanCollide = false
        dynamic.Parent = Workspace

        local wc = Instance.new("WeldConstraint")
        wc.Part0 = anchored
        wc.Part1 = dynamic
        wc.Parent = anchored

        _G.dynamic = dynamic
    "#,
    );

    for _ in 0..10 {
        instance.tick();
    }

    let initial_x = read_f64(&instance, r#"return _G.dynamic.Position.X"#);
    let initial_y = read_f64(&instance, r#"return _G.dynamic.Position.Y"#);
    println!("Initial: x={:.6}, y={:.6}", initial_x, initial_y);

    for _ in 0..600 {
        instance.tick();
    }

    let final_x = read_f64(&instance, r#"return _G.dynamic.Position.X"#);
    let final_y = read_f64(&instance, r#"return _G.dynamic.Position.Y"#);
    let drift_x = (final_x - initial_x).abs();
    let drift_y = (final_y - initial_y).abs();
    println!("After 600 ticks: x={:.6}, y={:.6}", final_x, final_y);
    println!("Drift: dx={:.6}, dy={:.6}", drift_x, drift_y);

    assert!(
        drift_x < 0.001,
        "X drift too large ({:.4}): joint is not stable",
        drift_x
    );
    assert!(
        drift_y < 0.001,
        "Y drift too large ({:.4}): joint is not stable",
        drift_y
    );
}

/// Cantilevered chain: 4 blocks welded in a horizontal row, anchored only at
/// one end. Gravity pulls the free end down. A stable system holds shape; an
/// unstable one droops.
#[test]
fn weld_cantilever_holds_shape_over_600_ticks() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    instance.load_script(
        r#"
        Workspace.Gravity = 30

        local prev = nil
        for i = 0, 3 do
            local p = Instance.new("Part")
            p.Name = "Block_" .. i
            p.Size = Vector3.new(0.75, 0.75, 0.75)
            p.Position = Vector3.new(i * 0.75, 5, 0)
            p.Anchored = (i == 0)
            p.CanCollide = false
            p.Parent = Workspace

            if prev then
                local wc = Instance.new("WeldConstraint")
                wc.Part0 = prev
                wc.Part1 = p
                wc.Parent = prev
            end
            prev = p
        end

        _G.tip = Workspace.Block_3
    "#,
    );

    for _ in 0..10 {
        instance.tick();
    }

    let initial_y = read_f64(&instance, r#"return _G.tip.Position.Y"#);
    println!("Cantilever tip initial Y: {:.6}", initial_y);

    for _ in 0..600 {
        instance.tick();
    }

    let final_y = read_f64(&instance, r#"return _G.tip.Position.Y"#);
    let droop = initial_y - final_y;
    println!(
        "Cantilever tip after 600 ticks Y: {:.6}, droop: {:.6}",
        final_y, droop
    );

    assert!(
        droop < 0.01,
        "Cantilever tip drooped {:.4} units — joints are not stable under gravity",
        droop
    );
}

#[test]
fn single_dynamic_block_on_floor_sleeps_after_residual_motion() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    instance.load_script(
        r#"
        Workspace.Gravity = 10

        local floor = Instance.new("Part")
        floor.Name = "Floor"
        floor.Size = Vector3.new(30, 1, 30)
        floor.Position = Vector3.new(0, -0.5, 0)
        floor.Anchored = true
        floor.CanCollide = true
        floor.Parent = Workspace

        local block = Instance.new("Part")
        block.Name = "Block"
        block.Size = Vector3.new(0.75, 0.75, 0.75)
        block.Position = Vector3.new(0, 0.375, 0)
        block.Anchored = false
        block.CanCollide = true
        block.Parent = Workspace

        _G.block = block
    "#,
    );

    for _ in 0..60 {
        instance.tick();
    }

    // Control case for the construction drift bug: an ordinary dynamic,
    // collidable block with a small residual spin should sleep once its motion
    // falls below Rapier's activation thresholds. If this fails, the problem is
    // broader than welds.
    instance.load_script("_G.block.AssemblyAngularVelocity = Vector3.new(0, 0.02, 0)");
    for _ in 0..300 {
        instance.tick();
    }

    let block_id = read_u64(&instance, &lua_debug_id_to_u64_script("_G.block"));
    let initial_x = read_f64(&instance, r#"return _G.block.Position.X"#);
    let initial_z = read_f64(&instance, r#"return _G.block.Position.Z"#);
    let initial_sleeping = physics_body_is_sleeping(&instance, block_id);

    for _ in 0..1200 {
        instance.tick();
    }

    let final_x = read_f64(&instance, r#"return _G.block.Position.X"#);
    let final_z = read_f64(&instance, r#"return _G.block.Position.Z"#);
    let final_sleeping = physics_body_is_sleeping(&instance, block_id);
    let drift = ((final_x - initial_x).powi(2) + (final_z - initial_z).powi(2)).sqrt();
    println!(
        "Single block sleeping: initial={}, final={}, drift={:.6}",
        initial_sleeping, final_sleeping, drift
    );

    assert!(
        initial_sleeping,
        "single block did not sleep after settling"
    );
    assert!(
        final_sleeping,
        "single block woke up during idle simulation"
    );
    assert!(
        drift < 0.001,
        "single dynamic block drifted {:.4} studs while idle",
        drift
    );
}

/// Construction-world regression: a fully dynamic, welded assembly resting on
/// an anchored floor should settle after tiny residual motion instead of
/// passively walking/rotating over time. This matches placed construction
/// blocks: each block remains dynamic and collidable so agents can later push
/// or carry the assembly.
#[test]
fn dynamic_welded_assembly_on_floor_does_not_drift() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    instance.load_script(
        r#"
        Workspace.Gravity = 10

        local floor = Instance.new("Part")
        floor.Name = "Floor"
        floor.Size = Vector3.new(30, 1, 30)
        floor.Position = Vector3.new(0, -0.5, 0)
        floor.Anchored = true
        floor.CanCollide = true
        floor.Parent = Workspace

        local blocks = {}
        _G.blocks = blocks

        function _G.addBlock(x, z)
            local p = Instance.new("Part")
            p.Name = "AssemblyBlock_" .. (#blocks + 1)
            p.Size = Vector3.new(0.75, 0.75, 0.75)
            p.CFrame = CFrame.new(x, 0.375, z)
            p.Anchored = true
            p.CanCollide = false
            p.Parent = Workspace

            p.Anchored = false
            p.CanCollide = true
            p.AssemblyLinearVelocity = Vector3.new(0, 0, 0)
            p.AssemblyAngularVelocity = Vector3.new(0, 0, 0)

            for _, other in ipairs(blocks) do
                local delta = p.Position - other.Position
                if delta.Magnitude <= 0.75 * 1.2 then
                    local wc = Instance.new("WeldConstraint")
                    wc.Part0 = p
                    wc.Part1 = other
                    wc.Parent = p
                end
            end
            table.insert(blocks, p)
        end

        function _G.centroid()
            local c = Vector3.new(0, 0, 0)
            for _, block in ipairs(_G.blocks) do
                c = c + block.Position
            end
            return c / #_G.blocks
        end

        function _G.yaw()
            local a = _G.blocks[1].Position
            local b = _G.blocks[4].Position
            local d = b - a
            return math.atan2(d.Z, d.X)
        end

        function _G.nudge()
            for _, block in ipairs(_G.blocks) do
                block.AssemblyAngularVelocity = Vector3.new(0, 0.02, 0)
            end
        end
    "#,
    );

    // Skewed, already-settled positions from the g008 construction-world heart
    // recording, centered near the origin. Blocks are added one at a time to
    // mimic construction placement: a newly placed block is made dynamic and
    // collidable, then welds to any adjacent construction blocks.
    let positions = [
        (-1.449_f32, -0.485_f32),
        (-0.709, -0.375),
        (0.031, -0.275),
        (0.781, -0.175),
        (1.531, -0.085),
        (-0.809, 0.365),
        (-0.059, 0.465),
        (0.681, 0.565),
    ];

    for (x, z) in positions {
        instance.load_script(&format!("_G.addBlock({x:.6}, {z:.6})"));
        for _ in 0..30 {
            instance.tick();
        }
    }

    for _ in 0..60 {
        instance.tick();
    }

    // The real recording shows passive drift after placement. A tiny residual
    // angular velocity models the kind of leftover motion/contact correction a
    // dynamic assembly can have after a placement animation or physical bump.
    // After 300 ticks, the single-block control has already gone to sleep; this
    // welded assembly currently remains awake because constraints are rebuilt
    // and wake the connected bodies every frame.
    instance.load_script("_G.nudge()");
    for _ in 0..300 {
        instance.tick();
    }

    let initial_x = read_f64(&instance, r#"return _G.centroid().X"#);
    let initial_z = read_f64(&instance, r#"return _G.centroid().Z"#);
    let initial_yaw = read_f64(&instance, r#"return _G.yaw()"#);
    let first_block_id = read_u64(&instance, &lua_debug_id_to_u64_script("_G.blocks[1]"));
    let initial_sleeping = physics_body_is_sleeping(&instance, first_block_id);
    println!(
        "Dynamic assembly initial centroid=({:.6}, {:.6}), yaw={:.6}, sleeping={}",
        initial_x, initial_z, initial_yaw, initial_sleeping
    );

    for _ in 0..1200 {
        instance.tick();
    }

    let final_x = read_f64(&instance, r#"return _G.centroid().X"#);
    let final_z = read_f64(&instance, r#"return _G.centroid().Z"#);
    let final_yaw = read_f64(&instance, r#"return _G.yaw()"#);
    let final_sleeping = physics_body_is_sleeping(&instance, first_block_id);
    let drift = ((final_x - initial_x).powi(2) + (final_z - initial_z).powi(2)).sqrt();
    let yaw_drift = (final_yaw - initial_yaw).abs();
    println!(
        "Dynamic assembly final centroid=({:.6}, {:.6}), yaw={:.6}, sleeping={}",
        final_x, final_z, final_yaw, final_sleeping
    );
    println!(
        "Dynamic assembly drift: centroid={:.6}, yaw={:.6}",
        drift, yaw_drift
    );

    assert!(
        drift < 0.001,
        "dynamic welded assembly drifted {:.4} studs while idle",
        drift
    );
    assert!(
        yaw_drift < 0.001,
        "dynamic welded assembly yaw drifted {:.4} rad while idle",
        yaw_drift
    );
}
