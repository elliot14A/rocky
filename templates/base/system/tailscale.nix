{
  config,
  environment,
  ...
}:

{
  age.secrets."tailscale-authkey".file =
    ../environments + "/${environment}/secrets/tailscale-authkey.age";

  services.tailscale = {
    enable = true;
    authKeyFile = config.age.secrets."tailscale-authkey".path;
  };

  services.resolved.enable = true;
}
