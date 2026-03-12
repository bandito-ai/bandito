use anyhow::{bail, Context, Result};
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde_json::Value;

use crate::config::Config;

pub struct HttpClient {
    client: Client,
    base_url: String,
    api_key: Option<String>,
    jwt: Option<String>,
}

impl HttpClient {
    pub fn from_config(config: &Config) -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        Ok(Self {
            client,
            base_url: config.base_url.trim_end_matches('/').to_string(),
            api_key: if config.api_key.is_empty() {
                None
            } else {
                Some(config.api_key.clone())
            },
            jwt: None,
        })
    }

    pub fn with_base_url(base_url: &str) -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: None,
            jwt: None,
        })
    }

    pub fn set_jwt(&mut self, jwt: String) {
        self.jwt = Some(jwt);
    }

    fn url(&self, path: &str) -> String {
        format!("{}/api/v1{}", self.base_url, path)
    }

    fn auth_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        if let Some(jwt) = &self.jwt {
            if let Ok(val) = HeaderValue::from_str(&format!("Bearer {}", jwt)) {
                headers.insert(AUTHORIZATION, val);
            }
        }
        if let Some(key) = &self.api_key {
            if let Ok(val) = HeaderValue::from_str(key) {
                headers.insert("X-API-Key", val);
            }
        }
        headers
    }

    /// POST with form-encoded body (for auth endpoints)
    pub fn post_form(&self, path: &str, params: &[(&str, &str)]) -> Result<Value> {
        let resp = self
            .client
            .post(self.url(path))
            .headers(self.auth_headers())
            .form(params)
            .send()
            .with_context(|| format!("POST {} failed", path))?;

        handle_response(resp, path)
    }

    /// POST with JSON body
    pub fn post_json(&self, path: &str, body: &Value) -> Result<Value> {
        let resp = self
            .client
            .post(self.url(path))
            .headers(self.auth_headers())
            .header(CONTENT_TYPE, "application/json")
            .json(body)
            .send()
            .with_context(|| format!("POST {} failed", path))?;

        handle_response(resp, path)
    }

    /// GET with optional query parameters
    pub fn get(&self, path: &str, params: &[(&str, &str)]) -> Result<Value> {
        let resp = self
            .client
            .get(self.url(path))
            .headers(self.auth_headers())
            .query(params)
            .send()
            .with_context(|| format!("GET {} failed", path))?;

        handle_response(resp, path)
    }

    /// DELETE (no body, tolerates 204 empty response)
    pub fn delete(&self, path: &str) -> Result<()> {
        let resp = self
            .client
            .delete(self.url(path))
            .headers(self.auth_headers())
            .send()
            .with_context(|| format!("DELETE {} failed", path))?;

        let status = resp.status();
        if status.is_success() {
            Ok(())
        } else {
            let body = resp.text().unwrap_or_default();
            let detail = serde_json::from_str::<Value>(&body)
                .ok()
                .and_then(|v| v.get("detail").cloned())
                .map(|d| {
                    if d.is_string() {
                        d.as_str().unwrap().to_string()
                    } else {
                        d.to_string()
                    }
                })
                .unwrap_or_else(|| body.clone());
            bail!("{} {} — {}", status.as_u16(), path, detail)
        }
    }

    /// PATCH with JSON body
    pub fn patch_json(&self, path: &str, body: &Value) -> Result<Value> {
        let resp = self
            .client
            .patch(self.url(path))
            .headers(self.auth_headers())
            .header(CONTENT_TYPE, "application/json")
            .json(body)
            .send()
            .with_context(|| format!("PATCH {} failed", path))?;

        handle_response(resp, path)
    }
}

fn handle_response(resp: reqwest::blocking::Response, path: &str) -> Result<Value> {
    let status = resp.status();
    let body = resp.text().unwrap_or_default();

    if status.is_success() {
        serde_json::from_str(&body)
            .with_context(|| format!("Failed to parse response from {}", path))
    } else {
        // Try to extract error detail from JSON response
        let detail = serde_json::from_str::<Value>(&body)
            .ok()
            .and_then(|v| v.get("detail").cloned())
            .map(|d| {
                if d.is_string() {
                    d.as_str().unwrap().to_string()
                } else {
                    d.to_string()
                }
            })
            .unwrap_or_else(|| body.clone());

        bail!("{} {} — {}", status.as_u16(), path, detail)
    }
}
