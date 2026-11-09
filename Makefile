.DEFAULT_GOAL := help
.PHONY: help build release test lint fmt check install uninstall clean bench skills-check dist

help: ## Show this list
	@awk 'BEGIN {FS = ":.*## "} /^[a-zA-Z_-]+:.*## / {printf "  \033[36m%-14s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

build: ## Debug build
	cargo build

release: ## Optimized build (target/release/replica)
	cargo build --release

test: ## Unit and integration tests
	cargo test

fmt: ## Format the code
	cargo fmt --all

lint: ## Check formatting and run clippy with warnings as errors
	cargo fmt --all -- --check
	cargo clippy --all-targets -- -D warnings

check: lint test ## Everything CI runs: lint + test

install: release ## Install to $(PREFIX)/bin (default /usr/local); INSTALL_LAUNCHD=1 for auto-gc
	./scripts/install.sh

uninstall: ## Remove the installed binary and launchd agent (keeps workspaces)
	./scripts/uninstall.sh

bench: release ## Run benchmarks (PROJECT=dir SETUP="cmd" RUNS=n CONCURRENCY="1,5,10"; see benchmarks/README.md)
	./benchmarks/run.sh $(if $(PROJECT),--project $(PROJECT)) $(if $(SETUP),--setup "$(SETUP)") $(if $(RUNS),--runs $(RUNS)) $(if $(CONCURRENCY),--concurrency $(CONCURRENCY))

skills-check: ## Validate every skills/*/SKILL.md has name and description frontmatter
	@for f in skills/*/SKILL.md; do \
		n=$$(awk '/^name:/{print $$2; exit}' "$$f"); \
		d=$$(awk '/^description:/{print "ok"; exit}' "$$f"); \
		dir=$$(basename $$(dirname "$$f")); \
		[ "$$n" = "$$dir" ] && [ "$$d" = "ok" ] || { echo "FAIL $$f (name must match directory '$$dir')"; exit 1; }; \
		echo "ok   $$f"; \
	done

dist: ## Build universal and per-arch binaries into bin/ with sha256sums.txt
	cargo build --release --target aarch64-apple-darwin
	cargo build --release --target x86_64-apple-darwin
	rm -rf bin && mkdir -p bin
	cp target/aarch64-apple-darwin/release/replica bin/replica-darwin-arm64
	cp target/x86_64-apple-darwin/release/replica bin/replica-darwin-x86_64
	lipo -create bin/replica-darwin-arm64 bin/replica-darwin-x86_64 -output bin/replica-darwin-universal
	cd bin && shasum -a 256 replica-darwin-arm64 replica-darwin-x86_64 replica-darwin-universal > sha256sums.txt
	@lipo -info bin/replica-darwin-universal
	@cat bin/sha256sums.txt

clean: ## Remove build output
	cargo clean
