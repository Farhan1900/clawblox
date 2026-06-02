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
