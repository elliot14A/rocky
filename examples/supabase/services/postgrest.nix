{
  config,
  pkgs,
  serviceHost,
  replicateService,
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

  serviceEnv = envVars.postgrest or { };
in
{
  age.secrets."jwt" = {
    file = secretsDir + "/jwt.age";
  };
  age.secrets."db-credentials" = {
    file = secretsDir + "/db-credentials.age";
  };

  systemd.services = replicateService {
    service = "postgrest";
    basePort = 3000;
    inherit envFiles secretHashes;
    mkUnit =
      {
        index,
        port,
        name,
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
