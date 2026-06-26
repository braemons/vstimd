PREFIX      ?= /usr
UNITDIR     ?= /lib/systemd/system
CONFDIR     ?= /etc/braemons
SHAREDIR    ?= /usr/share/braemons/gpiochip-daqd
BINARY      := ../target/release/gpiochip-daqd
SERVICE     := packaging/systemd/gpiochip-daqd.service
CONFIG      := config/default-config.toml
EXAMPLES    := config/jetson-orin-nano_in16_out4.toml

.PHONY: build install uninstall

build:
	cargo build --release --manifest-path ../Cargo.toml -p gpiochip-daqd

install:
	install -D -m 0755 $(BINARY)  $(DESTDIR)$(PREFIX)/bin/gpiochip-daqd
	install -D -m 0644 $(SERVICE) $(DESTDIR)$(UNITDIR)/gpiochip-daqd.service
	install -d -m 0755 $(DESTDIR)$(CONFDIR)
	test -f $(DESTDIR)$(CONFDIR)/gpiochip-daqd-config.toml || \
	  install -m 0644 $(CONFIG) $(DESTDIR)$(CONFDIR)/gpiochip-daqd-config.toml
	install -d -m 0755 $(DESTDIR)$(SHAREDIR)
	for f in $(EXAMPLES); do install -m 0644 $$f $(DESTDIR)$(SHAREDIR)/; done

uninstall:
	systemctl disable --now gpiochip-daqd 2>/dev/null || true
	rm -f $(DESTDIR)$(PREFIX)/bin/gpiochip-daqd
	rm -f $(DESTDIR)$(UNITDIR)/gpiochip-daqd.service
	for f in $(EXAMPLES); do rm -f $(DESTDIR)$(SHAREDIR)/$$(basename $$f); done
	rmdir --ignore-fail-on-non-empty $(DESTDIR)$(SHAREDIR) $(DESTDIR)$(CONFDIR) 2>/dev/null || true
	systemctl daemon-reload 2>/dev/null || true
