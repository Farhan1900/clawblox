use super::*;
use crate::game::lua::instance::AttributeValue;

#[test]
fn test_player_goto_action() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    // Load a simple script that creates a floor
    instance.load_script(
        r#"
            local floor = Instance.new("Part")
            floor.Name = "Floor"
            floor.Size = Vector3.new(100, 1, 100)
            floor.Position = Vector3.new(0, 0, 0)
            floor.Anchored = true
            floor.Parent = Workspace
        "#,
    );

    // Add a player
    let agent_id = Uuid::new_v4();
    assert!(instance.add_player(agent_id, "TestPlayer"));

    // Verify player has HRP registered
    assert!(instance.player_hrp_ids.contains_key(&agent_id));
    let hrp_id = *instance.player_hrp_ids.get(&agent_id).unwrap();

    // Get initial position
    let initial_pos = instance.physics.get_character_position(hrp_id).unwrap();
    println!("Initial player position: {:?}", initial_pos);

    // Set movement target directly via physics
    instance
        .physics
        .set_character_target(hrp_id, Some([10.0, 5.0, 10.0]));

    // Run 120 ticks (2 seconds at 60 Hz)
    for _ in 0..120 {
        instance.tick();
    }

    // Get final position
    let final_pos = instance.physics.get_character_position(hrp_id).unwrap();
    println!("Final player position: {:?}", final_pos);

    // Player should have moved towards target (10, 5, 10)
    let moved_x = final_pos[0] - initial_pos[0];
    let moved_z = final_pos[2] - initial_pos[2];
    let horizontal_distance = (moved_x * moved_x + moved_z * moved_z).sqrt();

    println!("Horizontal distance moved: {}", horizontal_distance);

    // At 16 studs/sec for 2 seconds, should move up to 32 studs
    // Target is ~14.14 studs away horizontally, should reach it
    assert!(
        horizontal_distance > 5.0,
        "Player should have moved significantly towards target"
    );
}

#[test]
fn test_player_moveto_via_lua_input() {
    // This test mimics what the Python test does: send MoveTo via agent input
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    // Load a simple script that creates a floor AND handles MoveTo input
    instance.load_script(r#"
            local AgentInputService = game:GetService("AgentInputService")
            local Players = game:GetService("Players")

            local floor = Instance.new("Part")
            floor.Name = "Floor"
            floor.Size = Vector3.new(100, 1, 100)
            floor.Position = Vector3.new(0, 0, 0)
            floor.Anchored = true
            floor.Parent = Workspace

            if AgentInputService then
                AgentInputService.InputReceived:Connect(function(player, inputType, data)
                    if inputType == "MoveTo" then
                        local humanoid = player.Character and player.Character:FindFirstChild("Humanoid")
                        if humanoid and data and data.position then
                            local pos = data.position
                            print("[Test] MoveTo " .. player.Name .. " -> (" .. pos[1] .. ", " .. pos[2] .. ", " .. pos[3] .. ")")
                            humanoid:MoveTo(Vector3.new(pos[1], pos[2], pos[3]))
                        end
                    end
                end)
            end
        "#);

    // Add a player
    let agent_id = Uuid::new_v4();
    assert!(instance.add_player(agent_id, "TestPlayer"));

    let hrp_id = *instance.player_hrp_ids.get(&agent_id).unwrap();

    // Don't wait for landing - queue MoveTo immediately like the working test
    let initial_pos = instance.physics.get_character_position(hrp_id).unwrap();
    println!("Initial position: {:?}", initial_pos);

    // Queue a MoveTo input via agent input service (like Python test does)
    let user_id = *instance.players.get(&agent_id).unwrap();
    instance.queue_agent_input(
        user_id,
        "MoveTo".to_string(),
        serde_json::json!({"position": [20.0, initial_pos[1], 0.0]}),
    );

    // Run 120 ticks (2 seconds)
    for i in 0..120 {
        instance.tick();
        if i % 30 == 0 {
            let pos = instance.physics.get_character_position(hrp_id).unwrap();
            println!(
                "Tick {}: pos=({:.2}, {:.2}, {:.2})",
                i, pos[0], pos[1], pos[2]
            );
        }
    }

    let final_pos = instance.physics.get_character_position(hrp_id).unwrap();
    println!("Final position: {:?}", final_pos);

    let moved_x = final_pos[0] - initial_pos[0];
    let moved_z = final_pos[2] - initial_pos[2];
    let horizontal_distance = (moved_x * moved_x + moved_z * moved_z).sqrt();
    println!("Horizontal distance moved: {}", horizontal_distance);

    // Should move significantly (at 16 studs/sec, should move ~32 studs in 2 sec)
    assert!(
        horizontal_distance > 10.0,
        "Player should have moved towards target via Lua MoveTo"
    );
}

#[test]
fn test_player_move_direction_via_lua_input() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    instance.load_script(
            r#"
            local AgentInputService = game:GetService("AgentInputService")

            local floor = Instance.new("Part")
            floor.Name = "Floor"
            floor.Size = Vector3.new(100, 1, 100)
            floor.Position = Vector3.new(0, 0, 0)
            floor.Anchored = true
            floor.Parent = Workspace

            if AgentInputService then
                AgentInputService.InputReceived:Connect(function(player, inputType, data)
                    if inputType == "Move" and data and data.direction then
                        local humanoid = player.Character and player.Character:FindFirstChild("Humanoid")
                        if humanoid then
                            local dir = data.direction
                            humanoid:Move(Vector3.new(dir[1], dir[2], dir[3]), false)
                        end
                    end
                end)
            end
        "#,
        );

    let agent_id = Uuid::new_v4();
    assert!(instance.add_player(agent_id, "MovePlayer"));
    let hrp_id = *instance.player_hrp_ids.get(&agent_id).unwrap();
    let user_id = *instance.players.get(&agent_id).unwrap();

    let initial_pos = instance.physics.get_character_position(hrp_id).unwrap();

    // Feed directional Move() input for ~2 seconds.
    for _ in 0..120 {
        instance.queue_agent_input(
            user_id,
            "Move".to_string(),
            serde_json::json!({ "direction": [1.0, 0.0, 0.0] }),
        );
        instance.tick();
    }

    let final_pos = instance.physics.get_character_position(hrp_id).unwrap();
    let moved_x = final_pos[0] - initial_pos[0];
    let moved_z = final_pos[2] - initial_pos[2];
    let horizontal_distance = (moved_x * moved_x + moved_z * moved_z).sqrt();

    assert!(
        horizontal_distance > 8.0,
        "Player should move with Humanoid:Move input. Initial={:?} Final={:?}",
        initial_pos,
        final_pos
    );
}

#[test]
fn test_agent_input_cframe_write_on_unanchored_part_applies_same_tick() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    instance.load_script(
        r#"
            local AgentInputService = game:GetService("AgentInputService")

            local floor = Instance.new("Part")
            floor.Name = "Floor"
            floor.Size = Vector3.new(100, 1, 100)
            floor.Position = Vector3.new(0, 0, 0)
            floor.Anchored = true
            floor.Parent = Workspace

            local p = Instance.new("Part")
            p.Name = "MoveByInput"
            p.Anchored = false
            p.Size = Vector3.new(2, 1, 4)
            p.Position = Vector3.new(0, 6, 0)
            p.Parent = Workspace

            if AgentInputService then
                AgentInputService.InputReceived:Connect(function(_player, inputType, _data)
                    if inputType ~= "PlaceObject" then
                        return
                    end
                    p.AssemblyLinearVelocity = Vector3.new(0, 0, 0)
                    p.AssemblyAngularVelocity = Vector3.new(0, 0, 0)
                    p.CFrame = CFrame.new(10, 6, 0) * CFrame.Angles(0, math.rad(30), 0)
                end)
            end
        "#,
    );

    let agent_id = Uuid::new_v4();
    assert!(instance.add_player(agent_id, "InputPlayer"));
    let user_id = *instance
        .players
        .get(&agent_id)
        .expect("Player user id should exist");

    // Initialize physics bodies.
    instance.tick();

    instance.queue_agent_input(user_id, "PlaceObject".to_string(), serde_json::json!({}));
    instance.tick();

    let runtime = instance
        .lua_runtime
        .as_ref()
        .expect("Lua runtime should exist");
    let moved = runtime
        .workspace()
        .get_descendants()
        .into_iter()
        .find(|inst| inst.name() == "MoveByInput")
        .expect("MoveByInput should exist");

    let data = moved.data.lock().unwrap();
    let part = data
        .part_data
        .as_ref()
        .expect("MoveByInput should be a part");
    assert!(
        (part.position.x - 10.0).abs() < 1e-3,
        "Expected X≈10 after same-tick input, got {}",
        part.position.x
    );
    assert!(
        (part.position.y - 6.0).abs() < 0.05,
        "Expected Y close to 6 after same-tick input, got {}",
        part.position.y
    );
    assert!(
        part.position.z.abs() < 1e-3,
        "Expected Z≈0 after same-tick input, got {}",
        part.position.z
    );
    assert!(
        part.cframe.look_vector().x.abs() > 0.1,
        "Expected non-zero yaw rotation after CFrame write"
    );
}

#[test]
fn test_humanoid_jump_input_moves_character_upward() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    instance.load_script(r#"
            local AgentInputService = game:GetService("AgentInputService")

            local floor = Instance.new("Part")
            floor.Name = "Floor"
            floor.Size = Vector3.new(100, 1, 100)
            floor.Position = Vector3.new(0, 0, 0)
            floor.Anchored = true
            floor.Parent = Workspace

            if AgentInputService then
                AgentInputService.InputReceived:Connect(function(player, inputType, data)
                    if inputType == "Jump" then
                        local humanoid = player.Character and player.Character:FindFirstChild("Humanoid")
                        if humanoid then
                            humanoid.Jump = true
                        end
                    end
                end)
            end
        "#);

    let agent_id = Uuid::new_v4();
    assert!(instance.add_player(agent_id, "TestPlayer"));
    let hrp_id = *instance.player_hrp_ids.get(&agent_id).unwrap();
    let user_id = *instance.players.get(&agent_id).unwrap();

    for _ in 0..30 {
        instance.tick();
    }

    let start = instance.physics.get_character_position(hrp_id).unwrap();
    instance.queue_agent_input(user_id, "Jump".to_string(), serde_json::json!({}));

    let mut max_y = start[1];
    for _ in 0..60 {
        instance.tick();
        let pos = instance.physics.get_character_position(hrp_id).unwrap();
        max_y = max_y.max(pos[1]);
    }

    assert!(
        max_y > start[1] + 1.0,
        "Jump should move the character upward. start_y={}, max_y={}",
        start[1],
        max_y
    );
}

#[test]
fn test_humanoid_state_changed_transitions_jump_freefall_running() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    instance.load_script(r#"
            local AgentInputService = game:GetService("AgentInputService")
            local Players = game:GetService("Players")

            local floor = Instance.new("Part")
            floor.Name = "Floor"
            floor.Size = Vector3.new(100, 1, 100)
            floor.Position = Vector3.new(0, 0, 0)
            floor.Anchored = true
            floor.Parent = Workspace

            local state = Instance.new("Part")
            state.Name = "StateMarker"
            state.Size = Vector3.new(2, 2, 2)
            state.Position = Vector3.new(0, 2, 15)
            state.Anchored = true
            state.Parent = Workspace
            state:SetAttribute("JumpingCount", 0)
            state:SetAttribute("FreefallCount", 0)
            state:SetAttribute("LandedCount", 0)
            state:SetAttribute("RunningCount", 0)
            state:SetAttribute("LastState", "")

            local function bindHumanoid(player)
                local character = player.Character
                if not character then return end
                local humanoid = character:FindFirstChild("Humanoid")
                if not humanoid then return end

                humanoid.StateChanged:Connect(function(_oldState, newState)
                    if newState == Enum.HumanoidStateType.Jumping then
                        state:SetAttribute("JumpingCount", (state:GetAttribute("JumpingCount") or 0) + 1)
                    elseif newState == Enum.HumanoidStateType.Freefall then
                        state:SetAttribute("FreefallCount", (state:GetAttribute("FreefallCount") or 0) + 1)
                    elseif newState == Enum.HumanoidStateType.Landed then
                        state:SetAttribute("LandedCount", (state:GetAttribute("LandedCount") or 0) + 1)
                    elseif newState == Enum.HumanoidStateType.Running then
                        state:SetAttribute("RunningCount", (state:GetAttribute("RunningCount") or 0) + 1)
                    end
                    state:SetAttribute("LastState", newState.Name)
                end)
            end

            Players.PlayerAdded:Connect(function(player)
                player.CharacterAdded:Connect(function()
                    bindHumanoid(player)
                end)
                bindHumanoid(player)
            end)

            if AgentInputService then
                AgentInputService.InputReceived:Connect(function(player, inputType, _data)
                    if inputType == "Jump" then
                        local humanoid = player.Character and player.Character:FindFirstChild("Humanoid")
                        if humanoid then
                            humanoid.Jump = true
                        end
                    end
                end)
            end
        "#);

    let agent_id = Uuid::new_v4();
    assert!(instance.add_player(agent_id, "TestPlayer"));
    let user_id = *instance.players.get(&agent_id).unwrap();

    // Settle character onto ground first.
    for _ in 0..45 {
        instance.tick();
    }

    instance.queue_agent_input(user_id, "Jump".to_string(), serde_json::json!({}));

    // Run enough frames to observe jump up, freefall down, and landing.
    for _ in 0..120 {
        instance.tick();
    }

    let jumping_count = get_trigger_attr(&instance, "StateMarker", "JumpingCount");
    let freefall_count = get_trigger_attr(&instance, "StateMarker", "FreefallCount");
    let landed_count = get_trigger_attr(&instance, "StateMarker", "LandedCount");
    let running_count = get_trigger_attr(&instance, "StateMarker", "RunningCount");
    let last_state = get_trigger_str_attr(&instance, "StateMarker", "LastState");

    assert!(
        jumping_count >= 1.0,
        "Expected at least one Jumping state transition"
    );
    assert!(
        freefall_count >= 1.0,
        "Expected at least one Freefall state transition"
    );
    assert!(
        landed_count >= 1.0,
        "Expected at least one Landed state transition"
    );
    assert!(
        running_count >= 1.0,
        "Expected a Running state transition after landing"
    );
    assert_eq!(
        last_state, "Running",
        "Expected final state to return to Running"
    );
}

#[test]
fn test_moveto_finished_fires_for_reach_and_cancel() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    instance.load_script(r#"
            local AgentInputService = game:GetService("AgentInputService")
            local Players = game:GetService("Players")

            local floor = Instance.new("Part")
            floor.Name = "Floor"
            floor.Size = Vector3.new(200, 1, 200)
            floor.Position = Vector3.new(0, 0, 0)
            floor.Anchored = true
            floor.Parent = Workspace

            local state = Instance.new("Part")
            state.Name = "SignalState"
            state.Size = Vector3.new(2, 2, 2)
            state.Position = Vector3.new(0, 2, 20)
            state.Anchored = true
            state.Parent = Workspace
            state:SetAttribute("ReachCount", 0)
            state:SetAttribute("CancelCount", 0)
            state:SetAttribute("LastReached", false)

            local function bindHumanoid(player)
                local character = player.Character
                if not character then return end
                local humanoid = character:FindFirstChild("Humanoid")
                if not humanoid then return end

                humanoid.MoveToFinished:Connect(function(reached)
                    if reached then
                        state:SetAttribute("ReachCount", (state:GetAttribute("ReachCount") or 0) + 1)
                    else
                        state:SetAttribute("CancelCount", (state:GetAttribute("CancelCount") or 0) + 1)
                    end
                    state:SetAttribute("LastReached", reached)
                end)
            end

            Players.PlayerAdded:Connect(function(player)
                player.CharacterAdded:Connect(function()
                    bindHumanoid(player)
                end)
                bindHumanoid(player)
            end)

            if AgentInputService then
                AgentInputService.InputReceived:Connect(function(player, inputType, data)
                    local humanoid = player.Character and player.Character:FindFirstChild("Humanoid")
                    if not humanoid then return end

                    if inputType == "MoveTo" and data and data.position then
                        local pos = data.position
                        humanoid:MoveTo(Vector3.new(pos[1], pos[2], pos[3]))
                    elseif inputType == "Stop" then
                        humanoid:CancelMoveTo()
                    end
                end)
            end
        "#);

    let agent_id = Uuid::new_v4();
    assert!(instance.add_player(agent_id, "TestPlayer"));
    let hrp_id = *instance.player_hrp_ids.get(&agent_id).unwrap();
    let user_id = *instance.players.get(&agent_id).unwrap();

    for _ in 0..20 {
        instance.tick();
    }

    let start = instance.physics.get_character_position(hrp_id).unwrap();
    instance.queue_agent_input(
        user_id,
        "MoveTo".to_string(),
        serde_json::json!({"position": [start[0] + 8.0, start[1], start[2]]}),
    );
    for _ in 0..180 {
        instance.tick();
    }

    let reach_count = get_trigger_attr(&instance, "SignalState", "ReachCount");
    assert!(
        reach_count >= 1.0,
        "MoveToFinished(true) should fire on reach"
    );

    let mid = instance.physics.get_character_position(hrp_id).unwrap();
    instance.queue_agent_input(
        user_id,
        "MoveTo".to_string(),
        serde_json::json!({"position": [mid[0] + 40.0, mid[1], mid[2]]}),
    );
    for _ in 0..20 {
        instance.tick();
    }
    instance.queue_agent_input(user_id, "Stop".to_string(), serde_json::json!({}));
    for _ in 0..10 {
        instance.tick();
    }

    let cancel_count = get_trigger_attr(&instance, "SignalState", "CancelCount");
    assert!(
        cancel_count >= 1.0,
        "MoveToFinished(false) should fire on cancel"
    );

    let last_reached = get_trigger_bool_attr(&instance, "SignalState", "LastReached");
    assert!(!last_reached, "LastReached should be false after cancel");
}

#[test]
fn test_moveto_finished_false_on_timeout_when_blocked() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    instance.load_script(r#"
            local AgentInputService = game:GetService("AgentInputService")
            local Players = game:GetService("Players")

            local floor = Instance.new("Part")
            floor.Name = "Floor"
            floor.Size = Vector3.new(300, 1, 300)
            floor.Position = Vector3.new(0, 0, 0)
            floor.Anchored = true
            floor.Parent = Workspace

            local wall = Instance.new("Part")
            wall.Name = "Wall"
            wall.Size = Vector3.new(2, 20, 120)
            wall.Position = Vector3.new(5, 10, 0)
            wall.Anchored = true
            wall.Parent = Workspace

            local state = Instance.new("Part")
            state.Name = "SignalState"
            state.Size = Vector3.new(2, 2, 2)
            state.Position = Vector3.new(0, 2, 40)
            state.Anchored = true
            state.Parent = Workspace
            state:SetAttribute("TimeoutCount", 0)
            state:SetAttribute("ReachedCount", 0)

            local function bindHumanoid(player)
                local character = player.Character
                if not character then return end
                local humanoid = character:FindFirstChild("Humanoid")
                if not humanoid then return end
                humanoid.MoveToFinished:Connect(function(reached)
                    if reached then
                        state:SetAttribute("ReachedCount", (state:GetAttribute("ReachedCount") or 0) + 1)
                    else
                        state:SetAttribute("TimeoutCount", (state:GetAttribute("TimeoutCount") or 0) + 1)
                    end
                end)
            end

            Players.PlayerAdded:Connect(function(player)
                player.CharacterAdded:Connect(function()
                    bindHumanoid(player)
                end)
                bindHumanoid(player)
            end)

            if AgentInputService then
                AgentInputService.InputReceived:Connect(function(player, inputType, data)
                    if inputType == "MoveTo" and data and data.position then
                        local humanoid = player.Character and player.Character:FindFirstChild("Humanoid")
                        if humanoid then
                            local pos = data.position
                            humanoid:MoveTo(Vector3.new(pos[1], pos[2], pos[3]))
                        end
                    end
                end)
            end
        "#);

    let agent_id = Uuid::new_v4();
    assert!(instance.add_player(agent_id, "TestPlayer"));
    let hrp_id = *instance.player_hrp_ids.get(&agent_id).unwrap();
    let user_id = *instance.players.get(&agent_id).unwrap();

    for _ in 0..20 {
        instance.tick();
    }

    let start = instance.physics.get_character_position(hrp_id).unwrap();
    instance.queue_agent_input(
        user_id,
        "MoveTo".to_string(),
        serde_json::json!({"position": [20.0, start[1], 0.0]}),
    );

    // Wait longer than MoveTo timeout window.
    for _ in 0..540 {
        instance.tick();
    }

    let timeout_count = get_trigger_attr(&instance, "SignalState", "TimeoutCount");
    let reached_count = get_trigger_attr(&instance, "SignalState", "ReachedCount");
    let final_pos = instance.physics.get_character_position(hrp_id).unwrap();

    assert!(
        timeout_count >= 1.0,
        "MoveToFinished(false) should fire on timeout when blocked. timeout_count={}",
        timeout_count
    );
    assert_eq!(
        reached_count, 0.0,
        "Blocked target should not report reached"
    );
    assert!(
        final_pos[0] < 6.0,
        "Character should remain blocked near wall. final_pos={:?}",
        final_pos
    );
}

#[test]
fn test_grounded_character_carried_by_moving_platform() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    instance.load_script(
        r#"
            local RunService = game:GetService("RunService")

            local floor = Instance.new("Part")
            floor.Name = "Floor"
            floor.Size = Vector3.new(300, 1, 300)
            floor.Position = Vector3.new(0, -1, 0)
            floor.Anchored = true
            floor.Parent = Workspace

            local elevator = Instance.new("Part")
            elevator.Name = "Elevator"
            elevator.Size = Vector3.new(30, 1, 30)
            elevator.Position = Vector3.new(-1.5, 1.0, 0.0)
            elevator.Anchored = true
            elevator.Parent = Workspace

            local elapsed = 0
            RunService.Heartbeat:Connect(function(dt)
                elapsed = elapsed + dt
                if elapsed < 1.0 then
                    elevator.Position = elevator.Position + Vector3.new(0, 0.12, 0)
                end
            end)
        "#,
    );

    let agent_id = Uuid::new_v4();
    assert!(instance.add_player(agent_id, "TestPlayer"));
    let hrp_id = *instance.player_hrp_ids.get(&agent_id).unwrap();

    for _ in 0..20 {
        instance.tick();
    }
    let y_before = instance.physics.get_character_position(hrp_id).unwrap()[1];

    for _ in 0..60 {
        instance.tick();
    }
    let y_after = instance.physics.get_character_position(hrp_id).unwrap()[1];

    assert!(
        y_after > y_before + 1.0,
        "Character should be carried upward by moving platform. before_y={}, after_y={}",
        y_before,
        y_after
    );
}

#[test]
fn test_observation_includes_world_entities() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    // Load a script that creates some geometry
    instance.load_script(
        r#"
            local floor = Instance.new("Part")
            floor.Name = "Floor"
            floor.Size = Vector3.new(100, 1, 100)
            floor.Position = Vector3.new(0, 0, 0)
            floor.Anchored = true
            floor.Parent = Workspace

            local platform = Instance.new("Part")
            platform.Name = "CenterPlatform"
            platform.Size = Vector3.new(10, 1, 10)
            platform.Position = Vector3.new(0, 5, 0)
            platform.Anchored = true
            platform.Parent = Workspace
        "#,
    );

    let agent_id = Uuid::new_v4();
    instance.add_player(agent_id, "TestPlayer");

    // Run a tick to sync physics
    instance.tick();

    let obs = instance.get_player_observation(agent_id).unwrap();

    // Should have world entities
    assert!(
        !obs.world.entities.is_empty(),
        "World should contain entities"
    );

    // Should have Floor and CenterPlatform
    let names: Vec<&str> = obs.world.entities.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"Floor"), "Should contain Floor");
    assert!(
        names.contains(&"CenterPlatform"),
        "Should contain CenterPlatform"
    );

    // Check Floor properties
    let floor = obs
        .world
        .entities
        .iter()
        .find(|e| e.name == "Floor")
        .unwrap();
    assert_eq!(floor.size, [100.0, 1.0, 100.0]);
    assert!(floor.anchored);
    let aabb = floor.aabb.as_ref().expect("observe should include AABB");
    assert_eq!(aabb.min, [-50.0, -0.5, -50.0]);
    assert_eq!(aabb.max, [50.0, 0.5, 50.0]);
    let obb = floor.obb.as_ref().expect("observe should include OBB");
    assert_eq!(obb.center, [0.0, 0.0, 0.0]);
    assert_eq!(obb.half_extents, [50.0, 0.5, 50.0]);
    assert_eq!(obb.corners.len(), 8);
}

#[test]
fn test_observation_radius_filters_world_entities_by_bounds() {
    let mut instance =
        GameInstance::new_with_config(Uuid::new_v4(), 8, 5.0, None, ErrorMode::Continue);

    instance.load_script(
        r#"
            local floor = Instance.new("Part")
            floor.Name = "Floor"
            floor.Size = Vector3.new(100, 1, 100)
            floor.Position = Vector3.new(0, 0, 0)
            floor.Anchored = true
            floor.Parent = Workspace

            local edge_platform = Instance.new("Part")
            edge_platform.Name = "EdgePlatform"
            edge_platform.Size = Vector3.new(4, 4, 4)
            edge_platform.Position = Vector3.new(7, 2, 0)
            edge_platform.Anchored = true
            edge_platform.Parent = Workspace

            local far_platform = Instance.new("Part")
            far_platform.Name = "FarPlatform"
            far_platform.Size = Vector3.new(2, 2, 2)
            far_platform.Position = Vector3.new(12, 2, 0)
            far_platform.Anchored = true
            far_platform.Parent = Workspace

            local game_state = Instance.new("Folder")
            game_state.Name = "GameState"
            game_state:SetAttribute("Phase", "active")
            game_state.Parent = Workspace
        "#,
    );

    let agent_id = Uuid::new_v4();
    instance.add_player(agent_id, "TestPlayer");

    for _ in 0..5 {
        instance.tick();
    }

    let hrp_id = *instance.player_hrp_ids.get(&agent_id).unwrap();
    instance
        .physics
        .set_character_position(hrp_id, [0.0, 2.0, 0.0]);

    let obs = instance.get_player_observation(agent_id).unwrap();
    let names: Vec<&str> = obs
        .world
        .entities
        .iter()
        .map(|entity| entity.name.as_str())
        .collect();

    assert!(
        names.contains(&"EdgePlatform"),
        "Part intersecting observation radius should remain visible"
    );
    assert!(
        !names.contains(&"FarPlatform"),
        "Part beyond observation radius should be filtered out"
    );
    assert!(
        names.contains(&"GameState"),
        "Non-spatial folder metadata should remain visible"
    );
}

#[test]
fn test_los_blocked_by_wall() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    // Create a wall between two player spawn points
    instance.load_script(
        r#"
            local floor = Instance.new("Part")
            floor.Name = "Floor"
            floor.Size = Vector3.new(100, 1, 100)
            floor.Position = Vector3.new(0, 0, 0)
            floor.Anchored = true
            floor.Parent = Workspace

            local wall = Instance.new("Part")
            wall.Name = "Wall"
            wall.Size = Vector3.new(20, 10, 2)
            wall.Position = Vector3.new(0, 5, 0)
            wall.Anchored = true
            wall.Parent = Workspace
        "#,
    );

    let agent_a = Uuid::new_v4();
    let agent_b = Uuid::new_v4();
    instance.add_player(agent_a, "PlayerA");
    instance.add_player(agent_b, "PlayerB");

    // Run a tick to sync physics
    instance.tick();

    // Position players on opposite sides of the wall
    // Player A at z=-10, Player B at z=+10, wall at z=0
    let hrp_a = *instance.player_hrp_ids.get(&agent_a).unwrap();
    let hrp_b = *instance.player_hrp_ids.get(&agent_b).unwrap();

    // Ensure characters have physics bodies initialized
    instance
        .physics
        .set_character_position(hrp_a, [0.0, 2.0, -10.0]);
    instance
        .physics
        .set_character_position(hrp_b, [0.0, 2.0, 10.0]);

    // Manually set positions behind the wall
    // Update query pipeline
    instance
        .physics
        .query_pipeline
        .update(&instance.physics.collider_set);

    let obs = instance.get_player_observation(agent_a).unwrap();

    // Agent A should NOT see Agent B (blocked by wall)
    assert!(
        obs.other_players.is_empty(),
        "Should not see player through wall"
    );
}

#[test]
fn test_los_clear() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    // Just a floor, no obstructions
    instance.load_script(
        r#"
            local floor = Instance.new("Part")
            floor.Name = "Floor"
            floor.Size = Vector3.new(100, 1, 100)
            floor.Position = Vector3.new(0, 0, 0)
            floor.Anchored = true
            floor.Parent = Workspace
        "#,
    );

    let agent_a = Uuid::new_v4();
    let agent_b = Uuid::new_v4();
    instance.add_player(agent_a, "PlayerA");
    instance.add_player(agent_b, "PlayerB");

    // Run several ticks to sync physics
    for _ in 0..5 {
        instance.tick();
    }

    // Position players at same height on the floor with clear LOS
    let hrp_a = *instance.player_hrp_ids.get(&agent_a).unwrap();
    let hrp_b = *instance.player_hrp_ids.get(&agent_b).unwrap();

    if let Some(state) = instance.physics.get_character_state(hrp_a) {
        if let Some(body) = instance.physics.rigid_body_set.get_mut(state.body_handle) {
            body.set_translation(rapier3d::prelude::vector![-10.0, 2.0, 0.0], true);
        }
    }
    if let Some(state) = instance.physics.get_character_state(hrp_b) {
        if let Some(body) = instance.physics.rigid_body_set.get_mut(state.body_handle) {
            body.set_translation(rapier3d::prelude::vector![10.0, 2.0, 0.0], true);
        }
    }

    // Update query pipeline
    instance
        .physics
        .query_pipeline
        .update(&instance.physics.collider_set);

    let obs = instance.get_player_observation(agent_a).unwrap();

    // Agent A should see Agent B (clear LOS)
    assert_eq!(
        obs.other_players.len(),
        1,
        "Should see other player with clear LOS"
    );
    assert_eq!(obs.other_players[0].id, agent_b);
}

#[test]
fn test_observation_radius_filters_other_players() {
    let mut instance =
        GameInstance::new_with_config(Uuid::new_v4(), 8, 5.0, None, ErrorMode::Continue);

    instance.load_script(
        r#"
            local floor = Instance.new("Part")
            floor.Name = "Floor"
            floor.Size = Vector3.new(100, 1, 100)
            floor.Position = Vector3.new(0, 0, 0)
            floor.Anchored = true
            floor.Parent = Workspace
        "#,
    );

    let agent_a = Uuid::new_v4();
    let agent_b = Uuid::new_v4();
    instance.add_player(agent_a, "PlayerA");
    instance.add_player(agent_b, "PlayerB");

    for _ in 0..5 {
        instance.tick();
    }

    let hrp_a = *instance.player_hrp_ids.get(&agent_a).unwrap();
    let hrp_b = *instance.player_hrp_ids.get(&agent_b).unwrap();

    if let Some(state) = instance.physics.get_character_state(hrp_a) {
        if let Some(body) = instance.physics.rigid_body_set.get_mut(state.body_handle) {
            body.set_translation(rapier3d::prelude::vector![0.0, 2.0, 0.0], true);
        }
    }
    if let Some(state) = instance.physics.get_character_state(hrp_b) {
        if let Some(body) = instance.physics.rigid_body_set.get_mut(state.body_handle) {
            body.set_translation(rapier3d::prelude::vector![10.0, 2.0, 0.0], true);
        }
    }

    instance
        .physics
        .query_pipeline
        .update(&instance.physics.collider_set);

    let obs = instance.get_player_observation(agent_a).unwrap();
    assert!(
        obs.other_players.is_empty(),
        "Player beyond configured observation radius should be filtered out"
    );
}

#[test]
fn test_shooting_and_killing() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    // Load a minimal shooting test script (no debug prints, fast startup)
    instance.load_script(r#"
            local Players = game:GetService("Players")
            local AgentInputService = game:GetService("AgentInputService")

            -- Create floor
            local floor = Instance.new("Part")
            floor.Name = "Floor"
            floor.Size = Vector3.new(100, 1, 100)
            floor.Position = Vector3.new(0, 0, 0)
            floor.Anchored = true
            floor.Parent = Workspace

            -- Initialize player weapons
            Players.PlayerAdded:Connect(function(player)
                player:SetAttribute("CurrentWeapon", 1)
                player:SetAttribute("Kills", 0)
            end)

            -- Handle Fire input
            if AgentInputService then
                AgentInputService.InputReceived:Connect(function(player, inputType, data)
                    if inputType == "Fire" and data and data.direction then
                        local character = player.Character
                        if not character then return end

                        local hrp = character:FindFirstChild("HumanoidRootPart")
                        if not hrp then return end

                        local origin = hrp.Position + Vector3.new(0, 1, 0)
                        local dir = Vector3.new(data.direction[1], data.direction[2], data.direction[3])

                        -- Raycast to find target
                        local raycastParams = RaycastParams.new()
                        raycastParams.FilterType = Enum.RaycastFilterType.Exclude
                        raycastParams.FilterDescendantsInstances = {character}

                        local result = Workspace:Raycast(origin, dir * 100, raycastParams)
                        if result then
                            -- Check if we hit a player
                            local hitPart = result.Instance
                            local current = hitPart
                            while current do
                                if current:FindFirstChild("Humanoid") then
                                    -- Found a character, deal damage
                                    local humanoid = current:FindFirstChild("Humanoid")
                                    humanoid:TakeDamage(25)
                                    print("[HIT] Dealt 25 damage, health now:", humanoid.Health)

                                    if humanoid.Health <= 0 then
                                        player:SetAttribute("Kills", (player:GetAttribute("Kills") or 0) + 1)
                                        player:SetAttribute("CurrentWeapon", (player:GetAttribute("CurrentWeapon") or 1) + 1)
                                        print("[KILL] Player got kill, weapon now:", player:GetAttribute("CurrentWeapon"))
                                    end
                                    break
                                end
                                current = current.Parent
                            end
                        else
                            print("[MISS] Raycast hit nothing")
                        end
                    end
                end)
            end
            print("Shooting test script loaded")
        "#);

    // Add two players
    let attacker_id = Uuid::new_v4();
    let victim_id = Uuid::new_v4();
    instance.add_player(attacker_id, "Attacker");
    instance.add_player(victim_id, "Victim");

    let attacker_user_id = *instance.players.get(&attacker_id).unwrap();

    // Run a few ticks to initialize
    for _ in 0..5 {
        instance.tick();
    }

    // Position players facing each other
    let hrp_attacker = *instance.player_hrp_ids.get(&attacker_id).unwrap();
    let hrp_victim = *instance.player_hrp_ids.get(&victim_id).unwrap();

    if let Some(state) = instance.physics.get_character_state(hrp_attacker) {
        if let Some(body) = instance.physics.rigid_body_set.get_mut(state.body_handle) {
            body.set_translation(rapier3d::prelude::vector![-5.0, 2.0, 0.0], true);
        }
    }
    if let Some(state) = instance.physics.get_character_state(hrp_victim) {
        if let Some(body) = instance.physics.rigid_body_set.get_mut(state.body_handle) {
            body.set_translation(rapier3d::prelude::vector![5.0, 2.0, 0.0], true);
        }
    }

    // Sync physics to Lua
    instance.tick();

    let initial_health = instance.get_player_health(victim_id).unwrap_or(100);

    // Fire towards victim (+X direction)
    let fire_input = serde_json::json!({ "direction": [1.0, 0.0, 0.0] });
    instance.queue_agent_input(attacker_user_id, "Fire".to_string(), fire_input);

    instance.tick();

    let health_after_shot = instance.get_player_health(victim_id).unwrap_or(100);

    assert!(
        health_after_shot < initial_health,
        "Victim should have taken damage: {} -> {}",
        initial_health,
        health_after_shot
    );

    // Keep shooting until dead
    let mut shots = 1;
    while instance.get_player_health(victim_id).unwrap_or(0) > 0 && shots < 10 {
        let fire_input = serde_json::json!({ "direction": [1.0, 0.0, 0.0] });
        instance.queue_agent_input(attacker_user_id, "Fire".to_string(), fire_input);
        instance.tick();
        shots += 1;
    }

    let final_health = instance.get_player_health(victim_id).unwrap_or(0);
    assert!(
        final_health <= 0,
        "Victim should be dead after {} shots",
        shots
    );

    // Check attacker stats
    if let Some(runtime) = &instance.lua_runtime {
        if let Some(player) = runtime.players().get_player_by_user_id(attacker_user_id) {
            let data = player.data.lock().unwrap();

            // Verify kills attribute was set to 1
            if let Some(AttributeValue::Number(kills)) = data.attributes.get("Kills") {
                assert_eq!(*kills as i32, 1, "Expected 1 kill");
            } else {
                panic!("Kills attribute not found or not a number");
            }

            // Verify weapon was upgraded to 2
            if let Some(AttributeValue::Number(weapon)) = data.attributes.get("CurrentWeapon") {
                assert_eq!(*weapon as i32, 2, "Expected weapon to be upgraded to 2");
            } else {
                panic!("CurrentWeapon attribute not found or not a number");
            }
        }
    }
}

#[test]
fn test_player_kick_from_lua() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    // Load a script that kicks a player after they join
    instance.load_script(
        r#"
            local Players = game:GetService("Players")

            local floor = Instance.new("Part")
            floor.Name = "Floor"
            floor.Size = Vector3.new(100, 1, 100)
            floor.Position = Vector3.new(0, 0, 0)
            floor.Anchored = true
            floor.Parent = Workspace

            _G.kickCalled = false

            Players.PlayerAdded:Connect(function(player)
                -- Schedule kick after a few ticks
                _G.playerToKick = player
            end)
        "#,
    );

    // Add a player
    let agent_id = Uuid::new_v4();
    assert!(instance.add_player(agent_id, "TestPlayer"));
    assert_eq!(instance.players.len(), 1);

    // Run a few ticks to process PlayerAdded
    for _ in 0..5 {
        instance.tick();
    }

    // Player should still be there
    assert_eq!(instance.players.len(), 1);

    // Now trigger the kick via Lua
    if let Some(runtime) = &instance.lua_runtime {
        let lua = runtime.lua();
        lua.load(
            r#"
                if _G.playerToKick then
                    _G.playerToKick:Kick("Test kick")
                    _G.kickCalled = true
                end
            "#,
        )
        .exec()
        .expect("Failed to run kick script");
    }

    // Run tick to process kick request
    instance.tick();

    // Player should be removed
    assert_eq!(instance.players.len(), 0, "Player should have been kicked");
}

#[test]
fn test_player_added_can_assign_player_character_roblox_parity() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    instance.load_script(
        r#"
            local Players = game:GetService("Players")

            local floor = Instance.new("Part")
            floor.Name = "Floor"
            floor.Size = Vector3.new(100, 1, 100)
            floor.Position = Vector3.new(0, 0, 0)
            floor.Anchored = true
            floor.Parent = Workspace

            local function createCharacter(player)
                local character = Instance.new("Model")
                character.Name = player.Name

                local hrp = Instance.new("Part")
                hrp.Name = "HumanoidRootPart"
                hrp.Size = Vector3.new(2, 2, 1)
                hrp.Anchored = false
                hrp.Parent = character

                local humanoid = Instance.new("Humanoid")
                humanoid.Parent = character

                character.Parent = Workspace
                -- Roblox parity: Player.Character assignment in PlayerAdded callback.
                player.Character = character
            end

            Players.PlayerAdded:Connect(function(player)
                createCharacter(player)
            end)
        "#,
    );

    let agent_id = Uuid::new_v4();
    assert!(instance.add_player(agent_id, "ParityPlayer"));
    assert!(
        instance.halted_error.is_none(),
        "Instance should not halt when Player.Character is assigned in PlayerAdded: {:?}",
        instance.halted_error
    );

    for _ in 0..3 {
        instance.tick();
    }

    if let Some(runtime) = &instance.lua_runtime {
        let user_id = *instance.players.get(&agent_id).unwrap();
        let player = runtime
            .players()
            .get_player_by_user_id(user_id)
            .expect("player should exist");
        let character = {
            let data = player.data.lock().unwrap();
            data.player_data
                .as_ref()
                .and_then(|p| p.character.as_ref())
                .and_then(|w| w.upgrade())
        };
        assert!(
            character.is_some(),
            "player.Character should be set by PlayerAdded callback"
        );
    } else {
        panic!("Lua runtime should be loaded");
    }
}

#[test]
fn test_afk_timeout() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    // Set a very short AFK timeout for testing (100ms)
    instance.set_afk_timeout(Duration::from_millis(100));

    instance.load_script(
        r#"
            local floor = Instance.new("Part")
            floor.Name = "Floor"
            floor.Size = Vector3.new(100, 1, 100)
            floor.Position = Vector3.new(0, 0, 0)
            floor.Anchored = true
            floor.Parent = Workspace
        "#,
    );

    // Add a player
    let agent_id = Uuid::new_v4();
    assert!(instance.add_player(agent_id, "AFKPlayer"));
    assert_eq!(instance.players.len(), 1);

    // Run some ticks without activity - but not enough to trigger AFK check
    for _ in 0..30 {
        instance.tick();
    }

    // Player should still be there (AFK check happens every 60 ticks)
    assert_eq!(instance.players.len(), 1);

    // Sleep to exceed AFK timeout
    std::thread::sleep(Duration::from_millis(150));

    // Run 60 ticks to trigger AFK check
    for _ in 0..60 {
        instance.tick();
    }

    // Player should be kicked for being AFK
    assert_eq!(
        instance.players.len(),
        0,
        "Player should have been kicked for AFK"
    );
}

#[test]
fn test_activity_resets_afk_timer() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    // Set a longer AFK timeout for this test to avoid race conditions (200ms)
    instance.set_afk_timeout(Duration::from_millis(200));

    instance.load_script(
        r#"
            local floor = Instance.new("Part")
            floor.Name = "Floor"
            floor.Size = Vector3.new(100, 1, 100)
            floor.Position = Vector3.new(0, 0, 0)
            floor.Anchored = true
            floor.Parent = Workspace
        "#,
    );

    // Add a player
    let agent_id = Uuid::new_v4();
    assert!(instance.add_player(agent_id, "ActivePlayer"));
    assert_eq!(instance.players.len(), 1);

    // Sleep almost to timeout
    std::thread::sleep(Duration::from_millis(150));

    // Record activity to reset AFK timer (before timeout)
    instance.record_player_activity(agent_id);

    // Process a tick
    instance.tick();

    // Sleep less than the full timeout again
    std::thread::sleep(Duration::from_millis(100));

    // Run AFK check (60 ticks)
    for _ in 0..60 {
        instance.tick();
    }

    // Player should still be there (activity reset the timer)
    // Total time since reset: ~100ms, which is less than 200ms timeout
    assert_eq!(
        instance.players.len(),
        1,
        "Player should NOT be kicked - activity reset timer"
    );
}

#[test]
fn test_humanoid_walk_speed_affects_movement() {
    // Test that custom WalkSpeed on humanoid affects actual movement speed
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    // Load a script that creates a floor and handles SetSpeed input
    instance.load_script(r#"
            local AgentInputService = game:GetService("AgentInputService")

            local floor = Instance.new("Part")
            floor.Name = "Floor"
            floor.Size = Vector3.new(200, 1, 200)
            floor.Position = Vector3.new(0, 0, 0)
            floor.Anchored = true
            floor.Parent = Workspace

            -- Handle SetSpeed input to change WalkSpeed
            if AgentInputService then
                AgentInputService.InputReceived:Connect(function(player, inputType, data)
                    if inputType == "SetSpeed" and data and data.speed then
                        local humanoid = player.Character and player.Character:FindFirstChild("Humanoid")
                        if humanoid then
                            humanoid.WalkSpeed = data.speed
                            print("[Test] Set WalkSpeed to " .. data.speed)
                        end
                    end
                end)
            end
        "#);

    // Add a player
    let agent_id = Uuid::new_v4();
    assert!(instance.add_player(agent_id, "SpeedTestPlayer"));

    let hrp_id = *instance.player_hrp_ids.get(&agent_id).unwrap();
    let user_id = *instance.players.get(&agent_id).unwrap();

    // Verify default walk speed is returned
    let default_speed = instance.get_humanoid_walk_speed(agent_id);
    assert!(
        default_speed.is_some(),
        "Should be able to get humanoid walk speed"
    );
    assert_eq!(
        default_speed.unwrap(),
        16.0,
        "Default walk speed should be 16.0"
    );

    // Get initial position and set movement target
    let initial_pos = instance.physics.get_character_position(hrp_id).unwrap();
    instance
        .physics
        .set_character_target(hrp_id, Some([50.0, initial_pos[1], 0.0]));

    // Run 60 ticks (1 second at default speed)
    for _ in 0..60 {
        instance.tick();
    }

    let pos_after_default = instance.physics.get_character_position(hrp_id).unwrap();
    let distance_default = (pos_after_default[0] - initial_pos[0]).abs();
    println!("Distance moved at default speed (16): {}", distance_default);

    // Now set a higher WalkSpeed via agent input (simulating speed upgrade)
    instance.queue_agent_input(
        user_id,
        "SetSpeed".to_string(),
        serde_json::json!({"speed": 32.0}),
    );

    // Process the input
    instance.tick();

    // Verify the new walk speed is returned
    let new_speed = instance.get_humanoid_walk_speed(agent_id);
    assert!(
        new_speed.is_some(),
        "Should still be able to get humanoid walk speed"
    );
    assert_eq!(new_speed.unwrap(), 32.0, "Walk speed should now be 32.0");

    // Reset position and move again
    let start_pos = instance.physics.get_character_position(hrp_id).unwrap();
    instance.physics.set_character_target(
        hrp_id,
        Some([start_pos[0] + 50.0, start_pos[1], start_pos[2]]),
    );

    // Run another 60 ticks (1 second at double speed)
    for _ in 0..60 {
        instance.tick();
    }

    let pos_after_fast = instance.physics.get_character_position(hrp_id).unwrap();
    let distance_fast = (pos_after_fast[0] - start_pos[0]).abs();
    println!("Distance moved at fast speed (32): {}", distance_fast);

    // Player should move roughly twice as far with double speed
    // Allow some tolerance for physics/rounding
    assert!(
        distance_fast > distance_default * 1.5,
        "Player should move significantly faster with higher WalkSpeed. Default: {}, Fast: {}",
        distance_default,
        distance_fast
    );
}

#[test]
fn test_remote_event_position_write_on_unanchored_part_moves_xz() {
    // Regression: scripts that move unanchored parts via Position writes each Heartbeat
    // should move in X/Z when driven by RemoteEvent input.
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    instance.load_script(
        r#"
            local Players = game:GetService("Players")
            local ReplicatedStorage = game:GetService("ReplicatedStorage")
            local RunService = game:GetService("RunService")

            local floor = Instance.new("Part")
            floor.Name = "Floor"
            floor.Size = Vector3.new(200, 1, 200)
            floor.Position = Vector3.new(0, 0, 0)
            floor.Anchored = true
            floor.Parent = Workspace

            local input = Instance.new("RemoteEvent")
            input.Name = "PlayerInput"
            input.Parent = ReplicatedStorage

            local moveByPlayer = {}
            local bodyByPlayer = {}

            Players.PlayerAdded:Connect(function(player)
                local body = Instance.new("Part")
                body.Name = player.Name .. "_Body"
                body.Size = Vector3.new(4, 2, 2)
                body.Anchored = false
                body.CanCollide = true
                body.Position = Vector3.new(0, 6, 0)
                body.Parent = Workspace

                bodyByPlayer[player] = body
                moveByPlayer[player] = Vector3.new(0, 0, 0)
            end)

            input.OnServerEvent:Connect(function(player, moveVec)
                moveByPlayer[player] = Vector3.new(moveVec.X, 0, moveVec.Z)
            end)

            RunService.Heartbeat:Connect(function(dt)
                for player, body in pairs(bodyByPlayer) do
                    if body and body.Parent then
                        local v = moveByPlayer[player] or Vector3.new(0, 0, 0)
                        body.Position = body.Position + v * 25 * dt
                    end
                end
            end)
        "#,
    );

    let agent_id = Uuid::new_v4();
    assert!(instance.add_player(agent_id, "Probe"));
    let user_id = *instance.players.get(&agent_id).unwrap();

    for _ in 0..10 {
        instance.tick();
    }

    let get_body_x = |instance: &GameInstance| {
        let obs = instance.get_player_observation(agent_id).unwrap();
        obs.world
            .entities
            .iter()
            .find(|e| e.name == "Probe_Body")
            .map(|e| e.position[0])
            .expect("Probe_Body should exist in world entities")
    };

    let x_before = get_body_x(&instance);

    for _ in 0..60 {
        instance.queue_agent_input(
            user_id,
            "RemoteEvent".to_string(),
            serde_json::json!({
                "name": "PlayerInput",
                "args": [{
                    "$type": "Vector3",
                    "x": 1.0,
                    "y": 0.0,
                    "z": 0.0
                }]
            }),
        );
        instance.tick();
    }

    let x_after = get_body_x(&instance);
    let dx = (x_after - x_before).abs();

    assert!(
            dx > 5.0,
            "Probe_Body should move in X with RemoteEvent-driven Position writes. before={}, after={}, dx={}",
            x_before,
            x_after,
            dx
        );
}

#[test]
fn test_player_userdata_identity_stable_for_remote_event_tables() {
    // Regression: scripts commonly key state by Player instance. The same player object
    // must be seen across PlayerAdded and RemoteEvent.OnServerEvent.
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    instance.load_script(
        r#"
            local Players = game:GetService("Players")
            local ReplicatedStorage = game:GetService("ReplicatedStorage")

            local input = Instance.new("RemoteEvent")
            input.Name = "PlayerInput"
            input.Parent = ReplicatedStorage

            local seen = {}
            local bodyByPlayer = {}
            _G.remoteHitCount = 0
            _G.identityOk = 0

            Players.PlayerAdded:Connect(function(player)
                local body = Instance.new("Part")
                body.Name = player.Name .. "_Body"
                body.Size = Vector3.new(2, 2, 2)
                body.Anchored = true
                body.Position = Vector3.new(0, 3, 0)
                body.Parent = Workspace

                seen[player] = true
                bodyByPlayer[player] = body
            end)

            input.OnServerEvent:Connect(function(player, _moveVec)
                _G.remoteHitCount = (_G.remoteHitCount or 0) + 1
                _G.identityOk = seen[player] and 1 or 0
                local body = bodyByPlayer[player]
                if body then
                    body:SetAttribute("RemoteHit", 1)
                    body:SetAttribute("IdentityOk", seen[player] and 1 or 0)
                end
            end)
        "#,
    );

    let agent_id = Uuid::new_v4();
    assert!(instance.add_player(agent_id, "ProbeId"));
    let user_id = *instance.players.get(&agent_id).unwrap();

    // Let PlayerAdded handlers run and part replication settle.
    for _ in 0..5 {
        instance.tick();
    }

    for _ in 0..8 {
        instance.queue_agent_input(
            user_id,
            "RemoteEvent".to_string(),
            serde_json::json!({
                "name": "PlayerInput",
                "args": [{
                    "$type": "Vector3",
                    "x": 1.0,
                    "y": 0.0,
                    "z": 0.0
                }]
            }),
        );
        instance.tick();
    }

    let globals = instance
        .lua_runtime
        .as_ref()
        .expect("Lua runtime should exist")
        .lua()
        .globals();
    let remote_hit_count: i64 = globals.get("remoteHitCount").unwrap_or(0);
    let identity_ok_global: i64 = globals.get("identityOk").unwrap_or(0);

    let obs = instance.get_player_observation(agent_id).unwrap();
    let body = obs
        .world
        .entities
        .iter()
        .find(|e| e.name == "ProbeId_Body")
        .expect("ProbeId_Body should exist");
    let attrs = body
        .attributes
        .as_ref()
        .expect("Body should have attributes");
    let remote_hit = attrs
        .get("RemoteHit")
        .and_then(|v| v.as_f64())
        .unwrap_or_default();
    let identity_ok = attrs
        .get("IdentityOk")
        .and_then(|v| v.as_f64())
        .unwrap_or_default();

    assert!(
        remote_hit_count > 0,
        "OnServerEvent should run (remoteHitCount={})",
        remote_hit_count
    );
    assert_eq!(
        identity_ok_global, 1,
        "identityOk global should be true after OnServerEvent"
    );
    assert!(
        remote_hit > 0.5,
        "Body attribute should confirm OnServerEvent path (RemoteHit={})",
        remote_hit
    );
    assert_eq!(
        identity_ok as i64, 1,
        "Player identity should be stable for table key lookups across callbacks"
    );
}

#[test]
fn test_collectible_touched_owner_player_lookup_path() {
    // Reproduces generated-game collectible logic:
    // hit:GetAttribute("Owner") -> Players:GetPlayerByUserId(ownerId)
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    instance.load_script(
            r#"
            local Players = game:GetService("Players")
            local ReplicatedStorage = game:GetService("ReplicatedStorage")
            local RunService = game:GetService("RunService")

            local input = Instance.new("RemoteEvent")
            input.Name = "PlayerInput"
            input.Parent = ReplicatedStorage

            local floor = Instance.new("Part")
            floor.Name = "Floor"
            floor.Size = Vector3.new(200, 1, 200)
            floor.Anchored = true
            floor.Position = Vector3.new(0, 0, 0)
            floor.Parent = Workspace

            local bodyByPlayer = {}
            local moveByPlayer = {}

            local collectible = Instance.new("Part")
            collectible.Name = "Collectible"
            collectible.Size = Vector3.new(2, 2, 2)
            collectible.Shape = Enum.PartType.Ball
            collectible.Anchored = true
            collectible.CanCollide = false
            collectible.Position = Vector3.new(20, 3, 0)
            collectible.Parent = Workspace
            collectible:SetAttribute("HitCount", 0)
            collectible:SetAttribute("LookupOk", 0)

            collectible.Touched:Connect(function(hit)
                collectible:SetAttribute("HitCount", (collectible:GetAttribute("HitCount") or 0) + 1)
                local ownerId = hit:GetAttribute("Owner")
                if ownerId then
                    local player = Players:GetPlayerByUserId(ownerId)
                    if player then
                        collectible:SetAttribute("LookupOk", 1)
                    end
                end
            end)

            Players.PlayerAdded:Connect(function(player)
                local body = Instance.new("Part")
                body.Name = player.Name .. "_Body"
                body.Size = Vector3.new(4, 2, 2)
                body.Anchored = false
                body.CanCollide = true
                body.Position = Vector3.new(0, 6, 0)
                body:SetAttribute("Owner", player.UserId)
                body.Parent = Workspace
                bodyByPlayer[player] = body
                moveByPlayer[player] = Vector3.new(0, 0, 0)
            end)

            input.OnServerEvent:Connect(function(player, moveVec)
                moveByPlayer[player] = Vector3.new(moveVec.X, 0, moveVec.Z)
            end)

            RunService.Heartbeat:Connect(function(dt)
                for player, body in pairs(bodyByPlayer) do
                    local v = moveByPlayer[player] or Vector3.new(0, 0, 0)
                    body.Position = body.Position + v * 25 * dt
                end
            end)
        "#,
        );

    let agent_id = Uuid::new_v4();
    assert!(instance.add_player(agent_id, "ProbeTouch"));
    let user_id = *instance.players.get(&agent_id).unwrap();

    for _ in 0..10 {
        instance.tick();
    }

    // Drive body roughly toward collectible at x=20,z=0
    for _ in 0..80 {
        instance.queue_agent_input(
            user_id,
            "RemoteEvent".to_string(),
            serde_json::json!({
                "name": "PlayerInput",
                "args": [{
                    "$type": "Vector3",
                    "x": 1.0,
                    "y": 0.0,
                    "z": 0.0
                }]
            }),
        );
        instance.tick();
    }
    for _ in 0..10 {
        instance.queue_agent_input(
            user_id,
            "RemoteEvent".to_string(),
            serde_json::json!({
                "name": "PlayerInput",
                "args": [{
                    "$type": "Vector3",
                    "x": 0.0,
                    "y": 0.0,
                    "z": 0.0
                }]
            }),
        );
        instance.tick();
    }

    let obs = instance.get_player_observation(agent_id).unwrap();
    let collectible = obs
        .world
        .entities
        .iter()
        .find(|e| e.name == "Collectible")
        .expect("Collectible should exist");
    let attrs = collectible
        .attributes
        .as_ref()
        .expect("Collectible should have attributes");

    let hit_count = attrs
        .get("HitCount")
        .and_then(|v| v.as_f64())
        .unwrap_or_default();
    let lookup_ok = attrs
        .get("LookupOk")
        .and_then(|v| v.as_f64())
        .unwrap_or_default();

    assert!(hit_count >= 1.0, "Touched should fire at least once");
    assert!(
        lookup_ok >= 1.0,
        "GetPlayerByUserId(ownerId) should resolve the touching player"
    );
}

#[test]
fn test_touched_events_fire_on_overlap() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    // Script: floor + trigger zone that sets an attribute when Touched fires
    instance.load_script(
        r#"
            local floor = Instance.new("Part")
            floor.Name = "Floor"
            floor.Size = Vector3.new(200, 1, 200)
            floor.Position = Vector3.new(0, 0, 0)
            floor.Anchored = true
            floor.Parent = Workspace

            -- Trigger zone: CanCollide=false, CanTouch=true
            local trigger = Instance.new("Part")
            trigger.Name = "Trigger"
            trigger.Size = Vector3.new(6, 6, 6)
            trigger.Position = Vector3.new(10, 3, 0)
            trigger.Anchored = true
            trigger.CanCollide = false
            trigger.Parent = Workspace

            trigger.Touched:Connect(function(otherPart)
                trigger:SetAttribute("TouchCount",
                    (trigger:GetAttribute("TouchCount") or 0) + 1)
                trigger:SetAttribute("LastTouched", otherPart.Name)
            end)

            trigger.TouchEnded:Connect(function(otherPart)
                trigger:SetAttribute("EndedCount",
                    (trigger:GetAttribute("EndedCount") or 0) + 1)
            end)
        "#,
    );

    // Add player — spawns away from trigger
    let agent_id = Uuid::new_v4();
    assert!(instance.add_player(agent_id, "TestPlayer"));
    let hrp_id = *instance.player_hrp_ids.get(&agent_id).unwrap();

    // Run a few ticks to settle
    for _ in 0..10 {
        instance.tick();
    }

    // Verify no touches yet
    let touch_count = get_trigger_attr(&instance, "Trigger", "TouchCount");
    assert_eq!(touch_count, 0.0, "No touches before moving to trigger");

    // Move player into the trigger zone through the character controller.
    // Roblox Touched is contact-driven; direct CFrame/Position teleports
    // should not require a full-world overlap poll to synthesize touches.
    instance
        .physics
        .set_character_target(hrp_id, Some([10.0, 3.0, 0.0]));

    for _ in 0..180 {
        instance.tick();
        if get_trigger_attr(&instance, "Trigger", "TouchCount") >= 1.0 {
            break;
        }
    }

    let touch_count = get_trigger_attr(&instance, "Trigger", "TouchCount");
    assert!(
        touch_count >= 1.0,
        "Touched should have fired. Got count={}",
        touch_count
    );

    let last_touched = get_trigger_str_attr(&instance, "Trigger", "LastTouched");
    assert_eq!(
        last_touched, "HumanoidRootPart",
        "Touched arg should be the character's HRP"
    );

    // Now move player away from trigger.
    instance
        .physics
        .set_character_target(hrp_id, Some([50.0, 3.0, 0.0]));

    for _ in 0..240 {
        instance.tick();
        if get_trigger_attr(&instance, "Trigger", "EndedCount") >= 1.0 {
            break;
        }
    }

    let ended_count = get_trigger_attr(&instance, "Trigger", "EndedCount");
    assert!(
        ended_count >= 1.0,
        "TouchEnded should have fired. Got count={}",
        ended_count
    );
}

#[test]
fn test_moveto_reaches_non_collidable_trigger() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    instance.load_script(r#"
            local AgentInputService = game:GetService("AgentInputService")
            local floor = Instance.new("Part")
            floor.Name = "Floor"
            floor.Size = Vector3.new(200, 2, 200)
            floor.Position = Vector3.new(0, -1, 0)
            floor.Anchored = true
            floor.Parent = Workspace

            local trigger = Instance.new("Part")
            trigger.Name = "Trigger"
            trigger.Size = Vector3.new(6, 6, 6)
            trigger.Position = Vector3.new(0, 3, -30)
            trigger.Anchored = true
            trigger.CanCollide = false
            trigger.Parent = Workspace

            trigger.Touched:Connect(function(otherPart)
                trigger:SetAttribute("TouchCount",
                    (trigger:GetAttribute("TouchCount") or 0) + 1)
            end)

            if AgentInputService then
                AgentInputService.InputReceived:Connect(function(player, inputType, data)
                    if inputType == "MoveTo" and data and data.position then
                        local humanoid = player.Character and player.Character:FindFirstChild("Humanoid")
                        if humanoid then
                            local pos = data.position
                            humanoid:MoveTo(Vector3.new(pos[1], pos[2], pos[3]))
                        end
                    end
                end)
            end
        "#);

    let agent_id = Uuid::new_v4();
    assert!(instance.add_player(agent_id, "TestPlayer"));
    let hrp_id = *instance.player_hrp_ids.get(&agent_id).unwrap();
    let user_id = *instance.players.get(&agent_id).unwrap();

    for _ in 0..10 {
        instance.tick();
    }

    let start = instance.physics.get_character_position(hrp_id).unwrap();
    instance.queue_agent_input(
        user_id,
        "MoveTo".to_string(),
        serde_json::json!({"position": [0.0, start[1], -30.0]}),
    );

    for _ in 0..240 {
        instance.tick();
    }

    let final_pos = instance.physics.get_character_position(hrp_id).unwrap();
    let dx = final_pos[0] - 0.0;
    let dz = final_pos[2] - (-30.0);
    let dist_xz = (dx * dx + dz * dz).sqrt();
    assert!(
        dist_xz < 4.0,
        "MoveTo should reach the non-collidable trigger. Final pos={:?}, dist_xz={}",
        final_pos,
        dist_xz
    );

    let touch_count = get_trigger_attr(&instance, "Trigger", "TouchCount");
    assert!(
        touch_count >= 1.0,
        "Touched should fire when entering trigger via MoveTo. Got count={}",
        touch_count
    );
}

#[test]
fn test_touched_requires_both_can_touch() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    // Script: trigger with CanTouch=false — Touched should NOT fire
    instance.load_script(
        r#"
            local floor = Instance.new("Part")
            floor.Name = "Floor"
            floor.Size = Vector3.new(200, 1, 200)
            floor.Position = Vector3.new(0, 0, 0)
            floor.Anchored = true
            floor.Parent = Workspace

            local trigger = Instance.new("Part")
            trigger.Name = "Trigger"
            trigger.Size = Vector3.new(6, 6, 6)
            trigger.Position = Vector3.new(10, 3, 0)
            trigger.Anchored = true
            trigger.CanCollide = false
            trigger.CanTouch = false
            trigger.Parent = Workspace

            trigger.Touched:Connect(function(otherPart)
                trigger:SetAttribute("TouchCount",
                    (trigger:GetAttribute("TouchCount") or 0) + 1)
            end)
        "#,
    );

    let agent_id = Uuid::new_v4();
    assert!(instance.add_player(agent_id, "TestPlayer"));
    let hrp_id = *instance.player_hrp_ids.get(&agent_id).unwrap();

    for _ in 0..10 {
        instance.tick();
    }

    // Teleport into trigger
    instance
        .physics
        .set_character_position(hrp_id, [10.0, 3.0, 0.0]);

    for _ in 0..5 {
        instance.tick();
    }

    let touch_count = get_trigger_attr(&instance, "Trigger", "TouchCount");
    assert_eq!(
        touch_count, 0.0,
        "Touched should NOT fire when CanTouch=false. Got count={}",
        touch_count
    );
}

#[test]
fn test_touched_skips_anchored_anchored() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    // Two overlapping anchored parts — Roblox never fires Touched for these
    instance.load_script(
        r#"
            local partA = Instance.new("Part")
            partA.Name = "PartA"
            partA.Size = Vector3.new(4, 4, 4)
            partA.Position = Vector3.new(0, 2, 0)
            partA.Anchored = true
            partA.Parent = Workspace

            local partB = Instance.new("Part")
            partB.Name = "PartB"
            partB.Size = Vector3.new(4, 4, 4)
            partB.Position = Vector3.new(1, 2, 0)
            partB.Anchored = true
            partB.Parent = Workspace

            partA.Touched:Connect(function(otherPart)
                partA:SetAttribute("TouchCount",
                    (partA:GetAttribute("TouchCount") or 0) + 1)
            end)
        "#,
    );

    // No players needed — just tick to detect overlaps
    for _ in 0..10 {
        instance.tick();
    }

    let touch_count = get_trigger_attr(&instance, "PartA", "TouchCount");
    assert_eq!(
        touch_count, 0.0,
        "Anchored+Anchored should never fire Touched. Got count={}",
        touch_count
    );
}

#[test]
fn test_touch_tracking_requires_touch_interest() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    instance.load_script(
        r#"
            for i = 1, 25 do
                local part = Instance.new("Part")
                part.Name = "InertTouchPart_" .. i
                part.Size = Vector3.new(4, 4, 4)
                part.Position = Vector3.new(0, 6 + i * 0.01, 0)
                part.Anchored = false
                part.CanTouch = true
                part.Parent = Workspace
            end
        "#,
    );

    for _ in 0..20 {
        instance.tick();
    }

    assert!(
        instance.prev_touches.is_empty(),
        "Parts with no Touched/TouchEnded listeners should not create touch state"
    );
}

#[test]
fn test_touched_respects_physics_service_collision_groups() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    instance.load_script(
        r#"
            local PhysicsService = game:GetService("PhysicsService")

            PhysicsService:RegisterCollisionGroup("PlayersGroup")
            PhysicsService:RegisterCollisionGroup("TriggerGroup")
            PhysicsService:CollisionGroupSetCollidable("PlayersGroup", "TriggerGroup", false)

            local floor = Instance.new("Part")
            floor.Name = "Floor"
            floor.Size = Vector3.new(200, 1, 200)
            floor.Position = Vector3.new(0, 0, 0)
            floor.Anchored = true
            floor.Parent = Workspace

            local trigger = Instance.new("Part")
            trigger.Name = "Trigger"
            trigger.Size = Vector3.new(6, 6, 6)
            trigger.Position = Vector3.new(10, 3, 0)
            trigger.Anchored = true
            trigger.CanCollide = false
            trigger.Parent = Workspace
            PhysicsService:SetPartCollisionGroup(trigger, "TriggerGroup")

            trigger.Touched:Connect(function(otherPart)
                trigger:SetAttribute("TouchCount",
                    (trigger:GetAttribute("TouchCount") or 0) + 1)
            end)

            local mover = Instance.new("Part")
            mover.Name = "Mover"
            mover.Size = Vector3.new(4, 4, 4)
            mover.Position = Vector3.new(10, 3, 0)
            mover.Anchored = false
            mover.Parent = Workspace
            PhysicsService:SetPartCollisionGroup(mover, "PlayersGroup")
        "#,
    );

    for _ in 0..20 {
        instance.tick();
    }

    let touch_count = get_trigger_attr(&instance, "Trigger", "TouchCount");
    assert_eq!(
        touch_count, 0.0,
        "Touched should not fire for non-collidable collision groups"
    );
}

#[test]
fn test_physics_service_collision_groups_filter_solver_contacts() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    instance.load_script(
        r#"
            local PhysicsService = game:GetService("PhysicsService")

            PhysicsService:RegisterCollisionGroup("Ghost")
            PhysicsService:RegisterCollisionGroup("Ground")
            PhysicsService:CollisionGroupSetCollidable("Ghost", "Ground", false)

            local floor = Instance.new("Part")
            floor.Name = "FilteredFloor"
            floor.Size = Vector3.new(40, 1, 40)
            floor.Position = Vector3.new(0, 0, 0)
            floor.Anchored = true
            floor.CollisionGroup = "Ground"
            floor.Parent = Workspace

            local ghost = Instance.new("Part")
            ghost.Name = "GhostBody"
            ghost.Size = Vector3.new(2, 2, 2)
            ghost.Position = Vector3.new(0, 8, 0)
            ghost.Anchored = false
            ghost.CollisionGroup = "Ghost"
            ghost.Parent = Workspace

            PhysicsService:RenameCollisionGroup("Ghost", "Phantom")
        "#,
    );

    for _ in 0..90 {
        instance.tick();
    }

    let runtime = instance.lua_runtime.as_ref().unwrap();
    let descendants = runtime.workspace().get_descendants();
    let ghost_y = descendants
        .iter()
        .find_map(|inst| {
            let data = inst.data.lock().unwrap();
            if data.name == "GhostBody" {
                data.part_data.as_ref().map(|part| part.position.y)
            } else {
                None
            }
        })
        .expect("GhostBody should exist");

    assert!(
        ghost_y < -2.0,
        "GhostBody should fall through a floor in a non-collidable group; y={ghost_y}"
    );
}

#[test]
fn test_physics_service_collision_groups_filter_character_controller_contacts() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    instance.load_script(
        r#"
            local PhysicsService = game:GetService("PhysicsService")

            PhysicsService:RegisterCollisionGroup("Ghost")
            PhysicsService:RegisterCollisionGroup("Ground")
            PhysicsService:CollisionGroupSetCollidable("Ghost", "Ground", false)

            local floor = Instance.new("Part")
            floor.Name = "CharacterFilteredFloor"
            floor.Size = Vector3.new(80, 1, 80)
            floor.Position = Vector3.new(0, 0, 0)
            floor.Anchored = true
            floor.CollisionGroup = "Ground"
            floor.Parent = Workspace
        "#,
    );

    let agent_id = Uuid::new_v4();
    assert!(instance.add_player(agent_id, "GhostPlayer"));
    let hrp_id = *instance.player_hrp_ids.get(&agent_id).unwrap();

    let runtime = instance.lua_runtime.as_ref().unwrap();
    let ghost_group_id = runtime
        .game()
        .physics_service()
        .resolve_collision_group_id("Ghost")
        .expect("Ghost collision group should be registered");
    for inst in runtime.workspace().get_descendants() {
        let mut data = inst.data.lock().unwrap();
        if data.id.0 == hrp_id {
            let part = data.part_data.as_mut().expect("HRP should be a part");
            part.collision_group = "Ghost".to_string();
            part.collision_group_id = ghost_group_id;
            part.interaction_dirty = true;
        }
    }

    for _ in 0..90 {
        instance.tick();
    }

    let character_y = instance
        .physics
        .get_character_position(hrp_id)
        .expect("character should exist")[1];

    assert!(
        character_y < -2.0,
        "Character controller should fall through a floor in a non-collidable group; y={character_y}"
    );
}

#[test]
fn test_character_off_center_load_torques_dynamic_support() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    instance.load_script(
        r#"
            Workspace.Gravity = 30

            local support = Instance.new("Part")
            support.Name = "DynamicSupport"
            support.Size = Vector3.new(8, 1, 4)
            support.Position = Vector3.new(0, 0, 0)
            support.Anchored = false
            support.CanCollide = true
            support.CustomPhysicalProperties = PhysicalProperties.new(2, 0.8, 0, 1, 1)
            support.Parent = Workspace

            _G.support = support
        "#,
    );

    let agent_id = Uuid::new_v4();
    assert!(instance.add_player(agent_id, "LoadPlayer"));
    let hrp_id = *instance.player_hrp_ids.get(&agent_id).unwrap();
    instance
        .physics
        .set_character_position(hrp_id, [3.0, 3.0, 0.0]);

    for _ in 0..12 {
        instance.tick();
    }

    let angular_z: f64 = instance
        .lua_runtime
        .as_ref()
        .unwrap()
        .lua()
        .load("return _G.support.AssemblyAngularVelocity.Z")
        .eval()
        .unwrap();

    assert!(
        angular_z.abs() > 0.01,
        "off-center character load should torque dynamic support, angular_z={angular_z}"
    );
}

#[test]
fn test_sufficient_raft_floats_with_centered_character_load() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    instance.load_script(
        r#"
            Workspace.Gravity = 30
            Workspace.Terrain:FillBlock(CFrame.new(0, -8, 0), Vector3.new(64, 16, 64), Enum.Material.Water)

            local raft = Instance.new("Part")
            raft.Name = "LargeCharacterRaft"
            raft.Size = Vector3.new(12, 1, 4)
            raft.Position = Vector3.new(0, -0.2, 0)
            raft.Anchored = false
            raft.CanCollide = true
            raft.CustomPhysicalProperties = PhysicalProperties.new(0.35, 0.8, 0, 1, 1)
            raft.Parent = Workspace

            _G.raft = raft
        "#,
    );

    let agent_id = Uuid::new_v4();
    assert!(instance.add_player(agent_id, "CenteredRaftPlayer"));
    let hrp_id = *instance.player_hrp_ids.get(&agent_id).unwrap();
    instance
        .physics
        .set_character_position(hrp_id, [0.0, 3.4, 0.0]);

    for _ in 0..540 {
        instance.tick();
    }

    let (raft_y, raft_speed, player_y): (f64, f64, f64) = instance
        .lua_runtime
        .as_ref()
        .unwrap()
        .lua()
        .load(
            r#"
            local hrp = game:GetService("Players"):GetPlayers()[1].Character:FindFirstChild("HumanoidRootPart")
            return _G.raft.Position.Y, _G.raft.AssemblyLinearVelocity.Magnitude, hrp.Position.Y
        "#,
        )
        .eval()
        .unwrap();

    // Raft volume is 48. Raft mass is 16.8. The default HRP is a 2x5x2
    // cylinder at density 0.7: pi * 1^2 * 5 * 0.7 ~= 10.996.
    // Expected submerged fraction ~= 27.796 / 48 = 0.579, so center_y
    // should be about 0.5 - 0.579 = -0.079 for a height-1 box.
    assert!(
        (-0.35..=0.20).contains(&raft_y),
        "large raft with centered character should float near predicted draft, raft_y={raft_y}"
    );
    assert!(
        raft_speed < 0.5,
        "large raft with centered character should be mostly settled, speed={raft_speed}"
    );
    assert!(
        player_y > raft_y + 2.0,
        "character should remain supported above raft, player_y={player_y}, raft_y={raft_y}"
    );
}

#[test]
fn test_off_center_character_load_rolls_floating_raft() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    instance.load_script(
        r#"
            Workspace.Gravity = 30
            Workspace.Terrain:FillBlock(CFrame.new(0, -8, 0), Vector3.new(64, 16, 64), Enum.Material.Water)

            local raft = Instance.new("Part")
            raft.Name = "OffCenterCharacterRaft"
            raft.Size = Vector3.new(12, 1, 4)
            raft.Position = Vector3.new(0, -0.2, 0)
            raft.Anchored = false
            raft.CanCollide = true
            raft.CustomPhysicalProperties = PhysicalProperties.new(0.35, 0.8, 0, 1, 1)
            raft.Parent = Workspace

            _G.raft = raft
            _G.maxAngularZ = 0
            game:GetService("RunService").Heartbeat:Connect(function()
                _G.maxAngularZ = math.max(_G.maxAngularZ, math.abs(raft.AssemblyAngularVelocity.Z))
            end)
        "#,
    );

    let agent_id = Uuid::new_v4();
    assert!(instance.add_player(agent_id, "OffCenterRaftPlayer"));
    let hrp_id = *instance.player_hrp_ids.get(&agent_id).unwrap();
    instance
        .physics
        .set_character_position(hrp_id, [4.0, 3.4, 0.0]);

    for _ in 0..240 {
        instance.tick();
    }

    let max_angular_z: f64 = instance
        .lua_runtime
        .as_ref()
        .unwrap()
        .lua()
        .load("return _G.maxAngularZ")
        .eval()
        .unwrap();

    assert!(
        max_angular_z > 0.02,
        "off-center character load should roll floating raft, max angular z={max_angular_z}"
    );
}

#[test]
fn test_touched_reports_specific_part_in_welded_assembly() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    instance.load_script(
        r#"
            Workspace.Gravity = 0

            local left = Instance.new("Part")
            left.Name = "Left"
            left.Size = Vector3.new(1, 1, 1)
            left.Position = Vector3.new(0, 3, 0)
            left.Anchored = false
            left.CanCollide = false
            left.Parent = Workspace

            local right = Instance.new("Part")
            right.Name = "Right"
            right.Size = Vector3.new(1, 1, 1)
            right.Position = Vector3.new(2, 3, 0)
            right.Anchored = false
            right.CanCollide = false
            right.Parent = Workspace

            local weld = Instance.new("WeldConstraint")
            weld.Part0 = left
            weld.Part1 = right
            weld.Parent = Workspace

            local trigger = Instance.new("Part")
            trigger.Name = "Trigger"
            trigger.Size = Vector3.new(1.1, 1.1, 1.1)
            trigger.Position = Vector3.new(2, 3, 0)
            trigger.Anchored = true
            trigger.CanCollide = false
            trigger.Parent = Workspace

            trigger.Touched:Connect(function(otherPart)
                trigger:SetAttribute("LastTouched", otherPart.Name)
            end)
        "#,
    );

    for _ in 0..10 {
        instance.tick();
    }

    let last_touched = get_trigger_str_attr(&instance, "Trigger", "LastTouched");
    assert_eq!(
        last_touched, "Right",
        "compound assembly colliders must preserve the specific Lua part identity"
    );
}

#[test]
fn test_game_instance_serverscriptservice_require_smoke() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    // Root script creates module + server script in ServerScriptService.
    instance.load_script(
        r#"
            local module = Instance.new("ModuleScript")
            module.Name = "SharedModule"
            module.Source = [[
                _G.module_runs = (_G.module_runs or 0) + 1
                return { value = 123 }
            ]]
            module.Parent = ServerScriptService

            local boot = Instance.new("Script")
            boot.Name = "BootScript"
            boot.Source = [[
                local m = ServerScriptService:FindFirstChild("SharedModule")
                local data = require(m)

                local marker = Instance.new("Folder")
                marker.Name = "ServerScriptMarker"
                marker:SetAttribute("Ran", true)
                marker:SetAttribute("ModuleValue", data.value)
                marker.Parent = Workspace
            ]]
            boot.Parent = ServerScriptService
        "#,
    );

    for _ in 0..4 {
        instance.tick();
    }

    assert!(
        get_trigger_bool_attr(&instance, "ServerScriptMarker", "Ran"),
        "ServerScriptService script should have executed in real game loop"
    );
    assert_eq!(
        get_trigger_attr(&instance, "ServerScriptMarker", "ModuleValue"),
        123.0,
        "Script should read value from ModuleScript require()"
    );
}

#[test]
fn test_game_instance_waitforchild_smoke() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    instance.load_script(
        r#"
            local waiter = Instance.new("Script")
            waiter.Name = "WaiterScript"
            waiter.Source = [[
                local part = Workspace:WaitForChild("DelayedPart", 1.0)
                local marker = Instance.new("Folder")
                marker.Name = "WaitForChildMarker"
                marker:SetAttribute("Found", part ~= nil)
                marker.Parent = Workspace
            ]]
            waiter.Parent = ServerScriptService

            task.delay(0.05, function()
                local p = Instance.new("Part")
                p.Name = "DelayedPart"
                p.Parent = Workspace
            end)
        "#,
    );

    std::thread::sleep(std::time::Duration::from_millis(80));
    for _ in 0..6 {
        instance.tick();
    }

    assert!(
        get_trigger_bool_attr(&instance, "WaitForChildMarker", "Found"),
        "WaitForChild should resolve in game loop when child appears"
    );
}

fn welded_pair_positions_after_write(write_script: &str) -> (f32, f32) {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    instance.load_script(
        r#"
            Workspace.Gravity = 0

            local left = Instance.new("Part")
            left.Name = "Left"
            left.Position = Vector3.new(0, 5, 0)
            left.CanCollide = false
            left.Parent = Workspace

            local right = Instance.new("Part")
            right.Name = "Right"
            right.Position = Vector3.new(2, 5, 0)
            right.CanCollide = false
            right.Parent = Workspace
            _G.right = right

            local weld = Instance.new("WeldConstraint")
            weld.Part0 = left
            weld.Part1 = right
            weld.Parent = Workspace
        "#,
    );

    for _ in 0..5 {
        instance.tick();
    }

    let runtime = instance.lua_runtime.as_ref().unwrap();
    runtime
        .lua()
        .load(write_script)
        .exec()
        .expect("failed to teleport welded part");

    for _ in 0..60 {
        instance.tick();
    }

    let runtime = instance.lua_runtime.as_ref().unwrap();
    let descendants = runtime.workspace().get_descendants();
    let mut left_x = None;
    let mut right_x = None;
    for inst in &descendants {
        let data = inst.data.lock().unwrap();
        match data.name.as_str() {
            "Left" => {
                left_x = data.part_data.as_ref().map(|part| part.position.x);
            }
            "Right" => {
                right_x = data.part_data.as_ref().map(|part| part.position.x);
            }
            _ => {}
        }
    }

    let left_x = left_x.expect("Left part missing");
    let right_x = right_x.expect("Right part missing");
    (left_x, right_x)
}

#[test]
fn test_weld_constraint_position_write_recomputes_offset() {
    let (left_x, right_x) =
        welded_pair_positions_after_write("_G.right.Position = Vector3.new(8, 5, 0)");

    // Roblox weld parity: assigning Position to one welded part changes
    // that part's offset within the assembly. It does not drag the other
    // welded parts; scripts that want to move the whole assembly use CFrame.
    assert!(
        left_x.abs() < 0.25,
        "Position write should not drag Part0; got left_x={left_x}, right_x={right_x}"
    );
    assert!(
        (right_x - 8.0).abs() < 0.25,
        "Position write should move only Part1; got left_x={left_x}, right_x={right_x}"
    );
}

#[test]
fn test_weld_constraint_cframe_write_moves_assembly() {
    let (left_x, right_x) =
        welded_pair_positions_after_write("_G.right.CFrame = CFrame.new(8, 5, 0)");
    let spacing = right_x - left_x;

    assert!(
        (left_x - 6.0).abs() < 0.25,
        "CFrame write should move the whole assembly; got left_x={left_x}, right_x={right_x}"
    );
    assert!(
            (spacing - 2.0).abs() < 0.25,
            "CFrame write should preserve welded spacing; got left_x={left_x}, right_x={right_x}, spacing={spacing}"
        );
}

#[test]
fn test_weld_constraint_root_cframe_write_preserves_all_offsets() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    instance.load_script(
        r#"
            Workspace.Gravity = 0

            local a = Instance.new("Part")
            a.Name = "Root"
            a.Position = Vector3.new(0, 5, 0)
            a.CanCollide = false
            a.Parent = Workspace

            local b = Instance.new("Part")
            b.Name = "Middle"
            b.Position = Vector3.new(2, 5, 0)
            b.CanCollide = false
            b.Parent = Workspace

            local c = Instance.new("Part")
            c.Name = "End"
            c.Position = Vector3.new(4, 5, 0)
            c.CanCollide = false
            c.Parent = Workspace

            local ab = Instance.new("WeldConstraint")
            ab.Part0 = a
            ab.Part1 = b
            ab.Parent = Workspace

            local bc = Instance.new("WeldConstraint")
            bc.Part0 = b
            bc.Part1 = c
            bc.Parent = Workspace

            _G.a = a
            _G.b = b
            _G.c = c
        "#,
    );

    for _ in 0..10 {
        instance.tick();
    }

    let runtime = instance.lua_runtime.as_ref().unwrap();
    runtime
        .lua()
        .load("_G.a.CFrame = CFrame.new(10, 8, 0)")
        .exec()
        .unwrap();

    for _ in 0..10 {
        instance.tick();
    }

    let runtime = instance.lua_runtime.as_ref().unwrap();
    let (a_x, b_x, c_x, a_y, b_y, c_y): (f32, f32, f32, f32, f32, f32) = runtime
            .lua()
            .load("return _G.a.Position.X, _G.b.Position.X, _G.c.Position.X, _G.a.Position.Y, _G.b.Position.Y, _G.c.Position.Y")
            .eval()
            .unwrap();

    assert!(
            (a_x - 10.0).abs() < 0.01 && (b_x - 12.0).abs() < 0.01 && (c_x - 14.0).abs() < 0.01,
            "root CFrame write should move the whole assembly while preserving X offsets: a={a_x}, b={b_x}, c={c_x}"
        );
    assert!(
        (a_y - 8.0).abs() < 0.01 && (b_y - 8.0).abs() < 0.01 && (c_y - 8.0).abs() < 0.01,
        "root CFrame write should preserve Y offsets: a={a_y}, b={b_y}, c={c_y}"
    );
}

#[test]
fn test_weld_merge_with_lower_root_keeps_single_compound_body() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    instance.load_script(
        r#"
            Workspace.Gravity = 0

            local low = Instance.new("Part")
            low.Name = "LowRoot"
            low.Position = Vector3.new(0, 5, 0)
            low.CanCollide = false
            low.Parent = Workspace

            local mid = Instance.new("Part")
            mid.Name = "MidRoot"
            mid.Position = Vector3.new(2, 5, 0)
            mid.CanCollide = false
            mid.Parent = Workspace

            local high = Instance.new("Part")
            high.Name = "HighPart"
            high.Position = Vector3.new(4, 5, 0)
            high.CanCollide = false
            high.Parent = Workspace

            local first = Instance.new("WeldConstraint")
            first.Part0 = mid
            first.Part1 = high
            first.Parent = Workspace

            _G.low = low
            _G.mid = mid
            _G.high = high
        "#,
    );

    for _ in 0..5 {
        instance.tick();
    }

    let runtime = instance.lua_runtime.as_ref().unwrap();
    runtime
        .lua()
        .load(
            r#"
                local merge = Instance.new("WeldConstraint")
                merge.Part0 = _G.low
                merge.Part1 = _G.mid
                merge.Parent = Workspace
            "#,
        )
        .exec()
        .expect("failed to merge welded assemblies");

    for _ in 0..10 {
        instance.tick();
    }

    let low_id = {
        let runtime = instance.lua_runtime.as_ref().unwrap();
        let debug_id: String = runtime
            .lua()
            .load("return _G.low:GetDebugId()")
            .eval()
            .unwrap();
        u64::from_str_radix(debug_id.trim_start_matches("RBX"), 16).unwrap()
    };
    let mid_id = {
        let runtime = instance.lua_runtime.as_ref().unwrap();
        let debug_id: String = runtime
            .lua()
            .load("return _G.mid:GetDebugId()")
            .eval()
            .unwrap();
        u64::from_str_radix(debug_id.trim_start_matches("RBX"), 16).unwrap()
    };
    let high_id = {
        let runtime = instance.lua_runtime.as_ref().unwrap();
        let debug_id: String = runtime
            .lua()
            .load("return _G.high:GetDebugId()")
            .eval()
            .unwrap();
        u64::from_str_radix(debug_id.trim_start_matches("RBX"), 16).unwrap()
    };

    let low_handle = instance.physics.get_handle(low_id).unwrap();
    let mid_handle = instance.physics.get_handle(mid_id).unwrap();
    let high_handle = instance.physics.get_handle(high_id).unwrap();
    assert_eq!(
        low_handle, mid_handle,
        "merged Low/Mid should share one body"
    );
    assert_eq!(
        low_handle, high_handle,
        "merged Low/High should share one body"
    );

    for _ in 0..120 {
        instance.tick();
    }

    let runtime = instance.lua_runtime.as_ref().unwrap();
    let (low_x, mid_x, high_x): (f32, f32, f32) = runtime
        .lua()
        .load("return _G.low.Position.X, _G.mid.Position.X, _G.high.Position.X")
        .eval()
        .unwrap();
    assert!(
        (mid_x - low_x - 2.0).abs() < 0.01 && (high_x - mid_x - 2.0).abs() < 0.01,
        "merged assembly spacing changed: low={low_x}, mid={mid_x}, high={high_x}"
    );
}

#[test]
fn test_align_position_moves_part_to_attachment_target() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    instance.load_script(
        r#"
            Workspace.Gravity = 0

            local mover = Instance.new("Part")
            mover.Name = "Mover"
            mover.Position = Vector3.new(0, 5, 0)
            mover.CanCollide = false
            mover.Parent = Workspace

            local target = Instance.new("Part")
            target.Name = "Target"
            target.Anchored = true
            target.Position = Vector3.new(10, 8, 0)
            target.CanCollide = false
            target.Parent = Workspace

            local a0 = Instance.new("Attachment")
            a0.Parent = mover

            local a1 = Instance.new("Attachment")
            a1.Parent = target

            local align = Instance.new("AlignPosition")
            align.Attachment0 = a0
            align.Attachment1 = a1
            align.RigidityEnabled = true
            align.Parent = Workspace
        "#,
    );

    for _ in 0..10 {
        instance.tick();
    }

    let runtime = instance.lua_runtime.as_ref().unwrap();
    let descendants = runtime.workspace().get_descendants();
    let mut mover_pos = None;
    for inst in &descendants {
        let data = inst.data.lock().unwrap();
        if data.name == "Mover" {
            mover_pos = data.part_data.as_ref().map(|part| part.position);
        }
    }

    let mover_pos = mover_pos.expect("Mover part missing");
    assert!(
        (mover_pos.x - 10.0).abs() < 0.25 && (mover_pos.y - 8.0).abs() < 0.25,
        "AlignPosition should move the attachment parent to the target; got ({}, {}, {})",
        mover_pos.x,
        mover_pos.y,
        mover_pos.z
    );
}

#[test]
fn test_align_orientation_matches_target_attachment_rotation() {
    let mut instance = GameInstance::new(Uuid::new_v4(), None);

    instance.load_script(
        r#"
            Workspace.Gravity = 0

            local mover = Instance.new("Part")
            mover.Name = "Mover"
            mover.Position = Vector3.new(0, 5, 0)
            mover.CanCollide = false
            mover.Parent = Workspace

            local target = Instance.new("Part")
            target.Name = "Target"
            target.Anchored = true
            target.Position = Vector3.new(0, 5, 0)
            target.CFrame = CFrame.Angles(0, math.rad(90), 0)
            target.CanCollide = false
            target.Parent = Workspace

            local a0 = Instance.new("Attachment")
            a0.Parent = mover

            local a1 = Instance.new("Attachment")
            a1.Parent = target

            local align = Instance.new("AlignOrientation")
            align.Attachment0 = a0
            align.Attachment1 = a1
            align.RigidityEnabled = true
            align.Parent = Workspace
        "#,
    );

    for _ in 0..10 {
        instance.tick();
    }

    let runtime = instance.lua_runtime.as_ref().unwrap();
    let descendants = runtime.workspace().get_descendants();
    let mut mover_look_x = None;
    let mut mover_look_z = None;
    for inst in &descendants {
        let data = inst.data.lock().unwrap();
        if data.name == "Mover" {
            if let Some(part) = data.part_data.as_ref() {
                mover_look_x = Some(-part.cframe.rotation[0][2]);
                mover_look_z = Some(-part.cframe.rotation[2][2]);
            }
        }
    }

    let mover_look_x = mover_look_x.expect("Mover part missing");
    let mover_look_z = mover_look_z.expect("Mover part missing");
    assert!(
        mover_look_x < -0.7 && mover_look_z.abs() < 0.3,
        "AlignOrientation should rotate the part toward the target attachment; got look=({}, {})",
        mover_look_x,
        mover_look_z
    );
}

/// Helper: get a numeric attribute from a named part in workspace
fn get_trigger_attr(instance: &GameInstance, name: &str, attr: &str) -> f64 {
    let runtime = instance.lua_runtime.as_ref().unwrap();
    let descendants = runtime.workspace().get_descendants();
    for inst in &descendants {
        let data = inst.data.lock().unwrap();
        if data.name == name {
            if let Some(AttributeValue::Number(v)) = data.attributes.get(attr) {
                return *v;
            }
            return 0.0;
        }
    }
    0.0
}

/// Helper: get a string attribute from a named part in workspace
fn get_trigger_str_attr(instance: &GameInstance, name: &str, attr: &str) -> String {
    let runtime = instance.lua_runtime.as_ref().unwrap();
    let descendants = runtime.workspace().get_descendants();
    for inst in &descendants {
        let data = inst.data.lock().unwrap();
        if data.name == name {
            if let Some(AttributeValue::String(v)) = data.attributes.get(attr) {
                return v.clone();
            }
            return String::new();
        }
    }
    String::new()
}

/// Helper: get a bool attribute from a named part in workspace
fn get_trigger_bool_attr(instance: &GameInstance, name: &str, attr: &str) -> bool {
    let runtime = instance.lua_runtime.as_ref().unwrap();
    let descendants = runtime.workspace().get_descendants();
    for inst in &descendants {
        let data = inst.data.lock().unwrap();
        if data.name == name {
            if let Some(AttributeValue::Bool(v)) = data.attributes.get(attr) {
                return *v;
            }
            return false;
        }
    }
    false
}
