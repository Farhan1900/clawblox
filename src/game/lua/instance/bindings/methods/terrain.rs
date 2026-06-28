use mlua::{Table, UserDataMethods, Value};

use crate::game::lua::instance::{Instance, TerrainCell, TERRAIN_VOXEL_SIZE};
use crate::game::lua::types::{CFrame, Material, Region3, Vector3};

pub(super) fn add_methods<M: UserDataMethods<Instance>>(methods: &mut M) {
    methods.add_method(
        "FillBlock",
        |_, this, (cframe, size, material): (CFrame, Vector3, Material)| -> mlua::Result<()> {
            let mut data = this.data.lock().unwrap();
            let Some(terrain) = data.terrain_data.as_mut() else {
                return Err(mlua::Error::runtime(
                    "FillBlock is only supported on Terrain",
                ));
            };

            terrain.fill_block(cframe, size, material);
            Ok(())
        },
    );

    methods.add_method(
        "ReadVoxelChannels",
        |lua, this, (region, resolution, channel_ids): (Region3, f32, Table)| {
            validate_terrain_resolution(resolution)?;
            let (min_cell, dims) = terrain_region_cells(region)?;
            let channel_ids = read_channel_ids(channel_ids)?;
            let data = this.data.lock().unwrap();
            let Some(terrain) = data.terrain_data.as_ref() else {
                return Err(mlua::Error::runtime(
                    "ReadVoxelChannels is only supported on Terrain",
                ));
            };

            let out = lua.create_table()?;
            out.set(
                "Size",
                Vector3::new(dims[0] as f32, dims[1] as f32, dims[2] as f32),
            )?;

            for channel in channel_ids {
                let channel_table = lua.create_table()?;
                channel_table.set(
                    "Size",
                    Vector3::new(dims[0] as f32, dims[1] as f32, dims[2] as f32),
                )?;

                for x in 0..dims[0] {
                    let x_table = lua.create_table()?;
                    for y in 0..dims[1] {
                        let y_table = lua.create_table()?;
                        for z in 0..dims[2] {
                            let cell = TerrainCell {
                                x: min_cell[0] + x,
                                y: min_cell[1] + y,
                                z: min_cell[2] + z,
                            };
                            let voxel = terrain.voxels.get(&cell).copied().unwrap_or_default();
                            match channel.as_str() {
                                "SolidMaterial" => {
                                    y_table.set(z + 1, voxel.solid_material)?;
                                }
                                "SolidOccupancy" => {
                                    y_table.set(z + 1, voxel.solid_occupancy)?;
                                }
                                "LiquidOccupancy" => {
                                    y_table.set(z + 1, voxel.liquid_occupancy)?;
                                }
                                _ => {
                                    return Err(mlua::Error::runtime(format!(
                                        "unsupported Terrain channel '{}'",
                                        channel
                                    )));
                                }
                            }
                        }
                        x_table.set(y + 1, y_table)?;
                    }
                    channel_table.set(x + 1, x_table)?;
                }
                out.set(channel, channel_table)?;
            }

            Ok(out)
        },
    );

    methods.add_method(
        "WriteVoxelChannels",
        |_, this, (region, resolution, channels): (Region3, f32, Table)| {
            validate_terrain_resolution(resolution)?;
            let (min_cell, dims) = terrain_region_cells(region)?;
            let mut data = this.data.lock().unwrap();
            let Some(terrain) = data.terrain_data.as_mut() else {
                return Err(mlua::Error::runtime(
                    "WriteVoxelChannels is only supported on Terrain",
                ));
            };

            let solid_materials: Option<Table> = channels.get("SolidMaterial")?;
            let solid_occupancies: Option<Table> = channels.get("SolidOccupancy")?;
            let liquid_occupancies: Option<Table> = channels.get("LiquidOccupancy")?;

            if solid_materials.is_none()
                && solid_occupancies.is_none()
                && liquid_occupancies.is_none()
            {
                return Err(mlua::Error::runtime(
                    "WriteVoxelChannels expects at least one supported channel",
                ));
            }

            for x in 0..dims[0] {
                for y in 0..dims[1] {
                    for z in 0..dims[2] {
                        let cell = TerrainCell {
                            x: min_cell[0] + x,
                            y: min_cell[1] + y,
                            z: min_cell[2] + z,
                        };
                        let mut voxel = terrain.voxels.get(&cell).copied().unwrap_or_default();

                        if let Some(table) = &solid_materials {
                            voxel.solid_material = get_3d_material(table, x, y, z)?;
                        }
                        if let Some(table) = &solid_occupancies {
                            voxel.solid_occupancy = get_3d_number(table, x, y, z)?.clamp(0.0, 1.0);
                        }
                        if let Some(table) = &liquid_occupancies {
                            voxel.liquid_occupancy = get_3d_number(table, x, y, z)?.clamp(0.0, 1.0);
                        }

                        if voxel.solid_occupancy <= 0.0 && voxel.liquid_occupancy <= 0.0 {
                            terrain.voxels.remove(&cell);
                        } else {
                            terrain.voxels.insert(cell, voxel);
                        }
                    }
                }
            }

            Ok(())
        },
    );
}

fn validate_terrain_resolution(resolution: f32) -> mlua::Result<()> {
    if (resolution - TERRAIN_VOXEL_SIZE).abs() > f32::EPSILON {
        return Err(mlua::Error::runtime(
            "Terrain voxel channel resolution must be 4",
        ));
    }
    Ok(())
}

fn terrain_region_cells(region: Region3) -> mlua::Result<([i32; 3], [i32; 3])> {
    let min = region.min;
    let max = region.max;
    for value in [min.x, min.y, min.z, max.x, max.y, max.z] {
        let scaled = value / TERRAIN_VOXEL_SIZE;
        if (scaled.round() - scaled).abs() > 1e-5 {
            return Err(mlua::Error::runtime(
                "Region3 must be aligned to the terrain voxel grid",
            ));
        }
    }

    let dims = [
        ((max.x - min.x) / TERRAIN_VOXEL_SIZE).round() as i32,
        ((max.y - min.y) / TERRAIN_VOXEL_SIZE).round() as i32,
        ((max.z - min.z) / TERRAIN_VOXEL_SIZE).round() as i32,
    ];
    if dims.iter().any(|dim| *dim <= 0) {
        return Err(mlua::Error::runtime(
            "Region3 must contain at least one voxel",
        ));
    }
    Ok((
        [
            (min.x / TERRAIN_VOXEL_SIZE).round() as i32,
            (min.y / TERRAIN_VOXEL_SIZE).round() as i32,
            (min.z / TERRAIN_VOXEL_SIZE).round() as i32,
        ],
        dims,
    ))
}

fn read_channel_ids(channel_ids: Table) -> mlua::Result<Vec<String>> {
    let mut out = Vec::new();
    for value in channel_ids.sequence_values::<String>() {
        out.push(value?);
    }
    Ok(out)
}

fn get_3d_value(table: &Table, x: i32, y: i32, z: i32) -> mlua::Result<Value> {
    let x_table: Table = table.get(x + 1)?;
    let y_table: Table = x_table.get(y + 1)?;
    y_table.get(z + 1)
}

fn get_3d_number(table: &Table, x: i32, y: i32, z: i32) -> mlua::Result<f32> {
    match get_3d_value(table, x, y, z)? {
        Value::Integer(n) => Ok(n as f32),
        Value::Number(n) => Ok(n as f32),
        other => Err(mlua::Error::runtime(format!(
            "Terrain occupancy channel expected number, got {}",
            other.type_name()
        ))),
    }
}

fn get_3d_material(table: &Table, x: i32, y: i32, z: i32) -> mlua::Result<Material> {
    match get_3d_value(table, x, y, z)? {
        Value::UserData(ud) => ud.borrow::<Material>().map(|material| *material),
        other => Err(mlua::Error::runtime(format!(
            "Terrain material channel expected Enum.Material, got {}",
            other.type_name()
        ))),
    }
}
