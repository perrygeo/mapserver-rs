_default:
	@echo "make dev|test|build|run"

dev:
	nix-shell --run "cargo watch -x 'run -- example/basic.map --verbose'"

test:
	nix-shell --run "cargo test"

build:
	nix-shell --run "cargo build --release"

run: build
	./target/release/mapserver-rs example/basic.map --verbose
