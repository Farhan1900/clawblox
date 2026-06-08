use std::time::{Duration, Instant};

use uuid::Uuid;

use crate::game::lua::services::AgentInput;

use super::GameInstance;

impl GameInstance {
    /// Adds a player to the game
    /// Returns false if player is already in game (does NOT check capacity - use has_capacity() first)
    pub fn add_player(&mut self, agent_id: Uuid, name: &str) -> bool {
        if self.players.contains_key(&agent_id) {
            return false;
        }

        // Clear empty_since since we now have a player
        self.empty_since = None;

        // Use a stable user_id derived from agent_id so DataStore keys persist across restarts.
        let user_id = Self::user_id_from_agent_id(agent_id);
        self.players.insert(agent_id, user_id);
        self.player_names.insert(agent_id, name.to_string());
        // Initialize activity timestamp for AFK tracking
        self.player_last_activity.insert(agent_id, Instant::now());

        if let Some(runtime) = &self.lua_runtime {
            let (player, hrp_id) = runtime.add_player(user_id, name);

            // Offset spawn position based on player count to avoid overlap
            let player_index = self.players.len() as f32;
            let spawn_x = (player_index % 4.0 - 1.5) * 3.0; // -4.5, -1.5, 1.5, 4.5
            let spawn_z = (player_index / 4.0).floor() * 3.0;

            // Register character controller for player movement
            self.physics
                .add_character(hrp_id, [spawn_x, 6.0, spawn_z], 1.0, 5.0);
            self.player_hrp_ids.insert(agent_id, hrp_id);

            if let Err(e) = runtime.fire_player_added(&player) {
                self.handle_lua_error("Failed to fire PlayerAdded", &e);
            }
        }

        true
    }

    /// Removes a player from the game
    pub fn remove_player(&mut self, agent_id: Uuid) -> bool {
        if let Some(user_id) = self.players.remove(&agent_id) {
            // Remove character controller
            if let Some(hrp_id) = self.player_hrp_ids.remove(&agent_id) {
                self.physics.remove_character(hrp_id);
            }
            // Remove player name
            self.player_names.remove(&agent_id);
            // Remove activity timestamp
            self.player_last_activity.remove(&agent_id);
            if let Ok(mut counts) = self.observation_log_counts.lock() {
                counts.remove(&agent_id);
            }
            if let Ok(mut counts) = self.humanoid_warn_counts.lock() {
                counts.remove(&agent_id);
            }

            // Fire PlayerRemoving and remove player from Lua
            // Split borrow: capture error first, then handle it after releasing runtime ref
            let lua_err = if let Some(runtime) = &self.lua_runtime {
                let err = runtime
                    .players()
                    .get_player_by_user_id(user_id)
                    .and_then(|player| runtime.fire_player_removing(&player).err());
                runtime.remove_player(user_id);
                err
            } else {
                None
            };
            if let Some(e) = lua_err {
                self.handle_lua_error("Failed to fire PlayerRemoving", &e);
            }

            // Track when instance becomes empty for garbage collection
            if self.players.is_empty() {
                self.empty_since = Some(Instant::now());
            }

            true
        } else {
            false
        }
    }

    /// Queues an agent input for the AgentInputService in Lua
    pub fn queue_agent_input(&self, user_id: u64, input_type: String, data: serde_json::Value) {
        if let Some(runtime) = &self.lua_runtime {
            let input = AgentInput::new(input_type, data);
            runtime.queue_agent_input(user_id, input);
        }
    }

    /// Drain Players:AddBot() requests queued from Lua and join them as server-managed players.
    pub(super) fn process_pending_bot_requests(&mut self) {
        let pending_bots: Vec<String> = if let Some(runtime) = &self.lua_runtime {
            runtime
                .players()
                .data
                .lock()
                .unwrap()
                .pending_bots
                .drain(..)
                .collect()
        } else {
            vec![]
        };
        for name in pending_bots {
            self.add_player(Uuid::new_v4(), &name);
        }
    }

    /// Process kick requests queued by Lua scripts (e.g., Player:Kick())
    pub(super) fn process_kick_requests(&mut self) {
        let Some(runtime) = &self.lua_runtime else {
            return;
        };

        // Drain kick requests from the game's kick queue
        let kick_requests = runtime.game().drain_kick_requests();

        for request in kick_requests {
            // Find the agent_id for this user_id
            let agent_id = self
                .players
                .iter()
                .find(|(_, &uid)| uid == request.user_id)
                .map(|(&aid, _)| aid);

            if let Some(agent_id) = agent_id {
                let name = self
                    .player_names
                    .get(&agent_id)
                    .cloned()
                    .unwrap_or_default();
                if let Some(msg) = &request.message {
                    eprintln!(
                        "[Kick] Removing player {} (user_id={}) - Reason: {}",
                        name, request.user_id, msg
                    );
                } else {
                    eprintln!(
                        "[Kick] Removing player {} (user_id={})",
                        name, request.user_id
                    );
                }
                self.remove_player(agent_id);
            } else {
                eprintln!(
                    "[Kick] Warning: No agent found for user_id={}",
                    request.user_id
                );
            }
        }
    }

    /// Check for AFK players and kick them if they've exceeded the timeout
    pub(super) fn check_afk_players(&mut self) {
        let now = Instant::now();
        let timeout = self.afk_timeout;

        // First, check which players have pending inputs (they're active)
        let mut active_from_inputs: Vec<Uuid> = Vec::new();
        if let Some(runtime) = &self.lua_runtime {
            for (&agent_id, &user_id) in &self.players {
                if runtime.agent_input_service().has_pending_inputs(user_id) {
                    active_from_inputs.push(agent_id);
                }
            }
        }

        // Update activity for players with pending inputs
        for agent_id in active_from_inputs {
            self.player_last_activity.insert(agent_id, now);
        }

        // Collect players to kick (can't modify while iterating)
        let mut to_kick: Vec<(Uuid, String)> = Vec::new();

        for (&agent_id, last_active) in &self.player_last_activity {
            let idle_duration = now.duration_since(*last_active);
            if idle_duration > timeout {
                let name = self
                    .player_names
                    .get(&agent_id)
                    .cloned()
                    .unwrap_or_else(|| format!("{}", agent_id));
                to_kick.push((agent_id, name));
            }
        }

        // Kick AFK players
        for (agent_id, name) in to_kick {
            eprintln!(
                "[AFK] Kicking player {} (idle for {:?})",
                name, self.afk_timeout
            );
            self.remove_player(agent_id);
        }
    }

    /// Set the AFK timeout duration
    pub fn set_afk_timeout(&mut self, timeout: Duration) {
        self.afk_timeout = timeout;
    }

    /// Get the current AFK timeout duration
    pub fn afk_timeout(&self) -> Duration {
        self.afk_timeout
    }

    /// Record player activity (resets AFK timer)
    pub fn record_player_activity(&mut self, agent_id: Uuid) {
        self.player_last_activity.insert(agent_id, Instant::now());
    }
}
