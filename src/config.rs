use std::path::PathBuf;

use anyhow::{Context, bail, ensure};

pub const STORE_DIR_NAME: &str = ".privconf";
pub const CONFIG_FILE_NAME: &str = "config.toml";
pub const STATE_FILE_NAME: &str = "state.toml";

pub fn store_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("PRIVCONF_DIR") {
        PathBuf::from(dir)
    } else {
        dirs::home_dir()
            .expect("cannot determine home directory")
            .join(STORE_DIR_NAME)
    }
}

pub fn config_path() -> PathBuf {
    store_dir().join(CONFIG_FILE_NAME)
}

pub fn state_path() -> PathBuf {
    store_dir().join(STATE_FILE_NAME)
}

pub fn project_dir(project: &str) -> PathBuf {
    store_dir().join("projects").join(project)
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub project: Vec<ProjectEntry>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectEntry {
    pub name: String,
    #[serde(default)]
    pub match_remote: Option<String>,
    #[serde(default)]
    pub match_path: Option<String>,
    pub files: Vec<String>,
}

impl ProjectEntry {
    pub fn matches_dir(&self, dir: &std::path::Path) -> bool {
        if let Some(pattern) = &self.match_path {
            let expanded = shellexpand::tilde(pattern);
            if let Ok(glob) = glob::Pattern::new(&expanded) {
                if glob.matches_path(dir) {
                    return true;
                }
            }
        }
        if let Some(remote) = &self.match_remote {
            if let Ok(git_remote) = get_git_remote(dir) {
                if git_remote.contains(remote) {
                    return true;
                }
            }
        }
        false
    }
}

fn get_git_remote(dir: &std::path::Path) -> anyhow::Result<String> {
    let output = std::process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(dir)
        .output()?;
    if !output.status.success() {
        bail!("no git remote origin");
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct State {
    #[serde(default)]
    pub linked: Vec<LinkedEntry>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct LinkedEntry {
    pub project: String,
    pub file: String,
    pub target: PathBuf,
    pub skip_worktree: bool,
}

pub fn load_config() -> anyhow::Result<Config> {
    let path = config_path();
    if !path.exists() {
        return Ok(Config { project: vec![] });
    }
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&content)
        .with_context(|| format!("parsing {}", path.display()))
}

pub fn save_config(config: &Config) -> anyhow::Result<()> {
    let path = config_path();
    let content = toml::to_string_pretty(config)?;
    std::fs::write(&path, content)
        .with_context(|| format!("writing {}", path.display()))
}

pub fn load_state() -> anyhow::Result<State> {
    let path = state_path();
    if !path.exists() {
        return Ok(State::default());
    }
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&content)
        .with_context(|| format!("parsing {}", path.display()))
}

pub fn save_state(state: &State) -> anyhow::Result<()> {
    let path = state_path();
    let content = toml::to_string_pretty(state)?;
    std::fs::write(&path, content)
        .with_context(|| format!("writing {}", path.display()))
}

pub fn find_project_for_dir<'a>(config: &'a Config, dir: &std::path::Path) -> Option<&'a ProjectEntry> {
    config.project.iter().find(|p| p.matches_dir(dir))
}

pub fn ensure_initialized() -> anyhow::Result<()> {
    let dir = store_dir();
    ensure!(dir.join(CONFIG_FILE_NAME).exists(), "privconf not initialized. Run `privconf init` first.");
    Ok(())
}

pub fn git_root(path: &std::path::Path) -> anyhow::Result<PathBuf> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(path)
        .output()?;
    if !output.status.success() {
        bail!("not inside a git repository");
    }
    Ok(PathBuf::from(String::from_utf8_lossy(&output.stdout).trim()))
}

pub fn git_is_tracked(git_root: &std::path::Path, rel_path: &std::path::Path) -> bool {
    let output = std::process::Command::new("git")
        .args(["ls-files", "--error-unmatch"])
        .arg(rel_path)
        .current_dir(git_root)
        .output();
    match output {
        Ok(out) => out.status.success(),
        Err(_) => false,
    }
}

pub fn git_add_to_exclude(git_root: &std::path::Path, rel_path: &std::path::Path) -> anyhow::Result<()> {
    let exclude_file = git_root.join(".git").join("info").join("exclude");
    let rel_str = rel_path.to_string_lossy().to_string();
    if exclude_file.exists() {
        let content = std::fs::read_to_string(&exclude_file)?;
        if content.lines().any(|line| line.trim() == rel_str) {
            return Ok(());
        }
    }
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&exclude_file)?;
    writeln!(file, "{rel_str}")?;
    Ok(())
}

pub fn git_remove_from_exclude(git_root: &std::path::Path, rel_path: &std::path::Path) -> anyhow::Result<()> {
    let exclude_file = git_root.join(".git").join("info").join("exclude");
    if !exclude_file.exists() {
        return Ok(());
    }
    let rel_str = rel_path.to_string_lossy().to_string();
    let content = std::fs::read_to_string(&exclude_file)?;
    let filtered: String = content
        .lines()
        .filter(|line| line.trim() != rel_str)
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&exclude_file, filtered)?;
    Ok(())
}

pub fn git_set_skip_worktree(git_root: &std::path::Path, rel_path: &std::path::Path) -> anyhow::Result<()> {
    let status = std::process::Command::new("git")
        .args(["update-index", "--skip-worktree"])
        .arg(rel_path)
        .current_dir(git_root)
        .status()?;
    ensure!(status.success(), "git update-index --skip-worktree failed");
    Ok(())
}

pub fn git_unset_skip_worktree(git_root: &std::path::Path, rel_path: &std::path::Path) -> anyhow::Result<()> {
    let status = std::process::Command::new("git")
        .args(["update-index", "--no-skip-worktree"])
        .arg(rel_path)
        .current_dir(git_root)
        .status()?;
    ensure!(status.success(), "git update-index --no-skip-worktree failed");
    Ok(())
}
