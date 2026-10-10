{ inputs, ... }:
{
  imports = [
    inputs.disko.nixosModules.disko
    ../../../../../../templates/base/system/disko/oracle.nix
  ];
  networking.hostName = "test-002";
  system.stateVersion = "26.05";
}
