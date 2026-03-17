mod commands;
mod config;
mod http;
mod judge_client;
mod s3;
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
    /// Install AI coding tool skill into current project
    Skill,
    /// Launch the grading workbench TUI
    Tui,
    /// LLM-as-judge: generate rubrics, calibrate, and augment grades
    Judge {
        #[command(subcommand)]
        cmd: JudgeCmd,
    },
}

#[derive(Subcommand)]
enum JudgeCmd {
    /// Set judge API key and model interactively
    Config,
    /// Generate a quality rubric for a bandit via LLM
    GenRubric {
        /// Bandit name
        bandit: String,
    },
    /// Open the rubric in $EDITOR for manual editing
    EditRubric {
        /// Bandit name
        bandit: String,
    },
    /// Score human-graded events to measure judge accuracy (no writes)
    Calibrate {
        /// Bandit name
        bandit: String,
        /// Total events to sample across all arms
        #[arg(long, default_value = "50")]
        sample: usize,
    },
    /// Grade ungraded events with LLM judge (writes to cloud)
    Augment {
        /// Bandit name
        bandit: String,
        /// Total events to grade across all arms
        #[arg(long, default_value = "100")]
        sample: usize,
    },
    /// Show events with both human and judge grades
    Review {
        /// Bandit name
        bandit: String,
        /// Show only events where human and judge disagree (delta >= 0.5)
        #[arg(long)]
        disagreements: bool,
    },
    /// Show judge configuration and metrics for a bandit
    Status {
        /// Bandit name
        bandit: String,
    },
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
    /// Activate a previously deactivated arm
    Activate {
        /// Bandit name
        bandit: String,
        /// Model name to activate
        model: String,
    },
    /// Deactivate an arm (soft-delete, keeps history)
    Deactivate {
        /// Bandit name
        bandit: String,
        /// Model name to deactivate
        model: String,
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
            ArmCmd::Activate { bandit, model } => commands::arm::activate(&bandit, &model)?,
            ArmCmd::Deactivate { bandit, model } => commands::arm::deactivate(&bandit, &model)?,
        },
        Commands::Leaderboard(args) => {
            commands::leaderboard::run(&args.bandit, args.graded, args.watch)?
        }
        Commands::Install { sdk } => commands::install::run(&sdk)?,
        Commands::Skill => commands::skill::run()?,
        Commands::Tui => tui::run()?,
        Commands::Judge { cmd } => match cmd {
            JudgeCmd::Config => commands::judge::config_judge()?,
            JudgeCmd::GenRubric { bandit } => commands::judge::gen_rubric(&bandit)?,
            JudgeCmd::EditRubric { bandit } => commands::judge::edit_rubric(&bandit)?,
            JudgeCmd::Calibrate { bandit, sample } => commands::judge::calibrate(&bandit, sample)?,
            JudgeCmd::Augment { bandit, sample } => commands::judge::augment(&bandit, sample)?,
            JudgeCmd::Review { bandit, disagreements } => commands::judge::review(&bandit, disagreements)?,
            JudgeCmd::Status { bandit } => commands::judge::status(&bandit)?,
        },
    }

    Ok(())
}
