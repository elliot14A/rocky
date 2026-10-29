{
  config,
  pkgs,
  replicateService,
  replicateServiceLoadBalancer,
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

  serviceEnv = envVars.postgrest or { };
in
{
  networking.nftables = replicateServiceLoadBalancer {
    service = "postgrest";
    basePort = 3000;
  };

  age.secrets = builtins.listToAttrs (
    map (n: {
      name = n;
      value = {
        file = secretsDir + "/${n}.age";
      };
    }) existingSecretNames
  );

  systemd.services = replicateService {
    service = "postgrest";
    basePort = 3000;
    inherit envFiles secretHashes;
    mkUnit =
      {
        index,
        port,
        ...
      }:
      {
        description = "PostgREST API server replica ${toString index}";
        wantedBy = [ "multi-user.target" ];
        after = [ "network.target" ];
        serviceConfig = {
          ExecStart = "${pkgs.postgrest}/bin/postgrest";
          Restart = "on-failure";
        };
        environment = serviceEnv // {
          PGRST_SERVER_PORT = toString port;
        };
      };
  };
}
