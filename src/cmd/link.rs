use anyhow::ensure;

pub fn run(quiet: bool, sync: bool) -> anyhow::Result<()> {
    crate::config::ensure_initialized()?;

    if sync {
        crate::cmd::sync::run()?;
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
    let mut skipped_count = 0usize;

    for file in &project.files {
        match crate::config::link_file(&project.name, file, &cwd, git_root.as_deref(), &mut state, quiet, true) {
            Ok(true) => linked_count += 1,
            Ok(false) => skipped_count += 1,
            Err(e) => {
                if !quiet {
                    eprintln!("  error linking {file}: {e}");
                }
                skipped_count += 1;
            }
        }
    }

    for file in &project.ignored {
        let already = state.linked.iter().any(|e| {
            e.project == project.name && e.file == *file && e.ignored
        });
        if already {
            skipped_count += 1;
            continue;
        }
        match crate::config::ignore_file(&project.name, file, &cwd, git_root.as_deref(), &mut state) {
            Ok(true) => ignored_count += 1,
            Ok(false) => skipped_count += 1,
            Err(e) => {
                if !quiet {
                    eprintln!("  error ignoring {file}: {e}");
                }
                skipped_count += 1;
            }
        }
    }

    crate::config::save_state(&state)?;
    if !quiet {
        eprintln!("linked {linked_count} file(s), ignored {ignored_count}, skipped {skipped_count}");
    }
    Ok(())
}
