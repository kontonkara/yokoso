# The VM tests, run by `nix flake check`: the greeter under the real greetd,
# PAM and cage in a NixOS VM (1280x800), driven by the keyboard and by
# wlrctl's pointer.
{ pkgs, self }:

let
  # yokoso, an X11 session (i3) to log in to, alice with the password
  # "foobar", and whatever the test adds.
  test =
    name: machine: script:
    pkgs.testers.runNixOSTest {
      name = "yokoso-${name}";
      nodes.machine = {
        imports = [
          self.nixosModules.default
          machine
        ];
        services.yokoso = {
          enable = true;
          settings.scale = 1.0;
        };
        services.xserver = {
          enable = true;
          windowManager.i3.enable = true;
        };
        users.users.alice = {
          isNormalUser = true;
          password = "foobar";
        };
        environment.systemPackages = [ pkgs.wlrctl ];
        virtualisation.memorySize = 2048;
      };
      testScript = helpers + script;
    };

  helpers = ''
    import time

    # wlrctl against the greeter's compositor.
    def wl(cmd):
        machine.succeed("cd /run/user/$(id -u greeter) && XDG_RUNTIME_DIR=$PWD WAYLAND_DISPLAY=$(ls wayland-* | grep -v lock | head -1) wlrctl " + cmd)

    def click(x, y):
        wl("pointer move -5000 -5000")
        wl(f"pointer move {x} {y}")
        time.sleep(0.3)
        wl("pointer click left")
        time.sleep(0.8)

    # A greeter that has asked for the password, not the one before it; its
    # Quickshell's pid.
    def greeter(old=""):
        machine.wait_until_succeeds(f"pgrep -u greeter -n quickshell | grep -vx '{old}'", timeout=60)
        machine.wait_until_succeeds("journalctl -b -t yokoso | grep -q auth_message", timeout=60)
        time.sleep(4)
        return machine.succeed("pgrep -u greeter -n quickshell").strip()

    def logged_in(user, session="i3"):
        machine.wait_until_succeeds(f"pgrep -u {user} '{session}'", timeout=90)

    def logout(user):
        machine.succeed(f"loginctl terminate-user {user}")
        machine.wait_until_fails(f"pgrep -u {user}", timeout=30)

    # The greeter's warnings (Quickshell colors them), none of them QML's
    # (yokoso's own aside).
    def quiet():
        out = machine.succeed(r"journalctl -b -t yokoso -o cat | sed 's/\x1b\[[0-9;]*m//g' | grep -E 'WARN|CRIT' | grep -vE 'Connection refused|unexpected greetd response|qml: yokoso:' || true")
        print(out)
        assert not any(word in out for word in ("WARN scene", "WARN qml", "CRIT")), "QML complained"

  '';
in
{
  # The pointer picks bob and the second session; a wrong password, then
  # the right one; after logging out the greeter comes back with bob and
  # that session; F2, Up, Enter pick alice.
  pick =
    test "pick"
      {
        services.xserver.windowManager.openbox.enable = true;
        users.users.bob = {
          isNormalUser = true;
          password = "barfoo";
        };
      }
      ''
        import json

        def proc(session):
            return session["id"].split("+")[-1].removesuffix(".desktop")

        machine.wait_for_unit("greetd.service")
        pid = greeter()
        sessions = json.loads(machine.succeed("cat /run/user/$(id -u greeter)/yokoso/env.json"))["sessions"]
        machine.screenshot("start")

        click(640, 415)
        machine.screenshot("users")
        click(640, 503)
        time.sleep(1)
        click(640, 458)
        machine.screenshot("sessions")
        click(640, 547)

        machine.send_chars("nope\n")
        time.sleep(4)
        machine.screenshot("wrong")
        machine.send_chars("barfoo\n")
        logged_in("bob", "i3|openbox")
        assert proc(sessions[1]) in machine.succeed("pgrep -u bob -l"), "bob got the wrong session"
        machine.succeed("grep -q '\"user\":\"bob\"' /var/lib/yokoso/state.json")

        logout("bob")
        pid = greeter(pid)
        machine.screenshot("remembered")
        machine.send_key("f2")
        time.sleep(1)
        machine.send_key("up")
        machine.send_key("ret")
        time.sleep(1.5)
        machine.send_chars("foobar\n")
        logged_in("alice", "i3|openbox")
        assert proc(sessions[0]) in machine.succeed("pgrep -u alice -l"), "alice got the wrong session"
        print(machine.succeed("cat /var/lib/yokoso/state.json"))
        quiet()
      '';

  # Idle (3 s): dark, the pointer too; the click that wakes it opens
  # nothing, the next one does; the key that wakes it isn't typed, so the
  # password after it logs in.
  idle =
    test "idle"
      {
        services.yokoso.settings.idle = 3;
        users.users.bob = {
          isNormalUser = true;
          password = "barfoo";
        };
      }
      ''
        machine.wait_for_unit("greetd.service")
        greeter()
        # On the avatar, so that a click opens the user list.
        wl("pointer move -5000 -5000")
        wl("pointer move 640 401")
        time.sleep(4.5)
        machine.screenshot("asleep")
        wl("pointer click left")
        time.sleep(1)
        machine.screenshot("woken")
        wl("pointer click left")
        time.sleep(1)
        machine.screenshot("list")
        machine.send_key("esc")
        time.sleep(4.5)
        machine.send_key("a")
        time.sleep(0.5)
        machine.send_chars("foobar\n")
        logged_in("alice")
        quiet()
      '';

  # One click arms "Power off" (a ring runs out around it), it disarms by
  # itself, and a login still works. logind offers power only with polkit;
  # the cursor is Bibata, over the buttons a hand.
  power =
    test "power"
      {
        security.polkit.enable = true;
        # A theme of its own, to see it on the buttons.
        services.yokoso.cursor = {
          package = pkgs.bibata-cursors;
          name = "Bibata-Modern-Classic";
        };
      }
      ''
        import json

        machine.wait_for_unit("greetd.service")
        greeter()
        power = json.loads(machine.succeed("cat /run/user/$(id -u greeter)/yokoso/env.json"))["power"]
        assert "poweroff" in power, f"no power off in {power}"
        width = len(power) * 48 + (len(power) - 1) * 14
        x = 640 - width // 2 + 24 + power.index("poweroff") * 62
        wl("pointer move -5000 -5000")
        wl(f"pointer move {x} 736")
        time.sleep(0.5)
        machine.screenshot("hover")
        time.sleep(0.5)
        wl("pointer click left")
        time.sleep(1.2)
        machine.screenshot("armed")
        time.sleep(2.5)
        machine.screenshot("disarmed")
        machine.send_chars("foobar\n")
        logged_in("alice")
        quiet()
      '';

  # PAM's info message (pam_unix's expiry warning) on the way in: Quickshell
  # acknowledges it and the login goes on.
  info = test "info" { } ''
    machine.wait_for_unit("greetd.service")
    greeter()
    # Changed 5 days ago, good for 7, warned 7 days ahead: 2 days left.
    machine.succeed("chage -d $(date -d '-5 days' +%F) -M 7 -W 7 alice")
    machine.send_chars("foobar\n")
    logged_in("alice")
    machine.succeed("journalctl -b -t yokoso -o cat | grep -q '\"auth_message_type\":\"info\"'")
    machine.fail("journalctl -b -t yokoso -o cat | grep -q 'Cannot respond'")
    quiet()
  '';

  # An expired password: greetd (0.10) can't change it, so the attempt
  # fails with a word on where to change it; changed, it logs in.
  expired = test "expired" { } ''
    machine.wait_for_unit("greetd.service")
    greeter()
    machine.succeed("chage -d 0 alice")
    machine.send_chars("foobar\n")
    machine.wait_until_succeeds("journalctl -b -u greetd | grep -q NEW_AUTHTOK_REQD", timeout=30)
    time.sleep(2)
    machine.screenshot("expired")
    machine.succeed("chage -d $(date +%F) alice")
    machine.send_chars("foobar\n")
    logged_in("alice")
    quiet()
  '';

  # A session that ends as soon as it starts: greetd brings the greeter
  # back, and the greeter says so.
  ended =
    test "ended"
      {
        services.displayManager.sessionPackages = [
          (
            (pkgs.writeTextDir "share/wayland-sessions/broken.desktop" ''
              [Desktop Entry]
              Name=Broken
              Exec=false
            '').overrideAttrs
              { passthru.providedSessions = [ "broken" ]; }
          )
        ];
        services.yokoso.settings.session = "broken";
      }
      ''
        machine.wait_for_unit("greetd.service")
        pid = greeter()
        machine.send_chars("foobar\n")
        greeter(pid)
        machine.succeed("journalctl -b -t yokoso | grep -q 'the session Broken ended'")
        machine.screenshot("ended")
        quiet()
      '';

  # An account that isn't listed (a system UID here, a directory's
  # elsewhere): "Other…" in the list, its name typed, then its password.
  other =
    test "other"
      {
        users.users.bob = {
          isSystemUser = true;
          group = "users";
          useDefaultShell = true;
          home = "/home/bob";
          createHome = true;
          password = "barfoo";
        };
      }
      ''
        machine.wait_for_unit("greetd.service")
        greeter()
        machine.send_key("f2")
        time.sleep(1)
        machine.send_key("down")
        machine.send_key("ret")
        time.sleep(1)
        machine.send_chars("bob\n")
        time.sleep(2)
        machine.screenshot("bob")
        machine.send_chars("barfoo\n")
        logged_in("bob")
        quiet()
      '';

  # Started without XKB_DEFAULT_* (as elsewhere than NixOS), the keyboard
  # layout comes from localed: German here.
  layout =
    test "layout"
      (
        { config, ... }:
        {
          services.xserver.xkb.layout = "de";
          services.greetd.settings.default_session.command = pkgs.lib.mkForce (
            pkgs.lib.getExe config.services.yokoso.package
          );
        }
      )
      ''
        machine.wait_for_unit("greetd.service")
        greeter()
        machine.succeed("tr '\\0' '\\n' </proc/$(pgrep -u greeter -n quickshell)/environ | grep -qx XKB_DEFAULT_LAYOUT=de")
        machine.send_chars("foobar\n")
        logged_in("alice")
        quiet()
      '';

  # Installed as elsewhere than NixOS: `make install`, greetd's own config,
  # the programs it needs on the PATH, and no module.
  plain = pkgs.testers.runNixOSTest {
    name = "yokoso-plain";
    nodes.machine =
      let
        yokoso = pkgs.stdenvNoCC.mkDerivation {
          name = "yokoso-installed";
          src = self;
          nativeBuildInputs = [ pkgs.gnumake ];
          dontBuild = true;
          installPhase = "make install PREFIX=$out";
        };
      in
      {
        services.greetd = {
          enable = true;
          settings.default_session.command = "${yokoso}/bin/yokoso";
        };
        systemd.tmpfiles.packages = [ yokoso ];
        environment.systemPackages = with pkgs; [
          quickshell
          cage
          jq
          gawk
          greetd
          wlrctl
        ];
        services.xserver = {
          enable = true;
          windowManager.i3.enable = true;
        };
        services.xserver.displayManager.startx.enable = true;
        users.users.alice = {
          isNormalUser = true;
          password = "foobar";
        };
        virtualisation.memorySize = 2048;
      };
    testScript = helpers + ''
      machine.wait_for_unit("greetd.service")
      greeter()
      machine.succeed("test -d /var/lib/yokoso -a -d /var/cache/yokoso")
      machine.send_chars("foobar\n")
      logged_in("alice")
      machine.succeed("grep -q '\"user\":\"alice\"' /var/lib/yokoso/state.json")
      quiet()
    '';
  };

  # A config that isn't JSON: left out, with a warning, and the login works.
  config =
    test "config"
      {
        environment.etc."yokoso/config.json".source = pkgs.lib.mkForce (
          pkgs.writeText "broken.json" ''{"theme": "light" "idle": 5}''
        );
      }
      ''
        machine.wait_for_unit("greetd.service")
        greeter()
        machine.succeed("journalctl -b -t yokoso | grep -q \"can't read /etc/yokoso/config.json\"")
        machine.fail("journalctl -b -t yokoso | grep -q 'falling back'")
        machine.send_chars("foobar\n")
        logged_in("alice")
        quiet()
      '';

  # A GPU whose driver loads after the greeter starts (not from the
  # initrd): until then there's only simpledrm, on EFI's framebuffer, which
  # goes away when the driver comes. The greeter waits for the driver's card.
  drm =
    test "drm"
      {
        virtualisation.useEFIBoot = true;
        boot.blacklistedKernelModules = [ "bochs" ];
        systemd.services.late-gpu = {
          wantedBy = [ "greetd.service" ];
          after = [ "greetd.service" ];
          serviceConfig.Type = "oneshot";
          # simpledrm goes a moment before the driver comes, as when the
          # driver takes over: for a second there's no card at all.
          script = ''
            sleep 3
            echo simple-framebuffer.0 >/sys/bus/platform/drivers/simple-framebuffer/unbind
            sleep 1
            ${pkgs.kmod}/bin/modprobe bochs
          '';
        };
      }
      ''
        machine.wait_for_unit("greetd.service")
        greeter()
        machine.succeed("journalctl -b -t yokoso | grep -q 'no GPU.s card yet'")
        machine.succeed("readlink -f /sys/class/drm/card0/device/driver | grep -q bochs")
        machine.fail("journalctl -b -t yokoso | grep -q 'falling back'")
        machine.send_chars("foobar\n")
        logged_in("alice")
        quiet()
      '';

  # A fingerprint, with libfprint's virtual reader: PAM asks for a finger
  # first, a wrong one fails, the right one logs in; after three wrong ones
  # the password does.
  finger =
    test "finger"
      {
        services.fprintd.enable = true;
        systemd.services.fprintd.environment.FP_VIRTUAL_DEVICE = "/run/fp-virtual";
        security.polkit.enable = true;
        environment.systemPackages = [ pkgs.socat ];
      }
      ''
        def scan(finger):
            machine.wait_until_succeeds("test -S /run/fp-virtual", timeout=60)
            machine.succeed(f"printf 'SCAN {finger}' | socat - UNIX-CONNECT:/run/fp-virtual")

        machine.wait_for_unit("greetd.service")
        pid = greeter()
        # Enrolled after that greeter asked for a password: a fresh one.
        machine.succeed("fprintd-enroll -f right-index-finger alice >/tmp/enroll.log 2>&1 &")
        for _ in range(12):
            time.sleep(0.7)
            scan("finger1")
            if machine.execute("grep -q enroll-completed /tmp/enroll.log")[0] == 0:
                break
        machine.succeed("grep -q enroll-completed /tmp/enroll.log")
        machine.succeed("systemctl restart greetd")
        machine.wait_until_succeeds(f"pgrep -u greeter -n quickshell | grep -vx '{pid}'", timeout=60)
        time.sleep(5)
        machine.screenshot("finger")
        scan("finger2")
        time.sleep(2)
        scan("finger1")
        logged_in("alice")
        logout("alice")

        machine.wait_until_succeeds("pgrep -u greeter quickshell", timeout=60)
        time.sleep(5)
        for _ in range(3):
            scan("finger2")
            time.sleep(2)
        time.sleep(2)
        machine.screenshot("password")
        machine.send_chars("foobar\n")
        logged_in("alice")
        quiet()
      '';
}
