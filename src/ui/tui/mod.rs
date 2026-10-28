pub mod load;
pub mod state;

pub use load::{load_fleet, Environment, Fleet, Host, Secret, Service};
pub use state::{ActivePopup, TreeItem, TuiState};

use std::path::Path;

use crate::error::Result;

pub async fn run(
    _fleet_root: &Path,
    _identity: Option<&Path>,
) -> Result<()> {
    Ok(())
}
