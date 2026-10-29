{
  fleet,
  serviceHost,
  ...
}:

let
  domain = fleet.settings.domain;
  acmeEmail = fleet.settings.acmeEmail or "admin@${domain}";

  postgrestHost = serviceHost "postgrest";
  gotrueHost = serviceHost "gotrue";
  storageHost = serviceHost "storage";
  studioHost = serviceHost "studio";
in
{
  networking.firewall.allowedTCPPorts = [
    80
    443
  ];

  networking.hosts."127.0.0.1" = [ domain ];

  services.caddy = {
    enable = true;
    email = acmeEmail;

    globalConfig = ''
      servers {
        protocols h1 h2 h2c
      }
      skip_install_trust
    '';

    virtualHosts."http://${domain}, https://${domain}".extraConfig = ''
      handle /rest/v1/* {
        uri strip_prefix /rest/v1
        reverse_proxy http://${postgrestHost}:3000
      }

      handle /auth/v1/* {
        uri strip_prefix /auth/v1
        reverse_proxy http://${gotrueHost}:9999
      }

      handle /storage/v1/* {
        uri strip_prefix /storage/v1
        reverse_proxy http://${storageHost}:5000
      }

      handle {
        reverse_proxy http://${studioHost}:3000
      }
    '';
  };
}
