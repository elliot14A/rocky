{
  config,
  lib,
  hostConfig ? { },
  ...
}:

let
  portsLib = import ../lib/ports.nix { inherit lib; };
  resolvedHost = config.networking.hostName;

  containerCollision = portsLib.validateAllPorts {
    services = hostConfig.services or { };
    containers = config.virtualisation.oci-containers.containers;
  };
in
{
  virtualisation.podman = {
    enable = true;
    dockerCompat = true;
    defaultNetwork.settings.dns_enabled = true;
    autoPrune = {
      enable = true;
      dates = "weekly";
      flags = [ "--all" ];
    };
  };

  virtualisation.oci-containers.backend = "podman";

  assertions = [
    {
      assertion = containerCollision == null;
      message = "host '${resolvedHost}': port collision on port ${
        toString (containerCollision.port or 0)
      } (${containerCollision.protocol or "tcp"}) between ${containerCollision.first or ""} and ${
        containerCollision.second or ""
      }";
    }
  ];
}
