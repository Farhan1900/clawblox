use std::collections::HashMap;

use crate::game::lua::instance::{Instance, TerrainCell, TerrainVoxel, TERRAIN_VOXEL_SIZE};
use crate::game::lua::types::{PartType, Vector3};

use super::GameInstance;

const WATER_DENSITY: f32 = 1.0;
// Parity note: Roblox does not expose exact Terrain water drag coefficients.
// These coefficients are engine calibration values for observable buoyancy and
// damping behavior, not a claim of exact Roblox water parity.
const LINEAR_WATER_DRAG: f32 = 8.0;
const QUADRATIC_WATER_DRAG: f32 = 0.9;
const MAX_FLUID_FORCE_PER_SAMPLE: f32 = 1_000_000.0;

#[derive(Debug, Clone, Copy)]
struct WaterVoxel {
    min: Vector3,
    max: Vector3,
}

#[derive(Debug, Clone)]
struct FluidPart {
    lua_id: u64,
    size: Vector3,
    shape: PartType,
    position: [f32; 3],
    rotation: [[f32; 3]; 3],
}

#[derive(Debug, Clone, Copy)]
struct FluidSample {
    local: Vector3,
    world: Vector3,
    volume: f32,
}

impl GameInstance {
    pub(super) fn apply_fluid_forces(&mut self) {
        let Some(runtime) = &self.lua_runtime else {
            return;
        };

        let water = collect_water_voxels(&runtime.workspace().get_descendants());
        if water.is_empty() {
            return;
        }

        let gravity = runtime.workspace().data.lock().unwrap().gravity.abs();

        let parts = self.collect_fluid_parts(runtime.workspace().get_descendants());
        for part in parts {
            let samples = fluid_samples_world(&part);
            if samples.is_empty() {
                continue;
            }

            for sample in samples {
                if !water_contains_point(sample.world, &water) {
                    continue;
                }

                let displaced_volume = sample.volume;
                let buoyancy = WATER_DENSITY * gravity * displaced_volume;
                let mut force = [0.0, buoyancy, 0.0];

                if let Some(velocity) = self
                    .physics
                    .get_velocity_at_position_for_part(part.lua_id, sample.world.to_array())
                {
                    let speed = vector_length(velocity);
                    let drag_scale = displaced_volume
                        * (LINEAR_WATER_DRAG + QUADRATIC_WATER_DRAG * speed).max(0.0);
                    force[0] -= velocity[0] * drag_scale;
                    force[1] -= velocity[1] * drag_scale;
                    force[2] -= velocity[2] * drag_scale;
                }

                if !force.iter().all(|value| value.is_finite()) {
                    continue;
                }
                clamp_force(&mut force);
                self.physics.add_force_at_position_to_part(
                    part.lua_id,
                    force,
                    sample.world.to_array(),
                );
            }
        }
    }

    fn collect_fluid_parts(&self, descendants: Vec<Instance>) -> Vec<FluidPart> {
        let mut out = Vec::new();
        for instance in descendants {
            let (lua_id, anchored, size, shape) = {
                let data = instance.data.lock().unwrap();
                let Some(part) = data.part_data.as_ref() else {
                    continue;
                };
                (data.id.0, part.anchored, part.size, part.shape)
            };
            if anchored || !self.physics.has_part(lua_id) {
                continue;
            }
            let Some(position) = self.physics.get_part_position(lua_id) else {
                continue;
            };
            let Some(rotation) = self.physics.get_part_rotation_matrix(lua_id) else {
                continue;
            };
            out.push(FluidPart {
                lua_id,
                size,
                shape,
                position,
                rotation,
            });
        }
        out
    }
}

fn collect_water_voxels(descendants: &[Instance]) -> HashMap<TerrainCell, WaterVoxel> {
    let mut out = HashMap::new();
    for instance in descendants {
        let voxels = {
            let data = instance.data.lock().unwrap();
            data.terrain_data
                .as_ref()
                .map(|terrain| terrain.water_voxels().collect::<Vec<_>>())
        };
        let Some(voxels) = voxels else {
            continue;
        };
        for (cell, voxel) in voxels {
            if let Some(water) = water_voxel_from_cell(cell, voxel) {
                out.insert(cell, water);
            }
        }
    }
    out
}

fn water_voxel_from_cell(cell: TerrainCell, voxel: TerrainVoxel) -> Option<WaterVoxel> {
    let liquid = voxel.liquid_occupancy.clamp(0.0, 1.0);
    if liquid <= 0.0 {
        return None;
    }
    let min = cell.min();
    let max = cell.max();
    Some(WaterVoxel {
        min,
        max: Vector3::new(max.x, min.y + TERRAIN_VOXEL_SIZE * liquid, max.z),
    })
}

fn water_contains_point(point: Vector3, water: &HashMap<TerrainCell, WaterVoxel>) -> bool {
    let cell = TerrainCell {
        x: (point.x / TERRAIN_VOXEL_SIZE).floor() as i32,
        y: (point.y / TERRAIN_VOXEL_SIZE).floor() as i32,
        z: (point.z / TERRAIN_VOXEL_SIZE).floor() as i32,
    };
    let Some(voxel) = water.get(&cell) else {
        return false;
    };
    point.x >= voxel.min.x
        && point.x <= voxel.max.x
        && point.y >= voxel.min.y
        && point.y <= voxel.max.y
        && point.z >= voxel.min.z
        && point.z <= voxel.max.z
}

fn fluid_samples(size: Vector3, shape: PartType) -> Vec<FluidSample> {
    let counts = sample_counts(size);
    let mut locals = Vec::new();

    for ix in 0..counts[0] {
        for iy in 0..counts[1] {
            for iz in 0..counts[2] {
                let local = Vector3::new(
                    local_sample_coord(ix, counts[0], size.x),
                    local_sample_coord(iy, counts[1], size.y),
                    local_sample_coord(iz, counts[2], size.z),
                );
                if !sample_inside_shape(local, size, shape) {
                    continue;
                }
                locals.push(local);
            }
        }
    }
    if locals.is_empty() {
        return Vec::new();
    }

    let volume = part_volume(size, shape) / locals.len() as f32;
    locals
        .into_iter()
        .map(|local| FluidSample {
            local,
            world: Vector3::zero(),
            volume,
        })
        .collect()
}

fn fluid_samples_world(part: &FluidPart) -> Vec<FluidSample> {
    fluid_samples(part.size, part.shape)
        .into_iter()
        .map(|mut sample| {
            sample.world = transform_local_to_world(part.position, &part.rotation, sample.local);
            sample
        })
        .collect()
}

fn sample_counts(size: Vector3) -> [usize; 3] {
    [
        sample_count_axis(size.x),
        sample_count_axis(size.y),
        sample_count_axis(size.z),
    ]
}

fn sample_count_axis(size: f32) -> usize {
    ((size.abs() / 2.0).ceil() as usize).clamp(2, 6)
}

fn local_sample_coord(index: usize, count: usize, size: f32) -> f32 {
    -size / 2.0 + (index as f32 + 0.5) * size / count as f32
}

fn part_volume(size: Vector3, shape: PartType) -> f32 {
    match shape {
        PartType::Block => size.x * size.y * size.z,
        PartType::Wedge => size.x * size.y * size.z * 0.5,
        PartType::Ball => {
            let radius = size.x.min(size.y).min(size.z) / 2.0;
            4.0 / 3.0 * std::f32::consts::PI * radius * radius * radius
        }
        PartType::Cylinder => {
            let radius = size.x.min(size.z) / 2.0;
            std::f32::consts::PI * radius * radius * size.y
        }
        PartType::Ring => size.x * size.y * size.z,
    }
    .max(0.0)
}

fn sample_inside_shape(local: Vector3, size: Vector3, shape: PartType) -> bool {
    match shape {
        PartType::Block | PartType::Ring => true,
        PartType::Ball => {
            let radius = size.x.min(size.y).min(size.z) / 2.0;
            local.x * local.x + local.y * local.y + local.z * local.z <= radius * radius
        }
        PartType::Cylinder => {
            let radius = size.x.min(size.z) / 2.0;
            local.x * local.x + local.z * local.z <= radius * radius
        }
        PartType::Wedge => {
            let half_x = size.x / 2.0;
            let y_top = size.y / 2.0;
            let y_bottom = -size.y / 2.0;
            let slope_alpha = ((local.x + half_x) / size.x).clamp(0.0, 1.0);
            let max_y = y_bottom + slope_alpha * size.y;
            local.y >= y_bottom && local.y <= max_y.min(y_top)
        }
    }
}

fn transform_local_to_world(
    position: [f32; 3],
    rotation: &[[f32; 3]; 3],
    local: Vector3,
) -> Vector3 {
    Vector3::new(
        position[0]
            + rotation[0][0] * local.x
            + rotation[0][1] * local.y
            + rotation[0][2] * local.z,
        position[1]
            + rotation[1][0] * local.x
            + rotation[1][1] * local.y
            + rotation[1][2] * local.z,
        position[2]
            + rotation[2][0] * local.x
            + rotation[2][1] * local.y
            + rotation[2][2] * local.z,
    )
}

fn vector_length(value: [f32; 3]) -> f32 {
    (value[0] * value[0] + value[1] * value[1] + value[2] * value[2]).sqrt()
}

fn clamp_force(force: &mut [f32; 3]) {
    for value in force {
        *value = value.clamp(-MAX_FLUID_FORCE_PER_SAMPLE, MAX_FLUID_FORCE_PER_SAMPLE);
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::GameInstance;

    #[test]
    fn terrain_water_buoyancy_lifts_low_density_part() {
        let mut instance = GameInstance::new(Uuid::new_v4(), None);
        instance.load_script(
            r#"
            Workspace.Terrain:FillBlock(CFrame.new(0, 4, 0), Vector3.new(32, 8, 32), Enum.Material.Water)

            local p = Instance.new("Part")
            p.Name = "FloatPart"
            p.Size = Vector3.new(4, 4, 4)
            p.Material = Enum.Material.Wood
            p.Position = Vector3.new(0, 2, 0)
            p.CanCollide = false
            p.Parent = Workspace

            _G.p = p
            _G.startY = p.Position.Y
        "#,
        );

        for _ in 0..45 {
            instance.tick();
        }

        let (start_y, end_y): (f64, f64) = instance
            .lua_runtime
            .as_ref()
            .unwrap()
            .lua()
            .load("return _G.startY, _G.p.Position.Y")
            .eval()
            .unwrap();
        assert!(
            end_y > start_y + 0.25,
            "wood part should rise in water: start={start_y}, end={end_y}"
        );
    }

    #[test]
    fn terrain_water_does_not_float_dense_metal_part() {
        let mut instance = GameInstance::new(Uuid::new_v4(), None);
        instance.load_script(
            r#"
            Workspace.Terrain:FillBlock(CFrame.new(0, 4, 0), Vector3.new(32, 8, 32), Enum.Material.Water)

            local p = Instance.new("Part")
            p.Name = "SinkPart"
            p.Size = Vector3.new(4, 4, 4)
            p.Material = Enum.Material.Metal
            p.Position = Vector3.new(0, 6, 0)
            p.CanCollide = false
            p.Parent = Workspace

            _G.p = p
            _G.startY = p.Position.Y
        "#,
        );

        for _ in 0..45 {
            instance.tick();
        }

        let (start_y, end_y): (f64, f64) = instance
            .lua_runtime
            .as_ref()
            .unwrap()
            .lua()
            .load("return _G.startY, _G.p.Position.Y")
            .eval()
            .unwrap();
        assert!(
            end_y < start_y - 0.25,
            "metal part should sink in water: start={start_y}, end={end_y}"
        );
    }

    #[test]
    fn terrain_water_drag_damps_submerged_motion() {
        let mut instance = GameInstance::new(Uuid::new_v4(), None);
        instance.load_script(
            r#"
            Workspace.Gravity = 0
            Workspace.Terrain:FillBlock(CFrame.new(0, 4, 0), Vector3.new(64, 8, 32), Enum.Material.Water)

            local p = Instance.new("Part")
            p.Name = "DragPart"
            p.Size = Vector3.new(4, 4, 4)
            p.Position = Vector3.new(0, 2, 0)
            p.CanCollide = false
            p.AssemblyLinearVelocity = Vector3.new(20, 0, 0)
            p.Parent = Workspace

            _G.p = p
        "#,
        );

        for _ in 0..30 {
            instance.tick();
        }

        let vx: f64 = instance
            .lua_runtime
            .as_ref()
            .unwrap()
            .lua()
            .load("return _G.p.AssemblyLinearVelocity.X")
            .eval()
            .unwrap();
        assert!(
            vx > 0.0 && vx < 20.0,
            "water drag should reduce submerged horizontal speed, got {vx}"
        );
    }
}
