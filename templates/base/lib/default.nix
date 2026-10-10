{ inputs }:

let
  inherit (inputs.nixpkgs) lib;

  normalize = import ./normalize.nix;
  resolveServiceHost = import ./address.nix;

  baseModules = [
    ../system/ssh.nix
    ../system/firewall.nix
    ../system/tailscale.nix
    ../system/baseline.nix
  ];

  environmentsDir = ../environments;
  servicesDir = ../services;

  # Discover all environment folders (e.g. environments/prod, environments/stag)
  environmentNames = builtins.attrNames (
    lib.filterAttrs (_name: type: type == "directory") (builtins.readDir environmentsDir)
  );

  # Load fleet declaration and environment variables for one environment
  loadEnvironment = envName: {
    name = envName;
    fleet = import (environmentsDir + "/${envName}/fleet.nix");
    envVars = import (environmentsDir + "/${envName}/env.nix");
  };

  environments = map loadEnvironment environmentNames;

  # Build the complete NixOS system and deploy-rs definition for a single host
  buildHost =
    env: hostName: hostConfig:
    let
      fullHostName = "${env.name}-${hostName}";
      systemArchitecture = hostConfig.system or "x86_64-linux";
      normalizedServices = normalize hostConfig.services;

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

      systemConfig = lib.nixosSystem {
        system = systemArchitecture;
        specialArgs = {
          inherit inputs hostName;
          hostConfig = hostConfig // {
            services = normalizedServices;
          };
          environment = env.name;
          fleet = env.fleet;
          envVars = env.envVars;
          serviceHost = resolveServiceHost {
            fleet = env.fleet;
            environment = env.name;
            thisHost = hostName;
          };
        };
        modules = [
          inputs.agenix.nixosModules.default
        ]
        ++ baseModules
        ++ [
          (../providers + "/${hostConfig.provider}.nix")
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
