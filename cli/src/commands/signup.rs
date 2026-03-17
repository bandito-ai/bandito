use anyhow::{bail, Result};
use dialoguer::{Input, Password, Select};

use crate::commands::template::{JS_TEMPLATE, PYTHON_TEMPLATE};
use crate::config::Config;
use crate::http::HttpClient;

pub fn run() -> Result<()> {
    println!("Create your Bandito account\n");

    let email: String = Input::new().with_prompt("Email").interact_text()?;

    let org_name: String = Input::new()
        .with_prompt("Organization name")
        .interact_text()?;

    let password = Password::new()
        .with_prompt("Password")
        .with_confirmation("Confirm password", "Passwords don't match")
        .interact()?;

    if password.len() < 8 {
        bail!("Password must be at least 8 characters");
    }

    let base_url = Config::default_base_url().to_string();
    let mut http = HttpClient::with_base_url(&base_url)?;

    // 1. Sign up
    print!("Creating account... ");
    let signup_resp = http.post_form(
        "/auth/signup",
        &[
            ("email", email.as_str()),
            ("password", password.as_str()),
            ("organization_name", org_name.as_str()),
        ],
    );

    match signup_resp {
        Ok(resp) => {
            println!("done.");
            let jwt = resp["access_token"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("No access_token in signup response"))?;
            http.set_jwt(jwt.to_string());
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("already") || msg.contains("exists") {
                bail!(
                    "Account already exists for that email. Use `bandito config` to set your API key."
                );
            }
            bail!("Signup failed: {}", msg);
        }
    }

    // 2. Create API key
    print!("Creating API key... ");
    let key_resp = http.post_json(
        "/auth/api-keys",
        &serde_json::json!({ "name": "cli" }),
    )?;
    let raw_key = key_resp["raw_key"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No raw_key in API key response"))?;
    println!("done.");

    // 3. Data storage preference
    let storage_options = &["local (default — text stays on your machine)", "cloud (text sent to Bandito cloud)"];
    let storage_idx = Select::new()
        .with_prompt("Data storage")
        .items(storage_options)
        .default(0)
        .interact()?;
    let data_storage = if storage_idx == 0 { "local" } else { "cloud" };

    // 4. Save config
    let config = Config {
        api_key: raw_key.to_string(),
        base_url,
        data_storage: data_storage.to_string(),
        s3: None,
        judge: Default::default(),
    };
    config.save()?;

    println!("\nAccount created for {} (org: {})", email, org_name);
    println!("API key: {}", raw_key);
    println!("(Shown once — saved in ~/.bandito/config.toml)");
    println!("Keep this secret — do not commit it to version control.\n");

    // 5. Offer to create first bandit
    let setup_options = &["Yes", "No, I'll do it later"];
    let setup_idx = Select::new()
        .with_prompt("Create your first bandit?")
        .items(setup_options)
        .default(0)
        .interact()?;

    if setup_idx == 1 {
        println!("\nCreate a bandit later:");
        println!("  bandito create                    # interactive");
        println!("  https://bandito.dev/dashboard     # web app");
        return Ok(());
    }

    // 6. Create bandit — reconnect with API key auth
    let http = HttpClient::from_config(&config)?;

    let bandit_name: String = Input::new()
        .with_prompt("Bandit name")
        .default("my-chatbot".to_string())
        .interact_text()?;

    let bandit_body = serde_json::json!({
        "name": bandit_name,
        "description": "",
        "type": "online",
        "cost_importance": 2,
        "latency_importance": 2,
        "optimization_mode": "base",
    });

    print!("Creating bandit \"{}\"... ", bandit_name);
    let bandit = http.post_json("/bandits", &bandit_body)?;
    let bandit_id = bandit["id"]
        .as_i64()
        .ok_or_else(|| anyhow::anyhow!("No id in bandit response"))?;
    println!("done (id: {}).", bandit_id);

    // 7. Add arms
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
        println!("\nNo arms added. Add them before using pull():");
        println!("  bandito arm add {} <model> <provider> \"<prompt>\"", bandit_name);
        return Ok(());
    }

    // 8. Print SDK snippet
    let sdk_options = &["Python", "JavaScript/TypeScript"];
    let sdk_idx = Select::new()
        .with_prompt("Which SDK?")
        .items(sdk_options)
        .default(0)
        .interact()?;

    let (template, install_cmd) = if sdk_idx == 0 {
        (PYTHON_TEMPLATE, "pip install bandito   # or: uv add bandito")
    } else {
        (JS_TEMPLATE, "pnpm add bandito   # or: npm install bandito")
    };

    let snippet = template.replace("my-chatbot", &bandit_name);

    println!("\n--- Paste this into your app ---\n");
    println!("{}", snippet);
    println!("--- Install the SDK ---\n");
    println!("  {}\n", install_cmd);
    println!("Once events are flowing:");
    println!("  bandito tui                       # grade responses");
    println!("  bandito leaderboard {}    # see which arm is winning", bandit_name);

    Ok(())
}
