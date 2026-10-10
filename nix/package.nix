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
        ]
      }

    runHook postInstall
  '';

  meta = {
    description = "A greetd greeter on Quickshell";
    platforms = lib.platforms.linux;
    mainProgram = "yokoso";
  };
}
