use std::time::Duration;

use rocky::error::Kind;
use rocky::ui;
use rocky::validate_name;

use crate::error::Result;
use crate::start::{box_dir, is_pid_alive, read_active_pid};

pub async fn stop_box(name: &str) -> Result<()> {
    validate_name(Kind::Host, name)?;

    let b_dir = box_dir(name)?;
    let pid_file = b_dir.join("pid");

    let pid = match read_active_pid(&pid_file)? {
        Some(p) => p,
        None => {
            ui::info(format!("Box '{name}' is not running"));
            tracing::info!(name = %name, "box is not running");
            return Ok(());
        }
    };

    ui::info(format!("Stopping box '{name}' (PID: {pid})..."));
    tracing::info!(name = %name, pid = pid, "stopping box");

    unsafe {
        libc::kill(pid as i32, libc::SIGTERM);
    }

    let stopped = wait_until_stopped(pid, 20, Duration::from_millis(250)).await;

    if stopped {
        let _ = std::fs::remove_file(&pid_file);
        ui::success(format!("Stopped box '{name}'"));
        tracing::info!(name = %name, pid = pid, "stopped box");
        return Ok(());
    }

    unsafe {
        libc::kill(pid as i32, libc::SIGKILL);
    }

    let _ = std::fs::remove_file(&pid_file);
    ui::warn(format!("Force killed box '{name}' (PID: {pid})"));
    tracing::warn!(name = %name, pid = pid, "force killed box");
    Ok(())
}

async fn wait_until_stopped(pid: u32, max_attempts: usize, interval: Duration) -> bool {
    let mut attempts = 0;
    while attempts < max_attempts {
        tokio::time::sleep(interval).await;
        if !is_pid_alive(pid) {
            return true;
        }
        attempts += 1;
    }
    false
}
