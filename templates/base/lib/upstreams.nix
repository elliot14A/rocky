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
  basePort ? null,
}:
let
  servicesOf = host: normalizeServices (fleet.hosts.${host}.services or [ ]);
  runsHost =
    host:
    let
      services = servicesOf host;
    in
    (builtins.hasAttr service services) && ((services.${service}.replicas or 1) > 0);
  hosts = builtins.filter runsHost (builtins.attrNames fleet.hosts);
  addrOf = host: if host == thisHost then "127.0.0.1" else "${environment}-${host}";
  instancesOn =
    host:
    let
      services = servicesOf host;
      svc = services.${service};
      replicas = svc.replicas or 1;
      actualBasePort =
        if svc ? basePort then
          svc.basePort
        else if basePort != null then
          basePort
        else
          throw "service '${service}' on host '${host}': basePort is required";
      addr = addrOf host;
    in
    builtins.genList (i: "${addr}:${toString (actualBasePort + i)}") replicas;
in
builtins.concatLists (map instancesOn hosts)
