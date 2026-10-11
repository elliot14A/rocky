pub mod spinner;

pub use spinner::{with_spinner, Spinner};

use std::io::IsTerminal;

pub fn use_colors() -> bool {
    std::io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

pub fn success(message: impl AsRef<str>) {
    if use_colors() {
        eprintln!("\x1b[32m✔\x1b[0m {}", message.as_ref());
    } else {
        eprintln!("✔ {}", message.as_ref());
    }
}

pub fn error(message: impl AsRef<str>) {
    if use_colors() {
        eprintln!("\x1b[31m✖\x1b[0m {}", message.as_ref());
    } else {
        eprintln!("✖ {}", message.as_ref());
    }
}

pub fn warn(message: impl AsRef<str>) {
    if use_colors() {
        eprintln!("\x1b[33m!\x1b[0m {}", message.as_ref());
    } else {
        eprintln!("! {}", message.as_ref());
    }
}

pub fn info(message: impl AsRef<str>) {
    if use_colors() {
        eprintln!("\x1b[34mℹ\x1b[0m {}", message.as_ref());
    } else {
        eprintln!("ℹ {}", message.as_ref());
    }
}

pub fn next_steps(steps: &[impl AsRef<str>]) {
    if steps.is_empty() {
        return;
    }
    eprintln!();
    if use_colors() {
        eprintln!("\x1b[1mNext steps:\x1b[0m");
        for (i, step) in steps.iter().enumerate() {
            eprintln!(
                "  \x1b[2m{}.\x1b[0m \x1b[36m{}\x1b[0m",
                i + 1,
                step.as_ref()
            );
        }
    } else {
        eprintln!("Next steps:");
        for (i, step) in steps.iter().enumerate() {
            eprintln!("  {}. {}", i + 1, step.as_ref());
        }
    }
}
