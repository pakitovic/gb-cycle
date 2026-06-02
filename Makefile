.DEFAULT_GOAL := help

.PHONY: help setup hooks tools coverage

help:
	@echo "Available targets:"
	@echo "  make setup                Configure git hooks and install local cargo tools"
	@echo "  make hooks                Configure repository git hooks"
	@echo "  make tools                Install local cargo tools used by this repository"
	@echo "  make coverage             Run workspace coverage, enforce per-crate gates, and emit the HTML report"

setup: hooks tools

hooks:
	git config core.hooksPath .githooks
	chmod +x .githooks/pre-commit
	@echo "Git hooks path configured to .githooks"

tools:
	cargo install --locked cargo-llvm-cov
	cargo install --locked cargo-deny
	cargo install --locked typos-cli

coverage:
	cargo cov-clean
	cargo cov-run
	cargo cov-check-core
	cargo cov-check-test-runner
	cargo cov-check-persistence
	cargo cov-check-cli
	cargo cov-check-desktop
	cargo cov-html
