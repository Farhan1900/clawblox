use mlua::{Function, RegistryKey, UserData, UserDataMethods, Value};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::game::async_bridge::AsyncBridge;

use super::super::instance::{Instance, InstanceData};
use super::super::services::{
    AgentInputService, DataStoreService, DebrisService, HttpService, PhysicsService,
    PlayersService, RemoteEventService, RunService, WorkspaceService,
};
use super::super::types::Vector3;

fn global_service(lua: &mlua::Lua, name: &str) -> mlua::Result<Value> {
    lua.globals().get(name)
}

/// A request to kick a player from the game
#[derive(Debug, Clone)]
pub struct KickRequest {
    pub user_id: u64,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum PhysicsCommand {
    ApplyImpulse {
        part_id: u64,
        impulse: Vector3,
    },
    ApplyImpulseAtPosition {
        part_id: u64,
        impulse: Vector3,
        position: Vector3,
    },
    ApplyAngularImpulse {
        part_id: u64,
        impulse: Vector3,
    },
}

#[derive(Default)]
pub(super) struct BindToCloseCallbacks {
    pub(super) callbacks: Vec<RegistryKey>,
}
pub struct GameDataModel {
    pub workspace: WorkspaceService,
    pub players: PlayersService,
    pub run_service: RunService,
    pub agent_input_service: AgentInputService,
    pub data_store_service: DataStoreService,
    pub debris_service: DebrisService,
    pub physics_service: PhysicsService,
    pub remote_event_service: RemoteEventService,
    pub server_script_service: Instance,
    pub server_storage: Instance,
    pub replicated_storage: Instance,
    /// Queue of pending kick requests from Lua scripts
    pub kick_requests: Vec<KickRequest>,
    /// One-frame physics commands issued by BasePart methods during Lua execution.
    pub physics_commands: Vec<PhysicsCommand>,
}

impl GameDataModel {
    pub fn new(game_id: Uuid, async_bridge: Option<Arc<AsyncBridge>>) -> Self {
        Self::with_config(game_id, 100, async_bridge)
    }

    pub fn with_config(
        game_id: Uuid,
        max_players: u32,
        async_bridge: Option<Arc<AsyncBridge>>,
    ) -> Self {
        Self {
            workspace: WorkspaceService::new(),
            players: PlayersService::with_max_players(max_players),
            run_service: RunService::new(true),
            agent_input_service: AgentInputService::new(),
            data_store_service: DataStoreService::new(game_id, async_bridge),
            debris_service: DebrisService::new(),
            physics_service: PhysicsService::new(),
            remote_event_service: RemoteEventService::new(),
            server_script_service: Instance::from_data(InstanceData::new_server_script_service()),
            server_storage: Instance::from_data(InstanceData::new_server_storage()),
            replicated_storage: Instance::from_data(InstanceData::new_replicated_storage()),
            kick_requests: Vec::new(),
            physics_commands: Vec::new(),
        }
    }
}

#[derive(Clone)]
pub struct Game {
    pub data_model: Arc<Mutex<GameDataModel>>,
}

impl Game {
    pub fn new(game_id: Uuid, async_bridge: Option<Arc<AsyncBridge>>) -> Self {
        Self::with_config(game_id, 100, async_bridge)
    }

    pub fn with_config(
        game_id: Uuid,
        max_players: u32,
        async_bridge: Option<Arc<AsyncBridge>>,
    ) -> Self {
        Self {
            data_model: Arc::new(Mutex::new(GameDataModel::with_config(
                game_id,
                max_players,
                async_bridge,
            ))),
        }
    }

    pub fn workspace(&self) -> WorkspaceService {
        self.data_model.lock().unwrap().workspace.clone()
    }

    pub fn players(&self) -> PlayersService {
        self.data_model.lock().unwrap().players.clone()
    }

    pub fn run_service(&self) -> RunService {
        self.data_model.lock().unwrap().run_service.clone()
    }

    pub fn agent_input_service(&self) -> AgentInputService {
        self.data_model.lock().unwrap().agent_input_service.clone()
    }

    pub fn data_store_service(&self) -> DataStoreService {
        self.data_model.lock().unwrap().data_store_service.clone()
    }

    pub fn remote_event_service(&self) -> RemoteEventService {
        self.data_model.lock().unwrap().remote_event_service.clone()
    }

    pub fn debris_service(&self) -> DebrisService {
        self.data_model.lock().unwrap().debris_service.clone()
    }

    pub fn physics_service(&self) -> PhysicsService {
        self.data_model.lock().unwrap().physics_service.clone()
    }

    pub fn server_script_service(&self) -> Instance {
        self.data_model
            .lock()
            .unwrap()
            .server_script_service
            .clone()
    }

    pub fn server_storage(&self) -> Instance {
        self.data_model.lock().unwrap().server_storage.clone()
    }

    pub fn replicated_storage(&self) -> Instance {
        self.data_model.lock().unwrap().replicated_storage.clone()
    }

    /// Queue a kick request for a player (called from Lua Player:Kick())
    pub fn queue_kick(&self, user_id: u64, message: Option<String>) {
        self.data_model
            .lock()
            .unwrap()
            .kick_requests
            .push(KickRequest { user_id, message });
    }

    /// Drain all pending kick requests (called from GameInstance tick)
    pub fn drain_kick_requests(&self) -> Vec<KickRequest> {
        std::mem::take(&mut self.data_model.lock().unwrap().kick_requests)
    }

    pub fn snapshot_kick_requests(&self) -> Vec<KickRequest> {
        self.data_model.lock().unwrap().kick_requests.clone()
    }

    pub fn restore_kick_requests(&self, requests: Vec<KickRequest>) {
        self.data_model.lock().unwrap().kick_requests = requests;
    }

    pub fn queue_physics_command(&self, command: PhysicsCommand) {
        self.data_model
            .lock()
            .unwrap()
            .physics_commands
            .push(command);
    }

    pub fn drain_physics_commands(&self) -> Vec<PhysicsCommand> {
        std::mem::take(&mut self.data_model.lock().unwrap().physics_commands)
    }

    pub fn snapshot_physics_commands(&self) -> Vec<PhysicsCommand> {
        self.data_model.lock().unwrap().physics_commands.clone()
    }

    pub fn restore_physics_commands(&self, commands: Vec<PhysicsCommand>) {
        self.data_model.lock().unwrap().physics_commands = commands;
    }
}

impl UserData for Game {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("GetService", |lua, this, name: String| {
            let dm = this.data_model.lock().unwrap();
            match name.as_str() {
                "Workspace" => global_service(lua, "Workspace"),
                "Players" => global_service(lua, "Players"),
                "RunService" => global_service(lua, "RunService"),
                "AgentInputService" => Ok(Value::UserData(
                    lua.create_userdata(dm.agent_input_service.clone())?,
                )),
                "DataStoreService" => Ok(Value::UserData(
                    lua.create_userdata(dm.data_store_service.clone())?,
                )),
                "Debris" | "DebrisService" => Ok(Value::UserData(
                    lua.create_userdata(dm.debris_service.clone())?,
                )),
                "PhysicsService" => global_service(lua, "PhysicsService"),
                "RemoteEventService" => global_service(lua, "RemoteEventService"),
                "ServerScriptService" => global_service(lua, "ServerScriptService"),
                "ServerStorage" => global_service(lua, "ServerStorage"),
                "ReplicatedStorage" => global_service(lua, "ReplicatedStorage"),
                "HttpService" => {
                    drop(dm); // Release lock before creating userdata
                    Ok(Value::UserData(lua.create_userdata(HttpService::new())?))
                }
                _ => Ok(Value::Nil),
            }
        });

        methods.add_meta_method(mlua::MetaMethod::Index, |lua, this, key: String| {
            let dm = this.data_model.lock().unwrap();
            match key.as_str() {
                "Workspace" => global_service(lua, "Workspace"),
                "Players" => global_service(lua, "Players"),
                "RunService" => global_service(lua, "RunService"),
                "PhysicsService" => global_service(lua, "PhysicsService"),
                "RemoteEventService" => global_service(lua, "RemoteEventService"),
                "Debris" | "DebrisService" => Ok(Value::UserData(
                    lua.create_userdata(dm.debris_service.clone())?,
                )),
                "ServerScriptService" => global_service(lua, "ServerScriptService"),
                "ServerStorage" => global_service(lua, "ServerStorage"),
                "ReplicatedStorage" => global_service(lua, "ReplicatedStorage"),
                _ => Ok(Value::Nil),
            }
        });

        methods.add_method("BindToClose", |lua, _this, callback: Function| {
            let key = lua.create_registry_value(callback)?;
            if let Some(mut existing) = lua.app_data_mut::<BindToCloseCallbacks>() {
                existing.callbacks.push(key);
            } else {
                lua.set_app_data(BindToCloseCallbacks {
                    callbacks: vec![key],
                });
            }
            Ok(())
        });
    }
}
