{
  pkgs,
  lib,
  modulesPath,
  ...
}:

let
  # Hetzner CAX (Ampere A1 ARM) uses ttyAMA0; Hetzner CX/CPX/CCX (x86_64) uses ttyS0
  serialPort = if pkgs.stdenv.hostPlatform.isAarch64 then "ttyAMA0" else "ttyS0";
in
{
  imports = [ (modulesPath + "/profiles/qemu-guest.nix") ];

  boot.loader.systemd-boot.enable = lib.mkDefault false;
  boot.loader.grub = {
    enable = lib.mkDefault true;
    # Hetzner Cloud x86 instances use "/dev/sda" by default.
    # For Hetzner CAX (ARM) or Dedicated Bare Metal (AX/EX with NVMe), change to [ "nodev" ] or [ "/dev/nvme0n1" ].
    devices = lib.mkDefault [ "/dev/sda" ];
    efiSupport = lib.mkDefault true;
    efiInstallAsRemovable = lib.mkDefault true;
    configurationLimit = lib.mkDefault 10;
  };
  boot.loader.efi.canTouchEfiVariables = lib.mkDefault false;
  boot.growPartition = lib.mkDefault true;

  # Serial console for Hetzner Cloud web dashboard emergency console
  boot.kernelParams = [
    "console=tty1"
    "console=${serialPort},115200"
  ];

  # Storage and network kernel drivers for Hetzner Cloud VMs
  boot.initrd.availableKernelModules = [
    "virtio_pci"
    "virtio_scsi"
    "virtio_net"
    "sd_mod"
  ];

  services.qemuGuest.enable = lib.mkDefault true;

  networking.useDHCP = lib.mkDefault true;

  # Hetzner Cloud Networks (vSwitch / VXLAN) MTU optimization.
  # Uncomment if connecting to a Hetzner Cloud private network:
  # networking.interfaces.enp7s0.mtu = 1450;

  # Hetzner Dedicated Server (AX/EX bare-metal robot servers) conversion:
  # If deploying on bare metal instead of Hetzner Cloud:
  # 1. Disable QEMU guest agent:
  #    services.qemuGuest.enable = lib.mkForce false;
  # 2. Enable CPU microcode updates (most AX servers run AMD Ryzen):
  #    hardware.cpu.amd.updateMicrocode = true;
  # 3. Allow touching EFI variables on bare metal:
  #    boot.loader.efi.canTouchEfiVariables = lib.mkForce true;
  # 4. Include real hardware modules for NVMe and physical NICs:
  #    boot.initrd.availableKernelModules = [ "nvme" "ahci" "igb" "r8169" ];
}
