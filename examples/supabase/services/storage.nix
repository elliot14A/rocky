{
  config,
  pkgs,
  serviceHost,
  replicateContainers,
  containerRestartTriggers,
  envVars,
  environment,
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

  serviceEnv = envVars.storage or { };
in
{
  age.secrets."jwt" = {
    file = secretsDir + "/jwt.age";
  };
  age.secrets."db-credentials" = {
    file = secretsDir + "/db-credentials.age";
  };

  virtualisation.oci-containers.containers = replicateContainers {
    service = "storage";
    basePort = 5000;
    containerPort = 5000;
    inherit envFiles;
    mkContainer =
      {
        index,
        port,
        name,
      }:
      {
        image = "docker.io/supabase/storage-api:v1.11.0";
        environment = serviceEnv // {
          POSTGREST_URL = "http://${serviceHost "postgrest"}:3000";
          PORT = "5000";
        };
        volumes = [
          "/var/lib/supabase-storage:/var/lib/storage"
        ];
      };
  };

  systemd.services = containerRestartTriggers {
    service = "storage";
    inherit secretHashes;
  };
}
