{ inputs, ... }:
{
  imports = [
    inputs.disko.nixosModules.disko
    ../../../../../../templates/base/system/disko/hetzner.nix
  ];
  networking.hostName = "test-001";
  system.stateVersion = "26.05";
}
