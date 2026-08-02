use crate::style;

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

    if all {
        for file in &files {
            if !config.all.ignored.contains(file) {
                config.all.ignored.push(file.clone());
            }
        }
        eprintln!("{} {} {}", style::check(), style::green("added"), style::dim(&format!("{} file(s) to all ignored", files.len())));
    } else if let Some(existing) = config.project.iter_mut().find(|p| p.name == name) {
        for file in &files {
            if !existing.ignored.contains(file) {
                existing.ignored.push(file.clone());
            }
        }
        eprintln!("{} {} {} {}", style::check(), style::green("added"), style::dim(&format!("{} file(s) to ignored list of", files.len())), style::bold(&name));
    } else {
        let project_dir = crate::config::project_dir(&name);
        std::fs::create_dir_all(&project_dir)?;

        let match_remote = git_root.as_ref().and_then(|root| {
            crate::config::get_git_remote_from_root(root)
        });

        config.project.push(crate::config::ProjectEntry {
            name: name.clone(),
            match_remote,
            match_path: None,
            files: vec![],
            ignored: files.clone(),
        });
        eprintln!("{} {} {}", style::check(), style::green("created"), style::bold(&format!("{name} (with ignored files)")));
    }

    crate::config::save_config(&config)?;

    let mut state = crate::config::load_state()?;
    for file in &files {
        crate::config::ignore_file(&name, file, &cwd, git_root.as_deref(), &mut state)?;
    }
    crate::config::save_state(&state)?;

    Ok(())
}
