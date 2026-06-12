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
    #[allow(dead_code)]
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

pub fn get_git_remote_from_root(git_root: &std::path::Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(git_root)
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

pub fn derive_project_name(git_root: &std::path::Path) -> Option<String> {
    let remote = get_git_remote_from_root(git_root)?;
    let url = remote.trim_end_matches(".git");
    let name = url.rsplit('/').next().unwrap_or(&url);
    Some(name.to_string())
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
    let git_remote = get_git_remote(dir).ok();
    if let Some(ref remote) = git_remote {
        if let Some(p) = config.project.iter().find(|p| {
            p.match_remote.as_ref().is_some_and(|r| remote.contains(r))
        }) {
            return Some(p);
        }
    }
    config.project.iter().find(|p| {
        p.match_path.as_ref().is_some_and(|pattern| {
            let expanded = shellexpand::tilde(pattern);
            glob::Pattern::new(&expanded)
                .is_ok_and(|glob| glob.matches_path(dir))
        })
    })
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

pub fn backup_path(path: &std::path::Path) -> PathBuf {
    if path.extension().is_some() {
        path.with_extension("privconf.bak")
    } else {
        let file_name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let parent = path.parent();
        let mut new_name = file_name;
        new_name.push_str(".privconf.bak");
        match parent {
            Some(p) => p.join(new_name),
            None => PathBuf::from(new_name),
        }
    }
}

pub fn link_file(
    project_name: &str,
    file: &str,
    cwd: &std::path::Path,
    git_root: Option<&std::path::Path>,
    state: &mut State,
    quiet: bool,
    backup: bool,
) -> anyhow::Result<bool> {
    let source = project_dir(project_name).join(file);
    if !source.exists() {
        if !quiet {
            eprintln!("  skip {file}: source not found in store");
        }
        return Ok(false);
    }

    let target = cwd.join(file);
    let is_dir = source.is_dir();

    if target.exists() {
        if target.is_symlink() {
            if let Ok(link_target) = target.read_link() {
                if link_target == source {
                    return Ok(false);
                }
            }
        }
        if backup {
            let bak = backup_path(&target);
            if target.is_dir() && !target.is_symlink() {
                std::fs::rename(&target, &bak)
                    .with_context(|| format!("backing up directory {}", target.display()))?;
                if !quiet {
                    eprintln!("  backed up {} -> {}", target.display(), bak.display());
                }
            } else if target.is_file() || target.is_symlink() {
                std::fs::rename(&target, &bak)
                    .with_context(|| format!("backing up {}", target.display()))?;
                if !quiet {
                    eprintln!("  backed up {} -> {}", target.display(), bak.display());
                }
            }
            if let Some(root) = git_root {
                let backup_rel = backup_path(&PathBuf::from(file));
                git_add_to_exclude(root, &backup_rel).ok();
            }
        } else {
            if target.is_dir() && !target.is_symlink() {
                std::fs::remove_dir_all(&target)
                    .with_context(|| format!("removing directory {}", target.display()))?;
            } else {
                std::fs::remove_file(&target)
                    .with_context(|| format!("removing {}", target.display()))?;
            }
        }
    }

    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::os::unix::fs::symlink(&source, &target)
        .with_context(|| format!("linking {} -> {}", target.display(), source.display()))?;

    if is_dir {
        if let Some(root) = git_root {
            git_add_to_exclude(root, &PathBuf::from(file)).ok();
        }
        state.linked.retain(|e| !(e.project == project_name && e.file == file));
        state.linked.push(LinkedEntry {
            project: project_name.to_string(),
            file: file.to_string(),
            target: target.clone(),
            skip_worktree: false,
        });
        if !quiet {
            eprintln!("  linked {file} (directory, excluded)");
        }
        return Ok(true);
    }

    let rel_path = PathBuf::from(file);
    let tracked = git_root.is_some_and(|root| git_is_tracked(root, &rel_path));

    if let Some(root) = git_root {
        if tracked {
            git_set_skip_worktree(root, &rel_path)?;
        } else {
            git_add_to_exclude(root, &rel_path)?;
        }
    }

    state.linked.retain(|e| !(e.project == project_name && e.file == file));
    state.linked.push(LinkedEntry {
        project: project_name.to_string(),
        file: file.to_string(),
        target: target.clone(),
        skip_worktree: tracked,
    });

    if !quiet {
        eprintln!("  linked {file}{}", if tracked { " (skip-worktree)" } else { " (excluded)" });
    }
    Ok(true)
}

pub fn unlink_file(
    entry: &LinkedEntry,
    git_root: Option<&std::path::Path>,
    state: &mut State,
) -> anyhow::Result<bool> {
    let target = &entry.target;
    if !target.is_symlink() {
        eprintln!("  skip {}: not a symlink", entry.file);
        return Ok(false);
    }

    std::fs::remove_file(target)
        .with_context(|| format!("removing symlink {}", target.display()))?;

    let backup = backup_path(target);
    if backup.exists() {
        std::fs::rename(&backup, target)?;
        eprintln!("  restored {} from backup", entry.file);
    } else if let Some(root) = git_root {
        let rel_path = PathBuf::from(&entry.file);
        if entry.skip_worktree {
            git_unset_skip_worktree(root, &rel_path).ok();
            let _ = std::process::Command::new("git")
                .args(["checkout", "HEAD", "--"])
                .arg(&rel_path)
                .current_dir(root)
                .status();
            eprintln!("  restored {} from git", entry.file);
        }
    }

    if let Some(root) = git_root {
        let rel_path = PathBuf::from(&entry.file);
        if entry.skip_worktree {
            git_unset_skip_worktree(root, &rel_path).ok();
        } else {
            git_remove_from_exclude(root, &rel_path).ok();
        }
        let backup_rel = backup_path(&PathBuf::from(&entry.file));
        git_remove_from_exclude(root, &backup_rel).ok();
    }

    state.linked.retain(|e| !(e.project == entry.project && e.file == entry.file));
    eprintln!("  unlinked {}", entry.file);
    Ok(true)
}
