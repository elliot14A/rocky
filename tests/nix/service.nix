let
  pkgs = import <nixpkgs> { };
  lib = pkgs.lib;

  normalize = import ../../templates/base/lib/normalize.nix;
  serviceHelpers = import ../../templates/base/lib/service.nix;
  portsLib = import ../../templates/base/lib/ports.nix { inherit lib; };

  normalizedServices = normalize [
    "plain-string-service"
    {
      "single-api" = {
        replicas = 1;
      };
    }
    {
      "multi-api" = {
        replicas = 3;
      };
    }
    {
      "central-api" = {
        replicas = 2;
        basePort = 7000;
      };
    }
    {
      "single-container" = {
        replicas = 1;
      };
    }
    {
      "multi-container" = {
        replicas = 2;
      };
    }
    {
      "worker-container" = {
        replicas = 1;
      };
    }
    {
      "multi-worker" = {
        replicas = 3;
      };
    }
    {
      "drained-api" = {
        replicas = 0;
      };
    }
  ];

  hostConfig = {
    services = normalizedServices;
  };

  helpers = serviceHelpers {
    inherit hostConfig lib;
  };

  singleService = helpers.replicateService {
    service = "single-api";
    basePort = 8000;
    envFiles = "/run/agenix/secret.env";
    secretHashes = "hash123";
    mkUnit =
      {
        port,
        name,
        ...
      }:
      {
        description = "Single API (${name})";
        serviceConfig = {
          ExecStart = "/bin/api";
          DynamicUser = true;
          EnvironmentFile = [ "/run/agenix/existing.env" ];
        };
        restartTriggers = [ "trigger-existing" ];
        environment = {
          PORT = toString port;
        };
      };
  };

  assertSingleServiceName = builtins.hasAttr "single-api" singleService;
  assertSingleServiceCount = (builtins.length (builtins.attrNames singleService)) == 1;
  assertSingleServicePort = singleService."single-api".environment.PORT == "8000";
  assertSingleServiceEnvFile =
    singleService."single-api".serviceConfig.EnvironmentFile
    == [
      "/run/agenix/secret.env"
      "/run/agenix/existing.env"
    ];
  assertSingleServiceHashes =
    singleService."single-api".restartTriggers == [ "trigger-existing" "hash123" ];
  assertSingleServicePreservedConfig =
    (singleService."single-api".serviceConfig.ExecStart == "/bin/api")
    && (singleService."single-api".serviceConfig.DynamicUser == true);

  plainStringService = helpers.replicateService {
    service = "plain-string-service";
    basePort = 4000;
    mkUnit =
      { port, ... }:
      {
        environment = {
          PORT = toString port;
        };
      };
  };

  assertPlainStringService =
    (builtins.hasAttr "plain-string-service" plainStringService)
    && (plainStringService."plain-string-service".environment.PORT == "4000");

  drainedService = helpers.replicateService {
    service = "drained-api";
    mkUnit =
      { port, ... }:
      {
        environment = {
          PORT = toString port;
        };
      };
  };

  assertDrainedServiceEmpty = drainedService == { };

  multiService = helpers.replicateService {
    service = "multi-api";
    basePort = 3000;
    envFiles = [ "/run/agenix/db.env" ];
    secretHashes = [ "hash456" ];
    mkUnit =
      {
        index,
        port,
        name,
      }:
      {
        description = "Multi API (${name})";
        environment = {
          PORT = toString port;
          INDEX = toString index;
        };
      };
  };

  assertMultiServiceNames =
    (builtins.hasAttr "multi-api-1" multiService)
    && (builtins.hasAttr "multi-api-2" multiService)
    && (builtins.hasAttr "multi-api-3" multiService);
  assertMultiServiceCount = (builtins.length (builtins.attrNames multiService)) == 3;
  assertMultiServicePorts =
    (multiService."multi-api-1".environment.PORT == "3000")
    && (multiService."multi-api-2".environment.PORT == "3001")
    && (multiService."multi-api-3".environment.PORT == "3002");
  assertMultiServiceIndices =
    (multiService."multi-api-1".environment.INDEX == "1")
    && (multiService."multi-api-2".environment.INDEX == "2")
    && (multiService."multi-api-3".environment.INDEX == "3");
  assertMultiServiceEnvFiles =
    (multiService."multi-api-1".serviceConfig.EnvironmentFile == [ "/run/agenix/db.env" ])
    && (multiService."multi-api-2".serviceConfig.EnvironmentFile == [ "/run/agenix/db.env" ])
    && (multiService."multi-api-3".serviceConfig.EnvironmentFile == [ "/run/agenix/db.env" ]);

  nullEnvService = helpers.replicateService {
    service = "single-api";
    basePort = 8000;
    envFiles = null;
    secretHashes = null;
    mkUnit =
      { port, ... }:
      {
        environment = {
          PORT = toString port;
        };
      };
  };

  assertNullEnvService =
    !(nullEnvService."single-api".serviceConfig ? EnvironmentFile)
    && (nullEnvService."single-api".restartTriggers == [ ]);

  centralPortService = helpers.replicateService {
    service = "central-api";
    basePort = 9999; # overridden by hostConfig.services.central-api.basePort = 7000
    mkUnit =
      { port, ... }:
      {
        environment = {
          PORT = toString port;
        };
      };
  };

  assertCentralPortService =
    (centralPortService."central-api-1".environment.PORT == "7000")
    && (centralPortService."central-api-2".environment.PORT == "7001");

  singleContainer = helpers.replicateContainers {
    service = "single-container";
    basePort = 8080;
    containerPort = 80;
    envFiles = "/run/agenix/container.env";
    mkContainer =
      { ... }:
      {
        image = "docker.io/example/single:latest";
      };
  };

  assertSingleContainerName = builtins.hasAttr "single-container" singleContainer;
  assertSingleContainerNoPorts = !(singleContainer."single-container" ? ports);
  assertSingleContainerNetworkHost =
    singleContainer."single-container".extraOptions == [ "--network=host" ];
  assertSingleContainerEnvFile =
    singleContainer."single-container".environmentFiles == [ "/run/agenix/container.env" ];

  bridgeContainer = helpers.replicateContainers {
    service = "single-container";
    basePort = 8080;
    containerPort = 80;
    mkContainer =
      { ... }:
      {
        image = "docker.io/example/single:latest";
        extraOptions = [ "--network=bridge" ];
      };
  };
  assertBridgeContainerPort = bridgeContainer."single-container".ports == [ "8080:80" ];

  multiContainer = helpers.replicateContainers {
    service = "multi-container";
    basePort = 5000;
    containerPort = 5000;
    mkContainer =
      { ... }:
      {
        image = "docker.io/example/multi:latest";
        extraOptions = [ "--network=bridge" ];
      };
  };

  assertMultiContainerNames =
    (builtins.hasAttr "multi-container-1" multiContainer)
    && (builtins.hasAttr "multi-container-2" multiContainer);
  assertMultiContainerPorts =
    (multiContainer."multi-container-1".ports == [ "5000:5000" ])
    && (multiContainer."multi-container-2".ports == [ "5001:5000" ]);

  workerContainer = helpers.replicateContainers {
    service = "worker-container";
    mkContainer =
      { ... }:
      {
        image = "docker.io/example/worker:latest";
      };
  };

  assertWorkerContainerNoPorts = !(workerContainer."worker-container" ? ports);

  drainedContainerWithPorts = helpers.replicateContainers {
    service = "drained-api";
    basePort = 8080;
    containerPort = 80;
    mkContainer =
      { ... }:
      {
        image = "docker.io/example/drained:latest";
      };
  };

  assertDrainedContainerEmpty = drainedContainerWithPorts == { };

  assertContainerPortMismatchThrows = !(builtins.tryEval (helpers.replicateContainers {
    service = "worker-container";
    containerPort = 80; # basePort omitted
    mkContainer = { ... }: { image = "example"; };
  })).success;

  containerTriggers = helpers.containerRestartTriggers {
    service = "multi-container";
    secretHashes = [ "hash789" ];
  };

  assertContainerTriggers =
    (containerTriggers."podman-multi-container-1".restartTriggers == [ "hash789" ])
    && (containerTriggers."podman-multi-container-2".restartTriggers == [ "hash789" ]);

  drainedTriggers = helpers.containerRestartTriggers {
    service = "drained-api";
    secretHashes = [ "hash000" ];
  };

  assertDrainedTriggersEmpty = drainedTriggers == { };

  customBindContainer = helpers.replicateContainers {
    service = "single-container";
    basePort = 9090;
    containerPort = 90;
    bindHost = "100.64.0.5";
    mkContainer =
      { ... }:
      {
        image = "docker.io/example/custom:latest";
        volumes = [ "/var/data:/data:ro" ];
        extraOptions = [ "--network=bridge" "--cap-add=NET_ADMIN" ];
      };
  };

  assertCustomBindContainer =
    (customBindContainer."single-container".ports == [ "100.64.0.5:9090:90" ])
    && (customBindContainer."single-container".volumes == [ "/var/data:/data:ro" ])
    && (customBindContainer."single-container".extraOptions == [ "--network=bridge" "--cap-add=NET_ADMIN" ]);

  loopbackBindContainer = helpers.replicateContainers {
    service = "single-container";
    basePort = 9091;
    containerPort = 91;
    bindHost = "127.0.0.1";
    mkContainer =
      { ... }:
      {
        image = "docker.io/example/loopback:latest";
        extraOptions = [ "--network=bridge" ];
      };
  };

  assertLoopbackBindContainer =
    loopbackBindContainer."single-container".ports == [ "127.0.0.1:9091:91" ];

  assertUndeclaredServiceThrows = !(builtins.tryEval (helpers.replicateService {
    service = "nonexistent";
    mkUnit = { ... }: { };
  })).success;

  assertNoCollision =
    (portsLib.validateHostServices {
      api = { replicas = 3; basePort = 8000; };
      worker = { replicas = 2; basePort = 8003; };
    }) == null;

  multiWorkerService = helpers.replicateService {
    service = "multi-worker";
    mkUnit =
      { index, port, name }:
      {
        description = "Multi Worker ${name}";
        environment = {
          INDEX = toString index;
          HAS_PORT = if port == null then "false" else "true";
        };
      };
  };

  assertMultiWorkerCount = (builtins.length (builtins.attrNames multiWorkerService)) == 3;
  assertMultiWorkerNames =
    (builtins.hasAttr "multi-worker-1" multiWorkerService)
    && (builtins.hasAttr "multi-worker-2" multiWorkerService)
    && (builtins.hasAttr "multi-worker-3" multiWorkerService);
  assertMultiWorkerNoPort =
    (multiWorkerService."multi-worker-1".environment.HAS_PORT == "false")
    && (multiWorkerService."multi-worker-2".environment.HAS_PORT == "false")
    && (multiWorkerService."multi-worker-3".environment.HAS_PORT == "false");

  collidingResult = portsLib.validateHostServices {
    api = { replicas = 3; basePort = 8000; };
    worker = { replicas = 2; basePort = 8002; };
  };

  assertPortCollisionDetected =
    collidingResult != null
    && collidingResult.port == 8002
    && (
      (collidingResult.first == "service 'api'" && collidingResult.second == "service 'worker'")
      || (collidingResult.first == "service 'worker'" && collidingResult.second == "service 'api'")
    );

  assertDrainedNoCollision =
    (portsLib.validateHostServices {
      api = { replicas = 3; basePort = 8000; };
      drained = { replicas = 0; basePort = 8001; };
    }) == null;

  assertNoBasePortNoCollision =
    (portsLib.validateHostServices {
      api = { replicas = 3; basePort = 8000; };
      redis = { replicas = 1; };
    }) == null;

  assertParsePortSimple =
    (portsLib.parsePortMapping "8080:80")
    == { hostIp = "0.0.0.0"; hostPort = 8080; protocol = "tcp"; };
  assertParsePortWithIp =
    (portsLib.parsePortMapping "127.0.0.1:8080:80")
    == { hostIp = "127.0.0.1"; hostPort = 8080; protocol = "tcp"; };
  assertParsePortWithIpv6 =
    (portsLib.parsePortMapping "[::1]:8080:80")
    == { hostIp = "::1"; hostPort = 8080; protocol = "tcp"; };
  assertParsePortProtocolUdp =
    (portsLib.parsePortMapping "53:53/udp")
    == { hostIp = "0.0.0.0"; hostPort = 53; protocol = "udp"; };
  assertParsePortIpv6Single =
    (portsLib.parsePortMapping "[::1]:8080")
    == { hostIp = "::1"; hostPort = 8080; protocol = "tcp"; };

  assertParsePortEmpty =
    (portsLib.parsePortMapping "") == null;

  assertDistinctIpsNoCollision =
    (portsLib.validateContainerPorts {
      c1 = { ports = [ "127.0.0.1:8080:80" ]; };
      c2 = { ports = [ "100.64.0.1:8080:80" ]; };
    }) == null;

  assertCollidingContainerPorts =
    (portsLib.validateContainerPorts {
      c1 = { ports = [ "8080:80" ]; };
      c2 = { ports = [ "100.64.0.1:8080:80" ]; };
    }) != null;

  assertDifferentProtocolsNoCollision =
    (portsLib.validateContainerPorts {
      dnsTcp = { ports = [ "53:53/tcp" ]; };
      dnsUdp = { ports = [ "53:53/udp" ]; };
    }) == null;

  assertCrossWorkloadCollision =
    (portsLib.validateAllPorts {
      services = {
        api = { replicas = 1; basePort = 8080; };
      };
      containers = {
        web = { ports = [ "8080:80" ]; };
      };
    }) != null;

  assertCrossWorkloadWhitelisted =
    (portsLib.validateAllPorts {
      services = {
        web = { replicas = 1; basePort = 8080; };
      };
      containers = {
        web = { ports = [ "8080:80" ]; };
      };
    }) == null;

  assertNormalizeNullInList =
    (normalize [ "web" null ]) == { web = { }; };

  assertValidateRawServiceList =
    (portsLib.validateHostServices [ "web" ]) == null;

  assertCrossWorkloadReplicaWhitelisted =
    (portsLib.validateAllPorts {
      services = {
        api = { replicas = 2; basePort = 8080; };
      };
      containers = {
        "api-1" = { ports = [ "8080:8080" ]; };
        "api-2" = { ports = [ "8081:8081" ]; };
      };
    }) == null;

  assertCrossWorkloadDistinctContainerPrefixDetected =
    (portsLib.validateAllPorts {
      services = {
        api = { replicas = 1; basePort = 8080; };
      };
      containers = {
        api-proxy = { ports = [ "8080:8080" ]; };
      };
    }) != null;
in
assert assertSingleServiceName;
assert assertSingleServiceCount;
assert assertSingleServicePort;
assert assertSingleServiceEnvFile;
assert assertSingleServiceHashes;
assert assertSingleServicePreservedConfig;

assert assertPlainStringService;
assert assertDrainedServiceEmpty;

assert assertMultiServiceNames;
assert assertMultiServiceCount;
assert assertMultiServicePorts;
assert assertMultiServiceIndices;
assert assertMultiServiceEnvFiles;
assert assertNullEnvService;

assert assertMultiWorkerCount;
assert assertMultiWorkerNames;
assert assertMultiWorkerNoPort;

assert assertCentralPortService;

assert assertSingleContainerName;
assert assertSingleContainerNoPorts;
assert assertBridgeContainerPort;
assert assertSingleContainerNetworkHost;
assert assertSingleContainerEnvFile;

assert assertMultiContainerNames;
assert assertMultiContainerPorts;

assert assertWorkerContainerNoPorts;
assert assertDrainedContainerEmpty;
assert assertContainerPortMismatchThrows;

assert assertContainerTriggers;
assert assertDrainedTriggersEmpty;
assert assertCustomBindContainer;
assert assertLoopbackBindContainer;

assert assertUndeclaredServiceThrows;

assert assertNoCollision;
assert assertPortCollisionDetected;
assert assertDrainedNoCollision;
assert assertNoBasePortNoCollision;
assert assertParsePortSimple;
assert assertParsePortWithIp;
assert assertParsePortWithIpv6;
assert assertParsePortProtocolUdp;
assert assertParsePortIpv6Single;
assert assertParsePortEmpty;
assert assertDistinctIpsNoCollision;
assert assertCollidingContainerPorts;
assert assertDifferentProtocolsNoCollision;
assert assertCrossWorkloadCollision;
assert assertCrossWorkloadWhitelisted;
assert assertNormalizeNullInList;
assert assertValidateRawServiceList;
assert assertCrossWorkloadReplicaWhitelisted;
assert assertCrossWorkloadDistinctContainerPrefixDetected;
{
  passed = true;
}
