{
  config,
  pkgs,
  lib,
  environment,
  ...
}:

let
  secretsDir = ../environments + "/${environment}/secrets";

  configFile = pkgs.writeText "pgdog.toml" ''
    [general]
    port = 6432
    default_pool_size = 50
    checkout_timeout = 5000

    [[databases]]
    name = "postgres"
    host = "127.0.0.1"
    port = 5432
    role = "primary"
  '';
in
{
  assertions = [
    {
      assertion = config.services.postgresql.enable;
      message = "pgdog must run on the host that runs postgres";
    }
  ];

  age.secrets."pgdog-users" = {
    file = secretsDir + "/pgdog-users.age";
    mode = "0400";
  };

  systemd.services.pgdog = {
    description = "PgDog PostgreSQL connection pooler and router";
    wantedBy = [ "multi-user.target" ];
    after = [
      "network.target"
      "postgresql.service"
    ];
    requires = [ "postgresql.service" ];
    restartTriggers = [
      (builtins.hashFile "sha256" (secretsDir + "/pgdog-users.age"))
    ];
    serviceConfig = {
      ExecStart = "${pkgs.pgdog}/bin/pgdog --config ${configFile} --users ${config.age.secrets."pgdog-users".path} run";
      Restart = "on-failure";
    };
  };
}
