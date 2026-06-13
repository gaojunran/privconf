use anyhow::ensure;

pub fn run() -> anyhow::Result<()> {
    crate::config::ensure_initialized()?;

    let store = crate::config::store_dir();

    let status = std::process::Command::new("git")
        .args(["pull"])
        .current_dir(&store)
        .status()?;
    ensure!(status.success(), "git pull failed");

    let status = std::process::Command::new("git")
        .args(["add", "-A"])
        .current_dir(&store)
        .status()?;
    ensure!(status.success(), "git add failed");

    let has_changes = std::process::Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .current_dir(&store)
        .status()
        .map(|s| !s.success())
        .unwrap_or(true);

    if has_changes {
        let status = std::process::Command::new("git")
            .args(["commit", "-m", "sync"])
            .current_dir(&store)
            .status()?;
        ensure!(status.success(), "git commit failed");
    }

    let has_remote = std::process::Command::new("git")
        .args(["remote"])
        .current_dir(&store)
        .output()
        .map(|o| !o.stdout.trim_ascii().is_empty())
        .unwrap_or(false);

    if has_remote {
        let status = std::process::Command::new("git")
            .args(["push"])
            .current_dir(&store)
            .status()?;
        ensure!(status.success(), "git push failed");
    }

    println!("synced");
    Ok(())
}
