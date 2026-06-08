use mlua::{UserData, UserDataFields, UserDataMethods};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use crate::game::lua::instance::Instance;

pub const MAX_COLLISION_GROUPS: usize = 32;
const DEFAULT_COLLISION_GROUP: &str = "Default";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollisionGroupMask {
    pub membership: u32,
    pub filter: u32,
}

impl Default for CollisionGroupMask {
    fn default() -> Self {
        Self {
            membership: 1,
            filter: u32::MAX,
        }
    }
}

/// Roblox-compatible PhysicsService surface API.
///
/// Clawblox tracks Roblox's 32 collision groups and pairwise collidable flags.
/// The physics sync path applies these masks to Rapier collider interaction and
/// solver groups so the matrix affects real contacts, not only scripts.
#[derive(Default)]
struct PhysicsServiceData {
    registered_groups: HashSet<String>,
    group_indices: HashMap<String, u8>,
    explicit_pair_collidable: HashMap<(String, String), bool>,
}

#[derive(Clone, Default)]
pub struct PhysicsService {
    data: Arc<Mutex<PhysicsServiceData>>,
}

impl PhysicsService {
    pub fn new() -> Self {
        let mut data = PhysicsServiceData::default();
        data.registered_groups
            .insert(DEFAULT_COLLISION_GROUP.to_string());
        data.group_indices
            .insert(DEFAULT_COLLISION_GROUP.to_string(), 0);
        Self {
            data: Arc::new(Mutex::new(data)),
        }
    }

    fn canonical_pair(group1: &str, group2: &str) -> (String, String) {
        if group1 <= group2 {
            (group1.to_string(), group2.to_string())
        } else {
            (group2.to_string(), group1.to_string())
        }
    }

    fn register_group(data: &mut PhysicsServiceData, group: &str) -> mlua::Result<()> {
        if group.is_empty() || group == DEFAULT_COLLISION_GROUP {
            return Err(mlua::Error::runtime(format!(
                "invalid collision group name '{group}'"
            )));
        }
        if data.registered_groups.contains(group) {
            return Ok(());
        }
        if data.group_indices.len() >= MAX_COLLISION_GROUPS {
            return Err(mlua::Error::runtime(format!(
                "cannot register more than {MAX_COLLISION_GROUPS} collision groups"
            )));
        }
        let next_index = (0..MAX_COLLISION_GROUPS)
            .find(|index| !data.group_indices.values().any(|&v| v as usize == *index))
            .expect("available collision group index");
        data.registered_groups.insert(group.to_string());
        data.group_indices
            .insert(group.to_string(), next_index as u8);
        Ok(())
    }

    fn ensure_registered_group(&self, group: &str) -> mlua::Result<()> {
        let mut data = self.data.lock().unwrap();
        Self::register_group(&mut data, group)
    }

    fn require_registered_group(data: &PhysicsServiceData, group: &str) -> mlua::Result<()> {
        if data.registered_groups.contains(group) {
            Ok(())
        } else {
            Err(mlua::Error::runtime(format!(
                "collision group '{group}' is not registered"
            )))
        }
    }

    pub fn is_collision_group_registered(&self, group: &str) -> bool {
        self.data.lock().unwrap().registered_groups.contains(group)
    }

    fn group_id(data: &PhysicsServiceData, group: &str) -> mlua::Result<u8> {
        data.group_indices.get(group).copied().ok_or_else(|| {
            mlua::Error::runtime(format!("collision group '{group}' is not registered"))
        })
    }

    fn group_name(data: &PhysicsServiceData, id: u8) -> Option<String> {
        data.group_indices
            .iter()
            .find_map(|(name, &index)| (index == id).then(|| name.clone()))
    }

    fn registered_group_snapshot(&self) -> Vec<(String, u8, u32)> {
        let data = self.data.lock().unwrap();
        let mut groups = data
            .group_indices
            .iter()
            .map(|(name, &index)| {
                let mut filter = 0u32;
                for (other_name, &other_index) in &data.group_indices {
                    let key = Self::canonical_pair(name, other_name);
                    if data
                        .explicit_pair_collidable
                        .get(&key)
                        .copied()
                        .unwrap_or(true)
                    {
                        filter |= 1u32 << other_index;
                    }
                }
                (name.clone(), index, filter)
            })
            .collect::<Vec<_>>();
        groups.sort_by_key(|(_, index, _)| *index);
        groups
    }

    fn unregister_group(data: &mut PhysicsServiceData, group: &str) -> mlua::Result<()> {
        if group == DEFAULT_COLLISION_GROUP {
            return Err(mlua::Error::runtime(
                "cannot unregister collision group 'Default'",
            ));
        }
        if !data.registered_groups.remove(group) {
            return Ok(());
        }
        data.group_indices.remove(group);
        data.explicit_pair_collidable
            .retain(|(a, b), _| a != group && b != group);
        Ok(())
    }

    fn rename_group(data: &mut PhysicsServiceData, from: &str, to: &str) -> mlua::Result<()> {
        if from.is_empty()
            || to.is_empty()
            || from == DEFAULT_COLLISION_GROUP
            || to == DEFAULT_COLLISION_GROUP
        {
            return Err(mlua::Error::runtime("invalid collision group name"));
        }
        if !data.registered_groups.contains(from) {
            return Ok(());
        }
        if data.registered_groups.contains(to) {
            return Err(mlua::Error::runtime(format!(
                "collision group '{to}' is already registered"
            )));
        }

        let index = data
            .group_indices
            .remove(from)
            .expect("registered group should have an index");
        data.registered_groups.remove(from);
        data.registered_groups.insert(to.to_string());
        data.group_indices.insert(to.to_string(), index);

        let mut renamed_pairs = HashMap::with_capacity(data.explicit_pair_collidable.len());
        for ((a, b), collidable) in data.explicit_pair_collidable.drain() {
            let next_a = if a == from { to.to_string() } else { a };
            let next_b = if b == from { to.to_string() } else { b };
            let key = PhysicsService::canonical_pair(&next_a, &next_b);
            renamed_pairs.insert(key, collidable);
        }
        data.explicit_pair_collidable = renamed_pairs;
        Ok(())
    }

    pub fn groups_collidable(&self, group1: &str, group2: &str) -> bool {
        let data = self.data.lock().unwrap();
        let key = Self::canonical_pair(group1, group2);
        data.explicit_pair_collidable
            .get(&key)
            .copied()
            .unwrap_or(true)
    }

    pub fn collision_group_mask(&self, group: &str) -> CollisionGroupMask {
        let data = self.data.lock().unwrap();
        let Some(&index) = data.group_indices.get(group) else {
            return CollisionGroupMask::default();
        };
        let membership = 1u32 << index;
        let mut filter = 0u32;
        for (other_name, &other_index) in &data.group_indices {
            let key = Self::canonical_pair(group, other_name);
            if data
                .explicit_pair_collidable
                .get(&key)
                .copied()
                .unwrap_or(true)
            {
                filter |= 1u32 << other_index;
            }
        }
        CollisionGroupMask { membership, filter }
    }
}

impl UserData for PhysicsService {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("Name", |_, _| Ok("PhysicsService".to_string()));
        fields.add_field_method_get("ClassName", |_, _| Ok("PhysicsService".to_string()));
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("RegisterCollisionGroup", |_, this, name: String| {
            this.ensure_registered_group(&name)?;
            Ok(())
        });
        methods.add_method("CreateCollisionGroup", |_, this, name: String| {
            this.ensure_registered_group(&name)?;
            let data = this.data.lock().unwrap();
            Ok(PhysicsService::group_id(&data, &name)? as i32)
        });
        methods.add_method("GetMaxCollisionGroups", |_, _, ()| {
            Ok(MAX_COLLISION_GROUPS as i32)
        });
        methods.add_method("IsCollisionGroupRegistered", |_, this, name: String| {
            Ok(this.is_collision_group_registered(&name))
        });
        methods.add_method("GetCollisionGroupId", |_, this, name: String| {
            let data = this.data.lock().unwrap();
            Ok(PhysicsService::group_id(&data, &name)? as i32)
        });
        methods.add_method("GetCollisionGroupName", |_, this, id: i32| {
            if !(0..MAX_COLLISION_GROUPS as i32).contains(&id) {
                return Ok(None::<String>);
            }
            let data = this.data.lock().unwrap();
            Ok(PhysicsService::group_name(&data, id as u8))
        });
        methods.add_method("GetRegisteredCollisionGroups", |lua, this, ()| {
            let table = lua.create_table()?;
            for (offset, (name, _id, mask)) in
                this.registered_group_snapshot().into_iter().enumerate()
            {
                let entry = lua.create_table()?;
                entry.set("name", name)?;
                entry.set("mask", mask as i64)?;
                table.set(offset + 1, entry)?;
            }
            Ok(table)
        });
        methods.add_method("GetCollisionGroups", |lua, this, ()| {
            let table = lua.create_table()?;
            for (offset, (name, id, mask)) in
                this.registered_group_snapshot().into_iter().enumerate()
            {
                let entry = lua.create_table()?;
                entry.set("name", name)?;
                entry.set("id", id as i32)?;
                entry.set("mask", mask as i64)?;
                table.set(offset + 1, entry)?;
            }
            Ok(table)
        });
        methods.add_method(
            "CollisionGroupsAreCollidable",
            |_, this, (group1, group2): (String, String)| {
                Ok(this.groups_collidable(&group1, &group2))
            },
        );
        methods.add_method(
            "CollisionGroupSetCollidable",
            |_, this, (group1, group2, collidable): (String, String, bool)| {
                let mut data = this.data.lock().unwrap();
                PhysicsService::require_registered_group(&data, &group1)?;
                PhysicsService::require_registered_group(&data, &group2)?;
                let key = PhysicsService::canonical_pair(&group1, &group2);
                data.explicit_pair_collidable.insert(key, collidable);
                Ok(())
            },
        );
        methods.add_method("UnregisterCollisionGroup", |_, this, name: String| {
            let mut data = this.data.lock().unwrap();
            PhysicsService::unregister_group(&mut data, &name)
        });
        methods.add_method("RemoveCollisionGroup", |_, this, name: String| {
            let mut data = this.data.lock().unwrap();
            PhysicsService::unregister_group(&mut data, &name)
        });
        methods.add_method(
            "RenameCollisionGroup",
            |_, this, (from, to): (String, String)| {
                let mut data = this.data.lock().unwrap();
                PhysicsService::rename_group(&mut data, &from, &to)
            },
        );
        methods.add_method(
            "SetPartCollisionGroup",
            |_, this, (part, group): (Instance, String)| {
                {
                    let data = this.data.lock().unwrap();
                    PhysicsService::require_registered_group(&data, &group)?;
                }
                let mut data = part.data.lock().unwrap();
                if let Some(part_data) = &mut data.part_data {
                    part_data.collision_group = group;
                    part_data.interaction_dirty = true;
                }
                Ok(())
            },
        );
    }
}
