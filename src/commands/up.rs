use std::collections::HashMap;
use std::path::PathBuf;

use clap::Args;
use snafu::{ensure, OptionExt, ResultExt};

use crate::error::{Kind, Result, TargetParseSnafu};
use crate::utils::validate_name;

#[derive(Args, Debug, Clone)]
pub struct UpArgs {
    pub env: String,

    #[arg(long = "target", value_name = "HOST=IP[:PORT]")]
    pub targets: Vec<String>,

    #[arg(short, long)]
    pub identity: Option<PathBuf>,

    #[arg(short, long)]
    pub yes: bool,

    #[arg(short = 'j', long, default_value_t = 4)]
    pub concurrency: usize,
}

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

pub fn resolve_targets<R: std::io::BufRead, W: std::io::Write>(
    pending_hosts: &[String],
    targets: &mut HashMap<String, Target>,
    mut reader: R,
    mut writer: W,
    is_terminal: bool,
) -> Result<()> {
    for host in pending_hosts {
        if targets.contains_key(host) {
            continue;
        }

        ensure!(
            is_terminal,
            crate::error::MissingTargetIpSnafu { host: host.clone() }
        );

        writeln!(writer, "Host '{host}' requires installation.").context(
            crate::error::IoSnafu {
                path: PathBuf::from("stdout"),
            },
        )?;
        write!(writer, "Target IP: ").context(crate::error::IoSnafu {
            path: PathBuf::from("stdout"),
        })?;
        writer.flush().context(crate::error::IoSnafu {
            path: PathBuf::from("stdout"),
        })?;

        let mut line = String::new();
        reader.read_line(&mut line).context(crate::error::IoSnafu {
            path: PathBuf::from("stdin"),
        })?;
        let input = line.trim();
        ensure!(
            !input.is_empty(),
            crate::error::MissingTargetIpSnafu { host: host.clone() }
        );

        let raw = if input.contains('=') {
            input.to_string()
        } else {
            format!("{host}={input}")
        };
        let target = parse_target(&raw)?;
        ensure!(
            target.host == *host,
            crate::error::TargetParseSnafu {
                raw: input.to_string(),
                reason: format!(
                    "target host '{}' does not match expected host '{host}'",
                    target.host
                ),
            }
        );
        targets.insert(host.clone(), target);
    }
    Ok(())
}

pub fn prompt_targets(
    pending_hosts: &[String],
    targets: &mut HashMap<String, Target>,
) -> Result<()> {
    use std::io::IsTerminal;
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let is_terminal = stdin.is_terminal();
    resolve_targets(
        pending_hosts,
        targets,
        stdin.lock(),
        stdout.lock(),
        is_terminal,
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallItem {
    pub host: String,
    pub target: Target,
    pub services: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeployItem {
    pub host: String,
    pub services: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetPlan {
    pub installs: Vec<InstallItem>,
    pub deploys: Vec<DeployItem>,
    pub removed: Vec<String>,
}

impl FleetPlan {
    pub fn is_empty(&self) -> bool {
        self.installs.is_empty() && self.deploys.is_empty() && self.removed.is_empty()
    }
}

pub fn build_plan(
    diff: &crate::nix::HostDiff,
    targets: &HashMap<String, Target>,
    services_map: &HashMap<String, Vec<String>>,
) -> Result<FleetPlan> {
    let mut installs = Vec::new();
    for host in &diff.pending_install {
        let target = targets
            .get(host)
            .cloned()
            .context(crate::error::MissingTargetIpSnafu { host: host.clone() })?;
        let services = services_map.get(host).cloned().unwrap_or_default();
        installs.push(InstallItem {
            host: host.clone(),
            target,
            services,
        });
    }

    let mut deploys = Vec::new();
    for host in &diff.installed {
        let services = services_map.get(host).cloned().unwrap_or_default();
        deploys.push(DeployItem {
            host: host.clone(),
            services,
        });
    }

    let removed = diff.removed.clone();
    Ok(FleetPlan {
        installs,
        deploys,
        removed,
    })
}

pub fn format_plan(plan: &FleetPlan) -> String {
    let mut sections = Vec::new();

    if !plan.installs.is_empty() {
        let mut s = String::from("Install:\n");
        for item in &plan.installs {
            if item.services.is_empty() {
                s.push_str(&format!(
                    "  {} ({})\n",
                    item.host,
                    item.target.address_with_port()
                ));
            } else {
                s.push_str(&format!(
                    "  {} ({}) [{}]\n",
                    item.host,
                    item.target.address_with_port(),
                    item.services.join(", ")
                ));
            }
        }
        sections.push(s.trim_end().to_string());
    }

    if !plan.deploys.is_empty() {
        let mut s = String::from("Deploy:\n");
        for item in &plan.deploys {
            if item.services.is_empty() {
                s.push_str(&format!("  {}\n", item.host));
            } else {
                s.push_str(&format!("  {} [{}]\n", item.host, item.services.join(", ")));
            }
        }
        sections.push(s.trim_end().to_string());
    }

    if !plan.removed.is_empty() {
        let mut s = String::from("Removed:\n");
        for host in &plan.removed {
            s.push_str(&format!("  {}\n", host));
        }
        sections.push(s.trim_end().to_string());
    }

    sections.join("\n\n")
}

pub fn confirm_plan<R: std::io::BufRead, W: std::io::Write>(
    plan: &str,
    yes: bool,
    mut reader: R,
    mut writer: W,
) -> Result<bool> {
    if plan.is_empty() {
        return Ok(true);
    }

    writeln!(writer, "{plan}").context(crate::error::IoSnafu {
        path: PathBuf::from("stdout"),
    })?;
    writeln!(writer).context(crate::error::IoSnafu {
        path: PathBuf::from("stdout"),
    })?;

    if yes {
        return Ok(true);
    }

    write!(writer, "Apply? [y/N]: ").context(crate::error::IoSnafu {
        path: PathBuf::from("stdout"),
    })?;
    writer.flush().context(crate::error::IoSnafu {
        path: PathBuf::from("stdout"),
    })?;

    let mut line = String::new();
    reader.read_line(&mut line).context(crate::error::IoSnafu {
        path: PathBuf::from("stdin"),
    })?;

    let answer = line.trim().to_lowercase();
    Ok(answer == "y" || answer == "yes")
}

pub fn prompt_confirmation(plan: &str, yes: bool) -> Result<bool> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    confirm_plan(plan, yes, stdin.lock(), stdout.lock())
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

    #[test]
    fn test_resolve_targets_already_provided() {
        let pending = vec!["node-01".to_string()];
        let mut targets = HashMap::new();
        targets.insert(
            "node-01".to_string(),
            Target {
                host: "node-01".into(),
                address: "10.0.0.1".into(),
                port: 22,
            },
        );

        let input = b"";
        let mut output = Vec::new();
        let res = resolve_targets(&pending, &mut targets, &input[..], &mut output, false);
        assert!(res.is_ok());
        assert!(output.is_empty());
    }

    #[test]
    fn test_resolve_targets_interactive_ip_and_port() {
        let pending = vec!["node-01".to_string()];
        let mut targets = HashMap::new();

        let input = b"127.0.0.1:2222\n";
        let mut output = Vec::new();
        let res = resolve_targets(&pending, &mut targets, &input[..], &mut output, true);
        assert!(res.is_ok());

        assert_eq!(
            targets["node-01"],
            Target {
                host: "node-01".into(),
                address: "127.0.0.1".into(),
                port: 2222,
            }
        );
        let out_str = String::from_utf8(output).unwrap();
        assert!(out_str.contains("Host 'node-01' requires installation."));
        assert!(out_str.contains("Target IP: "));
    }

    #[test]
    fn test_resolve_targets_interactive_host_equals_ip() {
        let pending = vec!["node-01".to_string()];
        let mut targets = HashMap::new();

        let input = b"node-01=10.0.0.1\n";
        let mut output = Vec::new();
        let res = resolve_targets(&pending, &mut targets, &input[..], &mut output, true);
        assert!(res.is_ok());
        assert_eq!(targets["node-01"].address, "10.0.0.1");
        assert_eq!(targets["node-01"].port, 22);
    }

    #[test]
    fn test_resolve_targets_non_interactive_missing_fails() {
        let pending = vec!["node-01".to_string()];
        let mut targets = HashMap::new();

        let input = b"";
        let mut output = Vec::new();
        let res = resolve_targets(&pending, &mut targets, &input[..], &mut output, false);
        assert!(res.is_err());
        assert!(matches!(
            res.unwrap_err(),
            crate::error::RockyError::MissingTargetIp { .. }
        ));
    }

    #[test]
    fn test_resolve_targets_empty_input_fails() {
        let pending = vec!["node-01".to_string()];
        let mut targets = HashMap::new();

        let input = b"   \n";
        let mut output = Vec::new();
        let res = resolve_targets(&pending, &mut targets, &input[..], &mut output, true);
        assert!(res.is_err());
        assert!(matches!(
            res.unwrap_err(),
            crate::error::RockyError::MissingTargetIp { .. }
        ));
    }

    #[test]
    fn test_resolve_targets_mismatched_host_fails() {
        let pending = vec!["node-01".to_string()];
        let mut targets = HashMap::new();

        let input = b"wrong-node=10.0.0.1\n";
        let mut output = Vec::new();
        let res = resolve_targets(&pending, &mut targets, &input[..], &mut output, true);
        assert!(res.is_err());
        assert!(matches!(
            res.unwrap_err(),
            crate::error::RockyError::TargetParse { .. }
        ));
    }

    #[test]
    fn test_format_plan_formatting() {
        let plan = FleetPlan {
            installs: vec![
                InstallItem {
                    host: "node-01".to_string(),
                    target: Target {
                        host: "node-01".into(),
                        address: "127.0.0.1".into(),
                        port: 2222,
                    },
                    services: vec!["caddy".into(), "api".into()],
                },
                InstallItem {
                    host: "node-02".to_string(),
                    target: Target {
                        host: "node-02".into(),
                        address: "192.168.1.50".into(),
                        port: 22,
                    },
                    services: vec![],
                },
            ],
            deploys: vec![
                DeployItem {
                    host: "node-03".to_string(),
                    services: vec!["postgres".into()],
                },
                DeployItem {
                    host: "node-04".to_string(),
                    services: vec![],
                },
            ],
            removed: vec!["old-node".to_string()],
        };

        let formatted = format_plan(&plan);
        let expected = "\
Install:
  node-01 (127.0.0.1:2222) [caddy, api]
  node-02 (192.168.1.50)

Deploy:
  node-03 [postgres]
  node-04

Removed:
  old-node";

        assert_eq!(formatted, expected);
    }

    #[test]
    fn test_confirm_plan_yes_flag() {
        let plan = "Deploy:\n  node-01 [api]";
        let input = b"";
        let mut output = Vec::new();
        let confirmed = confirm_plan(plan, true, &input[..], &mut output).unwrap();
        assert!(confirmed);
        let out_str = String::from_utf8(output).unwrap();
        assert!(out_str.contains("Deploy:\n  node-01 [api]"));
        assert!(!out_str.contains("Apply?"));
    }

    #[test]
    fn test_confirm_plan_interactive_yes() {
        let plan = "Deploy:\n  node-01 [api]";
        let input = b"y\n";
        let mut output = Vec::new();
        let confirmed = confirm_plan(plan, false, &input[..], &mut output).unwrap();
        assert!(confirmed);
        let out_str = String::from_utf8(output).unwrap();
        assert!(out_str.contains("Apply? [y/N]: "));
    }

    #[test]
    fn test_confirm_plan_interactive_no() {
        let plan = "Deploy:\n  node-01 [api]";
        let input = b"n\n";
        let mut output = Vec::new();
        let confirmed = confirm_plan(plan, false, &input[..], &mut output).unwrap();
        assert!(!confirmed);
    }

    #[test]
    fn test_build_plan_success_and_missing() {
        let diff = crate::nix::HostDiff {
            declared: vec!["n1".into(), "n2".into()],
            pending_install: vec!["n1".into()],
            installed: vec!["n2".into()],
            removed: vec!["n3".into()],
        };

        let mut targets = HashMap::new();
        targets.insert(
            "n1".into(),
            Target {
                host: "n1".into(),
                address: "10.0.0.1".into(),
                port: 22,
            },
        );

        let mut services = HashMap::new();
        services.insert("n1".into(), vec!["caddy".into()]);
        services.insert("n2".into(), vec!["db".into()]);

        let plan = build_plan(&diff, &targets, &services).unwrap();

        assert_eq!(plan.installs.len(), 1);
        assert_eq!(plan.installs[0].host, "n1");
        assert_eq!(plan.installs[0].services, vec!["caddy"]);

        assert_eq!(plan.deploys.len(), 1);
        assert_eq!(plan.deploys[0].host, "n2");
        assert_eq!(plan.deploys[0].services, vec!["db"]);

        assert_eq!(plan.removed, vec!["n3"]);

        // When pending install has no target
        let empty_targets = HashMap::new();
        assert!(build_plan(&diff, &empty_targets, &services).is_err());
    }
}
