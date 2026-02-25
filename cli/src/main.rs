mod commands;
mod config;
mod http;
mod store;
mod tui;
pub mod util;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "bandito", version, about = "Contextual bandit optimizer for LLM selection")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create account + first bandit + SDK snippet (all-in-one setup)
    Signup,
    /// Configure API key and data storage
    Config,
    /// Generate starter templates
    Template {
        #[command(subcommand)]
        cmd: TemplateCmd,
    },
    /// Create a bandit from a JSON template (or interactively)
    Create(CreateArgs),
    /// List bandits
    List,
    /// Manage arms
    Arm {
        #[command(subcommand)]
        cmd: ArmCmd,
    },
    /// Show arm performance leaderboard
    Leaderboard(LeaderboardArgs),
    /// Install the Python or JavaScript SDK
    Install {
        /// SDK to install (python, js)
        sdk: String,
    },
    /// Install Claude Code skill into current project
    Skill,
    /// Launch the grading workbench TUI
    Tui,
}

#[derive(Subcommand)]
enum TemplateCmd {
    /// Generate SDK starter script
    Script {
        /// SDK language
        #[arg(long, default_value = "python")]
        sdk: String,
    },
    /// Generate bandit+arms JSON skeleton
    Bandit {
        /// Bandit name
        name: String,
    },
}

#[derive(clap::Args)]
struct CreateArgs {
    /// Path to JSON template file (omit for interactive mode)
    file: Option<String>,
}

#[derive(Subcommand)]
enum ArmCmd {
    /// List arms for a bandit
    List {
        /// Bandit name
        bandit: String,
    },
    /// Add an arm to a bandit
    Add {
        /// Bandit name
        bandit: String,
        /// Model name (e.g. gpt-4o)
        model: String,
        /// Model provider (e.g. openai)
        provider: String,
        /// System prompt
        #[arg(default_value = "You are a helpful assistant.")]
        prompt: String,
        /// Read system prompt from file instead of positional arg
        #[arg(long)]
        prompt_file: Option<String>,
    },
}

#[derive(clap::Args)]
struct LeaderboardArgs {
    /// Bandit name
    bandit: String,
    /// Show only graded events
    #[arg(long)]
    graded: bool,
    /// Auto-refresh every 30s
    #[arg(long)]
    watch: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Signup => commands::signup::run()?,
        Commands::Config => commands::config::run()?,
        Commands::Template { cmd } => match cmd {
            TemplateCmd::Script { sdk } => commands::template::script(&sdk)?,
            TemplateCmd::Bandit { name } => commands::template::bandit(&name)?,
        },
        Commands::Create(args) => commands::create::run(args.file)?,
        Commands::List => commands::list::run()?,
        Commands::Arm { cmd } => match cmd {
            ArmCmd::List { bandit } => commands::arm::list(&bandit)?,
            ArmCmd::Add {
                bandit,
                model,
                provider,
                prompt,
                prompt_file,
            } => commands::arm::add(&bandit, &model, &provider, &prompt, prompt_file.as_deref())?,
        },
        Commands::Leaderboard(args) => {
            commands::leaderboard::run(&args.bandit, args.graded, args.watch)?
        }
        Commands::Install { sdk } => commands::install::run(&sdk)?,
        Commands::Skill => commands::skill::run()?,
        Commands::Tui => tui::run()?,
    }

    Ok(())
}
