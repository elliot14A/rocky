{
  config,
  pkgs,
  lib,
  environment,
  ...
}:

let
  secretsDir = ../environments + "/${environment}/secrets";
  hasCreds = builtins.pathExists (secretsDir + "/postgres-creds.age");
in
{
  age.secrets = lib.optionalAttrs hasCreds {
    "postgres-creds" = {
      file = secretsDir + "/postgres-creds.age";
      mode = "0400";
    };
  };

  services.postgresql = {
    enable = true;
    enableTCPIP = true;
    settings.listen_addresses = "*";
    authentication = ''
      host all all 100.64.0.0/10 md5
      host all all 127.0.0.1/32 trust
    '';
    ensureDatabases = [
      "postgres"
      "supabase_admin"
    ];
    ensureUsers = [
      {
        name = "postgres";
        ensureDBOwnership = true;
      }
      {
        name = "authenticator";
      }
      {
        name = "anon";
      }
      {
        name = "authenticated";
      }
      {
        name = "service_role";
      }
      {
        name = "supabase_admin";
        ensureDBOwnership = true;
      }
    ];
  };

  systemd.services.postgresql-set-passwords = {
    description = "Set PostgreSQL passwords and role permissions from agenix secrets";
    wantedBy = [ "multi-user.target" ];
    after = [ "postgresql.service" ];
    requires = [ "postgresql.service" ];
    restartTriggers =
      if hasCreds then [ (builtins.hashFile "sha256" (secretsDir + "/postgres-creds.age")) ] else [ ];
    serviceConfig = {
      Type = "oneshot";
      RemainAfterExit = true;
    };
    script = ''
      set -eu

      # Create storage schema and grant role permissions
      ${pkgs.util-linux}/bin/runuser -u postgres -- \
        ${config.services.postgresql.package}/bin/psql -v ON_ERROR_STOP=1 -d postgres << 'EOF'
      CREATE SCHEMA IF NOT EXISTS storage AUTHORIZATION supabase_admin;
      GRANT anon, authenticated, service_role TO authenticator;
      ALTER ROLE service_role BYPASSRLS;
      ALTER ROLE supabase_admin BYPASSRLS;
      GRANT ALL ON SCHEMA public TO postgres, anon, authenticated, service_role, supabase_admin;
      GRANT ALL ON SCHEMA storage TO postgres, anon, authenticated, service_role, supabase_admin;
      GRANT ALL ON ALL TABLES IN SCHEMA storage TO postgres, anon, authenticated, service_role, supabase_admin;
      GRANT ALL ON ALL SEQUENCES IN SCHEMA storage TO postgres, anon, authenticated, service_role, supabase_admin;
      GRANT ALL ON ALL ROUTINES IN SCHEMA storage TO postgres, anon, authenticated, service_role, supabase_admin;
      ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL ON TABLES TO anon, authenticated, service_role;
      ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL ON FUNCTIONS TO anon, authenticated, service_role;
      ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL ON SEQUENCES TO anon, authenticated, service_role;
      ALTER DEFAULT PRIVILEGES FOR ROLE supabase_admin IN SCHEMA storage GRANT ALL ON TABLES TO postgres, anon, authenticated, service_role, supabase_admin;
      ALTER DEFAULT PRIVILEGES FOR ROLE supabase_admin IN SCHEMA storage GRANT ALL ON SEQUENCES TO postgres, anon, authenticated, service_role, supabase_admin;
      ALTER DEFAULT PRIVILEGES FOR ROLE supabase_admin IN SCHEMA storage GRANT ALL ON ROUTINES TO postgres, anon, authenticated, service_role, supabase_admin;
      EOF

      if [ -f ${config.age.secrets."postgres-creds".path or "/nonexistent"} ]; then
        set -a
        source ${config.age.secrets."postgres-creds".path or "/nonexistent"}
        set +a

        if [ -n "''${POSTGRES_PASSWORD:-}" ]; then
          esc_pg=$(printf '%s' "$POSTGRES_PASSWORD" | sed "s/'/'''/g")
          ${pkgs.util-linux}/bin/runuser -u postgres -- \
            ${config.services.postgresql.package}/bin/psql -v ON_ERROR_STOP=1 -c "ALTER ROLE postgres PASSWORD '$esc_pg'"
        fi

        if [ -n "''${AUTHENTICATOR_PASSWORD:-}" ]; then
          esc_auth=$(printf '%s' "$AUTHENTICATOR_PASSWORD" | sed "s/'/'''/g")
          ${pkgs.util-linux}/bin/runuser -u postgres -- \
            ${config.services.postgresql.package}/bin/psql -v ON_ERROR_STOP=1 -c "ALTER ROLE authenticator PASSWORD '$esc_auth'"
        fi

        if [ -n "''${SUPABASE_ADMIN_PASSWORD:-}" ]; then
          esc_admin=$(printf '%s' "$SUPABASE_ADMIN_PASSWORD" | sed "s/'/'''/g")
          ${pkgs.util-linux}/bin/runuser -u postgres -- \
            ${config.services.postgresql.package}/bin/psql -v ON_ERROR_STOP=1 -c "ALTER ROLE supabase_admin PASSWORD '$esc_admin'"
        fi
      fi
    '';
  };
}
