let
  operators = {
    alice = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOperatorAliceKey";
    bob = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOperatorBobKey";
  };

  fleet = {
    hosts = {
      "001" = {
        services = [
          "postgres"
          "caddy"
        ];
      };
      "002" = {
        services = [
          "api"
        ];
      };
      "003" = {
        services = [
          "postgres"
        ];
      };
    };
  };

  hostKeys = {
    "001" = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIHost001Key";
    "002" = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIHost002Key";
  };

  recipients = import ../../templates/base/lib/secrets.nix {
    inherit operators fleet hostKeys;
  };

  postgresRecipients = recipients.recipientsFor [ "postgres" ];
  apiRecipients = recipients.recipientsFor [ "api" ];
  sharedRecipients = recipients.recipientsFor [
    "postgres"
    "api"
  ];
  tailscaleAuthKeyRecipients = recipients.allHosts;

  assertPostgresHasAlice = builtins.elem operators.alice postgresRecipients;
  assertPostgresHasBob = builtins.elem operators.bob postgresRecipients;
  assertPostgresHas001 = builtins.elem hostKeys."001" postgresRecipients;
  assertPostgresExcludes002 = !(builtins.elem hostKeys."002" postgresRecipients);

  assertApiHasAlice = builtins.elem operators.alice apiRecipients;
  assertApiHasBob = builtins.elem operators.bob apiRecipients;
  assertApiHas002 = builtins.elem hostKeys."002" apiRecipients;
  assertApiExcludes001 = !(builtins.elem hostKeys."001" apiRecipients);

  assertSharedHasBoth =
    (builtins.elem hostKeys."001" sharedRecipients) && (builtins.elem hostKeys."002" sharedRecipients);

  assertTailscaleHasAllOperators =
    (builtins.elem operators.alice tailscaleAuthKeyRecipients)
    && (builtins.elem operators.bob tailscaleAuthKeyRecipients);

  assertTailscaleHasKeyedHosts =
    (builtins.elem hostKeys."001" tailscaleAuthKeyRecipients)
    && (builtins.elem hostKeys."002" tailscaleAuthKeyRecipients);

  assertTailscaleOmitUnkeyed003 = (builtins.length tailscaleAuthKeyRecipients) == 4; # alice + bob + 001 + 002
  assertPostgresOmitUnkeyed003 = (builtins.length postgresRecipients) == 3; # alice + bob + 001
in
assert assertPostgresHasAlice;
assert assertPostgresHasBob;
assert assertPostgresHas001;
assert assertPostgresExcludes002;
assert assertApiHasAlice;
assert assertApiHasBob;
assert assertApiHas002;
assert assertApiExcludes001;
assert assertSharedHasBoth;
assert assertTailscaleHasAllOperators;
assert assertTailscaleHasKeyedHosts;
assert assertTailscaleOmitUnkeyed003;
assert assertPostgresOmitUnkeyed003;
{
  passed = true;
}
