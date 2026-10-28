#![allow(dead_code)]

use std::ffi::OsString;
use std::path::{Path, PathBuf};

static ENV_MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub struct ScopedEnvVar {
    vars: Vec<(&'static str, Option<OsString>)>,
    _guard: tokio::sync::MutexGuard<'static, ()>,
}

impl ScopedEnvVar {
    pub async fn set_editor(cmd: &str) -> Self {
        let guard = ENV_MUTEX.lock().await;
        let prev_visual = std::env::var_os("VISUAL");
        let prev_editor = std::env::var_os("EDITOR");
        std::env::set_var("VISUAL", cmd);
        std::env::set_var("EDITOR", cmd);
        Self {
            vars: vec![("VISUAL", prev_visual), ("EDITOR", prev_editor)],
            _guard: guard,
        }
    }
}

impl Drop for ScopedEnvVar {
    fn drop(&mut self) {
        for (key, previous) in &self.vars {
            if let Some(ref prev) = previous {
                std::env::set_var(key, prev);
            } else {
                std::env::remove_var(key);
            }
        }
    }
}

pub fn create_mock_editor(dir: &Path, content_to_append: &str) -> PathBuf {
    let script_path = dir.join(format!("mock_editor_{}.sh", std::process::id()));
    let script = format!("#!/bin/sh\necho '{content_to_append}' >> \"$1\"\n");
    std::fs::write(&script_path, script).expect("failed to write mock editor");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755))
            .expect("failed to set mock editor permissions");
    }
    script_path
}

pub fn create_syntax_error_editor(dir: &Path) -> PathBuf {
    let script_path = dir.join(format!("bad_syntax_editor_{}.sh", std::process::id()));
    let script = "#!/bin/sh\necho 'broken nix = {' >> \"$1\"\n";
    std::fs::write(&script_path, script).expect("failed to write bad editor");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755))
            .expect("failed to set bad editor permissions");
    }
    script_path
}
