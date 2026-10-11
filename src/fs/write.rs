use std::path::Path;

use snafu::ResultExt;
use tap::prelude::*;

use crate::error::{IoSnafu, Result};

pub fn write_file(path: &Path, content: &str) -> Result<()> {
    std::fs::write(path, content)
        .context(IoSnafu { path })
        .tap_ok(|_| tracing::debug!(path = %path.display(), "wrote file"))
}
