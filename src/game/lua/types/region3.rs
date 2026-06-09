use mlua::{FromLua, Lua, MetaMethod, Result, UserData, UserDataFields, UserDataMethods, Value};

use super::Vector3;

#[derive(Debug, Clone, Copy)]
pub struct Region3 {
    pub min: Vector3,
    pub max: Vector3,
}

impl Region3 {
    pub fn new(a: Vector3, b: Vector3) -> Self {
        Self {
            min: Vector3::new(a.x.min(b.x), a.y.min(b.y), a.z.min(b.z)),
            max: Vector3::new(a.x.max(b.x), a.y.max(b.y), a.z.max(b.z)),
        }
    }

    pub fn size(&self) -> Vector3 {
        Vector3::new(
            self.max.x - self.min.x,
            self.max.y - self.min.y,
            self.max.z - self.min.z,
        )
    }

    pub fn expand_to_grid(&self, resolution: f32) -> Self {
        let resolution = resolution.abs().max(f32::EPSILON);
        Self {
            min: Vector3::new(
                (self.min.x / resolution).floor() * resolution,
                (self.min.y / resolution).floor() * resolution,
                (self.min.z / resolution).floor() * resolution,
            ),
            max: Vector3::new(
                (self.max.x / resolution).ceil() * resolution,
                (self.max.y / resolution).ceil() * resolution,
                (self.max.z / resolution).ceil() * resolution,
            ),
        }
    }
}

impl FromLua for Region3 {
    fn from_lua(value: Value, _lua: &Lua) -> Result<Self> {
        match value {
            Value::UserData(ud) => ud.borrow::<Region3>().map(|v| *v),
            _ => Err(mlua::Error::FromLuaConversionError {
                from: value.type_name(),
                to: "Region3".to_string(),
                message: Some("expected Region3".to_string()),
            }),
        }
    }
}

impl UserData for Region3 {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("Size", |_, this| Ok(this.size()));
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("ExpandToGrid", |_, this, resolution: f32| {
            Ok(this.expand_to_grid(resolution))
        });
        methods.add_meta_method(MetaMethod::ToString, |_, this, ()| {
            Ok(format!(
                "Region3({}, {}, {})-({}, {}, {})",
                this.min.x, this.min.y, this.min.z, this.max.x, this.max.y, this.max.z
            ))
        });
    }
}

pub fn register_region3(lua: &Lua) -> Result<()> {
    let region3_table = lua.create_table()?;
    region3_table.set(
        "new",
        lua.create_function(|_, (min, max): (Vector3, Vector3)| Ok(Region3::new(min, max)))?,
    )?;
    lua.globals().set("Region3", region3_table)?;
    Ok(())
}
