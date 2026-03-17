use anyhow::{Context, Result};
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// SQLite schema — mirrors the SDK's events table.
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS events (
    local_event_uuid TEXT PRIMARY KEY,
    bandit_id        INTEGER NOT NULL,
    arm_id           INTEGER NOT NULL,
    payload          TEXT NOT NULL,
    status           TEXT NOT NULL DEFAULT 'pending',
    created_at       REAL NOT NULL,
    human_reward     REAL,
    graded_at        REAL,
    s3_exported      INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_events_status ON events(status);
";

/// Read-only access to the SDK's local SQLite event store.
/// Used by the TUI to display query/response text without cloud round-trips.
pub struct EventStore {
    conn: Connection,
}

/// Text fields extracted from a local event payload.
pub struct EventText {
    pub query_text: Option<String>,
    pub response: Option<String>,
    pub system_prompt: Option<String>,
}

/// Event fields needed for judge calibration / augmentation.
pub struct JudgeEvent {
    pub uuid: String,
    pub arm_id: i64,
    pub query_text: Option<String>,
    pub response: Option<String>,
    #[allow(dead_code)]  // stored for future prompt enrichment
    pub system_prompt: Option<String>,
    pub human_reward: Option<f64>,
}

impl EventStore {
    pub fn open() -> Result<Option<Self>> {
        let path = Self::db_path()?;
        if !path.exists() {
            return Ok(None);
        }
        let conn = Connection::open_with_flags(
            &path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .with_context(|| format!("Failed to open {}", path.display()))?;
        Ok(Some(Self { conn }))
    }

    /// Create a writable SQLite at `path` (for S3 TUI hydration).
    pub fn create_ephemeral(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to create ephemeral store at {}", path.display()))?;
        conn.execute_batch(SCHEMA)
            .context("Failed to initialize ephemeral store schema")?;
        Ok(Self { conn })
    }

    /// Insert a raw event payload downloaded from S3. No-op if UUID already present.
    pub fn insert_payload(&self, payload: &serde_json::Value, created_at: f64) -> Result<()> {
        let uuid = payload["local_event_uuid"].as_str().unwrap_or("").to_string();
        if uuid.is_empty() {
            return Ok(());
        }
        let bandit_id = payload["bandit_id"].as_i64().unwrap_or(0);
        let arm_id = payload["arm_id"].as_i64().unwrap_or(0);
        self.conn.execute(
            "INSERT OR IGNORE INTO events \
             (local_event_uuid, bandit_id, arm_id, payload, status, created_at) \
             VALUES (?1, ?2, ?3, ?4, 'flushed', ?5)",
            rusqlite::params![uuid, bandit_id, arm_id, payload.to_string(), created_at],
        )?;
        Ok(())
    }

    fn db_path() -> Result<PathBuf> {
        let home = dirs::home_dir().context("Could not determine home directory")?;
        Ok(home.join(".bandito").join("events.db"))
    }

    /// Fetch text fields for a list of event UUIDs.
    /// Returns a map from UUID to EventText.
    pub fn get_text(&self, uuids: &[String]) -> Result<HashMap<String, EventText>> {
        if uuids.is_empty() {
            return Ok(HashMap::new());
        }

        let placeholders: Vec<&str> = uuids.iter().map(|_| "?").collect();
        let sql = format!(
            "SELECT local_event_uuid, payload FROM events WHERE local_event_uuid IN ({})",
            placeholders.join(",")
        );

        let mut stmt = self.conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::types::ToSql> =
            uuids.iter().map(|u| u as &dyn rusqlite::types::ToSql).collect();

        let mut result = HashMap::new();
        let rows = stmt.query_map(params.as_slice(), |row| {
            let uuid: String = row.get(0)?;
            let payload: String = row.get(1)?;
            Ok((uuid, payload))
        })?;

        for row in rows {
            let (uuid, payload) = row?;
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&payload) {
                let text = EventText {
                    query_text: json
                        .get("query_text")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    response: extract_response_text(&json),
                    system_prompt: json
                        .get("system_prompt")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                };
                result.insert(uuid, text);
            }
        }

        Ok(result)
    }

    /// Fetch flushed events that have a human reward, stratified by arm.
    /// Returns up to `limit_per_arm` events per arm_id.
    pub fn get_graded_events(&self, bandit_id: i64, limit_per_arm: usize) -> Result<Vec<JudgeEvent>> {
        let mut stmt = self.conn.prepare(
            "SELECT local_event_uuid, arm_id, payload, human_reward \
             FROM events \
             WHERE bandit_id = ?1 AND human_reward IS NOT NULL AND status = 'flushed' \
             ORDER BY arm_id ASC, created_at DESC",
        )?;

        let rows = stmt.query_map([bandit_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, f64>(3)?,
            ))
        })?;

        let mut counts: HashMap<i64, usize> = HashMap::new();
        let mut events = Vec::new();

        for row in rows {
            let (uuid, arm_id, payload, human_reward) = row?;
            let count = counts.entry(arm_id).or_insert(0);
            if *count >= limit_per_arm {
                continue;
            }
            *count += 1;

            let json: serde_json::Value =
                serde_json::from_str(&payload).unwrap_or(serde_json::Value::Null);
            let query_text = json.get("query_text").and_then(|v| v.as_str()).map(String::from);
            let response = extract_response_text(&json);
            let system_prompt = json.get("system_prompt").and_then(|v| v.as_str()).map(String::from);

            events.push(JudgeEvent {
                uuid,
                arm_id,
                query_text,
                response,
                system_prompt,
                human_reward: Some(human_reward),
            });
        }

        Ok(events)
    }

    /// Fetch flushed events with no human reward, stratified by arm.
    /// Returns up to `limit_per_arm` events per arm_id.
    pub fn get_ungraded_events(&self, bandit_id: i64, limit_per_arm: usize) -> Result<Vec<JudgeEvent>> {
        let mut stmt = self.conn.prepare(
            "SELECT local_event_uuid, arm_id, payload \
             FROM events \
             WHERE bandit_id = ?1 AND human_reward IS NULL AND status = 'flushed' \
             ORDER BY arm_id ASC, created_at DESC",
        )?;

        let rows = stmt.query_map([bandit_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;

        let mut counts: HashMap<i64, usize> = HashMap::new();
        let mut events = Vec::new();

        for row in rows {
            let (uuid, arm_id, payload) = row?;
            let count = counts.entry(arm_id).or_insert(0);
            if *count >= limit_per_arm {
                continue;
            }
            *count += 1;

            let json: serde_json::Value =
                serde_json::from_str(&payload).unwrap_or(serde_json::Value::Null);
            let query_text = json.get("query_text").and_then(|v| v.as_str()).map(String::from);
            let response = extract_response_text(&json);
            let system_prompt = json.get("system_prompt").and_then(|v| v.as_str()).map(String::from);

            events.push(JudgeEvent {
                uuid,
                arm_id,
                query_text,
                response,
                system_prompt,
                human_reward: None,
            });
        }

        Ok(events)
    }
}

/// Extract response text from event payload.
/// Response can be a string or a dict with a "response" key.
fn extract_response_text(json: &serde_json::Value) -> Option<String> {
    match json.get("response") {
        Some(serde_json::Value::String(s)) => Some(s.clone()),
        Some(serde_json::Value::Object(obj)) => obj
            .get("response")
            .and_then(|v| v.as_str())
            .map(String::from),
        _ => None,
    }
}
