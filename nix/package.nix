{
  lib,
  rustPlatform,
  pkg-config,
  makeWrapper,
  wayland,
  libxkbcommon,
  libglvnd,
  fontconfig,
}:

rustPlatform.buildRustPackage {
  pname = "yokoso";
  version = "0.0.0";

  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      ../crates
      ../dist
    ];
  };

  cargoLock.lockFile = ../Cargo.lock;

  nativeBuildInputs = [
    pkg-config
    makeWrapper
  ];
  buildInputs = [
    wayland
    libxkbcommon
  ];

  # The stand needs a compositor and a GPU: its tests run there, not here.
  doCheck = false;

  postInstall = ''
    install -Dm644 dist/yokoso.service $out/lib/systemd/user/yokoso.service
    substituteInPlace $out/lib/systemd/user/yokoso.service --replace-fail /usr/bin/yokoso $out/bin/yokoso
    install -Dm644 dist/yokoso-fallback.service $out/lib/systemd/user/yokoso-fallback.service
  '';

  # EGL and GLES are opened at run time, through libglvnd; fontconfig too.
  postFixup = ''
    wrapProgram $out/bin/yokoso \
      --prefix LD_LIBRARY_PATH : ${
        lib.makeLibraryPath [
          libglvnd
          wayland
          libxkbcommon
          fontconfig
        ]
      }
  '';

  meta = {
    description = "A desktop shell for Wayland";
    license = lib.licenses.mit;
    mainProgram = "yokoso";
    platforms = lib.platforms.linux;
  };
}
