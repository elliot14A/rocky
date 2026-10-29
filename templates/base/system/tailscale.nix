{
  config,
  environment,
  fleet ? { },
  lib ? {
    mkIf = cond: val: if cond then val else { };
  },
  ...
}:

let
  loginServer = fleet.settings.tailscaleLoginServer or null;
  loginFlags = if loginServer != null then [ "--login-server=${loginServer}" ] else [ ];
  authKeyPath = ../environments + "/${environment}/secrets/tailscale-authkey.age";
  hasAuthKey = builtins.pathExists authKeyPath;
in
{
  age.secrets = lib.mkIf hasAuthKey {
    "tailscale-authkey".file = authKeyPath;
  };

  services.tailscale = {
    enable = true;
    authKeyFile = lib.mkIf hasAuthKey config.age.secrets."tailscale-authkey".path;
    extraUpFlags = loginFlags ++ [ "--ssh" ];
  };

  services.resolved.enable = true;
}
