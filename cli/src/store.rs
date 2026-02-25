use anyhow::{Context, Result};
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::PathBuf;

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
