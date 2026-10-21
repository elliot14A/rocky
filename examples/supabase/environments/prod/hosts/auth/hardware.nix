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
  boot.loader.systemd-boot.enable = lib.mkDefault false;
  boot.loader.grub = {
    enable = lib.mkDefault true;
    devices = lib.mkDefault [ "nodev" ];
    efiSupport = lib.mkDefault true;
    efiInstallAsRemovable = lib.mkDefault true;
    configurationLimit = lib.mkDefault 10;
    extraConfig = ''
      serial --unit=0 --speed=115200 --word=8 --parity=no --stop=1
      terminal_input serial
      terminal_output serial
    '';
  };
  boot.loader.efi.canTouchEfiVariables = lib.mkDefault false;
  boot.growPartition = lib.mkDefault true;

  boot.kernelParams = [
    "console=tty1"
    "console=${serialPort},115200"
  ];

  boot.initrd.availableKernelModules = [
    "virtio_pci"
    "virtio_scsi"
    "virtio_net"
    "sd_mod"
  ];

  networking.useDHCP = lib.mkDefault true;
}
