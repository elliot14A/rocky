let
  makeTailscaleConfig = { loginServer ? null }:
    let
      mockFleet = {
        settings = if loginServer != null then { tailscaleLoginServer = loginServer; } else { };
      };
      eval = import ../../templates/base/system/tailscale.nix {
        config = {
          age.secrets."tailscale-authkey".path = "/run/agenix/tailscale-authkey";
        };
        environment = "staging";
        fleet = mockFleet;
      };
    in eval;

  defaultConfig = makeTailscaleConfig { };
  headscaleConfig = makeTailscaleConfig { loginServer = "http://headscale.local:8080"; };

  hasDefaultNoLoginFlag = !(builtins.elem "--login-server=http://headscale.local:8080" (defaultConfig.services.tailscale.extraUpFlags or [ ]));
  hasHeadscaleLoginFlag = builtins.elem "--login-server=http://headscale.local:8080" headscaleConfig.services.tailscale.extraUpFlags;
  hasSshFlag = builtins.elem "--ssh" (defaultConfig.services.tailscale.extraUpFlags or [ ]);
in
assert hasDefaultNoLoginFlag;
assert hasHeadscaleLoginFlag;
assert hasSshFlag;
{
  passed = true;
}
