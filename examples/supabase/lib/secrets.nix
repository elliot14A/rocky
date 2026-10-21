{
  operators,
  fleet,
  hostKeys,
}:

let
  normalize = import ./normalize.nix;
  operatorKeys = builtins.attrValues operators;
  allHostNames = builtins.attrNames fleet.hosts;

  servicesOn = host: normalize (fleet.hosts.${host}.services or [ ]);

  hostsRunning =
    services:
    builtins.filter (
      host:
      let
        sOn = servicesOn host;
      in
      builtins.any (s: (builtins.hasAttr s sOn) && ((sOn.${s}.replicas or 1) > 0)) services
    ) allHostNames;

  # A host gets an SSH key only after nixos-anywhere has installed it. Until
  # then it is not an age recipient: secrets stay operator-only, and a later
  # `agenix --rekey` widens them once the key is saved in host-keys.nix.
  keysFor = hosts: map (host: hostKeys.${host}) (builtins.filter (host: hostKeys ? ${host}) hosts);
in
{
  recipientsFor = services: operatorKeys ++ keysFor (hostsRunning services);
  allHosts = operatorKeys ++ keysFor allHostNames;
}
