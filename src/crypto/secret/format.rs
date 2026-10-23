use crate::error::{Result, RockyError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretFormat {
    KeyValue,
    Raw,
}

fn parse_key_value_line(line: &str) -> Option<Result<(&str, &str), ()>> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
        return None;
    }
    let Some((key_part, val_part)) = trimmed.split_once('=') else {
        return Some(Err(()));
    };
    let key = key_part.trim();
    if !is_valid_identifier(key) {
        return Some(Err(()));
    }
    let val = val_part.trim();
    if is_invalid_quoted(val) {
        return Some(Err(()));
    }
    Some(Ok((key, val)))
}

fn is_valid_identifier(key: &str) -> bool {
    let mut chars = key.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphabetic() && first != '_' {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn is_invalid_quoted(val: &str) -> bool {
    (val.starts_with('"') && (!val.ends_with('"') || val.len() < 2))
        || (val.starts_with('\'') && (!val.ends_with('\'') || val.len() < 2))
}

fn strip_quotes(val: &str) -> &str {
    if (val.starts_with('"') && val.ends_with('"') && val.len() >= 2)
        || (val.starts_with('\'') && val.ends_with('\'') && val.len() >= 2)
    {
        &val[1..val.len() - 1]
    } else {
        val
    }
}

pub fn has_key_value_entries(content: &str) -> bool {
    content
        .lines()
        .any(|line| matches!(parse_key_value_line(line), Some(Ok(_))))
}

pub fn detect_secret_format(content: &str) -> SecretFormat {
    let mut has_entries = false;
    for line in content.lines() {
        match parse_key_value_line(line) {
            Some(Ok(_)) => {
                has_entries = true;
            }
            Some(Err(())) => {
                return SecretFormat::Raw;
            }
            None => {}
        }
    }

    if has_entries {
        SecretFormat::KeyValue
    } else {
        SecretFormat::Raw
    }
}

pub fn validate_key_value(content: &str) -> Result<()> {
    for (idx, line) in content.lines().enumerate() {
        let line_num = idx + 1;
        match parse_key_value_line(line) {
            Some(Ok(_)) => {}
            Some(Err(())) => {
                return Err(RockyError::InvalidSecretSyntax {
                    line: line_num,
                    line_content: line.to_string(),
                });
            }
            None => {}
        }
    }
    Ok(())
}

pub fn parse_key_values(content: &str) -> Result<Vec<(String, String)>> {
    validate_key_value(content)?;

    let mut entries = Vec::new();
    for line in content.lines() {
        if let Some(Ok((k, v))) = parse_key_value_line(line) {
            entries.push((k.to_string(), strip_quotes(v).to_string()));
        }
    }
    Ok(entries)
}

pub fn extract_key(content: &str, key: &str) -> Result<Option<String>> {
    let entries = parse_key_values(content)?;
    Ok(entries
        .into_iter()
        .rev()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_format_key_value() {
        let content = r#"# Database configuration
; Semicolon comment
DB_HOST=10.0.0.1
DB_PORT=5432
DB_USER="postgres"
EMPTY_VAL=
"#;
        assert_eq!(detect_secret_format(content), SecretFormat::KeyValue);

        assert_eq!(
            detect_secret_format("JWT_SECRET=supersecret\n"),
            SecretFormat::KeyValue
        );
    }

    #[test]
    fn test_detect_format_raw() {
        assert_eq!(detect_secret_format(""), SecretFormat::Raw);
        assert_eq!(detect_secret_format("   \n\t\n"), SecretFormat::Raw);
        assert_eq!(
            detect_secret_format("# Just a comment\n; Another comment\n"),
            SecretFormat::Raw
        );

        assert_eq!(
            detect_secret_format("tskey-auth-k123456789\n"),
            SecretFormat::Raw
        );

        let toml = r#"[[users]]
name = "postgres"
password = "secretpassword"
"#;
        assert_eq!(detect_secret_format(toml), SecretFormat::Raw);

        let yaml = "server:\n  port: 8080\n";
        assert_eq!(detect_secret_format(yaml), SecretFormat::Raw);

        let json = "{\n  \"key\": \"value\"\n}\n";
        assert_eq!(detect_secret_format(json), SecretFormat::Raw);

        let pem = "-----BEGIN PRIVATE KEY-----\nMIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQg...\n-----END PRIVATE KEY-----\n";
        assert_eq!(detect_secret_format(pem), SecretFormat::Raw);

        let mixed = "FOO=bar\nTHIS IS INVALID\nBAZ=qux\n";
        assert_eq!(detect_secret_format(mixed), SecretFormat::Raw);
    }

    #[test]
    fn test_validate_key_value_valid() {
        let content = r#"
# Comment line
; Semicolon comment
   # Indented hash comment
   ; Indented semicolon comment
FOO=bar
BAR="quoted value"
BAZ='single quoted'
EMPTY=
EMPTY_QUOTED=""
EMPTY_SINGLE_QUOTED=''
SPACED = "with spaces"
TABBED	=	"value"
LEADING_UNDERSCORE_KEY=1
_ANOTHER_KEY=2
DB_PORT_5432=5432
BASE64_KEY=c29tZXNlY3JldA==
URL=https://example.com/api?foo=1&bar=2
COMPLEX_PASS=P@$$w0rd!#%^&*()
JSON_IN_SINGLE_QUOTES='{"user": "alice", "active": true}'
SINGLE_QUOTE_IN_DOUBLE="it's fine"
DOUBLE_QUOTE_IN_SINGLE='say "hello"'
SPACES_PRESERVED="  hello world  "
"#;
        assert!(validate_key_value(content).is_ok());
    }

    #[test]
    fn test_validate_key_value_invalid_syntax() {
        let res = validate_key_value("FOO_BAR\n");
        assert!(matches!(
            res,
            Err(RockyError::InvalidSecretSyntax { line: 1, .. })
        ));

        let res = validate_key_value("=value\n");
        assert!(matches!(
            res,
            Err(RockyError::InvalidSecretSyntax { line: 1, .. })
        ));

        let res = validate_key_value("123INVALID=value\n");
        assert!(matches!(
            res,
            Err(RockyError::InvalidSecretSyntax { line: 1, .. })
        ));

        let res = validate_key_value("KEY-NAME=value\n");
        assert!(matches!(
            res,
            Err(RockyError::InvalidSecretSyntax { line: 1, .. })
        ));

        let res = validate_key_value("KEY.NAME=value\n");
        assert!(matches!(
            res,
            Err(RockyError::InvalidSecretSyntax { line: 1, .. })
        ));

        let res = validate_key_value("KEY NAME=value\n");
        assert!(matches!(
            res,
            Err(RockyError::InvalidSecretSyntax { line: 1, .. })
        ));

        let res = validate_key_value("KEY=\"unclosed\n");
        assert!(matches!(
            res,
            Err(RockyError::InvalidSecretSyntax { line: 1, .. })
        ));

        let res = validate_key_value("KEY='unclosed\n");
        assert!(matches!(
            res,
            Err(RockyError::InvalidSecretSyntax { line: 1, .. })
        ));

        let res = validate_key_value("KEY=\"\n");
        assert!(matches!(
            res,
            Err(RockyError::InvalidSecretSyntax { line: 1, .. })
        ));

        let res = validate_key_value("KEY='\n");
        assert!(matches!(
            res,
            Err(RockyError::InvalidSecretSyntax { line: 1, .. })
        ));

        let res = validate_key_value("KEY=\"hello'\n");
        assert!(matches!(
            res,
            Err(RockyError::InvalidSecretSyntax { line: 1, .. })
        ));

        let res = validate_key_value("KEY=\"hello\"extra\n");
        assert!(matches!(
            res,
            Err(RockyError::InvalidSecretSyntax { line: 1, .. })
        ));

        let multiline = "FOO=1\nBAR=2\nINVALID_SYNTAX\nBAZ=3\n";
        let res = validate_key_value(multiline);
        match res {
            Err(RockyError::InvalidSecretSyntax { line, line_content }) => {
                assert_eq!(line, 3);
                assert_eq!(line_content, "INVALID_SYNTAX");
            }
            other => panic!("expected InvalidSecretSyntax on line 3, got {other:?}"),
        }
    }

    #[test]
    fn test_extract_key() {
        let content = r#"
# Initial configuration
JWT_SECRET="super-secret-token"
ANON_KEY=anon-token-123
EMPTY=
EMPTY_QUOTED=""
SPACED="  keep spaces  "
JSON_PAYLOAD='{"api": "v1"}'
BASE64=aGVsbG8gd29ybGQ==
PORT=3000
PORT=4000
# PORT=5000
"#;
        assert_eq!(
            extract_key(content, "JWT_SECRET").unwrap(),
            Some("super-secret-token".to_string())
        );
        assert_eq!(
            extract_key(content, "ANON_KEY").unwrap(),
            Some("anon-token-123".to_string())
        );

        assert_eq!(extract_key(content, "NON_EXISTENT").unwrap(), None);
        assert_eq!(extract_key(content, "ANON").unwrap(), None);

        assert_eq!(extract_key(content, "EMPTY").unwrap(), Some("".to_string()));
        assert_eq!(
            extract_key(content, "EMPTY_QUOTED").unwrap(),
            Some("".to_string())
        );

        assert_eq!(
            extract_key(content, "SPACED").unwrap(),
            Some("  keep spaces  ".to_string())
        );

        assert_eq!(
            extract_key(content, "JSON_PAYLOAD").unwrap(),
            Some(r#"{"api": "v1"}"#.to_string())
        );

        assert_eq!(
            extract_key(content, "BASE64").unwrap(),
            Some("aGVsbG8gd29ybGQ==".to_string())
        );

        assert_eq!(
            extract_key(content, "PORT").unwrap(),
            Some("4000".to_string())
        );

        let invalid = "PORT=3000\nNO_EQUALS\n";
        assert!(extract_key(invalid, "PORT").is_err());
    }

    #[test]
    fn test_parse_key_values() {
        let content = r#"
FOO=bar
BAZ="qux"
"#;
        let pairs = parse_key_values(content).unwrap();
        assert_eq!(
            pairs,
            vec![
                ("FOO".to_string(), "bar".to_string()),
                ("BAZ".to_string(), "qux".to_string())
            ]
        );
    }
}
