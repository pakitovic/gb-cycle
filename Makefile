.DEFAULT_GOAL := ci

.PHONY: help setup hooks tools ci coverage coverage-check

help:
	@echo "Available targets:"
	@echo "  make setup                Configure git hooks and install local cargo tools"
	@echo "  make hooks                Configure repository git hooks"
	@echo "  make tools                Install local cargo tools used by this repository"
	@echo "  make ci                   Run the local pre-push gate (fmt, clippy, typos, deny, workspace tests via coverage, per-crate coverage check)"
	@echo "  make coverage-check       Run one workspace coverage sweep, then enforce per-crate coverage gates"
	@echo "  make coverage             Run complete workspace coverage and emit the HTML report"

setup: hooks tools

hooks:
	git config core.hooksPath .githooks
	chmod +x .githooks/pre-commit .githooks/pre-push
	@echo "Git hooks path configured to .githooks"

tools:
	cargo install --locked cargo-llvm-cov
	cargo install --locked cargo-deny
	cargo install --locked typos-cli

ci:
	cargo fmt-check
	cargo lint
	typos
	cargo deny-check
	$(MAKE) coverage-check

coverage-check:
	cargo cov-clean
	cargo cov-run
	cargo cov-check-core
	cargo cov-check-test-runner
	cargo cov-check-persistence
	cargo cov-check-cli
	cargo cov-check-desktop

coverage:
	cargo cov-html
