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
    /// Add files from current project to privconf
    Add {
        /// Project name
        name: String,
        /// Files to add
        files: Vec<String>,
    },
    /// Link private config files into current project directory
    Link {
        /// Suppress output
        #[arg(long, short)]
        quiet: bool,
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
        Commands::Add { name, files } => cmd::add::run(name, files),
        Commands::Link { quiet } => cmd::link::run(quiet),
        Commands::Unlink => cmd::unlink::run(),
        Commands::Status => cmd::status::run(),
        Commands::Sync => cmd::sync::run(),
        Commands::Hook { shell } => cmd::hook::run(&shell),
    }
}
