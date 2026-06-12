use anyhow::{Context, bail};

pub fn run() -> anyhow::Result<()> {
    crate::config::ensure_initialized()?;

    let cwd = std::env::current_dir()?;
    let git_root = crate::config::git_root(&cwd).ok();
    let state = crate::config::load_state()?;

    let entries: Vec<_> = state
        .linked
        .iter()
        .filter(|e| e.target.starts_with(&cwd))
        .cloned()
        .collect();

    if entries.is_empty() {
        bail!("no linked files found in current directory");
    }

    let mut state = state;
    let mut unlinked = 0usize;
    let mut not_linked = 0usize;

    for entry in &entries {
        let target = &entry.target;
        if !target.is_symlink() {
            eprintln!("  skip {}: not a symlink", entry.file);
            not_linked += 1;
            continue;
        }

        let is_dir = target.read_link().is_ok_and(|p| p.is_dir());

        std::fs::remove_file(target)
            .with_context(|| format!("removing symlink {}", target.display()))?;

        let backup = backup_path(target);
        if backup.exists() {
            std::fs::rename(&backup, target)?;
            eprintln!("  restored {} from backup", entry.file);
        } else if let Some(ref git_root) = git_root {
            let rel_path = std::path::PathBuf::from(&entry.file);
            if entry.skip_worktree {
                let _ = std::process::Command::new("git")
                    .args(["checkout", "HEAD", "--"])
                    .arg(&rel_path)
                    .current_dir(git_root)
                    .status();
                eprintln!("  restored {} from git", entry.file);
            }
        }

        if let Some(ref git_root) = git_root {
            let rel_path = std::path::PathBuf::from(&entry.file);
            if entry.skip_worktree {
                crate::config::git_unset_skip_worktree(git_root, &rel_path).ok();
            } else {
                crate::config::git_remove_from_exclude(git_root, &rel_path).ok();
            }
            let backup_rel = backup_path(&std::path::PathBuf::from(&entry.file));
            crate::config::git_remove_from_exclude(git_root, &backup_rel).ok();
        }

        state.linked.retain(|e| !(e.project == entry.project && e.file == entry.file));
        eprintln!("  unlinked {}", entry.file);
        unlinked += 1;
    }

    crate::config::save_state(&state)?;
    eprintln!("unlinked {unlinked} file(s), {not_linked} not symlinks");
    Ok(())
}

fn backup_path(path: &std::path::Path) -> std::path::PathBuf {
    if path.extension().is_some() {
        path.with_extension("privconf.bak")
    } else {
        let file_name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let parent = path.parent();
        let mut new_name = file_name;
        new_name.push_str(".privconf.bak");
        match parent {
            Some(p) => p.join(new_name),
            None => std::path::PathBuf::from(new_name),
        }
    }
}
