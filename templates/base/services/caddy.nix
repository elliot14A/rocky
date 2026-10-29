{
  fleet,
  ...
}:

let
  acmeEmail = fleet.settings.acmeEmail or null;
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
