use crate::style;
use anyhow::Context;

pub fn run(all: bool, project_name: Option<String>, files: Vec<String>) -> anyhow::Result<()> {
    crate::config::ensure_initialized()?;

    let files: Vec<String> = files.iter().map(|f| f.trim_end_matches('/').to_string()).collect();

    let cwd = std::env::current_dir()?;
    let git_root = crate::config::git_root(&cwd).ok();

    if all && project_name.is_some() {
        anyhow::bail!("--all cannot be used with -p");
    }

    let name = if all {
        crate::config::ALL_PROJECT_NAME.to_string()
    } else {
        match project_name {
            Some(n) => n,
            None => {
                let root = git_root.as_ref()
                    .ok_or_else(|| anyhow::anyhow!("not in a git repo. Use -p <name> to specify project name"))?;
                crate::config::derive_project_name(root)
                    .ok_or_else(|| anyhow::anyhow!("no git remote found. Use -p <name> to specify project name"))?
            }
        }
    };

    let mut config = crate::config::load_config()?;

    if !all {
        ensure_project_exists(&config, &name)?;
    }

    let mut state = crate::config::load_state()?;
    let mut removed_count = 0usize;

    for file in &files {
        let entry = state.linked.iter().find(|e| e.project == name && e.file == *file).cloned();
        if let Some(entry) = entry {
            if entry.ignored {
                crate::config::unignore_file(&entry, git_root.as_deref(), &mut state)?;
            } else {
                crate::config::unlink_file(&entry, git_root.as_deref(), &mut state)?;
            }
            removed_count += 1;
        } else {
            let target = cwd.join(file);
            if target.is_symlink() {
                std::fs::remove_file(&target)
                    .with_context(|| format!("removing symlink {}", target.display()))?;
                let backup = crate::config::backup_path(&target);
                if backup.exists() {
                    std::fs::rename(&backup, &target)?;
                    eprintln!("  {} {} {}", style::check(), style::dim("restored"), file);
                }
                if let Some(root) = git_root.as_ref() {
                    let rel_path = crate::config::rel_to_git_root(root, &target);
                    crate::config::git_remove_from_exclude(root, &rel_path).ok();
                    let backup_abs = crate::config::backup_path(&target);
                    let backup_rel = crate::config::rel_to_git_root(root, &backup_abs);
                    crate::config::git_remove_from_exclude(root, &backup_rel).ok();
                }
                removed_count += 1;
            }
        }

        let store_path = crate::config::project_dir(&name).join(file);
        if store_path.is_dir() {
            std::fs::remove_dir_all(&store_path)
                .with_context(|| format!("removing {} from store", file))?;
        } else if store_path.exists() {
            std::fs::remove_file(&store_path)
                .with_context(|| format!("removing {} from store", file))?;
        }
        eprintln!("  {} {} {}", style::check(), style::dim("removed"), file);

        if all {
            config.all.files.retain(|f| f != file);
            config.all.ignored.retain(|f| f != file);
        } else {
            let p = config.project.iter_mut().find(|p| p.name == name)
                .expect("project validated above");
            p.files.retain(|f| f != file);
            p.ignored.retain(|f| f != file);
        }
    }

    if !all {
        let should_remove = config.project.iter().find(|p| p.name == name)
            .is_some_and(|p| p.files.is_empty() && p.ignored.is_empty());
        if should_remove {
            let project_dir = crate::config::project_dir(&name);
            if project_dir.exists() {
                std::fs::remove_dir_all(&project_dir)?;
            }
            config.project.retain(|p| p.name != name);
            eprintln!("  {} {} {}", style::check(), style::dim("removed empty project"), style::bold(&name));
        }
    }

    crate::config::save_config(&config)?;
    crate::config::save_state(&state)?;

    eprintln!("{} {} {}", style::check(), style::green("removed"), style::dim(&format!("{removed_count} file(s)")));
    Ok(())
}

fn ensure_project_exists(config: &crate::config::Config, name: &str) -> anyhow::Result<()> {
    if !config.project.iter().any(|p| p.name == name) {
        anyhow::bail!("project '{name}' not found");
    }
    Ok(())
}
