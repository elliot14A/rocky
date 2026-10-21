{
  settings = {
    domain = "prod.rocky.internal";
    acmeEmail = "admin@prod.rocky.internal";
  };

  hosts = {
    "db" = {
      services = [ ];
    };

    "rest" = {
      services = [ ];
    };

    "auth" = {
      services = [ ];
    };

    "storage" = {
      services = [ ];
    };

    "web" = {
      services = [ ];
    };

    "edge" = {
      services = [ ];
    };
  };
}
