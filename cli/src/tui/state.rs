use anyhow::Result;
use std::collections::HashSet;

use crate::http::HttpClient;
use crate::store::EventStore;

#[derive(Clone, Copy, PartialEq)]
pub enum Screen {
    BanditSelect,
    Dashboard,
    Help,
}

#[derive(Clone, Copy)]
pub enum Section {
    Query,
    Response,
    Prompt,
}

pub struct App {
    pub screen: Screen,
    pub http: HttpClient,
    pub store: Option<EventStore>,

    // Bandit select
    pub bandits: Vec<BanditInfo>,
    pub bandit_index: usize,

    // Dashboard
    pub current_bandit: Option<BanditInfo>,
    pub events: Vec<MergedEvent>,
    pub event_index: usize,
    pub skipped: HashSet<String>,
    pub status_message: Option<(String, u8)>, // (message, remaining_renders)
}

#[derive(Clone)]
pub struct BanditInfo {
    pub id: i64,
    pub name: String,
    pub bandit_type: String,
    pub arm_count: i64,
    pub total_pulls: i64,
    pub mode: String,
}

#[derive(Clone)]
pub struct MergedEvent {
    pub uuid: String,
    pub model_name: String,
    pub model_provider: String,
    pub cost: Option<f64>,
    pub latency: Option<f64>,
    pub early_reward: Option<f64>,
    #[allow(dead_code)]
    pub grade: Option<f64>,
    pub created_at: String,
    pub query_text: Option<String>,
    pub response: Option<String>,
    pub system_prompt: Option<String>,
}

impl App {
    pub fn new(http: HttpClient, store: Option<EventStore>) -> Self {
        Self {
            screen: Screen::BanditSelect,
            http,
            store,
            bandits: Vec::new(),
            bandit_index: 0,
            current_bandit: None,
            events: Vec::new(),
            event_index: 0,
            skipped: HashSet::new(),
            status_message: None,
        }
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = Some((msg.into(), 3)); // show for 3 key presses
    }

    pub fn tick_status(&mut self) {
        if let Some((_, ref mut remaining)) = self.status_message {
            if *remaining == 0 {
                self.status_message = None;
            } else {
                *remaining -= 1;
            }
        }
    }

    pub fn status_text(&self) -> Option<&str> {
        self.status_message.as_ref().map(|(msg, _)| msg.as_str())
    }

    pub fn load_bandits(&mut self) -> Result<()> {
        match self.load_bandits_inner() {
            Ok(_) => Ok(()),
            Err(e) => {
                self.set_status(format!("Failed to load bandits: {}", e));
                Err(e)
            }
        }
    }

    fn load_bandits_inner(&mut self) -> Result<()> {
        let resp = self.http.get("/bandits", &[])?;
        let items = resp["items"].as_array().cloned().unwrap_or_default();

        self.bandits = items
            .iter()
            .map(|item| BanditInfo {
                id: item["id"].as_i64().unwrap_or(0),
                name: item["name"].as_str().unwrap_or("?").to_string(),
                bandit_type: item["type"].as_str().unwrap_or("?").to_string(),
                arm_count: item["arm_count"].as_i64().unwrap_or(0),
                total_pulls: item["total_pull_count"].as_i64().unwrap_or(0),
                mode: item["optimization_mode"].as_str().unwrap_or("?").to_string(),
            })
            .collect();

        self.bandit_index = 0;
        Ok(())
    }

    pub fn load_events(&mut self) {
        match self.load_events_inner() {
            Ok(_) => {}
            Err(e) => {
                self.set_status(format!("Failed to load events: {}", e));
            }
        }
    }

    fn load_events_inner(&mut self) -> Result<()> {
        let bandit = match &self.current_bandit {
            Some(b) => b.clone(),
            None => return Ok(()),
        };

        let resp = self.http.get(
            "/events",
            &[
                ("bandit_id", &bandit.id.to_string()),
                ("has_grade", "false"),
                ("limit", "50"),
            ],
        )?;

        let items = resp["items"].as_array().cloned().unwrap_or_default();

        // Extract UUIDs for local text lookup
        let uuids: Vec<String> = items
            .iter()
            .filter_map(|e| e["local_event_uuid"].as_str().map(String::from))
            .collect();

        let local_text = if let Some(store) = &self.store {
            store.get_text(&uuids).unwrap_or_default()
        } else {
            std::collections::HashMap::new()
        };

        self.events = items
            .iter()
            .map(|e| {
                let uuid = e["local_event_uuid"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let local = local_text.get(&uuid);

                MergedEvent {
                    uuid: uuid.clone(),
                    model_name: e["model_name"].as_str().unwrap_or("?").to_string(),
                    model_provider: e["model_provider"]
                        .as_str()
                        .unwrap_or("?")
                        .to_string(),
                    cost: e["cost"].as_f64(),
                    latency: e["latency"].as_f64(),
                    early_reward: e["early_reward"].as_f64(),
                    grade: e["grade"].as_f64(),
                    created_at: e["created_at"].as_str().unwrap_or("").to_string(),
                    query_text: local
                        .and_then(|t| t.query_text.clone())
                        .or_else(|| e["query_text"].as_str().map(String::from)),
                    response: local
                        .and_then(|t| t.response.clone())
                        .or_else(|| extract_response(&e["response"])),
                    system_prompt: local
                        .and_then(|t| t.system_prompt.clone())
                        .or_else(|| e["system_prompt"].as_str().map(String::from)),
                }
            })
            .collect();

        // Sort skipped to end
        self.events
            .sort_by_key(|e| self.skipped.contains(&e.uuid));

        self.event_index = 0;
        self.set_status(format!("{} ungraded events", self.events.len()));
        Ok(())
    }

    /// Count of non-skipped events remaining.
    fn unskipped_count(&self) -> usize {
        self.events
            .iter()
            .filter(|e| !self.skipped.contains(&e.uuid))
            .count()
    }

    pub fn grade_current(&mut self, grade: f64) {
        if self.events.is_empty() {
            return;
        }

        let event = &self.events[self.event_index];
        let uuid = event.uuid.clone();

        let body = serde_json::json!({
            "grade": grade,
            "is_graded": true,
        });

        match self
            .http
            .patch_json(&format!("/events/{}/grade", uuid), &body)
        {
            Ok(_) => {
                self.events.remove(self.event_index);
                if self.event_index >= self.events.len() && self.event_index > 0 {
                    self.event_index -= 1;
                }
                let label = if grade >= 0.5 { "good" } else { "bad" };
                self.set_status(format!(
                    "Graded {} ({} remaining)",
                    label,
                    self.unskipped_count()
                ));
            }
            Err(e) => {
                self.set_status(format!("Grade failed: {}", e));
            }
        }
    }

    pub fn skip_current(&mut self) {
        if self.events.is_empty() {
            return;
        }

        // Don't allow skipping if this is the last unskipped event
        if self.unskipped_count() <= 1 {
            self.set_status("Can't skip — last unskipped event");
            return;
        }

        let uuid = self.events[self.event_index].uuid.clone();
        self.skipped.insert(uuid);

        // Move skipped event to end
        let event = self.events.remove(self.event_index);
        self.events.push(event);

        // Clamp index to unskipped range
        let unskipped = self.unskipped_count();
        if self.event_index >= unskipped {
            self.event_index = 0;
        }
        self.set_status(format!("Skipped ({} remaining)", unskipped));
    }

    pub fn copy_section(&mut self, section: Section) {
        if self.events.is_empty() {
            return;
        }

        let event = &self.events[self.event_index];
        let (label, text) = match section {
            Section::Query => ("Query", event.query_text.clone()),
            Section::Response => ("Response", event.response.clone()),
            Section::Prompt => ("System prompt", event.system_prompt.clone()),
        };

        match text {
            Some(content) => match arboard::Clipboard::new() {
                Ok(mut clipboard) => match clipboard.set_text(&content) {
                    Ok(_) => self.set_status(format!("{} copied", label)),
                    Err(e) => self.set_status(format!("Copy failed: {}", e)),
                },
                Err(e) => self.set_status(format!("Clipboard unavailable: {}", e)),
            },
            None => self.set_status(format!("{} not available", label)),
        }
    }
}

/// Extract response text from cloud event JSON.
/// Response can be a string or a dict with "response" or "content" key.
fn extract_response(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Object(obj) => obj
            .get("response")
            .or_else(|| obj.get("content"))
            .and_then(|v| v.as_str())
            .map(String::from),
        _ => None,
    }
}
