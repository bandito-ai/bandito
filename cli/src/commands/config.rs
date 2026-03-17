use anyhow::{bail, Result};
use dialoguer::{Input, Password, Select};

use crate::config::{Config, S3Config};
use crate::http::HttpClient;

pub fn run() -> Result<()> {
    println!("Configure Bandito\n");

    let existing = Config::load().unwrap_or_default();

    // API key
    let api_key: String = Password::new().with_prompt("API key").interact()?;

    // Data storage — three modes
    let storage_options = &[
        "Bandito cloud — recommended (text stored in Bandito)",
        "Local SQLite — experimenting + privacy (text stays on your machine)",
        "SQLite + S3 — production + privacy (text goes to your S3, never Bandito)",
    ];
    let default_idx = match existing.data_storage.as_str() {
        "cloud" => 0,
        "s3" => 2,
        _ => 1, // "local" is default
    };
    let storage_idx = Select::new()
        .with_prompt("Data storage")
        .items(storage_options)
        .default(default_idx)
        .interact()?;
    let data_storage = match storage_idx {
        0 => "cloud",
        2 => "s3",
        _ => "local",
    };

    // S3 config (only if s3 mode selected)
    let s3_config: Option<S3Config> = if storage_idx == 2 {
        let existing_s3 = existing.s3.as_ref();
        let bucket: String = Input::new()
            .with_prompt("S3 bucket")
            .with_initial_text(existing_s3.map(|s| s.bucket.as_str()).unwrap_or(""))
            .interact_text()?;
        let prefix: String = Input::new()
            .with_prompt("S3 prefix")
            .with_initial_text(existing_s3.map(|s| s.prefix.as_str()).unwrap_or("bandito"))
            .interact_text()?;
        let region: String = Input::new()
            .with_prompt("AWS region")
            .with_initial_text(existing_s3.map(|s| s.region.as_str()).unwrap_or("us-east-1"))
            .interact_text()?;
        println!(
            "Note: AWS credentials are resolved via AWS_ACCESS_KEY_ID / AWS_SECRET_ACCESS_KEY\n\
             or ~/.aws/credentials — not stored in Bandito config."
        );
        Some(S3Config {
            bucket: bucket.trim().to_string(),
            prefix: prefix.trim().to_string(),
            region: region.trim().to_string(),
            endpoint: None,
        })
    } else {
        None
    };

    // Use default base URL (overridable via BANDITO_BASE_URL env var)
    let base_url = existing.base_url.clone();

    // Validate Bandito API key
    print!("Validating... ");
    let config = Config {
        api_key,
        base_url,
        data_storage: data_storage.to_string(),
        s3: s3_config,
        judge: existing.judge.clone(),
    };
    let http = HttpClient::from_config(&config)?;

    match http.post_json("/sync/connect", &serde_json::json!({})) {
        Ok(_) => println!("connected."),
        Err(e) => {
            println!("failed.");
            bail!(
                "Could not connect: {}\nCheck your API key and try again.",
                e
            );
        }
    }

    // Save (only after validation succeeds)
    config.save()?;
    println!("Config saved to ~/.bandito/config.toml");

    Ok(())
}
