use crate::error::{InvalidNameSnafu, Result};

pub fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

pub fn validate_name(kind: &'static str, name: &str) -> Result<()> {
    if is_valid_name(name) {
        Ok(())
    } else {
        InvalidNameSnafu {
            kind,
            name: name.to_string(),
        }
        .fail()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_names() {
        assert!(is_valid_name("base"));
        assert!(is_valid_name("prod-1"));
        assert!(is_valid_name("web_server"));
        assert!(validate_name("Template", "base").is_ok());
    }

    #[test]
    fn test_invalid_names() {
        assert!(!is_valid_name(""));
        assert!(!is_valid_name("../evil"));
        assert!(!is_valid_name("--help"));
        assert!(!is_valid_name("-flag"));
        assert!(!is_valid_name("path/to/thing"));
        assert!(!is_valid_name("name with spaces"));
        assert!(validate_name("Template", "../evil").is_err());
    }
}
