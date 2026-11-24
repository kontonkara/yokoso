{
  description = "yokoso: a desktop shell for Wayland";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

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

      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          packages = with pkgs; [
            cargo
            rustc
            clippy
            rustfmt
            rust-analyzer
            pkg-config
            # The stand: a headless sway with niri in it, and pictures of it.
            sway
            niri
            grim
            imagemagick
            jq
            wlrctl
            wtype
            dbus
          ];
          buildInputs = with pkgs; [
            wayland
            libxkbcommon
          ];
          # EGL and GLES are opened at run time, through libglvnd.
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (
            with pkgs;
            [
              libglvnd
              wayland
              libxkbcommon
              fontconfig
            ]
          );
        };
      });
    };
}
