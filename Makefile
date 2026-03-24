.DEFAULT_GOAL := check

FAMILIES ?= all

.PHONY: help setup hooks tools check coverage test fetch-test-roms test-blargg test-acid test-mealybug test-mooneye run-test-blargg run-test-acid run-test-mealybug run-test-mooneye

help:
	@echo "Available targets:"
	@echo "  make setup                Configure git hooks and install local cargo tools"
	@echo "  make hooks                Configure repository git hooks"
	@echo "  make tools                Install local cargo tools used by this repository"
	@echo "  make check                Run the local pre-push gate (fmt, clippy, test, typos, deny)"
	@echo "  make coverage             Run the repository coverage gate and emit lcov output"
	@echo "  make test                 Fetch ROMs if needed and run the supported external DMG block"
	@echo "  make fetch-test-roms      Materialize .roms/test from the pinned GBEmulatorShootout source using a temporary checkout"
	@echo "                           Set FAMILIES=all or FAMILIES=\"blargg acid\" to limit the fetch"
	@echo "  make test-blargg          Run the curated supported Blargg DMG family"
	@echo "  make test-acid            Run the curated supported Acid DMG family"
	@echo "  make test-mealybug        Run the exploratory Mealybug DMG family and update the report"
	@echo "  make test-mooneye         Run the exploratory Mooneye DMG family and update the report"

setup: hooks tools

hooks:
	git config core.hooksPath .githooks
	chmod +x .githooks/pre-commit .githooks/pre-push
	@echo "Git hooks path configured to .githooks"

tools:
	cargo install --locked cargo-llvm-cov
	cargo install --locked cargo-deny
	cargo install --locked typos-cli

check:
	cargo fmt-check
	cargo lint
	typos
	cargo tests
	cargo deny-check

coverage:
	cargo cov-check
	cargo cov-lcov

test:
	$(MAKE) run-test-blargg
	$(MAKE) run-test-acid

fetch-test-roms:
	cargo run -q -p gb-test-runner --bin fetch_test_roms -- $(FAMILIES)

test-blargg:
	$(MAKE) fetch-test-roms FAMILIES=blargg
	$(MAKE) run-test-blargg

test-acid:
	$(MAKE) fetch-test-roms FAMILIES=acid
	$(MAKE) run-test-acid

test-mealybug:
	$(MAKE) fetch-test-roms FAMILIES=mealybug-tearoom-tests
	$(MAKE) run-test-mealybug

test-mooneye:
	$(MAKE) fetch-test-roms FAMILIES=mooneye
	$(MAKE) run-test-mooneye

run-test-blargg:
	cargo test --release -p gb-test-runner --test external blargg_curated_suite_passes_from_repo_store -- --ignored --exact --nocapture

run-test-acid:
	cargo test --release -p gb-test-runner --test external acid_curated_suite_passes_from_repo_store -- --ignored --exact --nocapture

run-test-mealybug:
	cargo test --release -p gb-test-runner --test external mealybug_curated_suite_updates_report_from_repo_store -- --ignored --exact --nocapture

run-test-mooneye:
	cargo test --release -p gb-test-runner --test external mooneye_curated_suite_updates_report_from_repo_store -- --ignored --exact --nocapture
