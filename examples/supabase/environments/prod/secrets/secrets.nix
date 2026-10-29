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
{
  "jwt.age".publicKeys = recipientsFor [
    "postgrest"
    "storage"
    "gotrue"
    "studio"
  ];
  "postgres-creds.age".publicKeys = recipientsFor [
    "postgres"
    "pgdog"
    "postgrest"
    "storage"
    "gotrue"
    "meta"
  ];
  "pgdog-users.age".publicKeys = recipientsFor [ "pgdog" ];
  "rustic.age".publicKeys = recipientsFor [ "rustic" ];
  "tailscale-authkey.age".publicKeys = allHosts;
}
