{
  lib,
  fleet,
  environment,
  hostName,
  ...
}:

let
  acmeEmail =
    fleet.settings.acmeEmail or (throw "fleet settings.acmeEmail must be set for the caddy service");
in
{
  networking.firewall.allowedTCPPorts = [
    80
    443
  ];

  services.caddy = {
    enable = true;
    email = acmeEmail;

    globalConfig = ''
      servers {
        protocols h1 h2 h2c
      }
    '';
  };
}
