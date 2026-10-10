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
  greeter = config.services.greetd.settings.default_session.user;
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

        themes = lib.mkOption {
          type = lib.types.attrsOf json.type;
          default = { };
          example = {
            mine = {
              extends = "dark";
              colors.accent = "#fab387";
            };
          };
          description = "Themes by name, as /etc/yokoso/themes/<name>.json; settings.theme picks one.";
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
      }
      // lib.mapAttrs' (
        name: theme:
        lib.nameValuePair "yokoso/themes/${name}.json" {
          source = json.generate "yokoso-theme-${name}.json" theme;
        }
      ) cfg.themes;
    };

    systemd = {
      tmpfiles = {
        settings = {
          yokoso = {
            # Mesa's shader cache and Qt's, kept between boots.
            "/var/cache/yokoso".d = {
              user = greeter;
              group = config.users.users.${greeter}.group;
              mode = "0700";
            };
            # The last login's user and session.
            "/var/lib/yokoso".d = {
              user = greeter;
              group = config.users.users.${greeter}.group;
              mode = "0700";
            };
          };
        };
      };
    };
  };
}
