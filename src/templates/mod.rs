pub mod env;
mod fetch;
pub mod host;

pub(crate) const REPO: &str = "https://github.com/elliot14A/rocky.git";

pub use fetch::{fetch_partitions, fetch_provider, fetch_template};
