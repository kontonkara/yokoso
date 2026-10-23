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

      apps = forAllSystems (pkgs: {
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
        # tests/shots.sh's scenarios.
        shots = {
          type = "app";
          program = nixpkgs.lib.getExe (
            pkgs.writeShellApplication {
              name = "yokoso-shots";
              runtimeInputs = with pkgs; [
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
              ];
              text = ''exec sh ${./tests/shots.sh} "$@"'';
            }
          );
        };
      });

      # nix flake check: the VM tests in tests/vm, and shellcheck on the
      # scripts.
      checks.x86_64-linux =
        let
          pkgs = nixpkgs.legacyPackages.x86_64-linux;
        in
        import ./tests/vm { inherit pkgs self; }
        // {
          shellcheck = pkgs.runCommand "yokoso-shellcheck" { nativeBuildInputs = [ pkgs.shellcheck ]; } ''
            shellcheck -s sh ${./bin/yokoso} ${./tests/shots.sh}
            touch $out
          '';
        };

      nixosModules.default = import ./nix/module.nix self;
    };
}
