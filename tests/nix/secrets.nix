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
      "004" = {
        services = [
          { postgres.replicas = 0; }
        ];
      };
      "005" = { };
    };
  };

  hostKeys = {
    "001" = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIHost001Key";
    "002" = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIHost002Key";
    "004" = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIHost004Key";
    "005" = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIHost005Key";
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
  assertPostgresExcludesDrained004 = !(builtins.elem hostKeys."004" postgresRecipients);
  assertPostgresExcludesBare005 = !(builtins.elem hostKeys."005" postgresRecipients);

  assertAllHostsHasDrained004 = builtins.elem hostKeys."004" tailscaleAuthKeyRecipients;
  assertAllHostsHasBare005 = builtins.elem hostKeys."005" tailscaleAuthKeyRecipients;

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

  assertTailscaleOmitUnkeyed003 = (builtins.length tailscaleAuthKeyRecipients) == 6; # alice + bob + 001 + 002 + 004 + 005
  assertPostgresOmitUnkeyed003 = (builtins.length postgresRecipients) == 3; # alice + bob + 001
in
assert assertPostgresHasAlice;
assert assertPostgresHasBob;
assert assertPostgresHas001;
assert assertPostgresExcludes002;
assert assertPostgresExcludesDrained004;
assert assertPostgresExcludesBare005;
assert assertAllHostsHasDrained004;
assert assertAllHostsHasBare005;
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
