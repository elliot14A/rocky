{
  settings = {
    domain = "prod.rocky.internal";
    acmeEmail = "admin@prod.rocky.internal";
  };

  hosts = {
    "db" = {
      services = [
        "postgres"
        "pgdog"
        "rustic"
      ];
    };

    "app" = {
      services = {
        "postgrest" = {
          replicas = 3;
          basePort = 3000;
        };
        "gotrue" = {
          replicas = 2;
          basePort = 9999;
        };
        "storage" = {
          replicas = 2;
          basePort = 5000;
        };
      };
    };

    "edge" = {
      services = {
        "caddy" = { };
        "studio" = { };
        "meta" = {
          basePort = 8080;
        };
      };
    };
  };
}
