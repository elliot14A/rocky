{
  config,
  replicateContainers,
  replicateServiceLoadBalancer,
  containerRestartTriggers,
  envVars,
  environment,
  fleet,
  ...
}:

let
  secretsDir = ../environments + "/${environment}/secrets";
  secretNames = [
    "jwt"
    "postgres-creds"
  ];
  existingSecretNames = builtins.filter (n: builtins.pathExists (secretsDir + "/${n}.age")) secretNames;
  envFiles = map (n: config.age.secrets.${n}.path) existingSecretNames;
  secretHashes = map (n: builtins.hashFile "sha256" (secretsDir + "/${n}.age")) existingSecretNames;

  domain = fleet.settings.domain;
  serviceEnv = envVars.gotrue or { };
in
{
  networking.nftables = replicateServiceLoadBalancer {
    service = "gotrue";
    basePort = 9999;
  };

  age.secrets = builtins.listToAttrs (
    map (n: {
      name = n;
      value = {
        file = secretsDir + "/${n}.age";
      };
    }) existingSecretNames
  );

  virtualisation.oci-containers.containers = replicateContainers {
    service = "gotrue";
    basePort = 9999;
    containerPort = 9999;
    inherit envFiles;
    mkContainer =
      {
        port,
        ...
      }:
      {
        image = "docker.io/supabase/gotrue:v2.158.1";
        environment = serviceEnv // {
          GOTRUE_API_HOST = "0.0.0.0";
          PORT = toString port;
          API_EXTERNAL_URL = "http://${domain}/auth/v1";
          GOTRUE_SITE_URL = "http://${domain}";
          GOTRUE_DB_DRIVER = "postgres";
        };
      };
  };

  systemd.services = containerRestartTriggers {
    service = "gotrue";
    inherit secretHashes;
  };
}
