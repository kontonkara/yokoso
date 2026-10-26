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
          description = "yokoso's config.json; unset keys keep their defaults (see docs/config.md).";
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

        cursor = {
          package = lib.mkOption {
            type = lib.types.nullOr lib.types.package;
            default = null;
            example = lib.literalExpression "pkgs.bibata-cursors";
            description = "The cursor theme's package, for its share/icons; null keeps the system's default theme.";
          };

          name = lib.mkOption {
            type = lib.types.str;
            default = "";
            example = "Bibata-Modern-Classic";
            description = "The cursor theme in the package.";
          };

          size = lib.mkOption {
            type = lib.types.ints.positive;
            default = 24;
            description = "The cursor's size at scale 1; yokoso scales it with the UI.";
          };
        };

        avatars = lib.mkOption {
          type = lib.types.attrsOf lib.types.path;
          default = { };
          example = lib.literalExpression "{ alice = ./alice.png; }";
          description = "Pictures by user name, as /etc/yokoso/avatars/<name>; AccountsService's are used for the rest.";
        };

        backgrounds = lib.mkOption {
          type = lib.types.attrsOf lib.types.path;
          default = { };
          example = lib.literalExpression "{ alice = ./alice-wallpaper.jpg; }";
          description = "Backgrounds by user name, as /etc/yokoso/backgrounds/<name>, over the theme's while that user is picked.";
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
            # The console's keyboard layout (the password is typed in it)
            # and the cursor theme.
            command = lib.concatStringsSep " " (
              [
                "env"
                "YOKOSO_SESSION_DIRS=${sessions}/share/wayland-sessions:${sessions}/share/xsessions"
                "XKB_DEFAULT_LAYOUT=${lib.escapeShellArg xkb.layout}"
                "XKB_DEFAULT_VARIANT=${lib.escapeShellArg xkb.variant}"
                "XKB_DEFAULT_OPTIONS=${lib.escapeShellArg xkb.options}"
              ]
              ++ lib.optionals (cfg.cursor.package != null) [
                "XCURSOR_PATH=${cfg.cursor.package}/share/icons"
                "XCURSOR_THEME=${lib.escapeShellArg cfg.cursor.name}"
              ]
              ++ [
                "XCURSOR_SIZE=${toString cfg.cursor.size}"
                (lib.getExe cfg.package)
              ]
            );
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
      ) cfg.themes
      // lib.mapAttrs' (
        name: picture: lib.nameValuePair "yokoso/avatars/${name}" { source = picture; }
      ) cfg.avatars
      // lib.mapAttrs' (
        name: picture: lib.nameValuePair "yokoso/backgrounds/${name}" { source = picture; }
      ) cfg.backgrounds;
    };

    systemd = {
      # greetd's "idle" keeps a text greeter out of the boot messages by
      # holding it back until the rest of the boot is done (seconds, when a
      # service waits for the network). cage takes the screen, so yokoso
      # starts as soon as greetd does.
      services.greetd.serviceConfig.Type = lib.mkForce "simple";

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
