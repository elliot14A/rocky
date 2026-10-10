{
  settings = {
    domain = "test.rocky.internal";
    acmeEmail = "admin@rocky.internal";
  };
  hosts = {
    "001" = {
      provider = "hetzner";
      ip = "195.201.10.1";
      system = "x86_64-linux";
      services = [ "caddy" ];
    };
    "002" = {
      provider = "oracle";
      ip = "150.136.20.2";
      system = "aarch64-linux";
      services = [ ];
    };
  };
}
