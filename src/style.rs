pub const GREEN: &str = "\x1b[32m";
pub const YELLOW: &str = "\x1b[33m";
pub const RED: &str = "\x1b[31m";
pub const DIM: &str = "\x1b[2m";
pub const BOLD: &str = "\x1b[1m";
pub const RESET: &str = "\x1b[0m";

pub fn green(s: &str) -> String {
    format!("{GREEN}{s}{RESET}")
}

pub fn yellow(s: &str) -> String {
    format!("{YELLOW}{s}{RESET}")
}

pub fn red(s: &str) -> String {
    format!("{RED}{s}{RESET}")
}

pub fn dim(s: &str) -> String {
    format!("{DIM}{s}{RESET}")
}

pub fn bold(s: &str) -> String {
    format!("{BOLD}{s}{RESET}")
}

pub fn check() -> String {
    green("\u{2713}")
}

pub fn cross() -> String {
    red("\u{2717}")
}

pub fn arrow() -> String {
    dim("\u{2192}")
}
