use anyhow::Context;

pub fn run(project_name: Option<String>, files: Vec<String>) -> anyhow::Result<()> {
    crate::config::ensure_initialized()?;

    let cwd = std::env::current_dir()?;
    let git_root = crate::config::git_root(&cwd).ok();

    let name = match project_name {
        Some(n) => n,
        None => {
            let root = git_root.as_ref()
                .ok_or_else(|| anyhow::anyhow!("not in a git repo. Use -p <name> to specify project name"))?;
            crate::config::derive_project_name(root)
                .ok_or_else(|| anyhow::anyhow!("no git remote found. Use -p <name> to specify project name"))?
        }
    };

    let mut config = crate::config::load_config()?;
    let project = config.project.iter_mut().find(|p| p.name == name)
        .ok_or_else(|| anyhow::anyhow!("project '{name}' not found"))?;

    let mut state = crate::config::load_state()?;
    let mut removed_count = 0usize;

    for file in &files {
        let entry = state.linked.iter().find(|e| e.project == name && e.file == *file).cloned();
        if let Some(entry) = entry {
            crate::config::unlink_file(&entry, git_root.as_deref(), &mut state)?;
            removed_count += 1;
        } else {
            let target = cwd.join(file);
            if target.is_symlink() {
                std::fs::remove_file(&target)
                    .with_context(|| format!("removing symlink {}", target.display()))?;
                let backup = crate::config::backup_path(&target);
                if backup.exists() {
                    std::fs::rename(&backup, &target)?;
                    eprintln!("  restored {} from backup", file);
                }
                if let Some(root) = git_root.as_ref() {
                    let rel_path = std::path::PathBuf::from(file);
                    crate::config::git_remove_from_exclude(root, &rel_path).ok();
                    let backup_rel = crate::config::backup_path(&std::path::PathBuf::from(file));
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
        eprintln!("  removed {file} from store");

        project.files.retain(|f| f != file);
    }

    if project.files.is_empty() {
        let project_dir = crate::config::project_dir(&name);
        if project_dir.exists() {
            std::fs::remove_dir_all(&project_dir)?;
        }
        config.project.retain(|p| p.name != name);
        eprintln!("  removed empty project '{name}'");
    }

    crate::config::save_config(&config)?;
    crate::config::save_state(&state)?;

    eprintln!("removed {removed_count} file(s)");
    Ok(())
}
