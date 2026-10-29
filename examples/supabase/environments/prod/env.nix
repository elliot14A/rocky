{
  postgrest = {
    PGRST_DB_SCHEMAS = "public,storage";
    PGRST_DB_ANON_ROLE = "anon";
  };

  storage = {
    STORAGE_BACKEND = "file";
    FILE_STORAGE_BACKEND_PATH = "/var/lib/storage";
    GLOBAL_S3_BUCKET = "backups";
  };

  gotrue = {
    GOTRUE_LOG_LEVEL = "info";
    GOTRUE_DISABLE_SIGNUP = "false";
  };

  studio = {
    DEFAULT_ORGANIZATION_NAME = "Default Organization";
    DEFAULT_PROJECT_NAME = "Default Project";
  };

  meta = {
    PG_META_DB_PORT = "5432";
    PG_META_DB_NAME = "postgres";
    PG_META_DB_USER = "supabase_admin";
  };
}
