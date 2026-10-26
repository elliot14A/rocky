pub mod deploy;
pub mod exec;
pub mod install;
pub mod log;
pub mod mesh;
pub mod ssh;

pub use deploy::{
    build_deploy_args, deploy_fleet, deploy_host, deploy_single, DeployResult, HostDeploy,
};
pub use exec::exec;
pub use install::{build_install_args, install_fleet, install_host, HostInstall, InstallResult};
pub use log::{host_log_path, log_path, open_log_pair, tail_log};
pub use mesh::{
    check_workstation_tailscale, get_fleet_mesh_status, mesh_join, mesh_leave, resolve_domain,
    resolve_login_server, resolve_or_create_authkey, FleetHostStatus, FleetMeshStatus,
};
pub use ssh::{build_ssh_args, execute_ssh, parse_target};
