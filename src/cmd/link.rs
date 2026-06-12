use anyhow::{Context, ensure};

pub fn run(quiet: bool) -> anyhow::Result<()> {
    crate::config::ensure_initialized()?;

    let cwd = std::env::current_dir()?;
    let git_root = crate::config::git_root(&cwd).ok();
    let config = crate::config::load_config()?;

    let project = crate::config::find_project_for_dir(&config, &cwd)
        .ok_or_else(|| anyhow::anyhow!("no privconf project matches current directory"))?;

    let project_dir = crate::config::project_dir(&project.name);
    ensure!(project_dir.exists(), "project directory {} not found", project_dir.display());

    let mut state = crate::config::load_state()?;
    let mut linked_count = 0usize;
    let mut skipped_count = 0usize;

    for file in &project.files {
        let source = project_dir.join(file);
        if !source.exists() {
            if !quiet {
                eprintln!("  skip {file}: source not found in store");
            }
            skipped_count += 1;
            continue;
        }

        let target = cwd.join(file);
        let is_dir = source.is_dir();

        if target.exists() {
            if target.is_symlink() {
                if let Ok(link_target) = target.read_link() {
                    if link_target == source {
                        continue;
                    }
                }
            }
            let backup = backup_path(&target);
            if target.is_dir() && !target.is_symlink() {
                std::fs::rename(&target, &backup)
                    .with_context(|| format!("backing up directory {}", target.display()))?;
                if !quiet {
                    eprintln!("  backed up {} -> {}", target.display(), backup.display());
                }
            } else if target.is_file() || target.is_symlink() {
                std::fs::rename(&target, &backup)
                    .with_context(|| format!("backing up {}", target.display()))?;
                if !quiet {
                    eprintln!("  backed up {} -> {}", target.display(), backup.display());
                }
            }
            if let Some(ref root) = git_root {
                let backup_rel = backup_path(&std::path::PathBuf::from(file));
                crate::config::git_add_to_exclude(root, &backup_rel).ok();
            }
        }

        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::os::unix::fs::symlink(&source, &target)
            .with_context(|| format!("linking {} -> {}", target.display(), source.display()))?;

        if is_dir {
            if let Some(ref root) = git_root {
                crate::config::git_add_to_exclude(root, &std::path::PathBuf::from(file)).ok();
            }
            state.linked.retain(|e| !(e.project == project.name && e.file == *file));
            state.linked.push(crate::config::LinkedEntry {
                project: project.name.clone(),
                file: file.clone(),
                target: target.clone(),
                skip_worktree: false,
            });
            if !quiet {
                eprintln!("  linked {file} (directory, excluded)");
            }
            linked_count += 1;
            continue;
        }

        let rel_path = std::path::PathBuf::from(file);
        let tracked = git_root.as_ref().is_some_and(|root| crate::config::git_is_tracked(root, &rel_path));

        if let Some(ref root) = git_root {
            if tracked {
                crate::config::git_set_skip_worktree(root, &rel_path)?;
            } else {
                crate::config::git_add_to_exclude(root, &rel_path)?;
            }
        }

        state.linked.retain(|e| !(e.project == project.name && e.file == *file));
        state.linked.push(crate::config::LinkedEntry {
            project: project.name.clone(),
            file: file.clone(),
            target: target.clone(),
            skip_worktree: tracked,
        });

        if !quiet {
            eprintln!("  linked {file}{}", if tracked { " (skip-worktree)" } else { " (excluded)" });
        }
        linked_count += 1;
    }

    crate::config::save_state(&state)?;
    if !quiet {
        eprintln!("linked {linked_count} file(s), skipped {skipped_count}");
    }
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
