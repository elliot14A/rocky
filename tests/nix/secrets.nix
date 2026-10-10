let
  operators = {
    alice = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOperatorAliceKey";
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
  allHostRecipients = recipients.allHosts;

  assertPostgresHasAlice = builtins.elem operators.alice postgresRecipients;
  assertPostgresHas001 = builtins.elem hostKeys."001" postgresRecipients;
  assertPostgresExcludes002 = !(builtins.elem hostKeys."002" postgresRecipients);

  assertApiHasAlice = builtins.elem operators.alice apiRecipients;
  assertApiHas002 = builtins.elem hostKeys."002" apiRecipients;
  assertApiExcludes001 = !(builtins.elem hostKeys."001" apiRecipients);

  assertSharedHasBoth =
    (builtins.elem hostKeys."001" sharedRecipients) && (builtins.elem hostKeys."002" sharedRecipients);

  assertAllHosts =
    (builtins.elem operators.alice allHostRecipients)
    && (builtins.elem hostKeys."001" allHostRecipients)
    && (builtins.elem hostKeys."002" allHostRecipients);

  assertHost003Omitted = (builtins.length postgresRecipients) == 2;
in
assert assertPostgresHasAlice;
assert assertPostgresHas001;
assert assertPostgresExcludes002;
assert assertApiHasAlice;
assert assertApiHas002;
assert assertApiExcludes001;
assert assertSharedHasBoth;
assert assertAllHosts;
assert assertHost003Omitted;
{
  passed = true;
}
