{
  config,
  pkgs,
  serviceHost,
  replicateContainers,
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
    "db-credentials"
  ];
  envFiles = map (n: config.age.secrets.${n}.path) secretNames;
  secretHashes = map (n: builtins.hashFile "sha256" (secretsDir + "/${n}.age")) secretNames;

  domain = fleet.settings.domain or "prod.rocky.internal";
  serviceEnv = envVars.gotrue or { };
in
{
  age.secrets."jwt" = {
    file = secretsDir + "/jwt.age";
  };
  age.secrets."db-credentials" = {
    file = secretsDir + "/db-credentials.age";
  };

  virtualisation.oci-containers.containers = replicateContainers {
    service = "gotrue";
    basePort = 9999;
    containerPort = 9999;
    inherit envFiles;
    mkContainer =
      {
        index,
        port,
        name,
      }:
      {
        image = "docker.io/supabase/gotrue:v2.158.1";
        environment = serviceEnv // {
          GOTRUE_API_HOST = "0.0.0.0";
          PORT = "9999";
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
