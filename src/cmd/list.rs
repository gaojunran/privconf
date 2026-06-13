pub fn run() -> anyhow::Result<()> {
    crate::config::ensure_initialized()?;

    let config = crate::config::load_config()?;

    if config.project.is_empty() {
        println!("no projects");
        return Ok(());
    }

    for p in &config.project {
        let file_count = p.files.len();
        let ignored_count = p.ignored.len();
        let mut details = Vec::new();
        if file_count > 0 {
            details.push(format!("{file_count} file(s)"));
        }
        if ignored_count > 0 {
            details.push(format!("{ignored_count} ignored"));
        }
        if details.is_empty() {
            println!("{}", p.name);
        } else {
            println!("{} ({})", p.name, details.join(", "));
        }
    }

    Ok(())
}
