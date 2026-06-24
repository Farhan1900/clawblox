use crate::game::instance::{
    SpectatorEntity, SpectatorObservation, SpectatorPlayerInfo, SpectatorRender,
};
use crate::replay::ReplayActiveRange;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;
use uuid::Uuid;

const CHECKPOINT_INTERVAL: u64 = 60;
const WAL_CHECKPOINT_SECS: u64 = 300;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedChatAudibilityEntry {
    pub agent_id: String,
    pub agent_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance: Option<f32>,
    pub heard: bool,
}

// --- Delta format ---

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum DeltaEntry {
    Add {
        id: u32,
        add: SpectatorEntity,
    },
    Remove {
        id: u32,
        rm: bool,
    },
    Update {
        id: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        p: Option<[f32; 3]>,
        #[serde(skip_serializing_if = "Option::is_none")]
        r: Option<[[f32; 3]; 3]>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum PlayerDelta {
    Replace {
        replace: Vec<SpectatorPlayerInfo>,
    },
    Update {
        id: Uuid,
        #[serde(skip_serializing_if = "Option::is_none")]
        p: Option<[f32; 3]>,
        #[serde(skip_serializing_if = "Option::is_none")]
        health: Option<i32>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct FrameDelta {
    entities: Vec<DeltaEntry>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    players: Vec<PlayerDelta>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    sounds: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    recent_events: Vec<serde_json::Value>,
}

impl FrameDelta {
    fn is_empty(&self) -> bool {
        self.entities.is_empty()
            && self.players.is_empty()
            && self.sounds.is_empty()
            && self.recent_events.is_empty()
    }
}

// --- Writer ---

pub struct RecordingWriter {
    pool: SqlitePool,
    checkpoint_interval: u64,
    last_checkpoint_obs: Option<SpectatorObservation>,
    last_recorded_obs: Option<SpectatorObservation>,
    last_checkpoint_tick: u64,
    last_wal_checkpoint: Instant,
    latest_tick: u64,
    latest_time_ms: u64,
    tick_rate: u32,
}

pub struct RecordingFinalInfo {
    pub first_tick: u64,
    pub total_ticks: u64,
    pub tick_rate: u32,
    pub duration_ms: u64,
}

impl RecordingWriter {
    pub async fn create(
        path: &Path,
        world_name: &str,
        world_dir: &str,
        tick_rate: u32,
    ) -> Result<Self, sqlx::Error> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let opts = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal);

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await?;

        sqlx::query("CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT)")
            .execute(&pool)
            .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS checkpoints (
                tick INTEGER PRIMARY KEY,
                time_ms INTEGER NOT NULL,
                data BLOB NOT NULL
            )",
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS deltas (
                tick INTEGER NOT NULL,
                time_ms INTEGER NOT NULL,
                data BLOB NOT NULL
            )",
        )
        .execute(&pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_deltas_tick ON deltas(tick)")
            .execute(&pool)
            .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS inputs (
                tick INTEGER NOT NULL,
                time_ms INTEGER NOT NULL,
                agent_id TEXT NOT NULL,
                input_type TEXT NOT NULL,
                data TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_inputs_tick ON inputs(tick)")
            .execute(&pool)
            .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS chat_messages (
                tick INTEGER NOT NULL,
                time_ms INTEGER NOT NULL,
                message_id TEXT NOT NULL,
                agent_id TEXT NOT NULL,
                agent_name TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT '',
                delivery_mode TEXT NOT NULL DEFAULT 'global',
                hearing_radius REAL,
                heard_by TEXT NOT NULL DEFAULT '[]',
                audibility TEXT NOT NULL DEFAULT '[]'
            )",
        )
        .execute(&pool)
        .await?;

        ensure_chat_messages_column(&pool, "created_at TEXT NOT NULL DEFAULT ''").await?;
        ensure_chat_messages_column(&pool, "delivery_mode TEXT NOT NULL DEFAULT 'global'").await?;
        ensure_chat_messages_column(&pool, "hearing_radius REAL").await?;
        ensure_chat_messages_column(&pool, "heard_by TEXT NOT NULL DEFAULT '[]'").await?;
        ensure_chat_messages_column(&pool, "audibility TEXT NOT NULL DEFAULT '[]'").await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_chat_messages_tick ON chat_messages(tick)")
            .execute(&pool)
            .await?;

        let now = chrono::Utc::now().to_rfc3339();
        for (k, v) in [
            ("world_name", world_name),
            ("world_dir", world_dir),
            ("created_at", &now),
            ("tick_rate", &tick_rate.to_string()),
            ("version", "3"),
            ("tick_semantics", "monotonic_recording_tick"),
        ] {
            sqlx::query("INSERT OR REPLACE INTO meta (key, value) VALUES (?, ?)")
                .bind(k)
                .bind(v)
                .execute(&pool)
                .await?;
        }

        Ok(Self {
            pool,
            checkpoint_interval: CHECKPOINT_INTERVAL,
            last_checkpoint_obs: None,
            last_recorded_obs: None,
            last_checkpoint_tick: 0,
            last_wal_checkpoint: Instant::now(),
            latest_tick: 0,
            latest_time_ms: 0,
            tick_rate,
        })
    }

    pub async fn record_tick(&mut self, tick: u64, time_ms: u64, obs: &SpectatorObservation) {
        self.latest_tick = self.latest_tick.max(tick);
        self.latest_time_ms = self.latest_time_ms.max(time_ms);

        let is_checkpoint = tick == 0 || tick % self.checkpoint_interval == 0;

        if is_checkpoint {
            if let Err(e) = self.write_checkpoint(tick, time_ms, obs).await {
                eprintln!(
                    "Warning: failed to write checkpoint at tick {}: {}",
                    tick, e
                );
            }
            self.last_checkpoint_obs = Some(obs.clone());
            self.last_recorded_obs = Some(obs.clone());
            self.last_checkpoint_tick = tick;
            // Persist progress periodically so replay remains valid after abrupt shutdown.
            if let Err(e) = self
                .upsert_progress_meta(self.latest_tick, self.latest_time_ms)
                .await
            {
                eprintln!(
                    "Warning: failed to upsert recording progress at tick {}: {}",
                    tick, e
                );
            }
        } else if let Some(ref base) = self.last_recorded_obs {
            let delta = compute_delta(base, obs);
            if let Err(e) = self.write_delta(tick, time_ms, &delta).await {
                eprintln!("Warning: failed to write delta at tick {}: {}", tick, e);
            }
            self.last_recorded_obs = Some(obs.clone());
        } else {
            if let Err(e) = self.write_checkpoint(tick, time_ms, obs).await {
                eprintln!(
                    "Warning: failed to write initial checkpoint at tick {}: {}",
                    tick, e
                );
            }
            self.last_checkpoint_obs = Some(obs.clone());
            self.last_recorded_obs = Some(obs.clone());
            self.last_checkpoint_tick = tick;
        }

        // Periodic WAL checkpoint
        if self.last_wal_checkpoint.elapsed().as_secs() >= WAL_CHECKPOINT_SECS {
            let _ = sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
                .execute(&self.pool)
                .await;
            self.last_wal_checkpoint = Instant::now();
        }
    }

    pub async fn record_input(
        &self,
        tick: u64,
        time_ms: u64,
        agent_id: &Uuid,
        input_type: &str,
        data: &serde_json::Value,
    ) {
        let data_str = serde_json::to_string(data).unwrap_or_default();
        if let Err(e) = sqlx::query(
            "INSERT INTO inputs (tick, time_ms, agent_id, input_type, data) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(tick as i64)
        .bind(time_ms as i64)
        .bind(agent_id.to_string())
        .bind(input_type)
        .bind(&data_str)
        .execute(&self.pool)
        .await
        {
            eprintln!("Warning: failed to record input at tick {}: {}", tick, e);
        }
    }

    pub async fn record_chat(
        &self,
        tick: u64,
        time_ms: u64,
        message_id: &str,
        agent_id: &Uuid,
        agent_name: &str,
        content: &str,
        created_at: &str,
        delivery_mode: &str,
        hearing_radius: Option<f32>,
        heard_by: &[String],
        audibility: &[RecordedChatAudibilityEntry],
    ) {
        let heard_by_json = serde_json::to_string(heard_by).unwrap_or_else(|_| "[]".to_string());
        let audibility_json =
            serde_json::to_string(audibility).unwrap_or_else(|_| "[]".to_string());
        if let Err(e) = sqlx::query(
            "INSERT INTO chat_messages (
                tick, time_ms, message_id, agent_id, agent_name, content,
                created_at, delivery_mode, hearing_radius, heard_by, audibility
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(tick as i64)
        .bind(time_ms as i64)
        .bind(message_id)
        .bind(agent_id.to_string())
        .bind(agent_name)
        .bind(content)
        .bind(created_at)
        .bind(delivery_mode)
        .bind(hearing_radius.map(f64::from))
        .bind(heard_by_json)
        .bind(audibility_json)
        .execute(&self.pool)
        .await
        {
            eprintln!("Warning: failed to record chat at tick {}: {}", tick, e);
        }
    }

    pub async fn close(&mut self, total_ticks: u64, duration_ms: u64) -> RecordingFinalInfo {
        let final_ticks = total_ticks.max(self.latest_tick);
        let final_duration_ms = duration_ms.max(self.latest_time_ms);
        let _ = self
            .upsert_progress_meta(final_ticks, final_duration_ms)
            .await;
        let _ = sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(&self.pool)
            .await;
        let _ = sqlx::query("PRAGMA journal_mode=DELETE")
            .execute(&self.pool)
            .await;
        self.pool.close().await;
        RecordingFinalInfo {
            first_tick: 0,
            total_ticks: final_ticks,
            tick_rate: self.tick_rate,
            duration_ms: final_duration_ms,
        }
    }

    async fn upsert_progress_meta(
        &self,
        total_ticks: u64,
        duration_ms: u64,
    ) -> Result<(), sqlx::Error> {
        for (k, v) in [
            ("total_ticks", total_ticks.to_string()),
            ("duration_ms", duration_ms.to_string()),
        ] {
            sqlx::query("INSERT OR REPLACE INTO meta (key, value) VALUES (?, ?)")
                .bind(k)
                .bind(&v)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    async fn write_checkpoint(
        &self,
        tick: u64,
        time_ms: u64,
        obs: &SpectatorObservation,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_vec(obs)?;
        let compressed = zstd::encode_all(json.as_slice(), 3)?;
        sqlx::query("INSERT INTO checkpoints (tick, time_ms, data) VALUES (?, ?, ?)")
            .bind(tick as i64)
            .bind(time_ms as i64)
            .bind(&compressed)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn write_delta(
        &self,
        tick: u64,
        time_ms: u64,
        delta: &FrameDelta,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_vec(delta)?;
        let compressed = zstd::encode_all(json.as_slice(), 3)?;
        sqlx::query("INSERT INTO deltas (tick, time_ms, data) VALUES (?, ?, ?)")
            .bind(tick as i64)
            .bind(time_ms as i64)
            .bind(&compressed)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

fn compute_delta(base: &SpectatorObservation, current: &SpectatorObservation) -> FrameDelta {
    let mut entries = Vec::new();

    // Build lookup of base entities by id
    let base_entities: HashMap<u32, &SpectatorEntity> =
        base.entities.iter().map(|e| (e.id, e)).collect();
    let current_entities: HashMap<u32, &SpectatorEntity> =
        current.entities.iter().map(|e| (e.id, e)).collect();

    // Additions and changes
    for entity in &current.entities {
        match base_entities.get(&entity.id) {
            None => {
                entries.push(DeltaEntry::Add {
                    id: entity.id,
                    add: entity.clone(),
                });
            }
            Some(base_entity) => {
                if !spectator_entity_schema_matches(base_entity, entity) {
                    entries.push(DeltaEntry::Add {
                        id: entity.id,
                        add: entity.clone(),
                    });
                    continue;
                }

                let pos_changed = entity.position != base_entity.position;
                let rot_changed = entity.rotation != base_entity.rotation;
                if pos_changed || rot_changed {
                    entries.push(DeltaEntry::Update {
                        id: entity.id,
                        p: if pos_changed {
                            Some(entity.position)
                        } else {
                            None
                        },
                        r: if rot_changed { entity.rotation } else { None },
                    });
                }
            }
        }
    }

    // Removals
    for id in base_entities.keys() {
        if !current_entities.contains_key(id) {
            entries.push(DeltaEntry::Remove { id: *id, rm: true });
        }
    }

    let player_deltas = compute_player_delta(base, current);

    // Sounds and events: include full arrays if non-empty (they're small and stateful)
    let sounds: Vec<serde_json::Value> = if current.sounds != base.sounds {
        current
            .sounds
            .iter()
            .filter_map(|s| serde_json::to_value(s).ok())
            .collect()
    } else {
        Vec::new()
    };

    let recent_events: Vec<serde_json::Value> = if !current.recent_events.is_empty() {
        current
            .recent_events
            .iter()
            .filter_map(|e| serde_json::to_value(e).ok())
            .collect()
    } else {
        Vec::new()
    };

    FrameDelta {
        entities: entries,
        players: player_deltas,
        sounds,
        recent_events,
    }
}

fn spectator_render_matches(a: &SpectatorRender, b: &SpectatorRender) -> bool {
    a.kind == b.kind
        && a.role == b.role
        && a.preset_id == b.preset_id
        && a.primitive == b.primitive
        && a.material == b.material
        && a.color == b.color
        && a.is_static == b.is_static
        && a.casts_shadow == b.casts_shadow
        && a.receives_shadow == b.receives_shadow
        && a.visible == b.visible
        && a.double_sided == b.double_sided
        && a.transparency == b.transparency
}

fn spectator_entity_schema_matches(a: &SpectatorEntity, b: &SpectatorEntity) -> bool {
    let billboard_matches =
        serde_json::to_value(&a.billboard_gui).ok() == serde_json::to_value(&b.billboard_gui).ok();

    a.entity_type == b.entity_type
        && a.size == b.size
        && a.shape == b.shape
        && spectator_render_matches(&a.render, &b.render)
        && a.health == b.health
        && a.pickup_type == b.pickup_type
        && a.model_url == b.model_url
        && a.model_yaw_offset_deg == b.model_yaw_offset_deg
        && a.attributes == b.attributes
        && billboard_matches
}

fn compute_player_delta(
    base: &SpectatorObservation,
    current: &SpectatorObservation,
) -> Vec<PlayerDelta> {
    let base_ids: Vec<Uuid> = base.players.iter().map(|p| p.id).collect();
    let current_ids: Vec<Uuid> = current.players.iter().map(|p| p.id).collect();
    if base_ids != current_ids {
        return vec![PlayerDelta::Replace {
            replace: current.players.clone(),
        }];
    }

    let mut player_deltas = Vec::new();
    for (base_player, player) in base.players.iter().zip(current.players.iter()) {
        let pos_changed = player.position != base_player.position;
        let health_changed = player.health != base_player.health;
        let base_json = serde_json::to_value(base_player).ok();
        let current_json = serde_json::to_value(player).ok();

        if base_json != current_json && !(pos_changed || health_changed) {
            return vec![PlayerDelta::Replace {
                replace: current.players.clone(),
            }];
        }

        if pos_changed || health_changed {
            player_deltas.push(PlayerDelta::Update {
                id: player.id,
                p: pos_changed.then_some(player.position),
                health: health_changed.then_some(player.health),
            });
        }
    }

    player_deltas
}

// --- Reader ---

pub struct RecordingMeta {
    pub world_name: String,
    pub world_dir: String,
    pub created_at: String,
    pub tick_rate: u32,
    pub total_ticks: u64,
    pub duration_ms: u64,
}

pub struct RecordingReader {
    pool: SqlitePool,
    checkpoint_ticks: Vec<u64>,
}

impl RecordingReader {
    pub async fn open(path: &Path) -> Result<Self, sqlx::Error> {
        let opts = SqliteConnectOptions::new()
            .filename(path)
            .read_only(true)
            .immutable(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await?;

        let rows = sqlx::query("SELECT tick FROM checkpoints ORDER BY tick")
            .fetch_all(&pool)
            .await?;
        let checkpoint_ticks: Vec<u64> = rows
            .iter()
            .map(|r| r.get::<i64, _>("tick") as u64)
            .collect();

        Ok(Self {
            pool,
            checkpoint_ticks,
        })
    }

    pub async fn meta(&self) -> Result<RecordingMeta, sqlx::Error> {
        let rows = sqlx::query("SELECT key, value FROM meta")
            .fetch_all(&self.pool)
            .await?;
        let map: HashMap<String, String> = rows
            .iter()
            .map(|r| (r.get::<String, _>("key"), r.get::<String, _>("value")))
            .collect();

        let tick_rate = map
            .get("tick_rate")
            .and_then(|v| v.parse().ok())
            .unwrap_or(60);
        let mut total_ticks = map
            .get("total_ticks")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let mut duration_ms = map
            .get("duration_ms")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        let row = sqlx::query(
            "SELECT
                COALESCE(MAX(tick), 0) AS max_tick,
                COALESCE(MAX(time_ms), 0) AS max_time_ms
             FROM (
               SELECT tick, time_ms FROM checkpoints
               UNION ALL
               SELECT tick, time_ms FROM deltas
             )",
        )
        .fetch_one(&self.pool)
        .await?;
        let max_tick = row.get::<i64, _>("max_tick").max(0) as u64;
        let max_time_ms = row.get::<i64, _>("max_time_ms").max(0) as u64;

        total_ticks = total_ticks.max(max_tick);
        duration_ms = duration_ms.max(max_time_ms);
        if duration_ms == 0 && total_ticks > 0 {
            duration_ms = total_ticks.saturating_mul(1000) / tick_rate as u64;
        }

        Ok(RecordingMeta {
            world_name: map.get("world_name").cloned().unwrap_or_default(),
            world_dir: map.get("world_dir").cloned().unwrap_or_default(),
            created_at: map.get("created_at").cloned().unwrap_or_default(),
            tick_rate,
            total_ticks,
            duration_ms,
        })
    }

    pub fn total_ticks(&self) -> u64 {
        self.checkpoint_ticks.last().copied().unwrap_or(0)
    }

    pub fn first_tick(&self) -> Option<u64> {
        self.checkpoint_ticks.first().copied()
    }

    pub async fn get_state_at(
        &self,
        tick: u64,
    ) -> Result<SpectatorObservation, Box<dyn std::error::Error + Send + Sync>> {
        // Find nearest checkpoint <= tick
        let cp_tick = match self.checkpoint_ticks.binary_search(&tick) {
            Ok(i) => self.checkpoint_ticks[i],
            Err(0) => {
                return Err("tick before first checkpoint".into());
            }
            Err(i) => self.checkpoint_ticks[i - 1],
        };

        // Load checkpoint
        let row = sqlx::query("SELECT data FROM checkpoints WHERE tick = ?")
            .bind(cp_tick as i64)
            .fetch_one(&self.pool)
            .await?;
        let compressed: Vec<u8> = row.get("data");
        let json = zstd::decode_all(compressed.as_slice())?;
        let mut obs: SpectatorObservation = serde_json::from_slice(&json)?;

        if cp_tick == tick {
            return Ok(obs);
        }

        // Apply deltas from cp_tick+1 to tick
        let delta_rows =
            sqlx::query("SELECT data FROM deltas WHERE tick > ? AND tick <= ? ORDER BY tick")
                .bind(cp_tick as i64)
                .bind(tick as i64)
                .fetch_all(&self.pool)
                .await?;

        for delta_row in &delta_rows {
            let compressed: Vec<u8> = delta_row.get("data");
            let json = zstd::decode_all(compressed.as_slice())?;
            let delta: FrameDelta = serde_json::from_slice(&json)?;
            apply_delta(&mut obs, &delta);
        }

        obs.tick = tick;
        Ok(obs)
    }

    pub async fn active_tick_ranges(
        &self,
    ) -> Result<Vec<ReplayActiveRange>, Box<dyn std::error::Error + Send + Sync>> {
        let rows = sqlx::query("SELECT tick, data FROM deltas ORDER BY tick")
            .fetch_all(&self.pool)
            .await?;

        let mut ranges = Vec::new();
        let mut current_start: Option<u64> = None;
        let mut current_end: u64 = 0;

        for row in rows {
            let tick = row.get::<i64, _>("tick").max(0) as u64;
            let compressed: Vec<u8> = row.get("data");
            let json = zstd::decode_all(compressed.as_slice())?;
            let delta: FrameDelta = serde_json::from_slice(&json)?;

            if delta.is_empty() {
                if let Some(start_tick) = current_start.take() {
                    ranges.push(ReplayActiveRange {
                        start_tick,
                        end_tick: current_end,
                    });
                }
                continue;
            }

            if current_start.is_none() {
                current_start = Some(tick);
            }
            current_end = tick;
        }

        if let Some(start_tick) = current_start {
            ranges.push(ReplayActiveRange {
                start_tick,
                end_tick: current_end,
            });
        }

        Ok(ranges)
    }

    /// Get chat messages in a tick range (inclusive on both ends).
    pub async fn get_chat_messages(
        &self,
        start_tick: u64,
        end_tick: u64,
    ) -> Result<Vec<RecordedChatMessage>, sqlx::Error> {
        let has_created_at = self.chat_messages_has_column("created_at").await?;
        let has_delivery_mode = self.chat_messages_has_column("delivery_mode").await?;
        let has_hearing_radius = self.chat_messages_has_column("hearing_radius").await?;
        let has_heard_by = self.chat_messages_has_column("heard_by").await?;
        let has_audibility = self.chat_messages_has_column("audibility").await?;

        let mut select_fields = vec![
            "tick",
            "time_ms",
            "message_id",
            "agent_id",
            "agent_name",
            "content",
        ];
        if has_created_at {
            select_fields.push("created_at");
        }
        if has_delivery_mode {
            select_fields.push("delivery_mode");
        }
        if has_hearing_radius {
            select_fields.push("hearing_radius");
        }
        if has_heard_by {
            select_fields.push("heard_by");
        }
        if has_audibility {
            select_fields.push("audibility");
        }

        let query = format!(
            "SELECT {}
             FROM chat_messages
             WHERE tick >= ? AND tick <= ?
             ORDER BY tick, time_ms",
            select_fields.join(", ")
        );
        let rows = sqlx::query(&query)
            .bind(start_tick as i64)
            .bind(end_tick as i64)
            .fetch_all(&self.pool)
            .await?;

        Ok(rows
            .iter()
            .map(|r| RecordedChatMessage {
                tick: r.get::<i64, _>("tick") as u64,
                time_ms: r.get::<i64, _>("time_ms") as u64,
                message_id: r.get::<String, _>("message_id"),
                agent_id: r.get::<String, _>("agent_id"),
                agent_name: r.get::<String, _>("agent_name"),
                content: r.get::<String, _>("content"),
                created_at: if has_created_at {
                    r.get::<String, _>("created_at")
                } else {
                    String::new()
                },
                delivery_mode: if has_delivery_mode {
                    r.get::<String, _>("delivery_mode")
                } else {
                    "global".to_string()
                },
                hearing_radius: if has_hearing_radius {
                    r.get::<Option<f64>, _>("hearing_radius").map(|v| v as f32)
                } else {
                    None
                },
                heard_by: if has_heard_by {
                    serde_json::from_str(&r.get::<String, _>("heard_by")).unwrap_or_default()
                } else {
                    Vec::new()
                },
                audibility: if has_audibility {
                    serde_json::from_str(&r.get::<String, _>("audibility")).unwrap_or_default()
                } else {
                    Vec::new()
                },
            })
            .collect())
    }

    pub async fn get_spectator_delta_between(
        &self,
        from_tick: u64,
        to_tick: u64,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let delta_rows =
            sqlx::query("SELECT tick, time_ms, data FROM deltas WHERE tick > ? AND tick <= ? ORDER BY tick")
                .bind(from_tick as i64)
                .bind(to_tick as i64)
                .fetch_all(&self.pool)
                .await?;

        let mut entities_by_id = HashMap::<u32, DeltaEntry>::new();
        let mut players = Vec::<PlayerDelta>::new();
        let mut sounds = Vec::<serde_json::Value>::new();
        let mut recent_events = Vec::<serde_json::Value>::new();
        let mut latest_time_ms = None;

        for row in &delta_rows {
            latest_time_ms = Some(row.get::<i64, _>("time_ms").max(0) as u64);
            let compressed: Vec<u8> = row.get("data");
            let json = zstd::decode_all(compressed.as_slice())?;
            let delta: FrameDelta = serde_json::from_slice(&json)?;
            for entity in delta.entities {
                coalesce_entity_delta(&mut entities_by_id, entity);
            }
            players.extend(delta.players);
            sounds.extend(delta.sounds);
            recent_events.extend(delta.recent_events);
        }
        let mut entities = Vec::with_capacity(entities_by_id.len());
        for entry in entities_by_id.into_values() {
            entities.push(serde_json::to_value(entry)?);
        }

        let mut payload = serde_json::json!({
            "type": "spectator_delta",
            "tick": to_tick,
            "server_time_ms": latest_time_ms.unwrap_or(0),
            "entities": entities,
        });

        if !sounds.is_empty() {
            payload["sounds"] = serde_json::Value::Array(sounds);
        }
        if !recent_events.is_empty() {
            payload["recent_events"] = serde_json::Value::Array(recent_events);
        }
        if !players.is_empty() {
            payload["players"] = serde_json::to_value(players)?;
        }

        Ok(payload)
    }

    async fn chat_messages_has_column(&self, name: &str) -> Result<bool, sqlx::Error> {
        let rows = sqlx::query("PRAGMA table_info(chat_messages)")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.iter().any(|row| row.get::<String, _>("name") == name))
    }
}

pub struct RecordedChatMessage {
    pub tick: u64,
    pub time_ms: u64,
    pub message_id: String,
    pub agent_id: String,
    pub agent_name: String,
    pub content: String,
    pub created_at: String,
    pub delivery_mode: String,
    pub hearing_radius: Option<f32>,
    pub heard_by: Vec<String>,
    pub audibility: Vec<RecordedChatAudibilityEntry>,
}

async fn ensure_chat_messages_column(
    pool: &SqlitePool,
    column_def: &str,
) -> Result<(), sqlx::Error> {
    let query = format!("ALTER TABLE chat_messages ADD COLUMN {}", column_def);
    match sqlx::query(&query).execute(pool).await {
        Ok(_) => Ok(()),
        Err(sqlx::Error::Database(db_err))
            if db_err.message().contains("duplicate column name") =>
        {
            Ok(())
        }
        Err(err) => Err(err),
    }
}

fn apply_delta(obs: &mut SpectatorObservation, delta: &FrameDelta) {
    // Apply entity deltas
    for entry in &delta.entities {
        match entry {
            DeltaEntry::Add { id: _, add } => {
                if let Some(entity) = obs.entities.iter_mut().find(|e| e.id == add.id) {
                    *entity = add.clone();
                } else {
                    obs.entities.push(add.clone());
                }
            }
            DeltaEntry::Remove { id, rm: _ } => {
                obs.entities.retain(|e| e.id != *id);
            }
            DeltaEntry::Update { id, p, r } => {
                if let Some(entity) = obs.entities.iter_mut().find(|e| e.id == *id) {
                    if let Some(pos) = p {
                        entity.position = *pos;
                    }
                    if let Some(rot) = r {
                        entity.rotation = Some(*rot);
                    }
                }
            }
        }
    }

    // Apply player deltas
    for pd in &delta.players {
        match pd {
            PlayerDelta::Replace { replace } => {
                obs.players = replace.clone();
            }
            PlayerDelta::Update { id, p, health } => {
                if let Some(player) = obs.players.iter_mut().find(|p| p.id == *id) {
                    if let Some(pos) = p {
                        player.position = *pos;
                    }
                    if let Some(health) = health {
                        player.health = *health;
                    }
                }
            }
        }
    }

    // Replace sounds if delta has any
    if !delta.sounds.is_empty() {
        if let Ok(sounds) = serde_json::from_value(serde_json::Value::Array(delta.sounds.clone())) {
            obs.sounds = sounds;
        }
    }

    // Replace recent_events
    if !delta.recent_events.is_empty() {
        if let Ok(events) =
            serde_json::from_value(serde_json::Value::Array(delta.recent_events.clone()))
        {
            obs.recent_events = events;
        }
    } else {
        obs.recent_events.clear();
    }
}

fn coalesce_entity_delta(entries: &mut HashMap<u32, DeltaEntry>, entry: DeltaEntry) {
    match entry {
        DeltaEntry::Add { id, add } => {
            entries.insert(id, DeltaEntry::Add { id, add });
        }
        DeltaEntry::Remove { id, rm } => {
            entries.insert(id, DeltaEntry::Remove { id, rm });
        }
        DeltaEntry::Update { id, p, r } => match entries.get_mut(&id) {
            Some(DeltaEntry::Add { add, .. }) => {
                if let Some(pos) = p {
                    add.position = pos;
                }
                if r.is_some() {
                    add.rotation = r;
                }
            }
            Some(DeltaEntry::Update {
                p: existing_p,
                r: existing_r,
                ..
            }) => {
                if p.is_some() {
                    *existing_p = p;
                }
                if r.is_some() {
                    *existing_r = r;
                }
            }
            Some(DeltaEntry::Remove { .. }) => {}
            None => {
                entries.insert(id, DeltaEntry::Update { id, p, r });
            }
        },
    }
}

// Need PartialEq for SpectatorSound comparison in compute_delta
impl PartialEq for crate::game::instance::SpectatorSound {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.stream_id == other.stream_id
            && self.volume == other.volume
            && self.is_playing == other.is_playing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::instance::{SpectatorRender, SpectatorReplicatedEvent};
    use std::path::PathBuf;

    fn test_render(material: &str) -> SpectatorRender {
        SpectatorRender {
            kind: "primitive".to_string(),
            role: "part".to_string(),
            preset_id: None,
            primitive: "box".to_string(),
            material: material.to_string(),
            color: [1.0, 0.0, 0.0],
            is_static: false,
            casts_shadow: true,
            receives_shadow: true,
            visible: true,
            double_sided: false,
            transparency: None,
        }
    }

    fn test_entity(id: u32, position: [f32; 3], material: &str) -> SpectatorEntity {
        SpectatorEntity {
            id,
            entity_type: "part".to_string(),
            position,
            rotation: None,
            size: Some([1.0, 1.0, 1.0]),
            shape: Some("Block".to_string()),
            render: test_render(material),
            health: None,
            pickup_type: None,
            model_url: None,
            model_yaw_offset_deg: None,
            attributes: None,
            billboard_gui: None,
        }
    }

    fn test_player(id: Uuid, name: &str, position: [f32; 3]) -> SpectatorPlayerInfo {
        SpectatorPlayerInfo {
            id,
            name: name.to_string(),
            position,
            root_part_id: Some(100),
            health: 100,
            humanoid_state: Some("Running".to_string()),
            attributes: None,
            gui: None,
            active_animations: None,
        }
    }

    fn test_observation(tick: u64, entities: Vec<SpectatorEntity>) -> SpectatorObservation {
        SpectatorObservation {
            instance_id: Uuid::from_u128(1),
            tick,
            server_time_ms: tick * 16,
            game_status: "playing".to_string(),
            players: Vec::new(),
            entities,
            sounds: Vec::new(),
            recent_events: Vec::<SpectatorReplicatedEvent>::new(),
        }
    }

    fn temp_recording_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "clawblox-recording-test-{}-{}.clawrec",
            name,
            Uuid::new_v4()
        ))
    }

    #[test]
    fn applying_add_for_existing_entity_replaces_instead_of_duplicates() {
        let mut obs = test_observation(0, vec![test_entity(7, [0.0, 0.0, 0.0], "Plastic")]);
        let replacement = test_entity(7, [1.0, 0.0, 0.0], "Neon");
        let delta = FrameDelta {
            entities: vec![DeltaEntry::Add {
                id: 7,
                add: replacement,
            }],
            players: Vec::new(),
            sounds: Vec::new(),
            recent_events: Vec::new(),
        };

        apply_delta(&mut obs, &delta);

        assert_eq!(obs.entities.len(), 1);
        assert_eq!(obs.entities[0].position, [1.0, 0.0, 0.0]);
        assert_eq!(obs.entities[0].render.material, "Neon");
    }

    #[test]
    fn schema_changes_emit_replacement_entity() {
        let base = test_observation(0, vec![test_entity(7, [0.0, 0.0, 0.0], "Plastic")]);
        let current = test_observation(1, vec![test_entity(7, [0.0, 0.0, 0.0], "Neon")]);

        let delta = compute_delta(&base, &current);

        assert!(matches!(
            delta.entities.as_slice(),
            [DeltaEntry::Add { id: 7, add }] if add.render.material == "Neon"
        ));
    }

    #[test]
    fn player_list_changes_emit_full_replacement() {
        let mut base = test_observation(0, Vec::new());
        base.players = vec![test_player(Uuid::from_u128(1), "A", [0.0, 0.0, 0.0])];
        let mut current = test_observation(1, Vec::new());
        current.players = vec![
            test_player(Uuid::from_u128(1), "A", [1.0, 0.0, 0.0]),
            test_player(Uuid::from_u128(2), "B", [2.0, 0.0, 0.0]),
        ];

        let delta = compute_delta(&base, &current);
        let mut reconstructed = base.clone();
        apply_delta(&mut reconstructed, &delta);

        assert!(matches!(
            delta.players.as_slice(),
            [PlayerDelta::Replace { replace }] if replace.len() == 2
        ));
        assert_eq!(reconstructed.players.len(), 2);
        assert_eq!(reconstructed.players[0].position, [1.0, 0.0, 0.0]);
        assert_eq!(reconstructed.players[1].name, "B");
    }

    #[tokio::test]
    async fn writer_records_frame_relative_deltas_that_replay_without_duplicate_adds() {
        let path = temp_recording_path("frame-relative-deltas");
        let mut writer = RecordingWriter::create(&path, "test", "/tmp/test-world", 60)
            .await
            .unwrap();

        writer
            .record_tick(0, 0, &test_observation(0, Vec::new()))
            .await;
        writer
            .record_tick(
                1,
                16,
                &test_observation(1, vec![test_entity(7, [0.0, 0.0, 0.0], "Plastic")]),
            )
            .await;
        writer
            .record_tick(
                2,
                32,
                &test_observation(2, vec![test_entity(7, [0.0, 0.0, 0.0], "Plastic")]),
            )
            .await;
        writer.close(2, 32).await;

        let reader = RecordingReader::open(&path).await.unwrap();
        let reconstructed = reader.get_state_at(2).await.unwrap();

        assert_eq!(reconstructed.entities.len(), 1);
        assert_eq!(reconstructed.entities[0].id, 7);

        let _ = std::fs::remove_file(&path);
    }
}
