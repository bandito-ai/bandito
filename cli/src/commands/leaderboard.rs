use anyhow::{bail, Result};
use std::thread;
use std::time::Duration;

use crate::commands::arm::resolve_bandit_id;
use crate::config::Config;
use crate::http::HttpClient;
use crate::util::truncate;

pub fn run(bandit_name: &str, graded: bool, watch: bool) -> Result<()> {
    let config = Config::load()?;
    if !config.is_configured() {
        bail!("Not configured. Run `bandito signup` or `bandito config` first.");
    }
    let http = HttpClient::from_config(&config)?;
    let bandit_id = resolve_bandit_id(&http, bandit_name)?;

    if watch {
        loop {
            print!("\x1b[2J\x1b[H");
            match render_leaderboard(&http, bandit_name, bandit_id, graded) {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("Refresh failed: {}", e);
                    eprintln!("Retrying in 30s...");
                }
            }
            println!("\nAuto-refreshing every 30s. Press Ctrl+C to stop.");
            thread::sleep(Duration::from_secs(30));
        }
    } else {
        render_leaderboard(&http, bandit_name, bandit_id, graded)?;
    }

    Ok(())
}

fn render_leaderboard(
    http: &HttpClient,
    bandit_name: &str,
    bandit_id: i64,
    graded: bool,
) -> Result<()> {
    let resp = http.get(
        &format!("/analytics/{}/arms/performance", bandit_id),
        &[],
    )?;

    let total_events = resp["total_events"].as_i64().unwrap_or(0);
    let arms = resp["arms"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("Unexpected response format"))?;

    // When --graded, use graded-specific metrics and counts
    if graded {
        let total_graded: i64 = arms.iter().map(|a| a["graded_count"].as_i64().unwrap_or(0)).sum();
        println!("{} (graded only, {} events)\n", bandit_name, total_graded);
    } else {
        println!("{} ({} events)\n", bandit_name, total_events);
    }

    if arms.is_empty() {
        println!("No data yet. Make sure your app is calling bandito.update() after each LLM call.");
        return Ok(());
    }

    println!(
        "{:<30} {:>7} {:>7} {:>8} {:>10} {:>12}",
        "Arm", "Pulls", "Pull%", "Reward", "Avg Cost", "Avg Latency"
    );
    println!("{}", "-".repeat(78));

    let mut rows_printed = 0;
    for arm in arms {
        let model = arm["model_name"].as_str().unwrap_or("?");
        let provider = arm["model_provider"].as_str().unwrap_or("?");
        let arm_label = format!("{} / {}", model, provider);

        let event_count = arm["event_count"].as_i64().unwrap_or(0);
        let pull_share = arm["pull_share"].as_f64().unwrap_or(0.0);

        // --graded: show avg_grade and graded count; default: show avg_reward and total count
        let (reward, count) = if graded {
            (arm["avg_grade"].as_f64(), arm["graded_count"].as_i64().unwrap_or(0))
        } else {
            (arm["avg_reward"].as_f64(), event_count)
        };

        let avg_cost = arm["avg_cost"].as_f64();
        let avg_latency = arm["avg_latency"].as_f64();

        // Skip arms with no graded events when filtering
        if graded && count == 0 {
            continue;
        }

        println!(
            "{:<30} {:>7} {:>6.1}% {:>8} {:>10} {:>12}",
            truncate(&arm_label, 30),
            count,
            pull_share * 100.0,
            format_opt_f64(reward, 2),
            format_cost(avg_cost),
            format_latency(avg_latency),
        );
        rows_printed += 1;
    }

    if graded && rows_printed == 0 {
        println!("No graded events yet. Grade responses with `bandito tui`.");
    }

    Ok(())
}

fn format_opt_f64(v: Option<f64>, decimals: usize) -> String {
    match v {
        Some(val) => format!("{:.prec$}", val, prec = decimals),
        None => "-".to_string(),
    }
}

fn format_cost(v: Option<f64>) -> String {
    match v {
        Some(c) => format!("${:.4}", c),
        None => "-".to_string(),
    }
}

fn format_latency(v: Option<f64>) -> String {
    match v {
        Some(ms) => format!("{:.0}ms", ms),
        None => "-".to_string(),
    }
}
