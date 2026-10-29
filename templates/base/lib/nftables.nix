{
  lib,
}:
let
  resolveBasePort =
    service: svc: basePort:
    if svc ? basePort then
      svc.basePort
    else if basePort != null then
      basePort
    else
      throw "service '${service}': basePort is required for nftables load balancer";
in
{
  mkServiceLoadBalancer =
    {
      service,
      hostConfig,
      basePort ? null,
      protocol ? "tcp",
    }:
    let
      services = hostConfig.services or { };
      svc = services.${service} or { };
      replicas = svc.replicas or 1;
      actualBasePort =
        if replicas > 1 then
          resolveBasePort service svc basePort
        else
          null;
    in
    if replicas <= 1 then
      { }
    else
      let
        modCount = toString replicas;
        mkEntry = i: "${toString i} : ${toString (actualBasePort + i)}";
        entries = lib.concatStringsSep ", " (builtins.genList mkEntry replicas);
        rule = "${protocol} dport ${toString actualBasePort} redirect to :numgen inc mod ${modCount} map { ${entries} }";
        tableContent = ''
          chain prerouting {
            type nat hook prerouting priority dstnat; policy accept;
            ${rule}
          }

          chain output {
            type nat hook output priority dstnat; policy accept;
            ${rule}
          }
        '';
      in
      {
        enable = true;
        tables."rocky_lb_${service}" = {
          family = "ip";
          content = tableContent;
        };
      };
}
