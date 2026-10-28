pub mod deploy;
pub mod exec;
pub mod install;
pub mod journal;
pub mod log;
pub mod mesh;
pub mod ssh;

pub use deploy::{
    build_deploy_args, deploy_fleet, deploy_host, deploy_single, DeployResult, HostDeploy,
};
pub use exec::exec;
pub use install::{build_install_args, install_fleet, install_host, HostInstall, InstallResult};
pub use journal::{
    build_journalctl_args, detect_service_kind, execute_logs, resolve_service_target,
    validate_service_exists, JournalOptions, ResolvedLogTarget, ResolvedService,
};
pub use log::{host_log_path, log_path, open_log_pair, tail_log};
pub use mesh::{
    check_workstation_tailscale, get_fleet_mesh_status, mesh_join, mesh_leave, resolve_authkey,
    resolve_domain, resolve_login_server, FleetHostStatus, FleetMeshStatus,
};
pub use ssh::{build_ssh_args, execute_ssh, parse_target};
