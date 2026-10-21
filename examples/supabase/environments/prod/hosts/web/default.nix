{ inputs, ... }:
{
  imports = [
    inputs.disko.nixosModules.disko
    ./disko.nix
    ./hardware.nix
  ];
  networking.hostName = "prod-web";
  system.stateVersion = "26.05";
}
