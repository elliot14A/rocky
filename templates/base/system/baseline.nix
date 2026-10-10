{ pkgs, ... }:

{
  time.timeZone = "UTC";
  i18n.defaultLocale = "en_US.UTF-8";

  environment.systemPackages = with pkgs; [
    curl
    jq
    rsync
    dnsutils
  ];

  programs.neovim = {
    enable = true;
    defaultEditor = true;
    viAlias = true;
    vimAlias = true;
    configure.customRC = ''
      set number
      set cursorline
      set expandtab
      set shiftwidth=2
      set softtabstop=2
      set tabstop=2
      set smartindent
      set ignorecase
      set smartcase
      set incsearch
      set hlsearch
      set undofile
      syntax on
      filetype plugin indent on
    '';
  };

  services.fail2ban.enable = true;
  zramSwap.enable = true;
  boot.tmp.cleanOnBoot = true;

  services.journald.extraConfig = ''
    SystemMaxUse=500M
  '';

  boot.kernel.sysctl = {
    "net.ipv4.tcp_syncookies" = 1;
    "net.ipv4.conf.all.rp_filter" = 1;
    "net.ipv4.conf.default.rp_filter" = 1;
    "net.ipv4.conf.all.accept_redirects" = 0;
    "net.ipv4.conf.default.accept_redirects" = 0;
    "net.ipv4.conf.all.log_martians" = 1;
  };

  nix.settings = {
    experimental-features = [
      "nix-command"
      "flakes"
    ];
    auto-optimise-store = true;
    trusted-users = [
      "root"
      "rocky"
    ];
  };

  nix.gc = {
    automatic = true;
    dates = "weekly";
    options = "--delete-older-than 14d";
  };
}
