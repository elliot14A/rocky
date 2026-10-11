use std::ffi::OsStr;

use crate::error::Result;
use crate::process;

pub async fn fmt<I, S>(files: I) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let files_vec: Vec<_> = files.into_iter().collect();
    if files_vec.is_empty() {
        return Ok(());
    }

    if let Err(err) = process::exec("nixfmt", files_vec, None).await {
        tracing::warn!(%err, "nixfmt was not found or failed; skipped formatting");
    }

    Ok(())
}
