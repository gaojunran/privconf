mod cmd;
mod config;

use clap::Parser;

#[derive(Parser)]
#[command(name = "privconf", about = "Private config manager for project-specific files")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Initialize privconf store
    Init,
    /// Add files from current project to privconf and create symlinks
    Add {
        /// Project name (auto-detected from git remote if omitted)
        #[arg(long, short)]
        project: Option<String>,
        /// Files or directories to add (omit to create project only)
        files: Vec<String>,
    },
    /// Remove files from privconf and restore originals
    Remove {
        /// Project name (auto-detected from git remote if omitted)
        #[arg(long, short)]
        project: Option<String>,
        /// Files or directories to remove
        files: Vec<String>,
    },
    /// Link private config files into current project directory
    Link {
        /// Suppress output
        #[arg(long, short)]
        quiet: bool,
        /// Sync store with remote before linking
        #[arg(long, short)]
        sync: bool,
    },
    /// Unlink private config files from current project directory
    Unlink,
    /// Show link status for current directory
    Status,
    /// Sync privconf store with remote
    Sync,
    /// Print shell hook for auto-link on cd
    Hook {
        /// Shell type: bash, zsh, or fish
        shell: String,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init => cmd::init::run(),
        Commands::Add { project, files } => cmd::add::run(project, files),
        Commands::Remove { project, files } => cmd::remove::run(project, files),
        Commands::Link { quiet, sync } => cmd::link::run(quiet, sync),
        Commands::Unlink => cmd::unlink::run(),
        Commands::Status => cmd::status::run(),
        Commands::Sync => cmd::sync::run(),
        Commands::Hook { shell } => cmd::hook::run(&shell),
    }
}
