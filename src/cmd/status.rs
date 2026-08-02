use crate::style;

pub fn run() -> anyhow::Result<()> {
    crate::config::ensure_initialized()?;

    let cwd = std::env::current_dir()?;
    let config = crate::config::load_config()?;
    let state = crate::config::load_state()?;

    let project = crate::config::find_project_for_dir(&config, &cwd);
    let all_name = crate::config::ALL_PROJECT_NAME;

    match project {
        Some(p) => {
            println!("{} {}", style::bold(&p.name), style::dim("project"));

            for file in &p.files {
                let linked = state.linked.iter().any(|e| {
                    e.project == p.name && e.file == *file && e.target.starts_with(&cwd)
                });
                let target = cwd.join(file);
                let is_symlink = target.is_symlink();
                if is_symlink && linked {
                    println!("  {} {} {} {}", style::check(), file, style::arrow(), style::green("linked"));
                } else if is_symlink {
                    println!("  {} {} {} {}", style::yellow("!"), file, style::arrow(), style::yellow("linked (other dir)"));
                } else {
                    println!("  {} {} {} {}", style::cross(), file, style::arrow(), style::dim("not linked"));
                }
            }
            for file in &p.ignored {
                let ignored = state.linked.iter().any(|e| {
                    e.project == p.name && e.file == *file && e.ignored && e.target.starts_with(&cwd)
                });
                if ignored {
                    println!("  {} {} {} {}", style::dim("-"), file, style::arrow(), style::dim("ignored"));
                } else {
                    println!("  {} {} {} {}", style::cross(), file, style::arrow(), style::dim("not ignored"));
                }
            }
        }
        None => println!("{}", style::dim("no project matches current directory")),
    }

    // Show the [all] block (applies to every project).
    if !config.all.files.is_empty() || !config.all.ignored.is_empty() {
        println!("\n{} {}", style::bold("all"), style::dim("(applies to every project)"));

        for file in &config.all.files {
            // Skip if overridden by the matched project.
            if project.is_some_and(|p| p.files.contains(file)) {
                continue;
            }
            let linked = state.linked.iter().any(|e| {
                e.project == all_name && e.file == *file && e.target.starts_with(&cwd)
            });
            let target = cwd.join(file);
            let is_symlink = target.is_symlink();
            if is_symlink && linked {
                println!("  {} {} {} {}", style::check(), file, style::arrow(), style::green("linked"));
            } else if is_symlink {
                println!("  {} {} {} {}", style::yellow("!"), file, style::arrow(), style::yellow("linked (other dir)"));
            } else {
                println!("  {} {} {} {}", style::cross(), file, style::arrow(), style::dim("not linked"));
            }
        }
        for file in &config.all.ignored {
            if project.is_some_and(|p| p.ignored.contains(file)) {
                continue;
            }
            let ignored = state.linked.iter().any(|e| {
                e.project == all_name && e.file == *file && e.ignored && e.target.starts_with(&cwd)
            });
            if ignored {
                println!("  {} {} {} {}", style::dim("-"), file, style::arrow(), style::dim("ignored"));
            } else {
                println!("  {} {} {} {}", style::cross(), file, style::arrow(), style::dim("not ignored"));
            }
        }
    }

    let local_entries: Vec<_> = state
        .linked
        .iter()
        .filter(|e| e.target.starts_with(&cwd))
        .collect();

    if !local_entries.is_empty() {
        println!("\n{}", style::dim("linked files in this directory:"));
        for entry in &local_entries {
            let kind = if entry.ignored { "ignored" } else { "linked" };
            let kind_style = if entry.ignored { style::dim(kind) } else { style::green(kind) };
            let display_project = if entry.project == all_name { "all" } else { &entry.project };
            println!(
                "  {} {} {}",
                entry.file,
                kind_style,
                style::dim(&format!("({}, skip-worktree: {})", display_project, entry.skip_worktree))
            );
        }
    }

    Ok(())
}
