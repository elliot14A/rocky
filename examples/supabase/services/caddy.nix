{
  fleet,
  serviceHost,
  serviceUpstreams,
  lib,
  ...
}:

let
  domain = fleet.settings.domain or "prod.rocky.internal";
  acmeEmail = fleet.settings.acmeEmail or "admin@prod.rocky.internal";

  restUpstreams = serviceUpstreams {
    service = "postgrest";
    basePort = 3000;
  };
  authUpstreams = serviceUpstreams {
    service = "gotrue";
    basePort = 9999;
  };
  storageUpstreams = serviceUpstreams {
    service = "storage";
    basePort = 5000;
  };
  studioHost = serviceHost "studio";

  restList = lib.concatStringsSep " " restUpstreams;
  authList = lib.concatStringsSep " " authUpstreams;
  storageList = lib.concatStringsSep " " storageUpstreams;
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

    virtualHosts."http://${domain}, https://${domain}".extraConfig = ''
      handle /rest/v1/* {
        uri strip_prefix /rest/v1
        reverse_proxy ${restList} {
          lb_policy round_robin
        }
      }

      handle /auth/v1/* {
        reverse_proxy ${authList} {
          lb_policy round_robin
        }
      }

      handle /storage/v1/* {
        reverse_proxy ${storageList} {
          lb_policy round_robin
        }
      }

      handle {
        reverse_proxy http://${studioHost}:3000
      }
    '';
  };
}
