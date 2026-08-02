use crate::style;
use anyhow::Context;

use std::os::unix::fs::PermissionsExt;

pub fn run(all: bool, project_name: Option<String>, files: Vec<String>) -> anyhow::Result<()> {
    crate::config::ensure_initialized()?;

    let files: Vec<String> = files.iter().map(|f| f.trim_end_matches('/').to_string()).collect();

    let cwd = std::env::current_dir()?;
    let git_root = crate::config::git_root(&cwd).ok();

    if all && project_name.is_some() {
        anyhow::bail!("--all cannot be used with -p");
    }
    if all && files.is_empty() {
        anyhow::bail!("--all requires at least one file");
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

    let match_remote = git_root.as_ref().and_then(|root| {
        crate::config::get_git_remote_from_root(root)
    });

    let mut config = crate::config::load_config()?;

    if all {
        for file in &files {
            if !config.all.files.contains(file) {
                config.all.files.push(file.clone());
            }
        }
        eprintln!("{} {} {}", style::check(), style::green("added"), style::dim(&format!("{} file(s) to all", files.len())));
    } else if let Some(existing) = config.project.iter_mut().find(|p| p.name == name) {
        for file in &files {
            if !existing.files.contains(file) {
                existing.files.push(file.clone());
            }
        }
        if existing.match_remote.is_none() && match_remote.is_some() {
            existing.match_remote = match_remote.clone();
        }
        if files.is_empty() {
            eprintln!("{} {} {}", style::check(), style::bold(&name), style::dim("(already exists)"));
        } else {
            eprintln!("{} {} {} {}", style::check(), style::green("added"), style::dim(&format!("{} file(s) to", files.len())), style::bold(&name));
        }
    } else {
        let project_dir = crate::config::project_dir(&name);
        std::fs::create_dir_all(&project_dir)?;

        config.project.push(crate::config::ProjectEntry {
            name: name.clone(),
            match_remote,
            match_path: None,
            files: files.clone(),
            ignored: vec![],
        });
        eprintln!("{} project {}", style::check(), style::bold(&name));
    }

    let project_dir = crate::config::project_dir(&name);
    for file in &files {
        let source = cwd.join(file);
        if !source.exists() {
            eprintln!("{} {}: {}", style::yellow("skip"), file, style::dim("does not exist"));
            continue;
        }
        let dest = project_dir.join(file);
        if source.is_dir() {
            copy_dir_recursive(&source, &dest)?;
            eprintln!("  {} {} {}", style::check(), style::dim("copied"), style::dim(&format!("{file} (directory)")));
        } else {
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            copy_file_preserving(&source, &dest)
                .with_context(|| format!("copying {} to store", file))?;
            eprintln!("  {} {} {}", style::check(), style::dim("copied"), file);
        }
    }

    crate::config::save_config(&config)?;

    let mut state = crate::config::load_state()?;
    let mut linked_count = 0usize;
    for file in &files {
        if crate::config::link_file(&name, file, &cwd, git_root.as_deref(), &mut state, false, false)? {
            linked_count += 1;
        }
    }
    crate::config::save_state(&state)?;

    if linked_count > 0 {
        eprintln!("{} {} {}", style::check(), style::green("linked"), style::dim(&format!("{linked_count} file(s)")));
    }
    Ok(())
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
            copy_file_preserving(&src_path, &dest_path)
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
            copy_file_preserving(&src_path, &dest_path)
                .with_context(|| format!("copying {} to store", src_path.display()))?;
        }
    }
    Ok(())
}

fn copy_file_preserving(src: &std::path::Path, dest: &std::path::Path) -> anyhow::Result<()> {
    std::fs::copy(src, dest)
        .with_context(|| format!("copying {} to {}", src.display(), dest.display()))?;
    let mode = src.metadata()?.permissions().mode();
    std::fs::set_permissions(dest, std::fs::Permissions::from_mode(mode))?;
    Ok(())
}
