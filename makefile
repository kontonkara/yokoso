# Installs yokoso where greetd can run it, for systems without Nix:
#
#   make install PREFIX=/usr [DESTDIR=<package root>]
#
# The launcher finds the QML in $(PREFIX)/share/yokoso/shell next to it.
# yokoso lock's PAM service goes to /etc/pam.d whatever the prefix.

PREFIX ?= /usr/local
DESTDIR ?=

shell := $(shell find shell -type f)

install:
	install -Dm755 bin/yokoso $(DESTDIR)$(PREFIX)/bin/yokoso
	for f in $(shell); do install -Dm644 "$$f" "$(DESTDIR)$(PREFIX)/share/yokoso/$$f"; done
	install -Dm644 dist/tmpfiles.conf $(DESTDIR)$(PREFIX)/lib/tmpfiles.d/yokoso.conf
	install -Dm644 dist/pam.d/yokoso-lock $(DESTDIR)/etc/pam.d/yokoso-lock
	install -Dm644 license $(DESTDIR)$(PREFIX)/share/licenses/yokoso/license

uninstall:
	rm -f $(DESTDIR)$(PREFIX)/bin/yokoso $(DESTDIR)$(PREFIX)/lib/tmpfiles.d/yokoso.conf \
		$(DESTDIR)/etc/pam.d/yokoso-lock
	rm -rf $(DESTDIR)$(PREFIX)/share/yokoso $(DESTDIR)$(PREFIX)/share/licenses/yokoso

.PHONY: install uninstall
