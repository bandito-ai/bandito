/// LLM API caller for judge commands.
///
/// Supports three providers — detected from the model string:
/// - OpenAI (gpt-*, o1-*, o3-*, o4-*)  →  api.openai.com
/// - Anthropic (claude-*)               →  api.anthropic.com
/// - OpenRouter ("provider/model")      →  openrouter.ai (unified access to 300+ models)
///
/// OpenRouter model strings always contain a slash, e.g. "google/gemini-2.5-flash".
/// No new Cargo deps — uses the reqwest blocking client already present in the CLI.
use anyhow::{bail, Result};
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::time::Duration;

pub fn call_judge(api_key: &str, model: &str, prompt: &str) -> Result<String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(60))
        .build()?;

    if model.contains('/') {
        call_openrouter(&client, api_key, model, prompt)
    } else if model.starts_with("claude-") {
        call_anthropic(&client, api_key, model, prompt)
    } else {
        call_openai(&client, api_key, model, prompt)
    }
}

fn call_openai(client: &Client, api_key: &str, model: &str, prompt: &str) -> Result<String> {
    // o1-*, o3-*, o4-* reasoning models do not accept a temperature parameter.
    let is_reasoning = model.starts_with("o1")
        || model.starts_with("o3")
        || model.starts_with("o4");
    let body = if is_reasoning {
        json!({
            "model": model,
            "messages": [{"role": "user", "content": prompt}],
        })
    } else {
        json!({
            "model": model,
            "messages": [{"role": "user", "content": prompt}],
            "temperature": 0.0,
        })
    };

    let resp = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        bail!("OpenAI API error {}: {}", status, text);
    }

    let resp_json: Value = resp.json()?;
    let content = resp_json["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No content in OpenAI response"))?
        .to_string();

    Ok(content)
}

fn call_anthropic(client: &Client, api_key: &str, model: &str, prompt: &str) -> Result<String> {
    let body = json!({
        "model": model,
        "max_tokens": 256,
        "messages": [{"role": "user", "content": prompt}],
    });

    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("Content-Type", "application/json")
        .json(&body)
        .send()?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        bail!("Anthropic API error {}: {}", status, text);
    }

    let resp_json: Value = resp.json()?;
    let content = resp_json["content"][0]["text"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No content in Anthropic response"))?
        .to_string();

    Ok(content)
}

fn call_openrouter(client: &Client, api_key: &str, model: &str, prompt: &str) -> Result<String> {
    // OpenRouter uses the OpenAI-compatible chat completions format.
    // Reasoning models routed through OpenRouter (e.g. "openai/o1", "openai/o3-mini")
    // still reject temperature — detect by the model-name portion after the slash.
    let model_name = model.split('/').last().unwrap_or(model);
    let is_reasoning = model_name.starts_with("o1")
        || model_name.starts_with("o3")
        || model_name.starts_with("o4");

    let body = if is_reasoning {
        json!({
            "model": model,
            "messages": [{"role": "user", "content": prompt}],
        })
    } else {
        json!({
            "model": model,
            "messages": [{"role": "user", "content": prompt}],
            "temperature": 0.0,
        })
    };

    let resp = client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        bail!("OpenRouter API error {}: {}", status, text);
    }

    let resp_json: Value = resp.json()?;
    let content = resp_json["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No content in OpenRouter response"))?
        .to_string();

    Ok(content)
}
