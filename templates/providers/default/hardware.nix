{
  pkgs,
  lib,
  modulesPath,
  ...
}:

let
  # Serial console port based on CPU architecture:
  # x86_64 uses ttyS0, aarch64 (ARM Ampere) uses ttyAMA0
  serialPort = if pkgs.stdenv.hostPlatform.isAarch64 then "ttyAMA0" else "ttyS0";
in
{
  imports = [ (modulesPath + "/profiles/qemu-guest.nix") ];

  # Bootloader configuration: GRUB in EFI mode (default)
  boot.loader.systemd-boot.enable = lib.mkDefault false;
  boot.loader.grub = {
    enable = lib.mkDefault true;
    # "nodev" tells GRUB to install into the EFI partition (/boot) without touching raw disk sectors.
    # Change to a specific disk (e.g. [ "/dev/sda" ]) if booting legacy BIOS without UEFI.
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

  # Alternative systemd-boot bootloader (uncomment to use systemd-boot instead of GRUB):
  # boot.loader.systemd-boot.enable = lib.mkForce true;
  # boot.loader.grub.enable = lib.mkForce false;
  # boot.loader.efi.canTouchEfiVariables = lib.mkForce true;

  # Serial console kernel parameters for cloud dashboard emergency web consoles
  boot.kernelParams = [
    "console=tty1"
    "console=${serialPort},115200"
  ];

  # Common storage and network kernel drivers for virtualized and physical servers
  boot.initrd.availableKernelModules = [
    "virtio_pci"
    "virtio_scsi"
    "virtio_net"
    "ahci"
    "sd_mod"
    "nvme"
  ];

  # QEMU guest agent for hypervisor integration (graceful shutdown, memory ballooning, IP reporting)
  services.qemuGuest.enable = lib.mkDefault true;

  networking.useDHCP = lib.mkDefault true;

  # Bare-metal physical server conversion:
  # If running directly on physical hardware without a hypervisor:
  # 1. Disable QEMU guest agent:
  #    services.qemuGuest.enable = lib.mkForce false;
  # 2. Enable real motherboard UEFI NVRAM:
  #    boot.loader.efi.canTouchEfiVariables = lib.mkForce true;
  # 3. Enable CPU microcode updates (uncomment for your CPU vendor):
  #    hardware.cpu.amd.updateMicrocode = true;
  #    hardware.cpu.intel.updateMicrocode = true;
}
