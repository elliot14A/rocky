{
  pkgs,
  lib,
  ...
}:

let
  isAarch64 = pkgs.stdenv.hostPlatform.isAarch64;
  serialPort = if isAarch64 then "ttyAMA0" else "ttyS0";
in
{
  boot.loader.grub = {
    enable = true;
    devices = [ "nodev" ];
    efiSupport = true;
    efiInstallAsRemovable = true;
    configurationLimit = 10;
    extraConfig = ''
      serial --unit=0 --speed=115200 --word=8 --parity=no --stop=1
      terminal_input serial
      terminal_output serial
    '';
  };
  boot.loader.efi.canTouchEfiVariables = false;
  boot.growPartition = true;

  boot.kernelParams = [
    "console=tty1"
    "console=${serialPort},115200"
  ];

  networking.useDHCP = lib.mkDefault true;
}
