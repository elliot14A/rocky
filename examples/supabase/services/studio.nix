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
  ];
  envFiles = map (n: config.age.secrets.${n}.path) secretNames;
  secretHashes = map (n: builtins.hashFile "sha256" (secretsDir + "/${n}.age")) secretNames;

  domain = fleet.settings.domain or "prod.rocky.internal";
  serviceEnv = envVars.studio or { };
in
{
  age.secrets."jwt" = {
    file = secretsDir + "/jwt.age";
  };

  virtualisation.oci-containers.containers = replicateContainers {
    service = "studio";
    basePort = 3000;
    containerPort = 3000;
    inherit envFiles;
    mkContainer =
      {
        index,
        port,
        name,
      }:
      {
        image = "docker.io/supabase/studio:20240729-ef413b3";
        environment = serviceEnv // {
          STUDIO_PORT = "3000";
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
