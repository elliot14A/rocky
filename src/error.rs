use snafu::Snafu;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum RockyError {
    #[snafu(display("I/O error: {source}"))]
    Io { source: std::io::Error },

    #[snafu(display("Command '{command}' failed with exit code {status}"))]
    CommandExecution { command: String, status: i32 },

    #[snafu(display("Command '{command}' was terminated by signal"))]
    CommandSignaled { command: String },
}

pub type Result<T, E = RockyError> = std::result::Result<T, E>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = RockyError::CommandExecution {
            command: "nix build".to_string(),
            status: 1,
        };
        assert_eq!(err.to_string(), "Command 'nix build' failed with exit code 1");
    }
}
