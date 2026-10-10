self:
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.services.yokoso;
  json = pkgs.formats.json { };
  xkb = config.services.xserver.xkb;
  sessions = config.services.displayManager.sessionData.desktops;
in
{
  options = {
    services = {
      yokoso = {
        enable = lib.mkEnableOption "the yokoso greeter on greetd";

        package = lib.mkOption {
          type = lib.types.package;
          default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
          description = "The yokoso package.";
        };

        settings = lib.mkOption {
          inherit (json) type;
          default = { };
          description = "yokoso's config.json; unset keys keep their defaults (see the README).";
        };
      };
    };
  };

  config = lib.mkIf cfg.enable {
    services = {
      greetd = {
        enable = true;
        settings = {
          default_session = {
            # The console's keyboard layout: the password is typed in it.
            command = lib.concatStringsSep " " [
              "env"
              "YOKOSO_SESSION_DIRS=${sessions}/share/wayland-sessions:${sessions}/share/xsessions"
              "XKB_DEFAULT_LAYOUT=${lib.escapeShellArg xkb.layout}"
              "XKB_DEFAULT_VARIANT=${lib.escapeShellArg xkb.variant}"
              "XKB_DEFAULT_OPTIONS=${lib.escapeShellArg xkb.options}"
              (lib.getExe cfg.package)
            ];
          };
        };
      };

      xserver = {
        displayManager = {
          # X11 sessions start through startx (config.x11Wrapper), which
          # brings X up with NixOS's X server config.
          startx = {
            enable = lib.mkIf config.services.xserver.enable (lib.mkDefault true);
          };
        };
      };
    };

    environment = {
      etc = {
        "yokoso/config.json".source = json.generate "yokoso-config.json" cfg.settings;
      };
    };
  };
}
