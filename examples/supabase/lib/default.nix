{
  inputs,
  environmentsDir ? ../environments,
  servicesDir ? ../services,
}:

let
  inherit (inputs.nixpkgs) lib;

  normalize = import ./normalize.nix;
  resolveServiceHost = import ./address.nix;
  resolveServiceUpstreams = import ./upstreams.nix;
  serviceHelpers = import ./service.nix;
  portsLib = import ./ports.nix { inherit lib; };

  baseModules = [
    ../system/ssh.nix
    ../system/firewall.nix
    ../system/tailscale.nix
    ../system/baseline.nix
    ../system/containers.nix
  ];

  # Discover all environment folders (e.g. environments/prod, environments/stag)
  environmentNames =
    if builtins.pathExists environmentsDir then
      builtins.attrNames (
        lib.filterAttrs (_name: type: type == "directory") (builtins.readDir environmentsDir)
      )
    else
      [ ];

  # Load fleet declaration and environment variables for one environment
  loadEnvironment =
    envName:
    let
      fleet = import (environmentsDir + "/${envName}/fleet.nix");
    in
    assert (fleet.settings or { }) ? domain
      || throw "environment '${envName}': fleet.settings.domain must be set in fleet.nix";
    {
      name = envName;
      inherit fleet;
      envVars = import (environmentsDir + "/${envName}/env.nix");
    };

  environments = map loadEnvironment environmentNames;

  # Build the complete NixOS system and deploy-rs definition for a single host
  buildHost =
    env: hostName: hostConfig:
    let
      fullHostName = "${env.name}-${hostName}";
      systemArchitecture = hostConfig.system or "x86_64-linux";
      normalizedServices = normalize (hostConfig.services or [ ]);

      serviceModules = map (
        service:
        let
          path = servicesDir + "/${service}.nix";
        in
        if builtins.pathExists path then
          path
        else
          throw "environment '${env.name}', host '${hostName}': service module '${path}' does not exist"
      ) (builtins.attrNames normalizedServices);

      normalizedHostConfig = hostConfig // {
        services = normalizedServices;
      };

      helpers = serviceHelpers {
        hostConfig = normalizedHostConfig;
        inherit lib;
      };

      portCollision = portsLib.validateHostServices normalizedServices;

      systemConfig = lib.nixosSystem {
        system = systemArchitecture;
        specialArgs = {
          inherit inputs hostName;
          hostConfig = normalizedHostConfig;
          environment = env.name;
          fleet = env.fleet;
          envVars = env.envVars;
          serviceHost = resolveServiceHost {
            fleet = env.fleet;
            environment = env.name;
            thisHost = hostName;
          };
          serviceUpstreams = resolveServiceUpstreams {
            fleet = env.fleet;
            environment = env.name;
            thisHost = hostName;
          };
          replicateService = helpers.replicateService;
          replicateContainers = helpers.replicateContainers;
          containerRestartTriggers = helpers.containerRestartTriggers;
          replicateServiceLoadBalancer = helpers.replicateServiceLoadBalancer;
        };
        modules = [
          inputs.agenix.nixosModules.default
          {
            networking.hostName = lib.mkDefault fullHostName;
            assertions = [
              {
                assertion = portCollision == null;
                message = "host '${fullHostName}': port collision on port ${toString (portCollision.port or 0)} (${
                  portCollision.protocol or "tcp"
                }) between ${portCollision.first or ""} and ${portCollision.second or ""}";
              }
            ];
          }
        ]
        ++ baseModules
        ++ [
          (environmentsDir + "/${env.name}/hosts/${hostName}")
        ]
        ++ serviceModules;
      };
    in
    {
      name = fullHostName;
      system = systemConfig;
      deployNode = {
        hostname = fullHostName;
        sshUser = "rocky";
        profiles.system = {
          user = "root";
          path = inputs.deploy-rs.lib.${systemArchitecture}.activate.nixos systemConfig;
        };
      };
    };

  allHosts = lib.concatMap (env: lib.mapAttrsToList (buildHost env) env.fleet.hosts) environments;
in
{
  nixosConfigurations = lib.listToAttrs (
    map (host: lib.nameValuePair host.name host.system) allHosts
  );

  deploy.nodes = lib.listToAttrs (map (host: lib.nameValuePair host.name host.deployNode) allHosts);
}
