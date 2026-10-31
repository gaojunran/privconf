use crate::style;
use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;

use anyhow::{Context, bail, ensure};

pub const STORE_DIR_NAME: &str = ".privconf";
pub const CONFIG_FILE_NAME: &str = "config.toml";
pub const STATE_FILE_NAME: &str = "state.toml";

/// Sentinel project name for files that apply to ALL projects.
/// Stored under `projects/__all__/` in the store.
pub const ALL_PROJECT_NAME: &str = "__all__";

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

#[derive(Debug, serde::Serialize, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub project: Vec<ProjectEntry>,
    /// Files/ignored that apply to ALL projects, linked on every `privconf link`.
    #[serde(default)]
    pub all: AllEntry,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct AllEntry {
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub ignored: Vec<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectEntry {
    pub name: String,
    #[serde(default)]
    pub match_remote: Option<String>,
    #[serde(default)]
    pub match_path: Option<String>,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub ignored: Vec<String>,
}

impl ProjectEntry {
    #[allow(dead_code)]
    pub fn matches_dir(&self, dir: &std::path::Path) -> bool {
        if let Some(pattern) = &self.match_path {
            let expanded = shellexpand::tilde(pattern);
            if let Ok(glob) = glob::Pattern::new(&expanded)
                && glob.matches_path(dir)
            {
                return true;
            }
        }
        if let Some(remote) = &self.match_remote
            && let Ok(git_remote) = get_git_remote(dir)
            && git_remote.contains(remote)
        {
            return true;
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
    let name = url.rsplit('/').next().unwrap_or(url);
    Some(name.to_string())
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct State {
    #[serde(default)]
    pub linked: Vec<LinkedEntry>,
}

/// `PathBuf` in `state.toml`: percent-escaped so paths with non-UTF-8 bytes
/// survive a round trip (TOML strings cannot hold them). Ordinary ASCII paths
/// serialize unchanged, so existing state files keep working.
mod path_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    use std::path::{Path, PathBuf};

    pub fn serialize<S: Serializer>(path: &Path, serializer: S) -> Result<S::Ok, S::Error> {
        let mut encoded = String::new();
        for &byte in path.as_os_str().as_bytes() {
            match byte {
                b'%' => encoded.push_str("%25"),
                0x20..=0x7e => encoded.push(byte as char),
                _ => encoded.push_str(&format!("%{byte:02X}")),
            }
        }
        serializer.serialize_str(&encoded)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<PathBuf, D::Error> {
        let encoded = String::deserialize(deserializer)?;
        let bytes = encoded.as_bytes();
        let mut decoded = Vec::with_capacity(bytes.len());
        let mut index = 0;
        while index < bytes.len() {
            let hex = (bytes[index] == b'%')
                .then(|| encoded.get(index + 1..index + 3))
                .flatten()
                .and_then(|hex| u8::from_str_radix(hex, 16).ok());
            match hex {
                Some(byte) => {
                    decoded.push(byte);
                    index += 3;
                }
                None => {
                    decoded.push(bytes[index]);
                    index += 1;
                }
            }
        }
        Ok(PathBuf::from(std::ffi::OsString::from_vec(decoded)))
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct LinkedEntry {
    pub project: String,
    pub file: String,
    #[serde(with = "path_serde")]
    pub target: PathBuf,
    pub skip_worktree: bool,
    #[serde(default)]
    pub ignored: bool,
}

/// Outcome of `link_file`, so callers can tell applied work from no-ops.
pub enum LinkOutcome {
    Linked,
    /// Already a correct symlink; git visibility was re-applied.
    Unchanged,
    /// Nothing to link (source missing from the store).
    Skipped,
}

pub fn load_config() -> anyhow::Result<Config> {
    let path = config_path();
    if !path.exists() {
        return Ok(Config::default());
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
    if let Some(ref remote) = git_remote
        && let Some(p) = config.project.iter().find(|p| {
            p.match_remote.as_ref().is_some_and(|r| remote.contains(r))
        })
    {
        return Some(p);
    }
    config.project.iter().find(|p| {
        p.match_path.as_ref().is_some_and(|pattern| {
            let expanded = shellexpand::tilde(pattern);
            if expanded.contains(['*', '?', '[']) {
                return glob::Pattern::new(&expanded)
                    .is_ok_and(|glob| glob.matches_path(dir));
            }
            // An exact path also covers everything below it (project roots are directories).
            dir.starts_with(std::path::Path::new(expanded.as_ref()))
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
    Ok(PathBuf::from(OsString::from_vec(trim_newline(output.stdout))))
}

pub fn git_common_dir(path: &std::path::Path) -> anyhow::Result<PathBuf> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--git-common-dir"])
        .current_dir(path)
        .output()?;
    if !output.status.success() {
        bail!("not inside a git repository");
    }
    let dir = PathBuf::from(OsString::from_vec(trim_newline(output.stdout)));
    Ok(if dir.is_absolute() { dir } else { path.join(dir) })
}

fn trim_newline(mut bytes: Vec<u8>) -> Vec<u8> {
    while let Some(b'\n' | b'\r') = bytes.last().copied() {
        bytes.pop();
    }
    bytes
}

/// Git path arguments are pathspecs, so glob characters in file names (`*`,
/// `?`, `[`) would be interpreted as patterns; this prefix disables that.
fn literal_path(path: impl AsRef<std::ffi::OsStr>) -> std::ffi::OsString {
    let mut literal = std::ffi::OsString::from(":(literal)");
    literal.push(path.as_ref());
    literal
}

pub fn git_is_tracked(git_root: &std::path::Path, rel_path: &std::path::Path) -> bool {
    let output = std::process::Command::new("git")
        .args(["ls-files", "--error-unmatch", "--"])
        .arg(literal_path(rel_path))
        .current_dir(git_root)
        .output();
    match output {
        Ok(out) => out.status.success(),
        Err(_) => false,
    }
}

/// Render a repo-relative path as an anchored `.git/info/exclude` pattern.
/// The file uses gitignore syntax, so meta characters must be escaped and a
/// leading `/` keeps `debug.log` from also matching `sub/debug.log`. Bytes are
/// escaped in place so non-UTF-8 file names survive unchanged.
fn exclude_pattern(rel_path: &std::path::Path) -> Vec<u8> {
    let mut pattern = vec![b'/'];
    for &byte in rel_path.as_os_str().as_bytes() {
        if matches!(byte, b'\\' | b'#' | b'!' | b'*' | b'?' | b'[' | b']' | b' ') {
            pattern.push(b'\\');
        }
        pattern.push(byte);
    }
    pattern
}

/// Match both the escaped/anchored form and the raw form written by versions
/// before escaping existed, so old entries are still deduplicated and removed.
fn exclude_line_matches(line: &[u8], rel_path: &std::path::Path) -> bool {
    line == exclude_pattern(rel_path) || line == rel_path.as_os_str().as_bytes()
}

pub fn git_add_to_exclude(git_root: &std::path::Path, rel_path: &std::path::Path) -> anyhow::Result<()> {
    use std::io::Write;

    let exclude_file = git_common_dir(git_root)?.join("info").join("exclude");
    if let Some(parent) = exclude_file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = if exclude_file.exists() {
        std::fs::read(&exclude_file)?
    } else {
        Vec::new()
    };
    if content
        .split(|&byte| byte == b'\n')
        .any(|line| exclude_line_matches(line, rel_path))
    {
        return Ok(());
    }
    let mut line = exclude_pattern(rel_path);
    line.push(b'\n');
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&exclude_file)?;
    if !content.is_empty() && !content.ends_with(b"\n") {
        file.write_all(b"\n")?;
    }
    file.write_all(&line)?;
    Ok(())
}

pub fn git_remove_from_exclude(git_root: &std::path::Path, rel_path: &std::path::Path) -> anyhow::Result<()> {
    let exclude_file = git_common_dir(git_root)?.join("info").join("exclude");
    if !exclude_file.exists() {
        return Ok(());
    }
    let content = std::fs::read(&exclude_file)?;
    let kept: Vec<&[u8]> = content
        .split(|&byte| byte == b'\n')
        .filter(|line| !exclude_line_matches(line, rel_path))
        .collect();
    let newline: &[u8] = b"\n";
    std::fs::write(&exclude_file, kept.join(newline))?;
    Ok(())
}

/// Expand a path to the index entries it covers. `git update-index` only
/// accepts entries, so a directory path must be expanded to the entries
/// inside it (a directory itself is never an index entry).
fn git_index_entries(git_root: &std::path::Path, rel_path: &std::path::Path) -> Vec<OsString> {
    let output = std::process::Command::new("git")
        .args(["ls-files", "-z", "--"])
        .arg(literal_path(rel_path))
        .current_dir(git_root)
        .output();
    match output {
        Ok(out) if out.status.success() => out
            .stdout
            .split(|&byte| byte == 0)
            .filter(|entry| !entry.is_empty())
            .map(|entry| OsStr::from_bytes(entry).to_os_string())
            .collect(),
        _ => Vec::new(),
    }
}

fn git_update_skip_worktree(
    git_root: &std::path::Path,
    rel_path: &std::path::Path,
    flag: &str,
) -> anyhow::Result<()> {
    let entries = git_index_entries(git_root, rel_path);
    ensure!(!entries.is_empty(), "{} has no tracked files", rel_path.display());
    // `update-index` takes literal paths, not pathspecs.
    let status = std::process::Command::new("git")
        .args(["update-index", flag, "--"])
        .args(&entries)
        .current_dir(git_root)
        .status()?;
    ensure!(status.success(), "git update-index {flag} failed for {}", rel_path.display());
    Ok(())
}

pub fn git_set_skip_worktree(git_root: &std::path::Path, rel_path: &std::path::Path) -> anyhow::Result<()> {
    git_update_skip_worktree(git_root, rel_path, "--skip-worktree")
}

pub fn git_unset_skip_worktree(git_root: &std::path::Path, rel_path: &std::path::Path) -> anyhow::Result<()> {
    git_update_skip_worktree(git_root, rel_path, "--no-skip-worktree")
}

/// Make `target` invisible to git: untracked paths go to `info/exclude`,
/// tracked entries get `skip-worktree`. A directory gets both — tracked
/// content is hidden per entry, untracked content by the exclude rule — so
/// files added inside it later stay hidden too. Returns whether it is tracked.
pub fn git_hide_path(git_root: &std::path::Path, target: &std::path::Path) -> anyhow::Result<bool> {
    let rel_path = rel_to_git_root(git_root, target);
    ensure!(!rel_path.as_os_str().is_empty(), "cannot ignore the repository root");
    let tracked = git_is_tracked(git_root, &rel_path);
    if tracked {
        git_set_skip_worktree(git_root, &rel_path)?;
    }
    if !tracked || target.is_dir() {
        git_add_to_exclude(git_root, &rel_path)?;
    }
    Ok(tracked)
}

pub fn backup_path(path: &std::path::Path) -> PathBuf {
    let mut new_name = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    new_name.push(".privconf.bak");
    match path.parent() {
        Some(p) => p.join(new_name),
        None => PathBuf::from(new_name),
    }
}

/// Convert an absolute path to a path relative to git_root.
/// All git commands (ls-files, update-index, info/exclude) operate on
/// git-root-relative paths, but `file` stored in config is relative to cwd
/// which may be a subdirectory of git_root.
pub fn rel_to_git_root(git_root: &std::path::Path, abs_path: &std::path::Path) -> PathBuf {
    abs_path
        .strip_prefix(git_root)
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|_| abs_path.to_path_buf())
}

pub fn link_file(
    project_name: &str,
    file: &str,
    cwd: &std::path::Path,
    git_root: Option<&std::path::Path>,
    state: &mut State,
    quiet: bool,
    backup: bool,
) -> anyhow::Result<LinkOutcome> {
    let source = project_dir(project_name).join(file);
    if !source.exists() {
        if !quiet {
            eprintln!("{} {}: {}", style::yellow("skip"), file, style::dim("source not found in store"));
        }
        return Ok(LinkOutcome::Skipped);
    }

    let target = cwd.join(file);
    let is_dir = source.is_dir();

    if target.exists() {
        if target.is_symlink()
            && let Ok(link_target) = target.read_link()
            && link_target == source
        {
            if let Some(root) = git_root {
                git_hide_path(root, &target).ok();
            }
            return Ok(LinkOutcome::Unchanged);
        }
        if backup {
            let bak = backup_path(&target);
            if target.is_dir() && !target.is_symlink() {
                std::fs::rename(&target, &bak)
                    .with_context(|| format!("backing up directory {}", target.display()))?;
                if !quiet {
                    eprintln!("  {} {} {} {}", style::check(), style::dim("backed up"), target.display(), style::dim(&format!("-> {}", bak.display())));
                }
            } else if target.is_file() || target.is_symlink() {
                std::fs::rename(&target, &bak)
                    .with_context(|| format!("backing up {}", target.display()))?;
                if !quiet {
                    eprintln!("  {} {} {} {}", style::check(), style::dim("backed up"), target.display(), style::dim(&format!("-> {}", bak.display())));
                }
            }
            if let Some(root) = git_root {
                let backup_rel = rel_to_git_root(root, &bak);
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
            let rel_path = rel_to_git_root(root, &target);
            git_add_to_exclude(root, &rel_path).ok();
        }
        state.linked.retain(|e| !(e.project == project_name && e.file == file && e.target == target));
        state.linked.push(LinkedEntry {
            project: project_name.to_string(),
            file: file.to_string(),
            target: target.clone(),
            skip_worktree: false,
            ignored: false,
        });
        if !quiet {
            eprintln!("  {} {} {}", style::check(), style::green("linked"), style::dim(&format!("{file} (directory, excluded)")));
        }
        return Ok(LinkOutcome::Linked);
    }

    let tracked = match git_root {
        Some(root) => git_hide_path(root, &target)?,
        None => false,
    };

    state.linked.retain(|e| !(e.project == project_name && e.file == file && e.target == target));
    state.linked.push(LinkedEntry {
        project: project_name.to_string(),
        file: file.to_string(),
        target: target.clone(),
        skip_worktree: tracked,
        ignored: false,
    });

    if !quiet {
        eprintln!("  {} {} {}", style::check(), style::green("linked"), style::dim(&format!("{file}{}", if tracked { " (skip-worktree)" } else { " (excluded)" })));
    }
    Ok(LinkOutcome::Linked)
}

pub fn unlink_file(
    entry: &LinkedEntry,
    git_root: Option<&std::path::Path>,
    state: &mut State,
) -> anyhow::Result<bool> {
    let target = &entry.target;
    if !target.is_symlink() {
        eprintln!("{} {}: {}", style::yellow("skip"), entry.file, style::dim("not a symlink"));
        return Ok(false);
    }

    // remove_file works for both file and directory symlinks (it removes
    // the symlink itself, not the target directory)
    std::fs::remove_file(target)
        .with_context(|| format!("removing symlink {}", target.display()))?;

    let backup = backup_path(target);
    if backup.exists() {
        std::fs::rename(&backup, target)?;
        eprintln!("  {} {} {}", style::check(), style::dim("restored"), entry.file);
    } else if let Some(root) = git_root {
        let rel_path = rel_to_git_root(root, target);
        if entry.skip_worktree {
            git_unset_skip_worktree(root, &rel_path).ok();
            let _ = std::process::Command::new("git")
                .args(["checkout", "HEAD", "--"])
                .arg(literal_path(&rel_path))
                .current_dir(root)
                .status();
            eprintln!("  {} {} {}", style::check(), style::dim("restored"), style::dim(&format!("{} (from git)", entry.file)));
        }
    }

    if let Some(root) = git_root {
        let rel_path = rel_to_git_root(root, target);
        // Both are no-ops when not applicable; a directory may carry both.
        git_unset_skip_worktree(root, &rel_path).ok();
        git_remove_from_exclude(root, &rel_path).ok();
        let backup_rel = rel_to_git_root(root, &backup);
        git_remove_from_exclude(root, &backup_rel).ok();
    }

    state.linked.retain(|e| !(e.project == entry.project && e.file == entry.file && e.target == entry.target));
    eprintln!("  {} {} {}", style::check(), style::green("unlinked"), entry.file);
    Ok(true)
}

pub fn ignore_file(
    project_name: &str,
    file: &str,
    cwd: &std::path::Path,
    git_root: Option<&std::path::Path>,
    state: &mut State,
) -> anyhow::Result<bool> {
    let target = cwd.join(file);

    // A linked file becomes a plain ignored file: drop the symlink first.
    if target.is_symlink()
        && let Some(entry) = state
            .linked
            .iter()
            .find(|e| e.project == project_name && e.file == file && e.target == target)
            .cloned()
    {
        unlink_file(&entry, git_root, state)?;
    }

    let tracked = match git_root {
        Some(root) => git_hide_path(root, &target)?,
        None => false,
    };

    state.linked.retain(|e| !(e.project == project_name && e.file == file && e.target == target));
    state.linked.push(LinkedEntry {
        project: project_name.to_string(),
        file: file.to_string(),
        target: target.clone(),
        skip_worktree: tracked,
        ignored: true,
    });

    eprintln!("  {} {} {}", style::check(), style::dim("ignored"), style::dim(&format!("{file}{}", if tracked { " (skip-worktree)" } else { " (excluded)" })));
    Ok(true)
}

pub fn unignore_file(
    entry: &LinkedEntry,
    git_root: Option<&std::path::Path>,
    state: &mut State,
) -> anyhow::Result<bool> {
    if let Some(root) = git_root {
        let rel_path = rel_to_git_root(root, &entry.target);
        // Both are no-ops when not applicable; a directory may carry both.
        git_unset_skip_worktree(root, &rel_path).ok();
        git_remove_from_exclude(root, &rel_path).ok();
    }

    state.linked.retain(|e| !(e.project == entry.project && e.file == entry.file && e.target == entry.target));
    eprintln!("  {} {} {}", style::check(), style::dim("unignored"), entry.file);
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::ffi::OsStringExt;

    #[test]
    fn state_round_trips_non_utf8_paths() {
        #[derive(serde::Serialize, serde::Deserialize)]
        struct Holder {
            #[serde(with = "path_serde")]
            target: PathBuf,
        }

        let target = PathBuf::from(std::ffi::OsString::from_vec(b"/tmp/dir\xFF/file".to_vec()));
        let encoded = toml::to_string(&Holder { target: target.clone() }).unwrap();
        assert!(encoded.contains("%FF"), "{encoded}");
        assert_eq!(toml::from_str::<Holder>(&encoded).unwrap().target, target);
    }

    #[test]
    fn state_reads_plain_utf8_paths() {
        #[derive(serde::Serialize, serde::Deserialize)]
        struct Holder {
            #[serde(with = "path_serde")]
            target: PathBuf,
        }

        let decoded = toml::from_str::<Holder>("target = \"/tmp/plain/file\"\n").unwrap();
        assert_eq!(decoded.target, PathBuf::from("/tmp/plain/file"));
    }
}
