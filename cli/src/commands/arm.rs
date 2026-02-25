use anyhow::{bail, Context, Result};

use crate::config::Config;
use crate::http::HttpClient;
use crate::util::truncate;

pub fn list(bandit_name: &str) -> Result<()> {
    let config = Config::load()?;
    if !config.is_configured() {
        bail!("Not configured. Run `bandito signup` or `bandito config` first.");
    }
    let http = HttpClient::from_config(&config)?;

    let bandit_id = resolve_bandit_id(&http, bandit_name)?;
    let resp = http.get(&format!("/bandits/{}/arms", bandit_id), &[])?;
    let items = resp["items"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("Unexpected response format"))?;

    if items.is_empty() {
        println!(
            "No arms for \"{}\". Add one with:\n  bandito arm add {} <model> <provider> \"<prompt>\"",
            bandit_name, bandit_name
        );
        return Ok(());
    }

    println!(
        "{:<25} {:<15} {:<30} {:>7} {:<6}",
        "Model", "Provider", "Prompt", "Pulls", "Active"
    );
    println!("{}", "-".repeat(88));

    for item in items {
        let model = item["model_name"].as_str().unwrap_or("?");
        let provider = item["model_provider"].as_str().unwrap_or("?");
        let prompt = item["system_prompt"].as_str().unwrap_or("");
        let pulls = item["pull_count"].as_i64().unwrap_or(0);
        let active = item["is_active"].as_bool().unwrap_or(false);

        println!(
            "{:<25} {:<15} {:<30} {:>7} {:<6}",
            truncate(model, 25),
            truncate(provider, 15),
            truncate(prompt, 30),
            pulls,
            if active { "yes" } else { "no" },
        );
    }

    Ok(())
}

pub fn add(bandit_name: &str, model: &str, provider: &str, prompt: &str, prompt_file: Option<&str>) -> Result<()> {
    let config = Config::load()?;
    if !config.is_configured() {
        bail!("Not configured. Run `bandito signup` or `bandito config` first.");
    }
    let http = HttpClient::from_config(&config)?;

    let bandit_id = resolve_bandit_id(&http, bandit_name)?;

    let final_prompt = match prompt_file {
        Some(path) => std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read prompt file: {}", path))?,
        None => prompt.to_string(),
    };

    let body = serde_json::json!({
        "model_name": model,
        "model_provider": provider,
        "system_prompt": final_prompt,
    });

    http.post_json(&format!("/bandits/{}/arms", bandit_id), &body)?;
    println!("Added {}/{} to \"{}\".", model, provider, bandit_name);
    Ok(())
}

/// Resolve a bandit name to its ID by listing all bandits.
pub fn resolve_bandit_id(http: &HttpClient, name: &str) -> Result<i64> {
    let resp = http.get("/bandits", &[])?;
    let items = resp["items"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("Unexpected response format"))?;

    for item in items {
        if item["name"].as_str() == Some(name) {
            return item["id"]
                .as_i64()
                .ok_or_else(|| anyhow::anyhow!("Bandit has no id"));
        }
    }

    bail!(
        "Bandit \"{}\" not found. Run `bandito list` to see available bandits.",
        name
    )
}
