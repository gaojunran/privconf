use anyhow::bail;

pub fn run() -> anyhow::Result<()> {
    crate::config::ensure_initialized()?;

    let cwd = std::env::current_dir()?;
    let git_root = crate::config::git_root(&cwd).ok();
    let state = crate::config::load_state()?;

    let entries: Vec<_> = state
        .linked
        .iter()
        .filter(|e| e.target.starts_with(&cwd))
        .cloned()
        .collect();

    if entries.is_empty() {
        bail!("no linked files found in current directory");
    }

    let mut state = state;
    let mut unlinked = 0usize;
    let mut not_linked = 0usize;

    for entry in &entries {
        match crate::config::unlink_file(entry, git_root.as_deref(), &mut state) {
            Ok(true) => unlinked += 1,
            Ok(false) => not_linked += 1,
            Err(e) => {
                eprintln!("  error unlinking {}: {e}", entry.file);
                not_linked += 1;
            }
        }
    }

    crate::config::save_state(&state)?;
    eprintln!("unlinked {unlinked} file(s), {not_linked} not symlinks");
    Ok(())
}
