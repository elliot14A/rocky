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

    #[snafu(display("'{}' is not a directory", path.display()))]
    NotADirectory { path: PathBuf },

    #[snafu(display("Directory '{}' is not empty", path.display()))]
    TargetNotEmpty { path: PathBuf },

    #[snafu(display("Failed to execute process '{command}': {source}"))]
    ProcessSpawn {
        command: String,
        source: std::io::Error,
    },

    #[snafu(display(
        "Command '{command}' failed with exit code {status}{}: {stderr}",
        log_path.as_ref().map(|p| format!(" (see log: {})", p.display())).unwrap_or_default()
    ))]
    CommandExecution {
        command: String,
        status: i32,
        stderr: String,
        log_path: Option<PathBuf>,
    },

    #[snafu(display("Command '{command}' was terminated by signal"))]
    CommandSignaled { command: String },

    #[snafu(display("{kind} '{name}' already exists"))]
    AlreadyExists {
        kind: Kind,
        name: String,
        path: PathBuf,
    },

    #[snafu(display("{kind} '{name}' not found"))]
    NotFound {
        kind: Kind,
        name: String,
        path: PathBuf,
    },

    #[snafu(display("Invalid {kind} '{name}'"))]
    InvalidName { kind: Kind, name: String },

    #[snafu(display("Insecure path '{}'", path.display()))]
    InsecurePath { path: PathBuf, reason: String },

    #[snafu(display("Failed to parse Nix file at {}: {reason}", path.display()))]
    NixParse { path: PathBuf, reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Fleet,
    Template,
    Provider,
    Environment,
    Host,
    Service,
    Secret,
}

impl std::fmt::Display for Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fleet => write!(f, "Fleet"),
            Self::Template => write!(f, "Template"),
            Self::Provider => write!(f, "Provider"),
            Self::Environment => write!(f, "Environment"),
            Self::Host => write!(f, "Host"),
            Self::Service => write!(f, "Service"),
            Self::Secret => write!(f, "Secret"),
        }
    }
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
            log_path: None,
        };
        assert_eq!(
            err.to_string(),
            "Command 'nix build' failed with exit code 1: syntax error"
        );
    }

    #[test]
    fn test_command_execution_with_log_path_display() {
        let err = RockyError::CommandExecution {
            command: "nixos-anywhere".to_string(),
            status: 1,
            stderr: "disk error".to_string(),
            log_path: Some(PathBuf::from("/var/log/install.log")),
        };
        assert_eq!(
            err.to_string(),
            "Command 'nixos-anywhere' failed with exit code 1 (see log: /var/log/install.log): disk error"
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
            kind: Kind::Environment,
            name: "prod".to_string(),
            path: PathBuf::from("environments/prod"),
        };
        assert_eq!(err.to_string(), "Environment 'prod' already exists");
    }

    #[test]
    fn test_not_found_display() {
        let err = RockyError::NotFound {
            kind: Kind::Provider,
            name: "aws".to_string(),
            path: PathBuf::from("templates/providers/aws"),
        };
        assert_eq!(err.to_string(), "Provider 'aws' not found");
    }

    #[test]
    fn test_path_display_not_escaped() {
        let err = RockyError::TargetNotEmpty {
            path: PathBuf::from("/var/test"),
        };
        assert_eq!(err.to_string(), "Directory '/var/test' is not empty");
    }

    #[test]
    fn test_invalid_name_display() {
        let err = RockyError::InvalidName {
            kind: Kind::Template,
            name: "../evil".to_string(),
        };
        assert_eq!(err.to_string(), "Invalid Template '../evil'");
    }

    #[test]
    fn test_insecure_path_display() {
        let err = RockyError::InsecurePath {
            path: PathBuf::from("/etc/passwd"),
            reason: "symlink escapes source directory".to_string(),
        };
        assert_eq!(err.to_string(), "Insecure path '/etc/passwd'");
    }

    #[test]
    fn test_nix_parse_display() {
        let err = RockyError::NixParse {
            path: PathBuf::from("environments/prod/fleet.nix"),
            reason: "syntax error".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Failed to parse Nix file at environments/prod/fleet.nix: syntax error"
        );
    }
}
