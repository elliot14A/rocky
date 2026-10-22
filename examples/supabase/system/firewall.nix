{
  config,
  lib,
  hostConfig ? { },
  hostName ? "unknown",
  ...
}:

let
  services = hostConfig.services or { };
  isCaddyPresent =
    if builtins.isAttrs services then
      builtins.hasAttr "caddy" services
    else if builtins.isList services then
      builtins.elem "caddy" services
    else
      false;
  isIngressHost = isCaddyPresent || (hostConfig.ingress or false);
  allowPublicPorts = hostConfig.allowPublicPorts or false;
  violatingPorts = config.networking.firewall.allowedTCPPorts;
in
{
  networking.firewall = {
    enable = true;
    allowedTCPPorts = [ ];
    allowedUDPPorts = [ 41641 ];
    trustedInterfaces = [ "tailscale0" ];
  };

  assertions = [
    {
      assertion = allowPublicPorts || isIngressHost || violatingPorts == [ ];
      message = "host '${hostName}' exposes public TCP ports (${lib.concatStringsSep ", " (map toString violatingPorts)}) on external interfaces without running an ingress service (such as 'caddy'). In Rocky, internal backends must accept traffic strictly over Tailscale WireGuard ('tailscale0'). To deliberately expose these ports to the public internet, set 'allowPublicPorts = true' in this host's fleet.nix configuration.";
    }
  ];
}
