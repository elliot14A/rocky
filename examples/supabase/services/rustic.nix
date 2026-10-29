{
  config,
  pkgs,
  lib,
  serviceHost,
  environment,
  ...
}:

let
  secretsDir = ../environments + "/${environment}/secrets";
  hasRusticSecret = builtins.pathExists (secretsDir + "/rustic.age");
  storageEndpoint = "http://${serviceHost "storage"}:5000/storage/v1/s3";
  bucket = "backups";
  dumpDir = "/var/backup/supabase-postgres";
  pgDump = "${config.services.postgresql.package}/bin/pg_dump";
  rustic = "${pkgs.rustic}/bin/rustic";
in
{
  assertions = [
    {
      assertion = config.services.postgresql.enable;
      message = "rustic must run on the host that runs postgres";
    }
  ];

  age.secrets = lib.optionalAttrs hasRusticSecret {
    "rustic" = {
      file = secretsDir + "/rustic.age";
    };
  };

  systemd.services.rustic-backup = {
    description = "Automated PostgreSQL backup to internal Supabase Storage S3 via rustic";
    restartTriggers =
      if hasRusticSecret then [ (builtins.hashFile "sha256" (secretsDir + "/rustic.age")) ] else [ ];
    serviceConfig = {
      Type = "oneshot";
      EnvironmentFile =
        if hasRusticSecret then [ config.age.secrets."rustic".path ] else [ ];
    };
    script = ''
      set -eu

      repo_opts=(
        --repository "opendal:s3"
        --repo-opt "endpoint=${storageEndpoint}"
        --repo-opt "bucket=${bucket}"
        --repo-opt "region=us-east-1"
        --repo-opt "enable_virtual_host_style=false"
      )

      ${pkgs.coreutils}/bin/install -d -o postgres -g postgres -m 0700 ${dumpDir}
      ${pkgs.util-linux}/bin/runuser -u postgres -- \
        ${pgDump} -Fc -Z0 -f ${dumpDir}/supabase.dump postgres

      if ! ${rustic} "''${repo_opts[@]}" snapshots >/dev/null 2>&1; then
        ${rustic} "''${repo_opts[@]}" init
      fi

      ${rustic} "''${repo_opts[@]}" backup ${dumpDir}/supabase.dump

      ${rustic} "''${repo_opts[@]}" forget \
        --keep-daily 14 \
        --keep-weekly 8 \
        --keep-monthly 6 \
        --prune

      ${pkgs.coreutils}/bin/rm -f ${dumpDir}/supabase.dump
    '';
  };

  systemd.timers.rustic-backup = {
    description = "Daily timer for PostgreSQL rustic backup";
    wantedBy = [ "timers.target" ];
    timerConfig = {
      OnCalendar = "02:30";
      RandomizedDelaySec = "30m";
      Persistent = true;
    };
  };
}
