use anyhow::{bail, Result};
use std::process::Command;

pub fn run(sdk: &str) -> Result<()> {
    match sdk {
        "python" | "py" => install_python(),
        "javascript" | "js" | "typescript" | "ts" | "node" => install_javascript(),
        _ => bail!(
            "Unknown SDK: \"{}\". Use: bandito install python  or  bandito install js",
            sdk
        ),
    }
}

fn install_python() -> Result<()> {
    // Try uv first, fall back to pip
    let (cmd, args) = if command_exists("uv") {
        ("uv", vec!["add", "bandito"])
    } else if command_exists("pip") {
        ("pip", vec!["install", "bandito"])
    } else {
        bail!("Neither uv nor pip found. Install Python 3.12+ first.");
    };

    println!("Running: {} {}...", cmd, args.join(" "));
    let status = Command::new(cmd).args(&args).status()?;

    if status.success() {
        println!("\nPython SDK installed. Import it with:");
        println!("  import bandito");
    } else {
        bail!("{} exited with status {}", cmd, status);
    }

    Ok(())
}

fn install_javascript() -> Result<()> {
    // Try pnpm, npm, yarn in order
    let (cmd, args) = if command_exists("pnpm") {
        ("pnpm", vec!["add", "bandito"])
    } else if command_exists("npm") {
        ("npm", vec!["install", "bandito"])
    } else if command_exists("yarn") {
        ("yarn", vec!["add", "bandito"])
    } else {
        bail!("No JS package manager found (pnpm, npm, or yarn). Install Node.js 18+ first.");
    };

    println!("Running: {} {}...", cmd, args.join(" "));
    let status = Command::new(cmd).args(&args).status()?;

    if status.success() {
        println!("\nJavaScript SDK installed. Import it with:");
        println!("  import {{ connect, pull, update, close }} from \"bandito\";");
    } else {
        bail!("{} exited with status {}", cmd, status);
    }

    Ok(())
}

fn command_exists(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
