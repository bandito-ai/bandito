//! S3 hydration for the TUI grading workbench.
//!
//! When `data_storage = "s3"`, event text (query/response) lives in S3 rather
//! than the local SDK SQLite. This module downloads the relevant event objects
//! into a temporary SQLite so the rest of the TUI can read from it unchanged.
//!
//! Flow:
//!   cloud API  →  ungraded event UUIDs + created_at
//!   s3.rs      →  GetObject per UUID  →  ephemeral SQLite
//!   state.rs   →  EventStore::get_text()  (same path as local mode)

use anyhow::{Context, Result};
use aws_sdk_s3::config::Region;
use tokio::runtime::Runtime;

use crate::config::S3Config;
use crate::store::EventStore;

pub struct S3Hydrator {
    rt: Runtime,
    client: aws_sdk_s3::Client,
    pub config: S3Config,
}

impl S3Hydrator {
    pub fn new(config: S3Config) -> Result<Self> {
        let rt = Runtime::new().context("Failed to create Tokio runtime for S3")?;

        let client = rt.block_on(async {
            let aws_cfg = aws_config::defaults(aws_config::BehaviorVersion::latest())
                .region(Region::new(config.region.clone()))
                .load()
                .await;

            let mut builder = aws_sdk_s3::config::Builder::from(&aws_cfg)
                // Path-style required for MinIO and other self-hosted stores
                .force_path_style(true);

            if let Some(ref ep) = config.endpoint {
                builder = builder.endpoint_url(ep.as_str());
            }

            aws_sdk_s3::Client::from_conf(builder.build())
        });

        Ok(Self { rt, client, config })
    }

    /// Download S3 event objects for the given cloud API items.
    ///
    /// Creates a fresh ephemeral SQLite at `/tmp/bandito_tui_{pid}.db`,
    /// populates it with downloaded payloads, and returns it.
    ///
    /// Returns the store even if some fetches fail — missing events simply
    /// show no query/response text in the TUI (same as local mode with no DB).
    pub fn hydrate(
        &self,
        bandit_name: &str,
        items: &[serde_json::Value],
    ) -> Result<(EventStore, usize, usize)> {
        let db_path = std::env::temp_dir()
            .join(format!("bandito_tui_{}.db", std::process::id()));
        let _ = std::fs::remove_file(&db_path);
        let store = EventStore::create_ephemeral(&db_path)?;

        let sanitized = sanitize_name(bandit_name);
        let mut fetched = 0usize;
        let mut missing = 0usize;

        for item in items {
            let uuid = match item["local_event_uuid"].as_str() {
                Some(u) if !u.is_empty() => u,
                _ => continue,
            };
            let created_at_str = item["created_at"].as_str().unwrap_or("");
            let date_path = date_from_iso(created_at_str);
            let key = format!(
                "{}/{}/{}/{}.json",
                self.config.prefix, sanitized, date_path, uuid
            );
            let ts = ts_from_iso(created_at_str);

            match self.rt.block_on(self.get_object(&key)) {
                Ok(body) => {
                    if let Ok(payload) = serde_json::from_slice::<serde_json::Value>(&body) {
                        let _ = store.insert_payload(&payload, ts);
                        fetched += 1;
                    } else {
                        missing += 1;
                    }
                }
                Err(_) => {
                    missing += 1;
                }
            }
        }

        Ok((store, fetched, missing))
    }

    async fn get_object(&self, key: &str) -> Result<Vec<u8>> {
        let resp = self
            .client
            .get_object()
            .bucket(&self.config.bucket)
            .key(key)
            .send()
            .await
            .with_context(|| format!("S3 GetObject failed: {}", key))?;

        let bytes = resp
            .body
            .collect()
            .await
            .context("Failed to read S3 response body")?;

        Ok(bytes.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_name() {
        assert_eq!(sanitize_name("my-chatbot"), "my-chatbot");
        assert_eq!(sanitize_name("my/chatbot"), "my_chatbot");
        assert_eq!(sanitize_name("foo bar"), "foo_bar");
        assert_eq!(sanitize_name("v1.0"), "v1.0");
    }

    #[test]
    fn test_date_from_iso() {
        assert_eq!(date_from_iso("2024-03-17T10:30:00Z"), "2024/03/17");
        assert_eq!(date_from_iso("2025-01-01"), "2025/01/01");
        assert_eq!(date_from_iso(""), "1970/01/01");
    }

    #[test]
    fn test_ts_from_iso() {
        let ts = ts_from_iso("1970-01-01T00:00:00Z");
        assert_eq!(ts, 0.0);
        let ts2 = ts_from_iso("2024-01-01T00:00:00Z");
        assert!(ts2 > 0.0);
    }

    /// Integration test against a local MinIO instance.
    /// Run with: BANDITO_TEST_S3=1 cargo test -p bandito-cli -- s3::tests::test_hydrate_from_minio --nocapture
    #[test]
    fn test_hydrate_from_minio() {
        if std::env::var("BANDITO_TEST_S3").is_err() {
            return; // skip unless explicitly enabled
        }

        let config = crate::config::S3Config {
            bucket: "bandito-test".to_string(),
            prefix: "bandito".to_string(),
            region: "us-east-1".to_string(),
            endpoint: Some("http://localhost:9000".to_string()),
        };

        // Set MinIO credentials in env for this test
        std::env::set_var("AWS_ACCESS_KEY_ID", "minioadmin");
        std::env::set_var("AWS_SECRET_ACCESS_KEY", "minioadmin");

        let hydrator = S3Hydrator::new(config).expect("Failed to build S3Hydrator");

        // Simulate a cloud API item for the seeded event
        let items = vec![serde_json::json!({
            "local_event_uuid": "test-uuid-1234-5678-abcd",
            "created_at": "2025-03-17T00:00:00Z",
        })];

        let (store, fetched, missing) =
            hydrator.hydrate("my-chatbot", &items).expect("Hydrate failed");
        assert_eq!(fetched, 1, "Expected 1 event fetched");
        assert_eq!(missing, 0, "Expected 0 missing");

        let text = store
            .get_text(&["test-uuid-1234-5678-abcd".to_string()])
            .expect("get_text failed");
        let entry = text.get("test-uuid-1234-5678-abcd").expect("UUID not in store");
        assert_eq!(
            entry.query_text.as_deref(),
            Some("What is the capital of France?")
        );
        assert!(entry.response.is_some());
        println!("query: {:?}", entry.query_text);
        println!("response: {:?}", entry.response);
    }
}

/// Sanitize a bandit name for use in S3 keys — mirrors the SDK behaviour.
fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Extract YYYY/MM/DD path from an ISO 8601 timestamp string.
/// "2024-03-17T10:30:00Z" → "2024/03/17"
fn date_from_iso(s: &str) -> String {
    if s.len() >= 10 {
        s[..10].replace('-', "/")
    } else {
        "1970/01/01".to_string()
    }
}

/// Parse ISO 8601 to Unix timestamp (seconds).
fn ts_from_iso(s: &str) -> f64 {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.timestamp() as f64)
        .unwrap_or(0.0)
}
