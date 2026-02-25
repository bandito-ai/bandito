use anyhow::{bail, Result};

use crate::config::Config;
use crate::http::HttpClient;
use crate::util::{format_number, truncate};

pub fn run() -> Result<()> {
    let config = Config::load()?;
    if !config.is_configured() {
        bail!("Not configured. Run `bandito signup` or `bandito config` first.");
    }
    let http = HttpClient::from_config(&config)?;

    let resp = http.get("/bandits", &[])?;
    let items = resp["items"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("Unexpected response format"))?;

    if items.is_empty() {
        println!("No bandits yet. Create one with `bandito create`.");
        return Ok(());
    }

    println!(
        "{:<20} {:<10} {:>5} {:>8} {:<10}",
        "Name", "Type", "Arms", "Pulls", "Mode"
    );
    println!("{}", "-".repeat(58));

    for item in items {
        let name = item["name"].as_str().unwrap_or("?");
        let btype = item["type"].as_str().unwrap_or("?");
        let arms = item["arm_count"].as_i64().unwrap_or(0);
        let pulls = item["total_pull_count"].as_i64().unwrap_or(0);
        let mode = item["optimization_mode"].as_str().unwrap_or("?");

        println!(
            "{:<20} {:<10} {:>5} {:>8} {:<10}",
            truncate(name, 20),
            btype,
            arms,
            format_number(pulls),
            mode,
        );
    }

    Ok(())
}
