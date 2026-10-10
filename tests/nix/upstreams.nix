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

  assertMissingServiceThrows = !(builtins.tryEval (serviceHost001 "nonexistent")).success;
in
assert assertHost001Postgres;
assert assertHost001Api;
assert assertHost002Postgres;
assert assertUpstreams001Postgres;
assert assertUpstreams001Api;
assert assertUpstreams002Postgres;
assert assertNormalizeAttrs;
assert assertMissingServiceThrows;
{
  passed = true;
}
