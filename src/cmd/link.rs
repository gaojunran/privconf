use crate::style;
use anyhow::ensure;

pub fn run(quiet: bool, sync: bool) -> anyhow::Result<()> {
    crate::config::ensure_initialized()?;

    if sync {
        crate::cmd::sync::run(None, false)?;
    }

    let cwd = std::env::current_dir()?;
    let git_root = crate::config::git_root(&cwd).ok();
    let config = crate::config::load_config()?;

    let project = crate::config::find_project_for_dir(&config, &cwd)
        .ok_or_else(|| anyhow::anyhow!("no privconf project matches current directory"))?;

    let project_dir = crate::config::project_dir(&project.name);
    ensure!(project_dir.exists(), "project directory {} not found", project_dir.display());

    let mut state = crate::config::load_state()?;
    let mut linked_count = 0usize;
    let mut ignored_count = 0usize;
    let mut unchanged_count = 0usize;
    let mut skipped_count = 0usize;

    let all_name = crate::config::ALL_PROJECT_NAME;

    // Link files: all-files first (excluding those overridden by the project), then project files.
    for file in config
        .all
        .files
        .iter()
        .filter(|f| !project.files.contains(f) && !project.ignored.contains(f))
    {
        match crate::config::link_file(all_name, file, &cwd, git_root.as_deref(), &mut state, quiet, true) {
            Ok(crate::config::LinkOutcome::Linked) => linked_count += 1,
            Ok(crate::config::LinkOutcome::Unchanged) => unchanged_count += 1,
            Ok(crate::config::LinkOutcome::Skipped) => skipped_count += 1,
            Err(e) => {
                if !quiet {
                    eprintln!("{} link {}: {e}", style::cross(), file);
                }
                skipped_count += 1;
            }
        }
    }

    // A legacy config listing the same path in both lists: ignoring wins.
    for file in project.files.iter().filter(|f| !project.ignored.contains(f)) {
        match crate::config::link_file(&project.name, file, &cwd, git_root.as_deref(), &mut state, quiet, true) {
            Ok(crate::config::LinkOutcome::Linked) => linked_count += 1,
            Ok(crate::config::LinkOutcome::Unchanged) => unchanged_count += 1,
            Ok(crate::config::LinkOutcome::Skipped) => skipped_count += 1,
            Err(e) => {
                if !quiet {
                    eprintln!("{} link {}: {e}", style::cross(), file);
                }
                skipped_count += 1;
            }
        }
    }

    // Ignore files: all-ignored first (excluding those overridden by the project), then project ignored.
    for file in config
        .all
        .ignored
        .iter()
        .filter(|f| !project.ignored.contains(f) && !project.files.contains(f))
    {
        let already = state.linked.iter().any(|e| {
            e.project == all_name && e.file == *file && e.ignored && e.target.starts_with(&cwd)
        });
        if already {
            if let Some(root) = git_root.as_deref() {
                crate::config::git_hide_path(root, &cwd.join(file)).ok();
            }
            unchanged_count += 1;
        } else {
            match crate::config::ignore_file(all_name, file, &cwd, git_root.as_deref(), &mut state) {
                Ok(true) => ignored_count += 1,
                Ok(false) => skipped_count += 1,
                Err(e) => {
                    if !quiet {
                        eprintln!("{} ignore {}: {e}", style::cross(), file);
                    }
                    skipped_count += 1;
                }
            }
        }
    }

    for file in &project.ignored {
        let already = state.linked.iter().any(|e| {
            e.project == project.name && e.file == *file && e.ignored && e.target.starts_with(&cwd)
        });
        if already {
            if let Some(root) = git_root.as_deref() {
                crate::config::git_hide_path(root, &cwd.join(file)).ok();
            }
            unchanged_count += 1;
        } else {
            match crate::config::ignore_file(&project.name, file, &cwd, git_root.as_deref(), &mut state) {
                Ok(true) => ignored_count += 1,
                Ok(false) => skipped_count += 1,
                Err(e) => {
                    if !quiet {
                        eprintln!("{} ignore {}: {e}", style::cross(), file);
                    }
                    skipped_count += 1;
                }
            }
        }
    }

    crate::config::save_state(&state)?;
    if !quiet {
        let mut parts = Vec::new();
        if linked_count > 0 {
            parts.push(format!("{} linked", style::green(&linked_count.to_string())));
        }
        if ignored_count > 0 {
            parts.push(format!("{} ignored", style::dim(&ignored_count.to_string())));
        }
        if unchanged_count > 0 {
            parts.push(format!("{} unchanged", style::dim(&unchanged_count.to_string())));
        }
        if skipped_count > 0 {
            parts.push(format!("{} skipped", style::dim(&skipped_count.to_string())));
        }
        if !parts.is_empty() {
            eprintln!("{} {}", style::check(), parts.join(", "));
        }
    }
    Ok(())
}
