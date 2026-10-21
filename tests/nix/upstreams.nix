let
  fleet = {
    hosts = {
      "001" = {
        services = [
          "postgres"
          "caddy"
        ];
      };
      "002" = {
        services = [
          "api"
        ];
      };
    };
  };

  environment = "test";

  serviceHost001 = import ../../templates/base/lib/address.nix {
    inherit fleet environment;
    thisHost = "001";
  };

  serviceHost002 = import ../../templates/base/lib/address.nix {
    inherit fleet environment;
    thisHost = "002";
  };

  serviceUpstreams001 = import ../../templates/base/lib/upstreams.nix {
    inherit fleet environment;
    thisHost = "001";
  };

  serviceUpstreams002 = import ../../templates/base/lib/upstreams.nix {
    inherit fleet environment;
    thisHost = "002";
  };

  normalizeServices = import ../../templates/base/lib/normalize.nix;

  assertHost001Postgres = (serviceHost001 "postgres") == "127.0.0.1";
  assertHost001Api = (serviceHost001 "api") == "test-002";
  assertHost002Postgres = (serviceHost002 "postgres") == "test-001";

  assertUpstreams001Postgres =
    (serviceUpstreams001 {
      service = "postgres";
      basePort = 5432;
    }) == [ "127.0.0.1:5432" ];
  assertUpstreams001Api =
    (serviceUpstreams001 {
      service = "api";
      basePort = 8000;
    }) == [ "test-002:8000" ];
  assertUpstreams002Postgres =
    (serviceUpstreams002 {
      service = "postgres";
      basePort = 5432;
    }) == [ "test-001:5432" ];

  assertNormalizeAttrs =
    (normalizeServices [
      "postgres"
      { api.replicas = 2; }
    ]) == {
      postgres = { };
      api = {
        replicas = 2;
      };
    };

  assertNormalizeBareAttrs =
    (normalizeServices {
      api = { replicas = 3; };
      worker = { };
    }) == {
      api = { replicas = 3; };
      worker = { };
    };

  assertNormalizeNull = (normalizeServices null) == { };
  assertNormalizeEmpty = (normalizeServices [ ]) == { };

  assertMissingServiceThrows = !(builtins.tryEval (serviceHost001 "nonexistent")).success;

  fleetWithZeroReplicas = {
    hosts = {
      "001" = {
        services = [
          { api.replicas = 0; }
        ];
      };
    };
  };

  serviceHostZeroReplicas = import ../../templates/base/lib/address.nix {
    fleet = fleetWithZeroReplicas;
    inherit environment;
    thisHost = "002";
  };

  assertZeroReplicasThrows = !(builtins.tryEval (serviceHostZeroReplicas "api")).success;

  fleetWithCentralPort = {
    hosts = {
      "001" = {
        services = [
          {
            web = {
              replicas = 2;
              basePort = 9000;
            };
          }
        ];
      };
    };
  };

  serviceUpstreamsCentralPort = import ../../templates/base/lib/upstreams.nix {
    fleet = fleetWithCentralPort;
    inherit environment;
    thisHost = "002";
  };

  assertUpstreamsCentralPort =
    (serviceUpstreamsCentralPort {
      service = "web";
    }) == [ "test-001:9000" "test-001:9001" ];

  fleetWithOverriddenPort = {
    hosts = {
      "node-a" = {
        services = [
          {
            api = {
              replicas = 2;
              basePort = 9000;
            };
          }
        ];
      };
      "node-b" = {
        services = [
          "api"
        ];
      };
    };
  };

  serviceUpstreamsPortPrecedence = import ../../templates/base/lib/upstreams.nix {
    fleet = fleetWithOverriddenPort;
    inherit environment;
    thisHost = "edge";
  };

  assertUpstreamsHostPortPrecedence =
    (serviceUpstreamsPortPrecedence {
      service = "api";
      basePort = 8000;
    }) == [
      "test-node-a:9000"
      "test-node-a:9001"
      "test-node-b:8000"
    ];

  complexFleet = {
    hosts = {
      "host-a" = {
        services = [
          "postgres"
          {
            api = {
              replicas = 1;
              basePort = 8000;
            };
          }
        ];
      };
      "host-b" = {
        services = [
          {
            api = {
              replicas = 2;
              basePort = 8000;
            };
          }
        ];
      };
      "host-c" = {
        services = [
          { api.replicas = 0; }
        ];
      };
      "host-d" = {
        services = [ ];
      };
      "host-e" = { };
    };
  };

  complexHostA = import ../../templates/base/lib/address.nix {
    fleet = complexFleet;
    inherit environment;
    thisHost = "host-a";
  };

  complexHostB = import ../../templates/base/lib/address.nix {
    fleet = complexFleet;
    inherit environment;
    thisHost = "host-b";
  };

  complexHostC = import ../../templates/base/lib/address.nix {
    fleet = complexFleet;
    inherit environment;
    thisHost = "host-c";
  };

  complexHostE = import ../../templates/base/lib/address.nix {
    fleet = complexFleet;
    inherit environment;
    thisHost = "host-e";
  };

  complexUpstreamsA = import ../../templates/base/lib/upstreams.nix {
    fleet = complexFleet;
    inherit environment;
    thisHost = "host-a";
  };

  complexUpstreamsC = import ../../templates/base/lib/upstreams.nix {
    fleet = complexFleet;
    inherit environment;
    thisHost = "host-c";
  };

  assertComplexHostA = (complexHostA "api") == "127.0.0.1";
  assertComplexHostB = (complexHostB "api") == "127.0.0.1";
  assertComplexHostC = (complexHostC "api") == "test-host-a";
  assertComplexHostE = (complexHostE "api") == "test-host-a";

  assertComplexUpstreamsA =
    (complexUpstreamsA { service = "api"; }) == [
      "127.0.0.1:8000"
      "test-host-b:8000"
      "test-host-b:8001"
    ];

  assertComplexUpstreamsC =
    (complexUpstreamsC { service = "api"; }) == [
      "test-host-a:8000"
      "test-host-b:8000"
      "test-host-b:8001"
    ];
in
assert assertHost001Postgres;
assert assertHost001Api;
assert assertHost002Postgres;
assert assertUpstreams001Postgres;
assert assertUpstreams001Api;
assert assertUpstreams002Postgres;
assert assertNormalizeAttrs;
assert assertNormalizeBareAttrs;
assert assertNormalizeNull;
assert assertNormalizeEmpty;
assert assertMissingServiceThrows;
assert assertZeroReplicasThrows;
assert assertUpstreamsCentralPort;
assert assertUpstreamsHostPortPrecedence;

assert assertComplexHostA;
assert assertComplexHostB;
assert assertComplexHostC;
assert assertComplexHostE;
assert assertComplexUpstreamsA;
assert assertComplexUpstreamsC;
{
  passed = true;
}
