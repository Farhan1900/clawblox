use mlua::{AnyUserData, FromLua, Lua, Result, UserData, UserDataFields, UserDataMethods, Value};
use std::sync::{Arc, Mutex};

use crate::game::constants::physics as consts;
use crate::game::lua::instance::{ClassName, Instance};
use crate::game::lua::runtime::Game;
use crate::game::lua::services::PhysicsService;
use crate::game::lua::types::{CFrame, FluidForces, Material, Ray, RaycastFilterType, Vector3};
use crate::game::world_root::{OverlapQueryParams, RaycastHit, RaycastQueryParams, WorldRoot};

fn physics_service_from_lua(lua: &Lua) -> Result<PhysicsService> {
    let game_ud: AnyUserData = lua.globals().get("__clawblox_game")?;
    let game = game_ud.borrow::<Game>()?;
    Ok(game.physics_service())
}

#[derive(Clone)]
pub struct RaycastResult {
    pub instance: Instance,
    pub position: Vector3,
    pub normal: Vector3,
    pub distance: f32,
    pub material: Material,
}

impl UserData for RaycastResult {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("Instance", |_, this| Ok(this.instance.clone()));
        fields.add_field_method_get("Position", |_, this| Ok(this.position));
        fields.add_field_method_get("Normal", |_, this| Ok(this.normal));
        fields.add_field_method_get("Distance", |_, this| Ok(this.distance));
        fields.add_field_method_get("Material", |_, this| Ok(this.material));
    }
}

impl From<RaycastHit> for RaycastResult {
    fn from(hit: RaycastHit) -> Self {
        Self {
            instance: hit.instance,
            position: hit.position,
            normal: hit.normal,
            distance: hit.distance,
            material: hit.material,
        }
    }
}

#[derive(Clone)]
pub struct RaycastParams {
    pub filter_type: RaycastFilterType,
    pub filter_instances: Vec<Instance>,
    pub ignore_water: bool,
    pub collision_group: String,
    pub respect_can_collide: bool,
}

impl Default for RaycastParams {
    fn default() -> Self {
        Self {
            filter_type: RaycastFilterType::Exclude,
            filter_instances: Vec::new(),
            ignore_water: false,
            collision_group: "Default".to_string(),
            respect_can_collide: false,
        }
    }
}

impl RaycastParams {
    fn into_query_params(self) -> RaycastQueryParams {
        RaycastQueryParams {
            filter_type: self.filter_type,
            filter_instances: self.filter_instances,
            collision_group: self.collision_group,
            respect_can_collide: self.respect_can_collide,
            ignore_water: self.ignore_water,
        }
    }
}

impl FromLua for RaycastParams {
    fn from_lua(value: Value, _lua: &Lua) -> Result<Self> {
        match value {
            Value::UserData(ud) => ud.borrow::<RaycastParams>().map(|v| v.clone()),
            _ => Err(mlua::Error::FromLuaConversionError {
                from: value.type_name(),
                to: "RaycastParams".to_string(),
                message: Some("expected RaycastParams".to_string()),
            }),
        }
    }
}

impl UserData for RaycastParams {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("FilterType", |_, this| Ok(this.filter_type));
        fields.add_field_method_set("FilterType", |_, this, filter_type: RaycastFilterType| {
            this.filter_type = filter_type;
            Ok(())
        });

        fields.add_field_method_get("FilterDescendantsInstances", |_, this| {
            Ok(this.filter_instances.clone())
        });
        fields.add_field_method_set(
            "FilterDescendantsInstances",
            |_, this, instances: Vec<Instance>| {
                this.filter_instances = instances;
                Ok(())
            },
        );

        fields.add_field_method_get("IgnoreWater", |_, this| Ok(this.ignore_water));
        fields.add_field_method_set("IgnoreWater", |_, this, ignore_water: bool| {
            this.ignore_water = ignore_water;
            Ok(())
        });

        fields.add_field_method_get("CollisionGroup", |_, this| Ok(this.collision_group.clone()));
        fields.add_field_method_set("CollisionGroup", |_, this, collision_group: String| {
            this.collision_group = collision_group;
            Ok(())
        });

        fields.add_field_method_get("RespectCanCollide", |_, this| Ok(this.respect_can_collide));
        fields.add_field_method_set("RespectCanCollide", |_, this, respect_can_collide: bool| {
            this.respect_can_collide = respect_can_collide;
            Ok(())
        });
    }
}

#[derive(Clone)]
pub struct OverlapParams {
    pub filter_type: RaycastFilterType,
    pub filter_instances: Vec<Instance>,
    pub max_parts: usize,
    pub collision_group: String,
    pub respect_can_collide: bool,
}

impl Default for OverlapParams {
    fn default() -> Self {
        Self {
            filter_type: RaycastFilterType::Exclude,
            filter_instances: Vec::new(),
            max_parts: 0,
            collision_group: "Default".to_string(),
            respect_can_collide: false,
        }
    }
}

impl OverlapParams {
    fn into_query_params(self) -> OverlapQueryParams {
        OverlapQueryParams {
            filter_type: self.filter_type,
            filter_instances: self.filter_instances,
            max_parts: self.max_parts,
            collision_group: self.collision_group,
            respect_can_collide: self.respect_can_collide,
        }
    }
}

impl FromLua for OverlapParams {
    fn from_lua(value: Value, _lua: &Lua) -> Result<Self> {
        match value {
            Value::UserData(ud) => ud.borrow::<OverlapParams>().map(|v| v.clone()),
            _ => Err(mlua::Error::FromLuaConversionError {
                from: value.type_name(),
                to: "OverlapParams".to_string(),
                message: Some("expected OverlapParams".to_string()),
            }),
        }
    }
}

impl UserData for OverlapParams {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("FilterType", |_, this| Ok(this.filter_type));
        fields.add_field_method_set("FilterType", |_, this, filter_type: RaycastFilterType| {
            this.filter_type = filter_type;
            Ok(())
        });

        fields.add_field_method_get("FilterDescendantsInstances", |_, this| {
            Ok(this.filter_instances.clone())
        });
        fields.add_field_method_set(
            "FilterDescendantsInstances",
            |_, this, instances: Vec<Instance>| {
                this.filter_instances = instances;
                Ok(())
            },
        );

        fields.add_field_method_get("MaxParts", |_, this| Ok(this.max_parts as i32));
        fields.add_field_method_set("MaxParts", |_, this, max_parts: i32| {
            this.max_parts = max_parts.max(0) as usize;
            Ok(())
        });

        fields.add_field_method_get("CollisionGroup", |_, this| Ok(this.collision_group.clone()));
        fields.add_field_method_set("CollisionGroup", |_, this, collision_group: String| {
            this.collision_group = collision_group;
            Ok(())
        });

        fields.add_field_method_get("RespectCanCollide", |_, this| Ok(this.respect_can_collide));
        fields.add_field_method_set("RespectCanCollide", |_, this, respect_can_collide: bool| {
            this.respect_can_collide = respect_can_collide;
            Ok(())
        });
    }
}

pub fn register_raycast_params(lua: &Lua) -> Result<()> {
    let params_table = lua.create_table()?;

    params_table.set(
        "new",
        lua.create_function(|_, ()| Ok(RaycastParams::default()))?,
    )?;

    lua.globals().set("RaycastParams", params_table)?;

    Ok(())
}

pub fn register_overlap_params(lua: &Lua) -> Result<()> {
    let params_table = lua.create_table()?;

    params_table.set(
        "new",
        lua.create_function(|_, ()| Ok(OverlapParams::default()))?,
    )?;

    lua.globals().set("OverlapParams", params_table)?;

    Ok(())
}

pub struct WorkspaceServiceData {
    pub gravity: f32,
    pub fluid_forces: FluidForces,
    pub current_camera: Option<Instance>,
    pub children: Vec<Instance>,
}

impl WorkspaceServiceData {
    pub fn new() -> Self {
        Self {
            gravity: consts::DEFAULT_GRAVITY,
            fluid_forces: FluidForces::Default,
            current_camera: None,
            children: Vec::new(),
        }
    }
}

#[derive(Clone)]
pub struct WorkspaceService {
    pub instance: Instance,
    pub data: Arc<Mutex<WorkspaceServiceData>>,
}

impl WorkspaceService {
    pub fn new() -> Self {
        let instance = Instance::new(ClassName::Workspace, "Workspace");
        let terrain = Instance::from_data(crate::game::lua::instance::InstanceData::new_terrain(
            "Terrain",
        ));
        terrain.set_parent(Some(&instance));
        Self {
            instance,
            data: Arc::new(Mutex::new(WorkspaceServiceData::new())),
        }
    }

    pub fn add_child(&self, child: Instance) {
        child.set_parent(Some(&self.instance));
        self.data.lock().unwrap().children.push(child);
    }

    pub fn get_children(&self) -> Vec<Instance> {
        self.instance.get_children()
    }

    pub fn get_descendants(&self) -> Vec<Instance> {
        self.instance.get_descendants()
    }

    pub fn raycast(
        &self,
        origin: Vector3,
        direction: Vector3,
        params: Option<RaycastParams>,
        physics_service: &PhysicsService,
    ) -> Result<Option<RaycastResult>> {
        let world_root = WorldRoot::new(self.instance.clone());
        world_root
            .raycast(
                origin,
                direction,
                params.unwrap_or_default().into_query_params(),
                physics_service,
            )
            .map(|hit| hit.map(RaycastResult::from))
    }

    pub fn get_part_bounds_in_box(
        &self,
        cframe: CFrame,
        size: Vector3,
        params: Option<OverlapParams>,
        physics_service: &PhysicsService,
    ) -> Result<Vec<Instance>> {
        WorldRoot::new(self.instance.clone()).get_part_bounds_in_box(
            cframe,
            size,
            params.unwrap_or_default().into_query_params(),
            physics_service,
        )
    }

    pub fn get_part_bounds_in_radius(
        &self,
        position: Vector3,
        radius: f32,
        params: Option<OverlapParams>,
        physics_service: &PhysicsService,
    ) -> Result<Vec<Instance>> {
        WorldRoot::new(self.instance.clone()).get_part_bounds_in_radius(
            position,
            radius,
            params.unwrap_or_default().into_query_params(),
            physics_service,
        )
    }

    pub fn get_parts_in_part(
        &self,
        query_part: Instance,
        params: Option<OverlapParams>,
        physics_service: &PhysicsService,
    ) -> Result<Vec<Instance>> {
        WorldRoot::new(self.instance.clone()).get_parts_in_part(
            query_part,
            params.unwrap_or_default().into_query_params(),
            physics_service,
        )
    }
}

impl UserData for WorkspaceService {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("Gravity", |_, this| Ok(this.data.lock().unwrap().gravity));
        fields.add_field_method_set("Gravity", |_, this, gravity: f32| {
            this.data.lock().unwrap().gravity = gravity;
            Ok(())
        });
        fields.add_field_method_get("FluidForces", |_, this| {
            Ok(this.data.lock().unwrap().fluid_forces)
        });
        fields.add_field_method_set("FluidForces", |_, this, fluid_forces: FluidForces| {
            this.data.lock().unwrap().fluid_forces = fluid_forces;
            Ok(())
        });

        fields.add_field_method_get("CurrentCamera", |_, this| {
            Ok(this.data.lock().unwrap().current_camera.clone())
        });

        fields.add_field_method_get("Name", |_, _| Ok("Workspace".to_string()));
        fields.add_field_method_get("ClassName", |_, _| Ok("Workspace".to_string()));
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method(
            "Raycast",
            |lua, this, (origin, direction, params): (Vector3, Vector3, Option<RaycastParams>)| {
                let physics_service = physics_service_from_lua(lua)?;
                this.raycast(origin, direction, params, &physics_service)
            },
        );

        methods.add_method(
            "FindPartOnRayWithIgnoreList",
            |lua, this, (ray, ignore): (Ray, Value)| {
                let physics_service = physics_service_from_lua(lua)?;
                let mut params = RaycastParams::default();
                if let Value::Table(tbl) = ignore {
                    let mut filter_instances = Vec::new();
                    for value in tbl.sequence_values::<Value>() {
                        if let Value::UserData(ud) = value? {
                            if let Ok(inst) = ud.borrow::<Instance>() {
                                filter_instances.push(inst.clone());
                            }
                        }
                    }
                    params.filter_instances = filter_instances;
                }

                match this.raycast(ray.origin, ray.direction, Some(params), &physics_service)? {
                    Some(hit) => Ok((
                        Some(hit.instance),
                        Some(Value::UserData(lua.create_userdata(hit.position)?)),
                    )),
                    None => Ok((None::<Instance>, None::<Value>)),
                }
            },
        );

        methods.add_method("FindPartOnRay", |lua, this, ray: Ray| {
            let physics_service = physics_service_from_lua(lua)?;
            match this.raycast(ray.origin, ray.direction, None, &physics_service)? {
                Some(hit) => Ok((
                    Some(hit.instance),
                    Some(Value::UserData(lua.create_userdata(hit.position)?)),
                )),
                None => Ok((None::<Instance>, None::<Value>)),
            }
        });

        methods.add_method(
            "GetPartBoundsInBox",
            |lua, this, (cframe, size, params): (CFrame, Vector3, Option<OverlapParams>)| {
                let physics_service = physics_service_from_lua(lua)?;
                this.get_part_bounds_in_box(cframe, size, params, &physics_service)
            },
        );

        methods.add_method(
            "GetPartBoundsInRadius",
            |lua, this, (position, radius, params): (Vector3, f32, Option<OverlapParams>)| {
                let physics_service = physics_service_from_lua(lua)?;
                this.get_part_bounds_in_radius(position, radius, params, &physics_service)
            },
        );

        methods.add_method(
            "GetPartsInPart",
            |lua, this, (part, params): (Instance, Option<OverlapParams>)| {
                let physics_service = physics_service_from_lua(lua)?;
                this.get_parts_in_part(part, params, &physics_service)
            },
        );

        methods.add_method("GetChildren", |_, this, ()| Ok(this.get_children()));

        methods.add_method("GetDescendants", |_, this, ()| Ok(this.get_descendants()));

        methods.add_method(
            "FindFirstChild",
            |_, this, (name, recursive): (String, Option<bool>)| {
                Ok(this
                    .instance
                    .find_first_child(&name, recursive.unwrap_or(false)))
            },
        );

        methods.add_async_method(
            "WaitForChild",
            |lua, this, (name, timeout): (String, Option<f64>)| {
                let root = this.instance.clone();
                async move {
                    let start = std::time::Instant::now();
                    let timeout = timeout.map(|t| t.max(0.0));
                    let name_for_poll = name.clone();

                    let child = futures_util::future::poll_fn(move |_cx| {
                        if let Some(child) = root.find_first_child(&name_for_poll, false) {
                            return std::task::Poll::Ready(Some(child));
                        }

                        if let Some(t) = timeout {
                            if start.elapsed().as_secs_f64() >= t {
                                return std::task::Poll::Ready(None);
                            }
                        }

                        std::task::Poll::Pending
                    })
                    .await;

                    match child {
                        Some(child) => Ok(Value::UserData(lua.create_userdata(child)?)),
                        None => Ok(Value::Nil),
                    }
                }
            },
        );

        methods.add_method("FindFirstChildOfClass", |_, this, class_name: String| {
            Ok(this.instance.find_first_child_of_class(&class_name))
        });

        methods.add_meta_method(mlua::MetaMethod::Index, |_, this, key: String| {
            Ok(this.instance.find_first_child(&key, false))
        });
    }
}
