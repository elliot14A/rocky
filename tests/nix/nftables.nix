let
  pkgs = import <nixpkgs> { };
  lib = pkgs.lib;

  normalize = import ../../templates/base/lib/normalize.nix;
  serviceHelpers = import ../../templates/base/lib/service.nix;
  nftablesLib = import ../../templates/base/lib/nftables.nix { inherit lib; };

  hostConfig = {
    services = normalize [
      {
        "single-svc" = {
          replicas = 1;
          basePort = 8080;
        };
      }
      {
        "multi-svc" = {
          replicas = 3;
          basePort = 3000;
        };
      }
      {
        "udp-svc" = {
          replicas = 2;
          basePort = 5353;
        };
      }
    ];
  };

  helpers = serviceHelpers {
    inherit hostConfig lib;
  };

  # Test 1: Single replica service generates empty config (no nftables overhead)
  singleResult = helpers.replicateServiceLoadBalancer {
    service = "single-svc";
    basePort = 8080;
  };
  assertSingleEmpty = singleResult == { };

  # Test 2: Multi-replica service (3 replicas, basePort 3000) generates valid table and chains
  multiResult = helpers.replicateServiceLoadBalancer {
    service = "multi-svc";
    basePort = 3000;
  };
  assertMultiEnabled = multiResult.enable == true;
  assertMultiHasTable = multiResult.tables ? "rocky_lb_multi-svc";
  assertMultiFamily = multiResult.tables."rocky_lb_multi-svc".family == "ip";

  multiContent = multiResult.tables."rocky_lb_multi-svc".content;
  assertPreroutingChain = lib.hasInfix "chain prerouting" multiContent;
  assertOutputChain = lib.hasInfix "chain output" multiContent;
  assertNumgenMod3 = lib.hasInfix "numgen inc mod 3" multiContent;
  assertReplicaMap = lib.hasInfix "0 : 3000, 1 : 3001, 2 : 3002" multiContent;
  assertTcpProtocol = lib.hasInfix "tcp dport 3000 redirect" multiContent;

  # Test 3: Custom UDP protocol
  udpResult = helpers.replicateServiceLoadBalancer {
    service = "udp-svc";
    basePort = 5353;
    protocol = "udp";
  };
  udpContent = udpResult.tables."rocky_lb_udp-svc".content;
  assertUdpProtocol = lib.hasInfix "udp dport 5353 redirect" udpContent;
  assertUdpMod2 = lib.hasInfix "numgen inc mod 2" udpContent;
  assertUdpMap = lib.hasInfix "0 : 5353, 1 : 5354" udpContent;

  # Test 4: Direct library function test
  directResult = nftablesLib.mkServiceLoadBalancer {
    service = "direct-svc";
    inherit hostConfig;
    basePort = 9000;
  };
  assertDirectZeroOrOne = directResult == { };

in
assert assertSingleEmpty;
assert assertMultiEnabled;
assert assertMultiHasTable;
assert assertMultiFamily;
assert assertPreroutingChain;
assert assertOutputChain;
assert assertNumgenMod3;
assert assertReplicaMap;
assert assertTcpProtocol;
assert assertUdpProtocol;
assert assertUdpMod2;
assert assertUdpMap;
assert assertDirectZeroOrOne;
{
  passed = true;
}
