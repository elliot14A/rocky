{
  hostConfig,
  lib,
}:
let
  resolveBasePort =
    service: basePort:
    if hostConfig.services.${service} ? basePort then
      hostConfig.services.${service}.basePort
    else if basePort != null then
      basePort
    else
      throw "service '${service}': basePort is required";

  toList = x: if x == null then [ ] else if builtins.isList x then x else [ x ];

  mkInstances =
    service: f:
    let
      replicas = hostConfig.services.${service}.replicas or 1;
      mkPair =
        i:
        let
          index = i + 1;
          name = if replicas == 1 then service else "${service}-${toString index}";
        in
        f { inherit i index name; };
    in
    builtins.listToAttrs (builtins.genList mkPair replicas);
in
{
  replicateService =
    {
      service,
      basePort ? null,
      envFiles ? [ ],
      secretHashes ? [ ],
      mkUnit,
    }:
    assert lib.assertMsg (hostConfig.services ? ${service})
      "service '${service}' is not declared in hostConfig.services";
    let
      replicas = hostConfig.services.${service}.replicas or 1;
      hasExplicitBasePort = (hostConfig.services.${service} ? basePort) || basePort != null;
      actualBasePort =
        if (replicas > 0 && hasExplicitBasePort) then resolveBasePort service basePort else null;
    in
    mkInstances service (
      { i, index, name }:
      let
        port = if actualBasePort != null then actualBasePort + i else null;
        userUnit = mkUnit { inherit index port name; };
      in
      lib.nameValuePair name (
        userUnit
        // {
          restartTriggers = (toList (userUnit.restartTriggers or [ ])) ++ (toList secretHashes);
          serviceConfig = (userUnit.serviceConfig or { }) // (
            lib.optionalAttrs (envFiles != [ ] && envFiles != null) {
              EnvironmentFile = (toList envFiles) ++ (toList (userUnit.serviceConfig.EnvironmentFile or [ ]));
            }
          );
        }
      )
    );

  replicateContainers =
    {
      service,
      basePort ? null,
      containerPort ? null,
      bindHost ? null,
      envFiles ? [ ],
      mkContainer,
    }:
    assert lib.assertMsg (hostConfig.services ? ${service})
      "service '${service}' is not declared in hostConfig.services";
    let
      replicas = hostConfig.services.${service}.replicas or 1;
      hasExplicitBasePort = (hostConfig.services.${service} ? basePort) || basePort != null;
      actualBasePort =
        if (replicas > 0 && hasExplicitBasePort) then resolveBasePort service basePort else null;
      hasPorts = actualBasePort != null && containerPort != null;
    in
    assert lib.assertMsg (replicas == 0 || hasExplicitBasePort == (containerPort != null))
      "service '${service}': basePort and containerPort must either both be specified or both be omitted";
    mkInstances service (
      { i, index, name }:
      let
        port = if actualBasePort != null then actualBasePort + i else null;
        userContainer = mkContainer { inherit index port name; };
        portMapping =
          if bindHost != null then
            "${bindHost}:${toString port}:${toString containerPort}"
          else
            "${toString port}:${toString containerPort}";
      in
      lib.nameValuePair name (
        userContainer
        // (lib.optionalAttrs hasPorts {
          ports = [ portMapping ] ++ (toList (userContainer.ports or [ ]));
        })
        // (lib.optionalAttrs (envFiles != [ ] && envFiles != null) {
          environmentFiles = (toList envFiles) ++ (toList (userContainer.environmentFiles or [ ]));
        })
      )
    );

  containerRestartTriggers =
    {
      service,
      secretHashes ? [ ],
    }:
    assert lib.assertMsg (hostConfig.services ? ${service})
      "service '${service}' is not declared in hostConfig.services";
    mkInstances service (
      { name, ... }:
      lib.nameValuePair "podman-${name}" {
        restartTriggers = toList secretHashes;
      }
    );
}
