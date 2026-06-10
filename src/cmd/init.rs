use anyhow::ensure;

pub fn run() -> anyhow::Result<()> {
    let store = crate::config::store_dir();
    ensure!(!store.join(crate::config::CONFIG_FILE_NAME).exists(), "privconf already initialized at {}", store.display());

    std::fs::create_dir_all(store.join("projects"))?;

    let config = crate::config::Config { project: vec![] };
    crate::config::save_config(&config)?;

    let state = crate::config::State::default();
    crate::config::save_state(&state)?;

    let status = std::process::Command::new("git")
        .arg("init")
        .current_dir(&store)
        .status()?;
    ensure!(status.success(), "git init failed");

    println!("initialized privconf store at {}", store.display());
    println!("add a remote with: cd {} && git remote add origin <url>", store.display());
    Ok(())
}
