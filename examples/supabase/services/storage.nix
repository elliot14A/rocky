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
    "jwt"
    "postgres-creds"
  ];
  existingSecretNames = builtins.filter (n: builtins.pathExists (secretsDir + "/${n}.age")) secretNames;
  envFiles = map (n: config.age.secrets.${n}.path) existingSecretNames;
  secretHashes = map (n: builtins.hashFile "sha256" (secretsDir + "/${n}.age")) existingSecretNames;

  serviceEnv = envVars.storage or { };
in
{
  networking.nftables = replicateServiceLoadBalancer {
    service = "storage";
    basePort = 5000;
  };

  age.secrets = builtins.listToAttrs (
    map (n: {
      name = n;
      value = {
        file = secretsDir + "/${n}.age";
      };
    }) existingSecretNames
  );

  systemd.tmpfiles.rules = [
    "d /var/lib/supabase-storage 0750 root root -"
  ];

  virtualisation.oci-containers.containers = replicateContainers {
    service = "storage";
    basePort = 5000;
    containerPort = 5000;
    inherit envFiles;
    mkContainer =
      {
        port,
        ...
      }:
      {
        image = "docker.io/supabase/storage-api:v1.11.0";
        environment = serviceEnv // {
          POSTGREST_URL = "http://${serviceHost "postgrest"}:3000";
          PORT = toString port;
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
