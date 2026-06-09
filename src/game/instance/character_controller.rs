use super::super::physics::PhysicsWorld;

/// ControllerManager-like static configuration for character locomotion.
#[derive(Debug, Clone, Copy)]
pub struct ControllerManagerConfig {
    pub ground_query_distance: f32,
    pub platform_stick_distance: f32,
}

/// GroundSensor-like sample taken each frame for a character body.
#[derive(Debug, Clone, Copy)]
pub struct GroundSensorSample {
    pub grounded: bool,
    pub support_velocity: Option<[f32; 3]>,
    pub support_distance: Option<f32>,
}

/// GroundController output used by movement planning.
#[derive(Debug, Clone, Copy)]
pub struct GroundControllerDecision {
    pub carry_by_platform: bool,
    pub platform_velocity: Option<[f32; 3]>,
}

/// Sample ground support directly below the character.
pub fn sample_ground_sensor(
    physics: &PhysicsWorld,
    character_id: u64,
    grounded: bool,
    config: ControllerManagerConfig,
) -> GroundSensorSample {
    if let Some(support) =
        physics.get_ground_kinematic_support(character_id, config.ground_query_distance)
    {
        GroundSensorSample {
            grounded,
            support_velocity: Some(support.velocity),
            support_distance: Some(support.distance),
        }
    } else if let Some(support) =
        physics.get_ground_dynamic_support(character_id, config.ground_query_distance)
    {
        GroundSensorSample {
            grounded,
            support_velocity: physics
                .get_velocity_at_position_for_part(support.lua_id, support.point),
            support_distance: Some(support.distance),
        }
    } else {
        GroundSensorSample {
            grounded,
            support_velocity: None,
            support_distance: None,
        }
    }
}

/// Evaluate grounded/carry behavior from ground sensor data.
pub fn evaluate_ground_controller(
    sample: GroundSensorSample,
    config: ControllerManagerConfig,
) -> GroundControllerDecision {
    let near_support = sample
        .support_distance
        .map(|d| d <= config.platform_stick_distance)
        .unwrap_or(false);
    let carry_by_platform = sample.grounded || near_support;

    GroundControllerDecision {
        carry_by_platform,
        platform_velocity: sample.support_velocity,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::lua::types::PartType;
    use crate::game::physics::PhysicsWorld;

    #[test]
    fn dynamic_support_reports_platform_velocity_for_character_carry() {
        let mut physics = PhysicsWorld::new();
        let platform = physics.add_part(
            1,
            [0.0, 0.0, 0.0],
            &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [10.0, 1.0, 10.0],
            false,
            true,
            true,
            true,
            PartType::Block,
            None,
        );
        physics.set_velocity(platform, [0.0, 0.0, 4.0]);

        let character_id = 100;
        physics.add_character(character_id, [0.0, 3.0, 0.0], 1.0, 5.0);
        physics.query_pipeline.update(&physics.collider_set);

        let config = ControllerManagerConfig {
            ground_query_distance: 4.0,
            platform_stick_distance: 4.0,
        };
        let sample = sample_ground_sensor(&physics, character_id, false, config);
        let decision = evaluate_ground_controller(sample, config);

        assert!(decision.carry_by_platform);
        let velocity = decision
            .platform_velocity
            .expect("dynamic support should provide point velocity");
        assert!(
            velocity[2] > 3.9,
            "expected dynamic platform z velocity, got {velocity:?}"
        );
    }
}
