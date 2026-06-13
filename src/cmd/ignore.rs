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

    if let Some(existing) = config.project.iter_mut().find(|p| p.name == name) {
        for file in &files {
            if !existing.ignored.contains(file) {
                existing.ignored.push(file.clone());
            }
        }
        eprintln!("added files to ignored list of project '{name}'");
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
        eprintln!("created project '{name}' with ignored files");
    }

    crate::config::save_config(&config)?;

    let mut state = crate::config::load_state()?;
    for file in &files {
        crate::config::ignore_file(&name, file, &cwd, git_root.as_deref(), &mut state)?;
    }
    crate::config::save_state(&state)?;

    Ok(())
}
