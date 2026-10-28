use std::path::Path;

use crate::error::Result;

pub async fn run(
    _fleet_root: &Path,
    _identity: Option<&Path>,
) -> Result<()> {
    Ok(())
}
