use crate::style;

pub fn run() -> anyhow::Result<()> {
    crate::config::ensure_initialized()?;

    let config = crate::config::load_config()?;

    if config.project.is_empty() && config.all.files.is_empty() && config.all.ignored.is_empty() {
        println!("{} {}", style::dim("no projects found"), style::cross());
        return Ok(());
    }

    // Show the [all] block first.
    if !config.all.files.is_empty() || !config.all.ignored.is_empty() {
        let file_count = config.all.files.len();
        let ignored_count = config.all.ignored.len();
        let mut details = Vec::new();
        if file_count > 0 {
            details.push(style::green(&format!("{file_count} file(s)")));
        }
        if ignored_count > 0 {
            details.push(style::dim(&format!("{ignored_count} ignored")));
        }
        println!("{} {} {}", style::bold("all"), details.join(", "), style::dim("(every project)"));
    }

    for p in &config.project {
        let file_count = p.files.len();
        let ignored_count = p.ignored.len();
        let mut details = Vec::new();
        if file_count > 0 {
            details.push(style::green(&format!("{file_count} file(s)")));
        }
        if ignored_count > 0 {
            details.push(style::dim(&format!("{ignored_count} ignored")));
        }
        if !details.is_empty() {
            println!("{} {}", style::bold(&p.name), details.join(", "));
        } else {
            println!("{} {}", style::dim("-"), style::bold(&p.name));
        }
    }

    Ok(())
}
