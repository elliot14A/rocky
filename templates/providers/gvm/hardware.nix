{
  pkgs,
  lib,
  modulesPath,
  ...
}:

let
  # Google Tau T2A (ARM64) uses ttyAMA0; standard E2/N2/C2 (x86_64) uses ttyS0
  serialPort = if pkgs.stdenv.hostPlatform.isAarch64 then "ttyAMA0" else "ttyS0";
in
{
  imports = [ (modulesPath + "/profiles/qemu-guest.nix") ];

  boot.loader.systemd-boot.enable = lib.mkDefault false;
  boot.loader.grub = {
    enable = lib.mkDefault true;
    # Google Cloud Persistent Disks attach as /dev/sda (or /dev/nvme0n1 on Tau T2A and C3)
    devices = lib.mkDefault [ "/dev/sda" ];
    efiSupport = lib.mkDefault true;
    efiInstallAsRemovable = lib.mkDefault true;
    configurationLimit = lib.mkDefault 10;
  };
  boot.loader.efi.canTouchEfiVariables = lib.mkDefault false;
  boot.growPartition = lib.mkDefault true;

  # Google Cloud serial port logging and interactive web console
  boot.kernelParams = [
    "console=${serialPort},115200"
    "earlyprintk=${serialPort},115200"
  ];

  # Storage and network kernel drivers for Google Compute Engine
  boot.initrd.availableKernelModules = [
    "virtio_pci"
    "virtio_scsi"
    "virtio_net"
    "sd_mod"
    "nvme"
  ];

  services.qemuGuest.enable = lib.mkDefault true;

  networking.useDHCP = lib.mkDefault true;

  # Google Cloud VPC default MTU optimization.
  # Standard Google Cloud VPC networks use MTU 1460 (uncomment if applicable):
  # networking.interfaces.eth0.mtu = 1460;
}
