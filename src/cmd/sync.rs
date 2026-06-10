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

    let _status = std::process::Command::new("git")
        .args(["commit", "-m", "sync"])
        .current_dir(&store)
        .status();

    let status = std::process::Command::new("git")
        .args(["push"])
        .current_dir(&store)
        .status()?;
    ensure!(status.success(), "git push failed");

    println!("synced");
    Ok(())
}
