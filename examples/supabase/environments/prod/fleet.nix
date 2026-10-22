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

    "rest" = {
      services = {
        "postgrest" = {
          replicas = 3;
          basePort = 3000;
        };
      };
    };

    "auth" = {
      services = {
        "gotrue" = {
          replicas = 2;
          basePort = 9999;
        };
      };
    };

    "storage" = {
      services = {
        "storage" = {
          replicas = 2;
          basePort = 5000;
        };
      };
    };

    "web" = {
      services = [ "studio" ];
    };

    "edge" = {
      services = [ "caddy" ];
    };
  };
}
