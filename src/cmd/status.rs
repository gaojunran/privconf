pub fn run() -> anyhow::Result<()> {
    crate::config::ensure_initialized()?;

    let cwd = std::env::current_dir()?;
    let config = crate::config::load_config()?;
    let state = crate::config::load_state()?;

    let project = crate::config::find_project_for_dir(&config, &cwd);

    match project {
        Some(p) => {
            println!("project: {}", p.name);
            for file in &p.files {
                let linked = state.linked.iter().any(|e| {
                    e.project == p.name && e.file == *file && e.target.starts_with(&cwd)
                });
                let target = cwd.join(file);
                let is_symlink = target.is_symlink();
                println!("  {} {} {}", file, if is_symlink { "→" } else { "✗" }, if linked { "linked" } else { "not linked" });
            }
            for file in &p.ignored {
                let ignored = state.linked.iter().any(|e| {
                    e.project == p.name && e.file == *file && e.ignored
                });
                println!("  {} ✗ {}", file, if ignored { "ignored" } else { "not ignored" });
            }
        }
        None => println!("no project matches current directory"),
    }

    let local_entries: Vec<_> = state
        .linked
        .iter()
        .filter(|e| e.target.starts_with(&cwd))
        .collect();

    if !local_entries.is_empty() {
        println!("\nlinked files in this directory:");
        for entry in &local_entries {
            let kind = if entry.ignored { "ignored" } else { "linked" };
            println!(
                "  {} (project: {}, skip-worktree: {}, {})",
                entry.file, entry.project, entry.skip_worktree, kind
            );
        }
    }

    Ok(())
}
