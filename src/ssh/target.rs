use std::collections::HashMap;

use snafu::{ensure, OptionExt};

use crate::error::{Kind, Result, TargetParseSnafu};
use crate::utils::validate_name;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub host: String,
    pub address: String,
    pub port: u16,
}

impl Target {
    pub fn address_with_port(&self) -> String {
        if self.port == 22 {
            self.address.clone()
        } else if self.address.contains(':') && !self.address.starts_with('[') {
            format!("[{}]:{}", self.address, self.port)
        } else {
            format!("{}:{}", self.address, self.port)
        }
    }
}

pub fn parse_target(raw: &str) -> Result<Target> {
    let raw = raw.trim();
    let (host_str, addr_str) = raw.split_once('=').context(TargetParseSnafu {
        raw,
        reason: "expected format HOST=IP[:PORT]",
    })?;

    let host_str = host_str.trim();
    let addr_str = addr_str.trim();

    validate_name(Kind::Host, host_str)?;

    ensure!(
        !addr_str.is_empty(),
        TargetParseSnafu {
            raw,
            reason: "address cannot be empty",
        }
    );

    let (address, port) = if addr_str.starts_with('[') {
        let close = addr_str.find(']').context(TargetParseSnafu {
            raw,
            reason: "unclosed IPv6 bracket",
        })?;
        let addr = &addr_str[1..close];
        let rest = &addr_str[close + 1..];
        let port = if rest.is_empty() {
            22
        } else if let Some(port_str) = rest.strip_prefix(':') {
            port_str.parse::<u16>().map_err(|e| {
                TargetParseSnafu {
                    raw,
                    reason: format!("invalid port: {e}"),
                }
                .build()
            })?
        } else {
            return TargetParseSnafu {
                raw,
                reason: "unexpected characters after IPv6 bracket",
            }
            .fail();
        };
        (addr.to_string(), port)
    } else if let Some((addr, port_str)) = addr_str.rsplit_once(':') {
        if addr_str.matches(':').count() > 1 {
            (addr_str.to_string(), 22)
        } else {
            let port = port_str.parse::<u16>().map_err(|e| {
                TargetParseSnafu {
                    raw,
                    reason: format!("invalid port: {e}"),
                }
                .build()
            })?;
            (addr.to_string(), port)
        }
    } else {
        (addr_str.to_string(), 22)
    };

    ensure!(
        !address.is_empty(),
        TargetParseSnafu {
            raw,
            reason: "address cannot be empty",
        }
    );

    ensure!(
        !address.contains(char::is_whitespace),
        TargetParseSnafu {
            raw,
            reason: "address cannot contain whitespace",
        }
    );

    ensure!(
        !address.contains('/'),
        TargetParseSnafu {
            raw,
            reason: "address cannot contain slashes",
        }
    );

    ensure!(
        !address.starts_with('-') && !address.starts_with('.'),
        TargetParseSnafu {
            raw,
            reason: "address cannot start with a hyphen or dot",
        }
    );

    ensure!(
        address
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == ':'),
        TargetParseSnafu {
            raw,
            reason: "address contains invalid characters",
        }
    );

    ensure!(
        port > 0,
        TargetParseSnafu {
            raw,
            reason: "port must be greater than 0",
        }
    );

    Ok(Target {
        host: host_str.to_string(),
        address,
        port,
    })
}

pub fn parse_targets<I, S>(targets: I) -> Result<HashMap<String, Target>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut map = HashMap::new();
    for raw in targets {
        let raw_str = raw.as_ref();
        let target = parse_target(raw_str)?;
        ensure!(
            !map.contains_key(&target.host),
            TargetParseSnafu {
                raw: raw_str.to_string(),
                reason: format!("duplicate target specified for host '{}'", target.host),
            }
        );
        map.insert(target.host.clone(), target);
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_target_default_port() {
        let target = parse_target("node-01=192.168.1.50").unwrap();
        assert_eq!(
            target,
            Target {
                host: "node-01".to_string(),
                address: "192.168.1.50".to_string(),
                port: 22,
            }
        );
    }

    #[test]
    fn test_parse_target_custom_port() {
        let target = parse_target("node-01=127.0.0.1:2222").unwrap();
        assert_eq!(
            target,
            Target {
                host: "node-01".to_string(),
                address: "127.0.0.1".to_string(),
                port: 2222,
            }
        );
    }

    #[test]
    fn test_parse_target_ipv6_bracketed_with_port() {
        let target = parse_target("node-01=[2001:db8::1]:8022").unwrap();
        assert_eq!(
            target,
            Target {
                host: "node-01".to_string(),
                address: "2001:db8::1".to_string(),
                port: 8022,
            }
        );
    }

    #[test]
    fn test_parse_target_ipv6_bracketed_default_port() {
        let target = parse_target("node-01=[2001:db8::1]").unwrap();
        assert_eq!(
            target,
            Target {
                host: "node-01".to_string(),
                address: "2001:db8::1".to_string(),
                port: 22,
            }
        );
    }

    #[test]
    fn test_parse_target_ipv6_unbracketed_default_port() {
        let target = parse_target("node-01=2001:db8::1").unwrap();
        assert_eq!(
            target,
            Target {
                host: "node-01".to_string(),
                address: "2001:db8::1".to_string(),
                port: 22,
            }
        );
    }

    #[test]
    fn test_parse_target_rejects_missing_equals() {
        assert!(parse_target("node-01").is_err());
    }

    #[test]
    fn test_parse_target_rejects_empty_address() {
        assert!(parse_target("node-01=").is_err());
    }

    #[test]
    fn test_parse_target_rejects_invalid_host_name() {
        assert!(parse_target("../evil=1.2.3.4").is_err());
        assert!(parse_target("-bad=1.2.3.4").is_err());
        assert!(parse_target("name with spaces=1.2.3.4").is_err());
        assert!(parse_target("path/to/thing=1.2.3.4").is_err());
    }

    #[test]
    fn test_parse_target_rejects_invalid_port() {
        assert!(parse_target("node-01=1.2.3.4:0").is_err());
        assert!(parse_target("node-01=1.2.3.4:99999").is_err());
        assert!(parse_target("node-01=1.2.3.4:abc").is_err());
    }

    #[test]
    fn test_parse_target_rejects_slashes_and_whitespace() {
        assert!(parse_target("node-01=1.2.3.4/24").is_err());
        assert!(parse_target("node-01=1.2.3.4 foo").is_err());
    }

    #[test]
    fn test_parse_target_rejects_option_injection_and_invalid_chars() {
        assert!(parse_target("node-01=-oProxyCommand=calc.exe").is_err());
        assert!(parse_target("node-01=--flag").is_err());
        assert!(parse_target("node-01=.hidden.host").is_err());
        assert!(parse_target("node-01=1.2.3.4;rm -rf /").is_err());
        assert!(parse_target("node-01=1.2.3.4&echo hi").is_err());
        assert!(parse_target("node-01=1.2.3.4$FOO").is_err());
    }

    #[test]
    fn test_parse_targets_collection() {
        let targets = vec!["node-01=10.0.0.1", "node-02=10.0.0.2:2222"];
        let map = parse_targets(targets).unwrap();
        assert_eq!(map.len(), 2);
        assert_eq!(map["node-01"].port, 22);
        assert_eq!(map["node-02"].port, 2222);
    }

    #[test]
    fn test_parse_targets_rejects_duplicate() {
        let targets = vec!["node-01=10.0.0.1", "node-01=10.0.0.2"];
        let res = parse_targets(targets);
        assert!(res.is_err());
        assert!(matches!(
            res.unwrap_err(),
            crate::error::RockyError::TargetParse { .. }
        ));
    }

    #[test]
    fn test_target_address_with_port() {
        let t1 = Target {
            host: "n1".into(),
            address: "192.168.1.50".into(),
            port: 22,
        };
        assert_eq!(t1.address_with_port(), "192.168.1.50");

        let t2 = Target {
            host: "n2".into(),
            address: "127.0.0.1".into(),
            port: 2222,
        };
        assert_eq!(t2.address_with_port(), "127.0.0.1:2222");

        let t3 = Target {
            host: "n3".into(),
            address: "2001:db8::1".into(),
            port: 22,
        };
        assert_eq!(t3.address_with_port(), "2001:db8::1");

        let t4 = Target {
            host: "n4".into(),
            address: "2001:db8::1".into(),
            port: 8022,
        };
        assert_eq!(t4.address_with_port(), "[2001:db8::1]:8022");
    }
}
