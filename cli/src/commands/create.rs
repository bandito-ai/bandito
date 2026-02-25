use anyhow::{bail, Context, Result};
use dialoguer::Input;
use serde_json::Value;
use std::fs;

use crate::config::Config;
use crate::http::HttpClient;

pub fn run(file: Option<String>) -> Result<()> {
    let config = Config::load()?;
    if !config.is_configured() {
        bail!("Not configured. Run `bandito signup` or `bandito config` first.");
    }
    let http = HttpClient::from_config(&config)?;

    match file {
        Some(path) => create_from_file(&http, &path),
        None => create_interactive(&http),
    }
}

fn create_from_file(http: &HttpClient, path: &str) -> Result<()> {
    let contents =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path))?;
    let template: Value =
        serde_json::from_str(&contents).with_context(|| format!("Invalid JSON in {}", path))?;

    let name = template["name"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing 'name' field in template"))?;

    // Create bandit
    let bandit_body = serde_json::json!({
        "name": name,
        "description": template.get("description").and_then(|v| v.as_str()).unwrap_or(""),
        "type": template.get("type").and_then(|v| v.as_str()).unwrap_or("online"),
        "cost_importance": template.get("cost_importance").and_then(|v| v.as_i64()).unwrap_or(2),
        "latency_importance": template.get("latency_importance").and_then(|v| v.as_i64()).unwrap_or(2),
        "optimization_mode": template.get("optimization_mode").and_then(|v| v.as_str()).unwrap_or("base"),
    });

    print!("Creating bandit \"{}\"... ", name);
    let bandit = http.post_json("/bandits", &bandit_body)?;
    let bandit_id = bandit["id"]
        .as_i64()
        .ok_or_else(|| anyhow::anyhow!("No id in bandit response"))?;
    println!("done (id: {}).", bandit_id);

    // Create arms
    let arms = template
        .get("arms")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    for (i, arm) in arms.iter().enumerate() {
        let model = arm
            .get("model")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Arm {} is missing required 'model' field", i + 1))?;
        let provider = arm
            .get("provider")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Arm {} is missing required 'provider' field", i + 1))?;
        let prompt = arm
            .get("prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let arm_body = serde_json::json!({
            "model_name": model,
            "model_provider": provider,
            "system_prompt": prompt,
        });

        print!("  Adding arm {} ({}/{})... ", i + 1, model, provider);
        http.post_json(&format!("/bandits/{}/arms", bandit_id), &arm_body)?;
        println!("done.");
    }

    println!(
        "\nReady. Use bandito.pull(\"{}\") in your code.",
        name
    );
    Ok(())
}

fn create_interactive(http: &HttpClient) -> Result<()> {
    println!("Create a new bandit\n");

    let name: String = Input::new()
        .with_prompt("Bandit name")
        .interact_text()?;

    let description: String = Input::new()
        .with_prompt("Description (optional)")
        .default(String::new())
        .interact_text()?;

    let bandit_body = serde_json::json!({
        "name": name,
        "description": description,
        "type": "online",
        "cost_importance": 2,
        "latency_importance": 2,
        "optimization_mode": "base",
    });

    print!("Creating bandit... ");
    let bandit = http.post_json("/bandits", &bandit_body)?;
    let bandit_id = bandit["id"]
        .as_i64()
        .ok_or_else(|| anyhow::anyhow!("No id in bandit response"))?;
    println!("done (id: {}).", bandit_id);

    println!("\nAdd arms (enter blank model name to finish):\n");
    let mut arm_count = 0;

    loop {
        let model: String = Input::new()
            .with_prompt(format!("Arm {} — model name", arm_count + 1))
            .allow_empty(true)
            .interact_text()?;

        if model.is_empty() {
            break;
        }

        let provider: String = Input::new()
            .with_prompt("  provider")
            .interact_text()?;

        let prompt: String = Input::new()
            .with_prompt("  system prompt")
            .default("You are a helpful assistant.".to_string())
            .interact_text()?;

        let arm_body = serde_json::json!({
            "model_name": model,
            "model_provider": provider,
            "system_prompt": prompt,
        });

        http.post_json(&format!("/bandits/{}/arms", bandit_id), &arm_body)?;
        arm_count += 1;
        println!("  Added.\n");
    }

    if arm_count == 0 {
        println!("\nCreated \"{}\" with no arms.", name);
        println!("Add arms before using pull():");
        println!("  bandito arm add {} <model> <provider> \"<prompt>\"", name);
    } else {
        println!(
            "\nCreated \"{}\" with {} arm(s). Use bandito.pull(\"{}\") in your code.",
            name, arm_count, name
        );
    }
    Ok(())
}
