use anyhow::{ensure, bail};

pub fn run(message: Option<&str>, dry_run: bool) -> anyhow::Result<()> {
    crate::config::ensure_initialized()?;

    let store = crate::config::store_dir();
    let commit_message = message.unwrap_or("sync");

    let has_remote = std::process::Command::new("git")
        .args(["remote"])
        .current_dir(&store)
        .output()
        .map(|o| !o.stdout.trim_ascii().is_empty())
        .unwrap_or(false);

    if has_remote {
        if dry_run {
            println!("would run: git pull");
        } else {
            let output = std::process::Command::new("git")
                .args(["pull", "--rebase"])
                .current_dir(&store)
                .output()?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                if stderr.contains("CONFLICT") || stderr.contains("Merge conflict") {
                    bail!("merge conflict detected. Resolve conflicts in {} and retry.", store.display());
                }
                bail!("git pull failed: {}", stderr.trim());
            }
        }
    }

    if dry_run {
        let output = std::process::Command::new("git")
            .args(["diff", "--name-only"])
            .current_dir(&store)
            .output()?;
        let changed = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !changed.is_empty() {
            println!("would stage and commit the following files:");
            for line in changed.lines() {
                println!("  {line}");
            }
            println!("commit message: {commit_message}");
        } else {
            println!("no local changes to commit");
        }
        if has_remote {
            println!("would run: git push");
        }
        return Ok(());
    }

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
            .args(["commit", "-m", commit_message])
            .current_dir(&store)
            .status()?;
        ensure!(status.success(), "git commit failed");
        eprintln!("committed changes");
    } else {
        eprintln!("no changes to commit");
    }

    if has_remote {
        let status = std::process::Command::new("git")
            .args(["push"])
            .current_dir(&store)
            .status()?;
        ensure!(status.success(), "git push failed");
        eprintln!("pushed to remote");
    } else {
        eprintln!("no remote configured; skipping push");
    }

    Ok(())
}
