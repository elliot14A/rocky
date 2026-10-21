{ lib }:
let
  normalize = import ./normalize.nix;

  isWildcardIp = ip: ip == "0.0.0.0" || ip == "" || ip == "::" || ip == "[::]";

  parsePortMapping =
    portStr:
    let
      splitProto = lib.splitString "/" portStr;
      rawMapping = builtins.head splitProto;
      protocol =
        if builtins.length splitProto > 1 then
          lib.toLower (builtins.elemAt splitProto 1)
        else
          "tcp";
    in
    if lib.hasInfix "]:" rawMapping then
      let
        splitBracket = lib.splitString "]:" rawMapping;
        ip = lib.removePrefix "[" (builtins.head splitBracket);
        rest = builtins.elemAt splitBracket 1;
        parts = lib.splitString ":" rest;
        hostPortStr = builtins.head parts;
      in
      if parts == [ ] || hostPortStr == "" then
        null
      else
        {
          hostIp = ip;
          hostPort = lib.toInt hostPortStr;
          protocol = protocol;
        }
    else
      let
        parts = lib.splitString ":" rawMapping;
        len = builtins.length parts;
      in
      if len == 1 || len == 2 then
        let
          hostPortStr = builtins.head parts;
        in
        if hostPortStr == "" then
          null
        else
          {
            hostIp = "0.0.0.0";
            hostPort = lib.toInt hostPortStr;
            protocol = protocol;
          }
      else if len == 3 then
        let
          hostPortStr = builtins.elemAt parts 1;
        in
        if hostPortStr == "" then
          null
        else
          {
            hostIp = builtins.head parts;
            hostPort = lib.toInt hostPortStr;
            protocol = protocol;
          }
      else
        null;

  isReplica =
    service: container:
    container == service
    || (
      let
        prefix = "${service}-";
        prefixLen = builtins.stringLength prefix;
      in
      (builtins.substring 0 prefixLen container == prefix)
      && (builtins.match "[0-9]+" (builtins.substring prefixLen (-1) container) != null)
    );

  checkCollision =
    a: b:
    let
      sameWorkload =
        (
          (a ? serviceName && b ? containerName)
          && isReplica a.serviceName b.containerName
        )
        || (
          (b ? serviceName && a ? containerName)
          && isReplica b.serviceName a.containerName
        );

      sameProtocol = a.protocol == b.protocol;
      samePort = a.hostPort == b.hostPort;
      ipOverlap = isWildcardIp a.hostIp || isWildcardIp b.hostIp || a.hostIp == b.hostIp;
    in
    !sameWorkload && sameProtocol && samePort && ipOverlap;

  findPortCollision =
    entries:
    let
      find =
        remaining:
        if remaining == [ ] then
          null
        else
          let
            head = builtins.head remaining;
            tail = builtins.tail remaining;
            match = lib.findFirst (other: checkCollision head other) null tail;
          in
          if match != null then
            {
              port = head.hostPort;
              protocol = head.protocol;
              first = head.owner;
              second = match.owner;
              firstIp = head.hostIp;
              secondIp = match.hostIp;
            }
          else
            find tail;
    in
    find entries;

  collectServiceEntries =
    rawServices:
    let
      services = normalize rawServices;
    in
    lib.concatLists (
      lib.mapAttrsToList (
        name: svc:
        let
          replicas = svc.replicas or 1;
          basePort = svc.basePort or null;
        in
        if basePort != null && replicas > 0 then
          builtins.genList (i: {
            owner = "service '${name}'";
            serviceName = name;
            hostIp = "0.0.0.0";
            hostPort = basePort + i;
            protocol = "tcp";
          }) replicas
        else
          [ ]
      ) services
    );

  collectContainerEntries =
    containers:
    lib.concatLists (
      lib.mapAttrsToList (
        containerName: containerConfig:
        lib.filter (e: e != null) (
          map (
            p:
            let
              parsed = parsePortMapping p;
            in
            if parsed != null then
              {
                owner = "container '${containerName}'";
                containerName = containerName;
                hostIp = parsed.hostIp;
                hostPort = parsed.hostPort;
                protocol = parsed.protocol;
              }
            else
              null
          ) (containerConfig.ports or [ ])
        )
      ) containers
    );

  validateHostServices = services: findPortCollision (collectServiceEntries services);

  validateContainerPorts = containers: findPortCollision (collectContainerEntries containers);

  validateAllPorts =
    {
      services ? { },
      containers ? { },
    }:
    findPortCollision ((collectServiceEntries services) ++ (collectContainerEntries containers));
in
{
  inherit
    isWildcardIp
    parsePortMapping
    findPortCollision
    validateHostServices
    validateContainerPorts
    validateAllPorts
    ;
}
