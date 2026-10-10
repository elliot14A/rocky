let
  normalizeServices = import ./normalize.nix;
in
{
  fleet,
  environment,
  thisHost,
}:
{
  service,
  basePort,
}:
let
  servicesOf = host: normalizeServices (fleet.hosts.${host}.services or [ ]);
  runsHost = host: builtins.hasAttr service (servicesOf host);
  hosts = builtins.filter runsHost (builtins.attrNames fleet.hosts);
  addrOf = host: if host == thisHost then "127.0.0.1" else "${environment}-${host}";
  instancesOn =
    host:
    let
      replicas = (servicesOf host).${service}.replicas or 1;
      addr = addrOf host;
    in
    builtins.genList (i: "${addr}:${toString (basePort + i)}") replicas;
in
builtins.concatLists (map instancesOn hosts)
