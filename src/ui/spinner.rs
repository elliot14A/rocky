use std::time::Duration;
use tap::prelude::*;

use indicatif::{ProgressBar, ProgressStyle};

pub struct Spinner {
    bar: ProgressBar,
}

impl Spinner {
    pub fn new(message: impl Into<String>) -> Self {
        let bar = ProgressBar::new_spinner();
        let style = ProgressStyle::with_template("{spinner:.cyan} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner())
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏");
        bar.set_style(style);
        bar.set_message(message.into());
        bar.enable_steady_tick(Duration::from_millis(80));

        Self { bar }
    }

    pub fn set_message(&self, message: impl Into<String>) {
        self.bar.set_message(message.into());
    }

    pub fn success(&self, message: impl AsRef<str>) {
        self.bar.finish_and_clear();
        super::success(message);
    }

    pub fn error(&self, message: impl AsRef<str>) {
        self.bar.finish_and_clear();
        super::error(message);
    }

    pub fn finish_and_clear(&self) {
        self.bar.finish_and_clear();
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        if !self.bar.is_finished() {
            self.bar.finish_and_clear();
        }
    }
}

pub async fn with_spinner<T, E, F, Fut>(
    pending: impl Into<String>,
    success: impl AsRef<str>,
    task: F,
) -> Result<T, E>
where
    E: std::fmt::Display,
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
{
    let spinner = Spinner::new(pending);

    task()
        .await
        .tap_ok(|_| spinner.success(success.as_ref()))
        .tap_err(|_| spinner.finish_and_clear())
}
