use anyhow::{bail, Result};
use dialoguer::{Input, Password, Select};

use crate::config::Config;
use crate::http::HttpClient;

pub fn run() -> Result<()> {
    println!("Configure Bandito\n");

    let existing = Config::load().unwrap_or_default();

    // API key
    let api_key: String = Password::new().with_prompt("API key").interact()?;

    // Data storage
    let storage_options = &[
        "local (text stays on your machine)",
        "cloud (text sent to Bandito cloud)",
    ];
    let default_idx = if existing.data_storage == "cloud" {
        1
    } else {
        0
    };
    let storage_idx = Select::new()
        .with_prompt("Data storage")
        .items(storage_options)
        .default(default_idx)
        .interact()?;
    let data_storage = if storage_idx == 0 { "local" } else { "cloud" };

    // Base URL
    let base_url: String = Input::new()
        .with_prompt("Base URL")
        .default(existing.base_url.clone())
        .interact_text()?;

    // Validate by connecting
    print!("Validating... ");
    let config = Config {
        api_key,
        base_url,
        data_storage: data_storage.to_string(),
    };
    let http = HttpClient::from_config(&config)?;

    match http.post_json("/sync/connect", &serde_json::json!({})) {
        Ok(_) => println!("connected."),
        Err(e) => {
            println!("failed.");
            bail!(
                "Could not connect: {}\nCheck your API key and base URL, then try again.",
                e
            );
        }
    }

    // Save (only after validation succeeds)
    config.save()?;
    println!("Config saved to ~/.bandito/config.toml");

    Ok(())
}
