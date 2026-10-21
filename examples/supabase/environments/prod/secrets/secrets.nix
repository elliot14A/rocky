let
  operators = import ../../../keys.nix;
  fleet = import ../fleet.nix;
  hostKeys = import ../keys.nix;

  inherit
    (import ../../../lib/secrets.nix {
      inherit operators fleet hostKeys;
    })
    recipientsFor
    allHosts
    ;
in
{ }
