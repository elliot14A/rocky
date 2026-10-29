{
  config,
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
  ];
  existingSecretNames = builtins.filter (n: builtins.pathExists (secretsDir + "/${n}.age")) secretNames;
  envFiles = map (n: config.age.secrets.${n}.path) existingSecretNames;
  secretHashes = map (n: builtins.hashFile "sha256" (secretsDir + "/${n}.age")) existingSecretNames;

  domain = fleet.settings.domain;
  serviceEnv = envVars.studio or { };
  metaHost = serviceHost "meta";
in
{
  age.secrets = builtins.listToAttrs (
    map (n: {
      name = n;
      value = {
        file = secretsDir + "/${n}.age";
      };
    }) existingSecretNames
  );

  virtualisation.oci-containers.containers = replicateContainers {
    service = "studio";
    basePort = 3000;
    containerPort = 3000;
    inherit envFiles;
    mkContainer =
      { ... }:
      {
        image = "docker.io/supabase/studio:20241230-e6efee1";
        environment = serviceEnv // {
          HOSTNAME = "0.0.0.0";
          STUDIO_PORT = "3000";
          STUDIO_PG_META_URL = "http://${metaHost}:8080";
          SUPABASE_URL = "http://${domain}";
          SUPABASE_PUBLIC_URL = "http://${domain}";
          SUPABASE_REST_URL = "http://${domain}/rest/v1";
        };
      };
  };

  systemd.services = containerRestartTriggers {
    service = "studio";
    inherit secretHashes;
  };
}
