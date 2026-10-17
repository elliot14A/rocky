use std::ffi::OsStr;

use tap::prelude::*;

use crate::error::Result;
use crate::process;

pub async fn fmt<I, S>(files: I) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let files: Vec<_> = files
        .into_iter()
        .map(|s| s.as_ref().to_os_string())
        .collect();

    if files.is_empty() {
        return Ok(());
    }

    let _ = process::exec("nixfmt", &files, None)
        .await
        .tap_err(|err| tracing::warn!(%err, "nixfmt was not found or failed; skipped formatting"));

    Ok(())
}
