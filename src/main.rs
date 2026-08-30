mod cmd;
mod config;
mod style;

use usage::{Cli, Subcommands};

#[derive(Cli)]
#[usage(
    bin = "privconf",
    about = "Private config manager for project-specific files",
    completion,
    unknown_flags = "error"
)]
struct Cli {
    #[usage(subcommand)]
    command: Commands,
}

#[derive(Subcommands)]
enum Commands {
    /// Initialize privconf store
    Init {
        /// Remote URL to clone (if omitted, creates a new store)
        remote: Option<String>,
        /// Skip git init — use when embedding privconf inside an existing git repo (e.g. chezmoi)
        #[usage(long)]
        no_git: bool,
    },
    /// Add files from current project to privconf and create symlinks
    Add {
        /// Project name (auto-detected from git remote if omitted)
        #[usage(long, short)]
        project: Option<String>,
        /// Apply to ALL projects (stored in the `[all]` block, linked on every `privconf link`)
        #[usage(long, short)]
        all: bool,
        /// Files or directories to add (omit to create project only)
        files: Vec<String>,
    },
    /// Remove files from privconf and restore originals
    Remove {
        /// Project name (auto-detected from git remote if omitted)
        #[usage(long, short)]
        project: Option<String>,
        /// Remove from the `[all]` block (applies to ALL projects)
        #[usage(long, short)]
        all: bool,
        /// Files or directories to remove
        files: Vec<String>,
    },
    /// Link private config files into current project directory
    Link {
        /// Suppress output
        #[usage(long, short)]
        quiet: bool,
        /// Sync store with remote before linking
        #[usage(long, short)]
        sync: bool,
    },
    /// Unlink private config files from current project directory
    Unlink,
    /// Show link status for current directory
    Status,
    /// Sync privconf store with remote
    Sync {
        /// Commit message (default: "sync")
        #[usage(long, short)]
        message: Option<String>,
        /// Show what would be done without making changes
        #[usage(long)]
        dry_run: bool,
    },
    /// List all projects in the store
    List,
    /// Print shell hook for auto-link on cd
    Hook {
        /// Shell type: bash, zsh, or fish
        shell: String,
    },
    /// Ignore files in current project (add to .git/info/exclude or skip-worktree, no symlink)
    Ignore {
        /// Project name (auto-detected from git remote if omitted)
        #[usage(long, short)]
        project: Option<String>,
        /// Apply to ALL projects (stored in the `[all]` block)
        #[usage(long, short)]
        all: bool,
        /// Files to ignore
        files: Vec<String>,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { remote, no_git } => cmd::init::run(remote.as_deref(), no_git),
        Commands::Add { project, all, files } => cmd::add::run(all, project, files),
        Commands::Remove { project, all, files } => cmd::remove::run(all, project, files),
        Commands::Link { quiet, sync } => cmd::link::run(quiet, sync),
        Commands::Unlink => cmd::unlink::run(),
        Commands::Status => cmd::status::run(),
        Commands::Sync { message, dry_run } => cmd::sync::run(message.as_deref(), dry_run),
        Commands::List => cmd::list::run(),
        Commands::Hook { shell } => cmd::hook::run(&shell),
        Commands::Ignore { project, all, files } => cmd::ignore::run(all, project, files),
    }
}