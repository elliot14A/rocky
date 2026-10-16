{
  pkgs,
  lib,
  ...
}:

let
  isAarch64 = pkgs.stdenv.hostPlatform.isAarch64;
  # Oracle Ampere A1 (ARM64) uses ttyAMA0; AMD E2/E4 (x86_64) uses ttyS0
  serialPort = if isAarch64 then "ttyAMA0" else "ttyS0";
in
{
  boot.loader.systemd-boot.enable = lib.mkDefault false;
  boot.loader.grub = {
    enable = lib.mkDefault true;
    # Oracle Cloud compute instances boot exclusively via UEFI; "nodev" writes to ESP without MBR
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

  # Serial console required for Oracle Cloud Web Console Connections
  boot.kernelParams = [
    "console=tty1"
    "console=${serialPort},115200"
  ];

  # Storage and network kernel drivers for Oracle Cloud virtual instances
  boot.initrd.availableKernelModules = [
    "virtio_pci"
    "virtio_scsi"
    "virtio_net"
    "sd_mod"
  ];

  networking.useDHCP = lib.mkDefault true;

  # Oracle Cloud VCN Jumbo Frames optimization.
  # Uncomment if your VCN subnet allows MTU 9000:
  # networking.interfaces.enp0s3.mtu = 9000;
}
