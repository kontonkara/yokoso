{
  lib,
  stdenvNoCC,
  makeWrapper,
  quickshell,
  cage,
  jq,
  gawk,
  coreutils,
  gnugrep,
  systemd,
  greetd,
  getent,
  qt6,
  # Video backgrounds: Qt Multimedia, ffmpeg and GStreamer, a large closure
  # (over a gigabyte), so only when asked for.
  video ? false,
}:

stdenvNoCC.mkDerivation {
  pname = "yokoso";
  version = "0.1.0";

  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../bin
      ../shell
    ];
  };

  nativeBuildInputs = [ makeWrapper ];

  installPhase = ''
    runHook preInstall

    mkdir -p $out/share/yokoso
    cp -r shell $out/share/yokoso/shell
    install -Dm755 bin/yokoso $out/libexec/yokoso
    # From greetd: agreety is the fallback, fakegreet is for `yokoso preview`.
    makeWrapper $out/libexec/yokoso $out/bin/yokoso \
      --set-default YOKOSO_SHELL_DIR $out/share/yokoso/shell \
      --prefix PATH : ${
        lib.makeBinPath [
          quickshell
          cage
          jq
          gawk
          coreutils
          gnugrep
          systemd
          greetd
          getent
        ]
      } \
      ${lib.optionalString video "--prefix QML_IMPORT_PATH : ${qt6.qtmultimedia}/lib/qt-6/qml --prefix QT_PLUGIN_PATH : ${qt6.qtmultimedia}/lib/qt-6/plugins"}

    runHook postInstall
  '';

  meta = {
    description = "A greetd greeter on Quickshell";
    platforms = lib.platforms.linux;
    mainProgram = "yokoso";
  };
}
