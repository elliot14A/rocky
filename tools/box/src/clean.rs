use rocky::error::Kind;
use rocky::ui;
use rocky::utils::validate_name;
use snafu::ResultExt;
use tap::prelude::*;

use crate::error::{IoSnafu, NotFoundSnafu, Result};
use crate::start::{box_dir, boxes_dir};
use crate::stop::stop_box;

pub async fn clean_box(name: &str) -> Result<()> {
    validate_name(Kind::Host, name)?;

    let b_dir = box_dir(name)?;
    if !b_dir.exists() {
        return NotFoundSnafu {
            name: name.to_string(),
        }
        .fail()
        .tap_err(|err| tracing::error!(%err, name = %name, "box not found"));
    }

    stop_box(name).await?;

    ui::info(format!("Cleaning box '{name}' disk and state files..."));
    tracing::info!(name = %name, path = %b_dir.display(), "cleaning box disk and state files");
    std::fs::remove_dir_all(&b_dir)
        .context(IoSnafu { path: &b_dir })
        .tap_ok(|_| {
            ui::success(format!("Cleaned box '{name}'"));
            tracing::info!(name = %name, "cleaned box successfully");
        })
        .tap_err(|err| tracing::error!(%err, name = %name, "failed to remove box directory"))?;

    Ok(())
}

pub async fn clean_all_boxes() -> Result<()> {
    let all_dir = boxes_dir()?;
    if !all_dir.exists() {
        ui::info("No boxes found to clean");
        return Ok(());
    }

    let names: Vec<String> = std::fs::read_dir(&all_dir)
        .context(IoSnafu { path: &all_dir })?
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| entry.file_name().to_str().map(ToString::to_string))
        .collect();

    if names.is_empty() {
        ui::info("No boxes found to clean");
        return Ok(());
    }

    let mut failed = false;
    for name in &names {
        if let Err(err) = clean_box(name).await {
            failed = true;
            tracing::warn!(name = %name, %err, "failed to clean box");
        }
    }

    if failed {
        ui::warn("Some boxes could not be cleaned");
    } else {
        ui::success("All local boxes cleaned");
    }

    tracing::info!(count = names.len(), failed = failed, "cleaned local boxes");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_clean_box_rejects_path_traversal() {
        assert!(clean_box("../sneaky").await.is_err());
        assert!(clean_box("..").await.is_err());
        assert!(clean_box("/root").await.is_err());
    }

    #[tokio::test]
    async fn test_clean_box_nonexistent_returns_not_found() {
        let res = clean_box("nonexistent-box-12345").await;
        assert!(res.is_err());
    }
}
