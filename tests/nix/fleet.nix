let
  flake = builtins.getFlake (toString ../..);
  baseLib = import ../../templates/base/lib {
    inputs = flake.inputs;
    environmentsDir = ./environments;
    servicesDir = ../../templates/base/services;
  };

  # Check configurations generated
  configs = baseLib.nixosConfigurations;
  nodes = baseLib.deploy.nodes;

  # Test assertions
  has001 = builtins.hasAttr "test-001" configs;
  has002 = builtins.hasAttr "test-002" configs;
  hasNode001 = builtins.hasAttr "test-001" nodes;
  hasNode002 = builtins.hasAttr "test-002" nodes;

  host001Arch = configs."test-001".config.nixpkgs.system == "x86_64-linux";
  host002Arch = configs."test-002".config.nixpkgs.system == "aarch64-linux";

  caddyEnabled = configs."test-001".config.services.caddy.enable == true;
  caddyPorts =
    configs."test-001".config.networking.firewall.allowedTCPPorts == [
      80
      443
    ];
  host002NoTcpPorts = configs."test-002".config.networking.firewall.allowedTCPPorts == [ ];

  allPassed =
    has001
    && has002
    && hasNode001
    && hasNode002
    && host001Arch
    && host002Arch
    && caddyEnabled
    && caddyPorts
    && host002NoTcpPorts;
in
assert allPassed;
{
  passed = true;
}
