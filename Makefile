.PHONY: build release test lint fmt check install uninstall clean bench zip

build:
	cargo build

release:
	cargo build --release

test:
	cargo test

fmt:
	cargo fmt --all

lint:
	cargo fmt --all -- --check
	cargo clippy --all-targets -- -D warnings

check: lint test

install: release
	./scripts/install.sh

uninstall:
	./scripts/uninstall.sh

bench:
	./scripts/bench.sh $(PROJECT)

clean:
	cargo clean
