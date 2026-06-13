use anyhow::{Context, ensure};

pub fn run(remote: Option<&str>) -> anyhow::Result<()> {
    let store = crate::config::store_dir();
    ensure!(!store.join(crate::config::CONFIG_FILE_NAME).exists(), "privconf already initialized at {}", store.display());

    if let Some(url) = remote {
        let tmp = tempfile::tempdir()
            .with_context(|| "creating temp directory for clone")?;
        let tmp_path = tmp.path().join("store");

        let status = std::process::Command::new("git")
            .args(["clone", url])
            .arg(&tmp_path)
            .status()?;
        ensure!(status.success(), "git clone failed");

        std::fs::create_dir_all(&store)?;
        move_dir_contents(&tmp_path, &store)?;

        let projects = store.join("projects");
        if !projects.exists() {
            std::fs::create_dir_all(&projects)?;
        }

        if !store.join(crate::config::CONFIG_FILE_NAME).exists() {
            let config = crate::config::Config { project: vec![] };
            crate::config::save_config(&config)?;
        }
        if !store.join(crate::config::STATE_FILE_NAME).exists() {
            let state = crate::config::State::default();
            crate::config::save_state(&state)?;
        }

        println!("initialized privconf store from {url}");
    } else {
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

        let status = std::process::Command::new("git")
            .args(["config", "user.name", "privconf"])
            .current_dir(&store)
            .status()?;
        ensure!(status.success(), "git config user.name failed");

        let status = std::process::Command::new("git")
            .args(["config", "user.email", "privconf@localhost"])
            .current_dir(&store)
            .status()?;
        ensure!(status.success(), "git config user.email failed");

        let status = std::process::Command::new("git")
            .args(["add", "-A"])
            .current_dir(&store)
            .status()?;
        ensure!(status.success(), "git add failed");

        let status = std::process::Command::new("git")
            .args(["commit", "-m", "init privconf store"])
            .current_dir(&store)
            .status()?;
        ensure!(status.success(), "git commit failed");

        println!("initialized privconf store at {}", store.display());
        println!("add a remote with: cd {} && git remote add origin <url>", store.display());
    }
    Ok(())
}

fn move_dir_contents(src: &std::path::Path, dest: &std::path::Path) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let src_path = entry.path();
        let dest_path = dest.join(&file_name);
        if src_path.is_dir() {
            std::fs::create_dir_all(&dest_path)?;
            move_dir_contents(&src_path, &dest_path)?;
            std::fs::remove_dir(&src_path)?;
        } else {
            std::fs::rename(&src_path, &dest_path)
                .with_context(|| format!("moving {} to {}", src_path.display(), dest_path.display()))?;
        }
    }
    Ok(())
}
