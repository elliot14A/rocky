let
  flake = builtins.getFlake (toString ../..);
  pkgs = flake.inputs.nixpkgs.legacyPackages.x86_64-linux;
  agenix = flake.inputs.agenix;
  tls-cert = pkgs.runCommand "selfSignedCerts" { buildInputs = [ pkgs.openssl ]; } ''
    openssl req \
      -x509 -newkey rsa:4096 -sha256 -days 365 \
      -nodes -out cert.pem -keyout key.pem \
      -subj '/CN=headscale' -addext "subjectAltName=DNS:headscale"
    mkdir -p $out
    cp key.pem cert.pem $out
  '';
in
pkgs.testers.runNixOSTest {
  name = "rocky-mesh-test";

  nodes = {
    headscale = { ... }: {
      services.headscale = {
        enable = true;
        port = 8080;
        settings = {
          server_url = "https://headscale";
          prefixes.v4 = "100.64.0.0/10";
          derp = {
            urls = [ ];
            server = {
              enabled = true;
              region_id = 999;
              stun_listen_addr = "0.0.0.0:3478";
            };
          };
          dns = {
            base_domain = "rocky.mesh";
            magic_dns = true;
            override_local_dns = false;
          };
        };
      };
      services.nginx = {
        enable = true;
        virtualHosts.headscale = {
          addSSL = true;
          sslCertificate = "${tls-cert}/cert.pem";
          sslCertificateKey = "${tls-cert}/key.pem";
          locations."/" = {
            proxyPass = "http://127.0.0.1:8080";
            proxyWebsockets = true;
          };
        };
      };
      networking.firewall.allowedTCPPorts = [ 80 443 ];
      networking.firewall.allowedUDPPorts = [ 3478 ];
      environment.systemPackages = [ pkgs.headscale ];
    };

    ingress = { ... }: {
      imports = [
        agenix.nixosModules.default
        ../../templates/base/system/tailscale.nix
        ../../templates/base/system/firewall.nix
      ];
      _module.args = {
        environment = "staging";
        fleet = {
          settings = {
            tailscaleLoginServer = "https://headscale";
          };
        };
      };
      security.pki.certificateFiles = [ "${tls-cert}/cert.pem" ];
      services.caddy = {
        enable = true;
        extraConfig = ''
          :80 {
            reverse_proxy http://staging-node-02:8000
          }
        '';
      };
      networking.firewall.allowedTCPPorts = [ 80 ];
    };

    backend = { ... }: {
      imports = [
        agenix.nixosModules.default
        ../../templates/base/system/tailscale.nix
        ../../templates/base/system/firewall.nix
      ];
      _module.args = {
        environment = "staging";
        fleet = {
          settings = {
            tailscaleLoginServer = "https://headscale";
          };
        };
      };
      security.pki.certificateFiles = [ "${tls-cert}/cert.pem" ];
      systemd.services.api = {
        wantedBy = [ "multi-user.target" ];
        serviceConfig.ExecStart = "${pkgs.python3Minimal}/bin/python3 -m http.server 8000";
      };
    };
  };

  testScript = ''
    start_all()
    headscale.wait_for_unit("headscale")
    headscale.wait_for_open_port(443)
    headscale.succeed("headscale users create test")
    authkey = headscale.succeed("headscale preauthkeys create -u 1 --reusable").strip()

    backend.wait_for_unit("tailscaled")
    backend.succeed(f"tailscale up --login-server=https://headscale --auth-key={authkey} --hostname=staging-node-02")
    backend.wait_for_unit("api")

    ingress.wait_for_unit("tailscaled")
    ingress.succeed(f"tailscale up --login-server=https://headscale --auth-key={authkey} --hostname=staging-node-01")
    ingress.wait_for_unit("caddy")

    ingress.wait_until_succeeds("tailscale ping staging-node-02")

    hosts = ingress.succeed("cat /etc/hosts")
    assert "staging-node-02" not in hosts, "staging-node-02 must resolve via MagicDNS, not /etc/hosts"

    ingress.wait_until_succeeds("curl -fsS http://127.0.0.1:80/")

    backend_ip = backend.succeed("ip -4 addr show eth1 | grep -oP '(?<=inet\\s)\\d+(\\.\\d+){3}'").strip()
    ingress.fail(f"curl --connect-timeout 2 http://{backend_ip}:8000/")
  '';
}
