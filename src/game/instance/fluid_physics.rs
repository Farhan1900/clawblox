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
    density: f32,
}

#[derive(Debug, Clone, Copy)]
struct FluidSample {
    local: Vector3,
    world: Vector3,
    volume: f32,
    vertical_span: f32,
}

impl GameInstance {
    pub(super) fn apply_fluid_forces(&mut self, dt: f32) {
        let Some(runtime) = &self.lua_runtime else {
            return;
        };
        if dt <= f32::EPSILON {
            return;
        }

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
                let submerged_fraction =
                    water_submerged_fraction(sample.world, sample.vertical_span, &water);
                if submerged_fraction <= 0.0 {
                    continue;
                }

                let displaced_volume = sample.volume * submerged_fraction;
                let buoyancy = WATER_DENSITY * gravity * displaced_volume;
                let mut force = [0.0, buoyancy, 0.0];

                if let Some(velocity) = self
                    .physics
                    .get_velocity_at_position_for_part(part.lua_id, sample.world.to_array())
                {
                    let speed = vector_length(velocity);
                    let drag_scale = displaced_volume
                        * (LINEAR_WATER_DRAG + QUADRATIC_WATER_DRAG * speed).max(0.0);
                    // Drag is explicitly integrated (computed from pre-step
                    // velocity, applied for the whole step), so it must be
                    // capped to stay dissipative: limit the drag impulse to
                    // the momentum of this sample's share of the part mass.
                    // It can then stop local motion within one step but never
                    // reverse it; uncapped, any submerged part moving faster
                    // than ~2 * density / (drag_coeff * dt) overshoots,
                    // flips its velocity with gain each tick, and explodes.
                    let sample_mass = (part.density * sample.volume).max(1e-6);
                    let drag_scale = drag_scale.min(sample_mass / dt);
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
            let (lua_id, anchored, size, shape, density) = {
                let data = instance.data.lock().unwrap();
                let Some(part) = data.part_data.as_ref() else {
                    continue;
                };
                (
                    data.id.0,
                    part.anchored,
                    part.size,
                    part.shape,
                    part.effective_physical_properties().density,
                )
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
                density,
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

fn water_submerged_fraction(
    point: Vector3,
    vertical_span: f32,
    water: &HashMap<TerrainCell, WaterVoxel>,
) -> f32 {
    let span = vertical_span.abs().max(0.001);
    let bottom = point.y - span * 0.5;
    let top = point.y + span * 0.5;
    let cell_x = (point.x / TERRAIN_VOXEL_SIZE).floor() as i32;
    let cell_z = (point.z / TERRAIN_VOXEL_SIZE).floor() as i32;
    let min_y = (bottom / TERRAIN_VOXEL_SIZE).floor() as i32;
    let max_y = (top / TERRAIN_VOXEL_SIZE).floor() as i32;

    let mut submerged = 0.0;
    for y in min_y..=max_y {
        let cell = TerrainCell {
            x: cell_x,
            y,
            z: cell_z,
        };
        let Some(voxel) = water.get(&cell) else {
            continue;
        };
        if point.x < voxel.min.x
            || point.x > voxel.max.x
            || point.z < voxel.min.z
            || point.z > voxel.max.z
        {
            continue;
        }
        let overlap_bottom = bottom.max(voxel.min.y);
        let overlap_top = top.min(voxel.max.y);
        if overlap_top > overlap_bottom {
            submerged += overlap_top - overlap_bottom;
        }
    }
    (submerged / span).clamp(0.0, 1.0)
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
    let local_cell_size = Vector3::new(
        size.x.abs() / counts[0] as f32,
        size.y.abs() / counts[1] as f32,
        size.z.abs() / counts[2] as f32,
    );
    locals
        .into_iter()
        .map(|local| FluidSample {
            local,
            world: Vector3::zero(),
            volume,
            vertical_span: local_cell_size.y.max(0.001),
        })
        .collect()
}

fn fluid_samples_world(part: &FluidPart) -> Vec<FluidSample> {
    let counts = sample_counts(part.size);
    let local_cell_size = Vector3::new(
        part.size.x.abs() / counts[0] as f32,
        part.size.y.abs() / counts[1] as f32,
        part.size.z.abs() / counts[2] as f32,
    );
    let world_vertical_span = (part.rotation[1][0] * local_cell_size.x).abs()
        + (part.rotation[1][1] * local_cell_size.y).abs()
        + (part.rotation[1][2] * local_cell_size.z).abs();

    fluid_samples(part.size, part.shape)
        .into_iter()
        .map(|mut sample| {
            sample.world = transform_local_to_world(part.position, &part.rotation, sample.local);
            sample.vertical_span = world_vertical_span.max(0.001);
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

    #[test]
    fn terrain_water_empty_wood_raft_settles_to_archimedes_draft() {
        let mut instance = GameInstance::new(Uuid::new_v4(), None);
        instance.load_script(
            r#"
            local waterSurfaceY = 0
            Workspace.Terrain:FillBlock(CFrame.new(0, waterSurfaceY - 8, 0), Vector3.new(48, 16, 48), Enum.Material.Water)

            local raft = Instance.new("Part")
            raft.Name = "DraftRaft"
            raft.Size = Vector3.new(10, 1, 4)
            raft.CustomPhysicalProperties = PhysicalProperties.new(0.35, 0.7, 0.0, 1, 1)
            raft.Position = Vector3.new(0, -0.7, 0)
            raft.CanCollide = false
            raft.Parent = Workspace

            _G.raft = raft
        "#,
        );

        for _ in 0..420 {
            instance.tick();
        }

        let (center_y, speed): (f64, f64) = instance
            .lua_runtime
            .as_ref()
            .unwrap()
            .lua()
            .load("return _G.raft.Position.Y, _G.raft.AssemblyLinearVelocity.Magnitude")
            .eval()
            .unwrap();

        // For an axis-aligned box with density 0.35 in water density 1.0,
        // submerged fraction should be 0.35. With height 1, center is
        // water + (0.5 - 0.35) * height = 0.15.
        assert!(
            (center_y - 0.15).abs() < 0.15,
            "empty wood raft draft should settle near center_y=0.15, got {center_y}"
        );
        assert!(
            speed < 0.35,
            "empty wood raft should be near equilibrium, speed={speed}"
        );
    }

    #[test]
    fn terrain_water_load_equivalent_raft_settles_deeper_but_floats() {
        let mut instance = GameInstance::new(Uuid::new_v4(), None);
        instance.load_script(
            r#"
            local waterSurfaceY = 0
            Workspace.Terrain:FillBlock(CFrame.new(0, waterSurfaceY - 8, 0), Vector3.new(64, 16, 64), Enum.Material.Water)

            local raft = Instance.new("Part")
            raft.Name = "LoadedDraftRaft"
            raft.Size = Vector3.new(12, 1, 4)
            -- Equivalent to a wood raft carrying centered additional load:
            -- total supported mass / raft volume = 0.70, still below water.
            raft.CustomPhysicalProperties = PhysicalProperties.new(0.70, 0.7, 0.0, 1, 1)
            raft.Position = Vector3.new(0, -0.9, 0)
            raft.CanCollide = false
            raft.Parent = Workspace

            _G.raft = raft
        "#,
        );

        for _ in 0..480 {
            instance.tick();
        }

        let (center_y, speed): (f64, f64) = instance
            .lua_runtime
            .as_ref()
            .unwrap()
            .lua()
            .load("return _G.raft.Position.Y, _G.raft.AssemblyLinearVelocity.Magnitude")
            .eval()
            .unwrap();

        // Submerged fraction 0.70, height 1: center_y = 0.5 - 0.70 = -0.20.
        assert!(
            (center_y + 0.20).abs() < 0.18,
            "loaded raft should settle near center_y=-0.20, got {center_y}"
        );
        assert!(
            speed < 0.4,
            "loaded raft should be near equilibrium, speed={speed}"
        );
    }

    #[test]
    fn terrain_water_overloaded_raft_sinks() {
        let mut instance = GameInstance::new(Uuid::new_v4(), None);
        instance.load_script(
            r#"
            Workspace.Terrain:FillBlock(CFrame.new(0, -8, 0), Vector3.new(48, 16, 48), Enum.Material.Water)

            local raft = Instance.new("Part")
            raft.Name = "OverloadedRaft"
            raft.Size = Vector3.new(10, 1, 4)
            raft.CustomPhysicalProperties = PhysicalProperties.new(1.15, 0.7, 0.0, 1, 1)
            raft.Position = Vector3.new(0, -0.5, 0)
            raft.CanCollide = false
            raft.Parent = Workspace

            _G.raft = raft
            _G.startY = raft.Position.Y
        "#,
        );

        for _ in 0..180 {
            instance.tick();
        }

        let (start_y, end_y): (f64, f64) = instance
            .lua_runtime
            .as_ref()
            .unwrap()
            .lua()
            .load("return _G.startY, _G.raft.Position.Y")
            .eval()
            .unwrap();
        assert!(
            end_y < start_y - 0.5,
            "density > water should sink: start={start_y}, end={end_y}"
        );
    }

    #[test]
    fn terrain_water_floats_welded_wood_raft_assembly() {
        let mut instance = GameInstance::new(Uuid::new_v4(), None);
        instance.load_script(
            r#"
            local waterSurfaceY = 0
            Workspace.Terrain:FillBlock(CFrame.new(0, waterSurfaceY - 8, 0), Vector3.new(48, 16, 48), Enum.Material.Water)

            local function plank(name, x)
                local p = Instance.new("Part")
                p.Name = name
                p.Size = Vector3.new(5.25, 0.35, 0.9)
                p.Material = Enum.Material.Wood
                p.Position = Vector3.new(x, -4, 0)
                p.CanCollide = true
                p.Parent = Workspace
                return p
            end

            local root = plank("RaftPlankRoot", -1.9)
            local middle = plank("RaftPlankMiddle", 0)
            local right = plank("RaftPlankRight", 1.9)

            local w1 = Instance.new("WeldConstraint")
            w1.Part0 = root
            w1.Part1 = middle
            w1.Parent = root

            local w2 = Instance.new("WeldConstraint")
            w2.Part0 = middle
            w2.Part1 = right
            w2.Parent = middle

            _G.root = root
            _G.startY = root.Position.Y
            _G.waterSurfaceY = waterSurfaceY
        "#,
        );

        for _ in 0..180 {
            instance.tick();
        }

        let (connected_count, start_y, end_y, water_surface_y, speed): (i64, f64, f64, f64, f64) =
            instance
                .lua_runtime
                .as_ref()
                .unwrap()
                .lua()
                .load(
                    r#"
                return #_G.root:GetConnectedParts(true),
                    _G.startY,
                    _G.root.Position.Y,
                    _G.waterSurfaceY,
                    _G.root.AssemblyLinearVelocity.Magnitude
            "#,
                )
                .eval()
                .unwrap();

        assert_eq!(
            connected_count, 2,
            "raft should remain a welded three-part assembly"
        );
        assert!(
            end_y > start_y,
            "wood raft should rise from its initial submerged position: start={start_y}, end={end_y}"
        );
        assert!(
            (water_surface_y - 0.75..=water_surface_y + 1.5).contains(&end_y),
            "wood raft should settle near the water surface ({water_surface_y}), got y={end_y}"
        );
        assert!(
            speed < 1.0,
            "wood raft should be mostly settled after simulation, speed={speed}"
        );
    }
}
