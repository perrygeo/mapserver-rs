_default:
	@echo "make dev|run|release"

dev:
	nix-shell --run "cargo watch"

build:
	nix-shell --run "cargo build --release"

run: build
	./target/release/mapserver-rs example/basic.map --verbose
