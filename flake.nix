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

      # nix run .#preview: the greeter in a window, logging in against
      # fakegreet (password "password", then "9").
      apps = forAllSystems (pkgs: {
        preview = {
          type = "app";
          program = toString (
            pkgs.writeShellScript "yokoso-preview" ''
              exec ${nixpkgs.lib.getExe self.packages.${pkgs.stdenv.hostPlatform.system}.default} preview
            ''
          );
        };
      });

      nixosModules.default = import ./nix/module.nix self;
    };
}
