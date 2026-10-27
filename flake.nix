{
  description = "yokoso: a customizable greetd greeter on Quickshell";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs =
    { self, nixpkgs }:
    let
      forAllSystems =
        f:
        nixpkgs.lib.genAttrs [
          "x86_64-linux"
          "aarch64-linux"
        ] (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = forAllSystems (pkgs: {
        default = pkgs.callPackage ./nix/package.nix { };
      });

      apps = forAllSystems (
        pkgs:
        let
          # What the headless shots run with.
          shooting = with pkgs; [
            quickshell
            cage
            jq
            gawk
            coreutils
            gnugrep
            gnused
            systemd
            greetd
            wtype
            wlrctl
            imagemagick
            util-linux
          ];
        in
        {
          # nix run .#preview: the greeter in a window, logging in against
          # fakegreet (password "password", then "9").
          preview = {
            type = "app";
            program = toString (
              pkgs.writeShellScript "yokoso-preview" ''
                exec ${nixpkgs.lib.getExe self.packages.${pkgs.stdenv.hostPlatform.system}.default} preview
              ''
            );
          };

          # nix run .#shots -- <out>: the working tree's greeter, headless, in
          # tests/shots.sh's scenarios; with Qt Multimedia, for a video, ffmpeg
          # to make one, and Qt Virtual Keyboard.
          shots = {
            type = "app";
            program = nixpkgs.lib.getExe (
              pkgs.writeShellApplication {
                name = "yokoso-shots";
                runtimeInputs = shooting ++ [ pkgs.ffmpeg-headless ];
                text = ''
                  export QML_IMPORT_PATH=${pkgs.qt6.qtmultimedia}/lib/qt-6/qml:${pkgs.qt6.qtvirtualkeyboard}/lib/qt-6/qml
                  export QT_PLUGIN_PATH=${pkgs.qt6.qtmultimedia}/lib/qt-6/plugins:${pkgs.qt6.qtvirtualkeyboard}/lib/qt-6/plugins
                  exec sh ${./tests/shots.sh} "$@"
                '';
              }
            );
          };

          # nix run .#docs-shots: the documentation's pictures in docs/images,
          # from the working tree.
          docs-shots = {
            type = "app";
            program = nixpkgs.lib.getExe (
              pkgs.writeShellApplication {
                name = "yokoso-docs-shots";
                runtimeInputs = shooting;
                text = ''exec sh ${./docs/shots.sh} "$@"'';
              }
            );
          };
        }
      );

      # nix flake check: the VM tests in tests/vm, shellcheck on the
      # scripts, and the schemas: valid, with every key of the core's
      # defaults and of the dark theme, and the themes and examples
      # passing.
      checks.x86_64-linux =
        let
          pkgs = nixpkgs.legacyPackages.x86_64-linux;
        in
        import ./tests/vm { inherit pkgs self; }
        // {
          shellcheck = pkgs.runCommand "yokoso-shellcheck" { nativeBuildInputs = [ pkgs.shellcheck ]; } ''
            shellcheck -s sh ${./bin/yokoso} ${./tests/shots.sh} ${./docs/shots.sh}
            touch $out
          '';
          schema =
            pkgs.runCommand "yokoso-schema"
              {
                nativeBuildInputs = [
                  pkgs.check-jsonschema
                  pkgs.jq
                ];
              }
              ''
                s=${./shell}
                check-jsonschema --check-metaschema $s/schema/*.json
                check-jsonschema --schemafile $s/schema/theme.json $s/themes/*.json ${./examples}/*.json
                # The defaults' keys, one per line between "defaults: ({" and "})".
                awk '/readonly property var defaults: \(\{/ { on = 1; next } on && /^\t\}\)/ { exit }
                  on && /^\t\t[A-Za-z0-9]+:/ { sub(/^\t\t/, ""); sub(/:.*/, ""); print }' $s/shell.qml >defaults
                [ "$(wc -l <defaults)" -gt 10 ] || { echo "no defaults found in shell.qml"; exit 1; }
                jq -r '(keys[] | select(IN("colors", "layout") | not)),
                  (("colors", "layout") as $g | .[$g] | keys[] | "\($g).\(.)")' $s/themes/dark.json >theme
                missing=$(cat defaults theme | while read -r key; do
                  case $key in
                  colors.* | layout.*) path=".properties.''${key%%.*}.properties.''${key#*.}" ;;
                  *) path=".properties.$key" ;;
                  esac
                  jq -n -e "(input | $path) // (input | $path)" $s/schema/config.json $s/schema/theme.json >/dev/null ||
                    echo "$key"
                done)
                [ -z "$missing" ] || { echo "not in the schemas:" $missing; exit 1; }
                touch $out
              '';
        };

      nixosModules.default = import ./nix/module.nix self;
    };
}
