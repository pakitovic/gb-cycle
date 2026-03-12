.PHONY: help setup hooks tools typos check ci

help:
	@echo "Available targets:"
	@echo "  make setup  - Configure project environment and install local cargo tools"
	@echo "  make hooks  - Configure git hooks path to .githooks"
	@echo "  make tools  - Install local cargo tools used by this repository"
	@echo "  make typos  - Run typos spellcheck"
	@echo "  make check  - Run local checks (fmt, clippy, test, typos, deny)"
	@echo "  make ci     - Run CI-like pipeline locally including LCOV coverage"

setup: hooks tools

hooks:
	git config core.hooksPath .githooks
	chmod +x .githooks/pre-commit .githooks/pre-push
	@echo "Git hooks path configured to .githooks"

tools:
	cargo install --locked cargo-llvm-cov
	cargo install --locked cargo-deny
	cargo install --locked typos-cli

typos:
	typos

check:
	cargo fmt-check
	cargo lint
	cargo tests
	$(MAKE) typos
	cargo deny-check

ci: check
	cargo cov-lcov
