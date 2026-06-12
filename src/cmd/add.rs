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

    let match_remote = git_root.as_ref().and_then(|root| {
        crate::config::get_git_remote_from_root(root)
    });

    let expanded = expand_paths(&cwd, &files)?;

    let mut config = crate::config::load_config()?;

    if let Some(existing) = config.project.iter_mut().find(|p| p.name == name) {
        for file in &expanded {
            if !existing.files.contains(file) {
                existing.files.push(file.clone());
            }
        }
        if existing.match_remote.is_none() && match_remote.is_some() {
            existing.match_remote = match_remote.clone();
        }
        if expanded.is_empty() {
            eprintln!("project '{name}' already exists");
        } else {
            eprintln!("added files to existing project '{name}'");
        }
    } else {
        let project_dir = crate::config::project_dir(&name);
        std::fs::create_dir_all(&project_dir)?;

        config.project.push(crate::config::ProjectEntry {
            name: name.clone(),
            match_remote,
            match_path: None,
            files: expanded.clone(),
        });
        eprintln!("created project '{name}'");
    }

    let project_dir = crate::config::project_dir(&name);
    for file in &expanded {
        let source = cwd.join(file);
        if !source.exists() {
            eprintln!("  warning: {} does not exist, skipping copy", file);
            continue;
        }
        let dest = project_dir.join(file);
        if source.is_dir() {
            copy_dir_recursive(&source, &dest)?;
            eprintln!("  copied directory {} to store", file);
        } else {
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&source, &dest)
                .with_context(|| format!("copying {} to store", file))?;
            eprintln!("  copied {} to store", file);
        }
    }

    crate::config::save_config(&config)?;

    let mut state = crate::config::load_state()?;
    let mut linked_count = 0usize;
    for file in &expanded {
        if crate::config::link_file(&name, file, &cwd, git_root.as_deref(), &mut state, false, false)? {
            linked_count += 1;
        }
    }
    crate::config::save_state(&state)?;

    if linked_count > 0 {
        eprintln!("linked {linked_count} file(s)");
    }
    Ok(())
}

fn expand_paths(cwd: &std::path::Path, files: &[String]) -> anyhow::Result<Vec<String>> {
    let mut result = Vec::new();
    for file in files {
        let path = cwd.join(file);
        if !path.exists() {
            result.push(file.clone());
        } else {
            result.push(file.clone());
        }
    }
    Ok(result)
}

fn copy_dir_recursive(src: &std::path::Path, dest: &std::path::Path) -> anyhow::Result<()> {
    if dest.exists() {
        merge_dir_recursive(src, dest)?;
    } else {
        std::fs::create_dir_all(dest)?;
        copy_dir_contents(src, dest)?;
    }
    Ok(())
}

fn copy_dir_contents(src: &std::path::Path, dest: &std::path::Path) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());
        if src_path.is_dir() {
            std::fs::create_dir_all(&dest_path)?;
            copy_dir_contents(&src_path, &dest_path)?;
        } else {
            std::fs::copy(&src_path, &dest_path)
                .with_context(|| format!("copying {} to store", src_path.display()))?;
        }
    }
    Ok(())
}

fn merge_dir_recursive(src: &std::path::Path, dest: &std::path::Path) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());
        if src_path.is_dir() {
            std::fs::create_dir_all(&dest_path)?;
            merge_dir_recursive(&src_path, &dest_path)?;
        } else if !dest_path.exists() {
            std::fs::copy(&src_path, &dest_path)
                .with_context(|| format!("copying {} to store", src_path.display()))?;
        }
    }
    Ok(())
}
