PREFIX      ?= /usr
UNITDIR     ?= /lib/systemd/system
SYSUSERSDIR ?= /usr/lib/sysusers.d
BINARY      := target/release/vstimd
SERVICE     := packaging/systemd/vstimd.service
SYSUSERS    := packaging/sysusers/vstimd.conf

.PHONY: build install uninstall setup-user

build:
	cargo build --release

install:
	install -D -m 0755 $(BINARY)   $(DESTDIR)$(PREFIX)/bin/vstimd
	install -D -m 0644 $(SERVICE)  $(DESTDIR)$(UNITDIR)/vstimd.service
	install -D -m 0644 $(SYSUSERS) $(DESTDIR)$(SYSUSERSDIR)/vstimd.conf

uninstall:
	systemctl disable --now vstimd 2>/dev/null || true
	rm -f $(DESTDIR)$(PREFIX)/bin/vstimd
	rm -f $(DESTDIR)$(UNITDIR)/vstimd.service
	rm -f $(DESTDIR)$(SYSUSERSDIR)/vstimd.conf
	systemctl daemon-reload 2>/dev/null || true

setup-user:
	systemd-sysusers $(SYSUSERS)
