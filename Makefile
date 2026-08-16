.DEFAULT_GOAL := build
plugins_dir := '/etc/coolercontrol/plugins'
executable := 'cc-plugin-aorus'
service_id := 'cc-plugin-aorus'

.PHONY: clean build install run uninstall

clean:
	@-$(RM) -rf target
	@-$(RM) -rf vendor

build:
	@cargo build --locked --release

install: build
	@sudo mkdir -p $(plugins_dir)/$(service_id)
	@sudo install -m755 ./target/release/$(executable) $(plugins_dir)/$(service_id)
	@sudo install -m644 ./manifest.toml $(plugins_dir)/$(service_id)

run: build
	@sudo ./target/release/$(executable)

uninstall:
	@-sudo $(RM) -rf $(plugins_dir)/$(service_id)
