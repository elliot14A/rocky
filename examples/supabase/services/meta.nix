{
  config,
  serviceHost,
  replicateContainers,
  replicateServiceLoadBalancer,
  containerRestartTriggers,
  envVars,
  environment,
  ...
}:

let
  secretsDir = ../environments + "/${environment}/secrets";
  secretNames = [
    "postgres-creds"
  ];
  existingSecretNames = builtins.filter (n: builtins.pathExists (secretsDir + "/${n}.age")) secretNames;
  envFiles = map (n: config.age.secrets.${n}.path) existingSecretNames;
  secretHashes = map (n: builtins.hashFile "sha256" (secretsDir + "/${n}.age")) existingSecretNames;

  serviceEnv = envVars.meta or { };
  dbHost = serviceHost "postgres";
in
{
  networking.nftables = replicateServiceLoadBalancer {
    service = "meta";
    basePort = 8080;
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
    service = "meta";
    basePort = 8080;
    containerPort = 8080;
    inherit envFiles;
    mkContainer =
      {
        port,
        ...
      }:
      {
        image = "docker.io/supabase/postgres-meta:v0.84.2";
        environment = serviceEnv // {
          PG_META_PORT = toString port;
          PG_META_DB_HOST = dbHost;
        };
      };
  };

  systemd.services = containerRestartTriggers {
    service = "meta";
    inherit secretHashes;
  };
}
