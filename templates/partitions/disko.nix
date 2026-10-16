{ lib, ... }:

{
  disko.devices.disk.main = {
    type = "disk";

    # Target block device for installation.
    # Check your device name with 'lsblk' in rescue mode before running 'rocky up'.
    # Common values:
    #   - Standard Cloud VPS (Hetzner CPX, Oracle VM, GCP, AWS EBS): /dev/sda
    #   - VirtIO / KVM / Proxmox: /dev/vda
    #   - NVMe / Bare Metal (Hetzner CAX ARM, Hetzner Robot AX/EX, AWS Nitro): /dev/nvme0n1
    device = lib.mkDefault "/dev/sda";

    content = {
      type = "gpt";
      partitions = {
        # 1MB BIOS boot partition for GRUB MBR legacy boot fallback on GPT disks
        boot = {
          size = "1M";
          type = "EF02";
        };

        # 512MB EFI System Partition (ESP) for modern UEFI bootloaders
        esp = {
          size = "512M";
          type = "EF00";
          content = {
            type = "filesystem";
            format = "vfat";
            mountpoint = "/boot";
            mountOptions = [ "umask=0077" ];
          };
        };

        # Swap partition (recommended for small cloud instances with 1-4 GB RAM).
        # Uncomment if you want dedicated swap space:
        # swap = {
        #   size = "4G";
        #   content = {
        #     type = "swap";
        #     randomEncryption = true;
        #     resumeDevice = false;
        #   };
        # };

        # Root filesystem taking the remainder of the disk (Ext4 default)
        root = {
          size = "100%";
          content = {
            type = "filesystem";
            format = "ext4";
            mountpoint = "/";
          };
        };

        # Alternative Btrfs root layout with subvolumes (uncomment to replace ext4):
        # root = {
        #   size = "100%";
        #   content = {
        #     type = "btrfs";
        #     extraArgs = [ "-f" ];
        #     subvolumes = {
        #       "@" = {
        #         mountpoint = "/";
        #         mountOptions = [ "compress=zstd" "noatime" ];
        #       };
        #       "@nix" = {
        #         mountpoint = "/nix";
        #         mountOptions = [ "compress=zstd" "noatime" ];
        #       };
        #       "@persist" = {
        #         mountpoint = "/persist";
        #         mountOptions = [ "compress=zstd" "noatime" ];
        #       };
        #     };
        #   };
        # };

        # Alternative LUKS encrypted root layout (uncomment to replace ext4):
        # root = {
        #   size = "100%";
        #   content = {
        #     type = "luks";
        #     name = "crypted";
        #     content = {
        #       type = "filesystem";
        #       format = "ext4";
        #       mountpoint = "/";
        #     };
        #   };
        # };
      };
    };
  };

  # Secondary attached storage volume (e.g. for database or container state).
  # Uncomment and adjust when attaching an extra block volume:
  # disko.devices.disk.data = {
  #   type = "disk";
  #   device = "/dev/sdb";
  #   content = {
  #     type = "gpt";
  #     partitions = {
  #       data = {
  #         size = "100%";
  #         content = {
  #           type = "filesystem";
  #           format = "ext4";
  #           mountpoint = "/var/lib/data";
  #         };
  #       };
  #     };
  #   };
  # };
}
