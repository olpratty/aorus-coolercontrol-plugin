.DEFAULT_GOAL := build
DESTDIR ?=
plugins_dir ?= /var/lib/coolercontrol/plugins
plugin_dir = $(DESTDIR)$(plugins_dir)/cc-plugin-aorus
binary := target/release/cc-plugin-aorus

.PHONY: clean build install run uninstall

clean:
	cargo clean

build:
	cargo build --locked --release --bin cc-plugin-aorus

# Build as an ordinary user before invoking install with elevated permissions.
install:
	test -f "$(binary)"
	install -Dm755 "$(binary)" "$(plugin_dir)/cc-plugin-aorus"
	install -Dm644 manifest.toml "$(plugin_dir)/manifest.toml"

# The current user must have the required D-Bus permissions.
run:
	./$(binary)

uninstall:
	rm -f "$(plugin_dir)/cc-plugin-aorus" "$(plugin_dir)/manifest.toml"
	@if [ -d "$(plugin_dir)" ]; then rmdir --ignore-fail-on-non-empty "$(plugin_dir)"; fi
