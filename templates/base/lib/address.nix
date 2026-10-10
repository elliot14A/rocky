let
  normalizeServices = import ./normalize.nix;
in
{
  fleet,
  environment,
  thisHost,
}:
service:
let
  runs = host: builtins.hasAttr service (normalizeServices (fleet.hosts.${host}.services or [ ]));
  candidates = builtins.filter runs (builtins.attrNames fleet.hosts);
in
if runs thisHost then
  "127.0.0.1"
else if candidates == [ ] then
  throw "environment '${environment}': no host runs service '${service}'"
else
  "${environment}-${builtins.head candidates}"
