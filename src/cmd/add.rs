use anyhow::Context;

pub fn run(name: String, files: Vec<String>) -> anyhow::Result<()> {
    crate::config::ensure_initialized()?;

    let cwd = std::env::current_dir()?;
    let git_root = crate::config::git_root(&cwd).ok();

    let mut config = crate::config::load_config()?;

    let match_remote = git_root.as_ref().and_then(|root| {
        let output = std::process::Command::new("git")
            .args(["remote", "get-url", "origin"])
            .current_dir(root)
            .output()
            .ok()?;
        if output.status.success() {
            Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            None
        }
    });

    if let Some(existing) = config.project.iter_mut().find(|p| p.name == name) {
        for file in &files {
            if !existing.files.contains(file) {
                existing.files.push(file.clone());
            }
        }
        if existing.match_remote.is_none() && match_remote.is_some() {
            existing.match_remote = match_remote.clone();
        }
        eprintln!("added files to existing project '{}'", name);
    } else {
        let project_dir = crate::config::project_dir(&name);
        std::fs::create_dir_all(&project_dir)?;

        config.project.push(crate::config::ProjectEntry {
            name: name.clone(),
            match_remote,
            match_path: None,
            files: files.clone(),
        });
        eprintln!("created project '{}'", name);
    }

    let project_dir = crate::config::project_dir(&name);
    for file in &files {
        let source = cwd.join(file);
        if !source.exists() {
            eprintln!("  warning: {} does not exist, skipping copy", file);
            continue;
        }
        let dest = project_dir.join(file);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&source, &dest)
            .with_context(|| format!("copying {} to store", file))?;
        eprintln!("  copied {} to store", file);
    }

    crate::config::save_config(&config)?;
    eprintln!("run `privconf link` to create symlinks");
    Ok(())
}
