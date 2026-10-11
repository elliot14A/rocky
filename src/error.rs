use snafu::Snafu;
use std::path::PathBuf;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum RockyError {
    #[snafu(display("I/O error at {}: {source}", path.display()))]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },

    #[snafu(display("Failed to copy from {} to {}: {source}", src.display(), dst.display()))]
    Copy {
        src: PathBuf,
        dst: PathBuf,
        source: std::io::Error,
    },

    #[snafu(display("Path '{}' is not a directory", path.display()))]
    NotADirectory { path: PathBuf },

    #[snafu(display("Target directory '{}' is not empty", path.display()))]
    TargetNotEmpty { path: PathBuf },

    #[snafu(display("Failed to execute process '{command}': {source}"))]
    ProcessSpawn {
        command: String,
        source: std::io::Error,
    },

    #[snafu(display("Command '{command}' failed with exit code {status}: {stderr}"))]
    CommandExecution {
        command: String,
        status: i32,
        stderr: String,
    },

    #[snafu(display("Command '{command}' was terminated by signal"))]
    CommandSignaled { command: String },

    #[snafu(display("{kind} '{name}' already exists at {}", path.display()))]
    AlreadyExists {
        kind: &'static str,
        name: String,
        path: PathBuf,
    },

    #[snafu(display("{kind} '{name}' not found at {}", path.display()))]
    NotFound {
        kind: &'static str,
        name: String,
        path: PathBuf,
    },
}

pub type Result<T, E = RockyError> = std::result::Result<T, E>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_execution_error_display() {
        let err = RockyError::CommandExecution {
            command: "nix build".to_string(),
            status: 1,
            stderr: "syntax error".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Command 'nix build' failed with exit code 1: syntax error"
        );
    }

    #[test]
    fn test_copy_error_display() {
        let err = RockyError::Copy {
            src: PathBuf::from("/src/test"),
            dst: PathBuf::from("/dst/test"),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "file not found"),
        };
        assert_eq!(
            err.to_string(),
            "Failed to copy from /src/test to /dst/test: file not found"
        );
    }

    #[test]
    fn test_already_exists_display() {
        let err = RockyError::AlreadyExists {
            kind: "Environment",
            name: "prod".to_string(),
            path: PathBuf::from("environments/prod"),
        };
        assert_eq!(
            err.to_string(),
            "Environment 'prod' already exists at environments/prod"
        );
    }

    #[test]
    fn test_not_found_display() {
        let err = RockyError::NotFound {
            kind: "Provider",
            name: "aws".to_string(),
            path: PathBuf::from("templates/providers/aws"),
        };
        assert_eq!(
            err.to_string(),
            "Provider 'aws' not found at templates/providers/aws"
        );
    }

    #[test]
    fn test_path_display_not_escaped() {
        let err = RockyError::TargetNotEmpty {
            path: PathBuf::from("/var/test"),
        };
        assert_eq!(err.to_string(), "Target directory '/var/test' is not empty");
    }
}
