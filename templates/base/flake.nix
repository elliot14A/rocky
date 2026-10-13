{
  description = "rocky fleet deployment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";

    deploy-rs.url = "github:serokell/deploy-rs";
    deploy-rs.inputs.nixpkgs.follows = "nixpkgs";

    disko.url = "github:nix-community/disko";
    disko.inputs.nixpkgs.follows = "nixpkgs";

    agenix.url = "github:ryantm/agenix";
    agenix.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs =
    inputs@{
      self,
      nixpkgs,
      deploy-rs,
      ...
    }:
    let
      config = {
        defaultProvider = "hetzner";
      };

      fleet = import ./lib {
        inherit inputs config;
      };
    in
    {
      rockyFleet = true;
      inherit config;
      inherit (fleet) nixosConfigurations deploy;
    };
}
