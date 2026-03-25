.DEFAULT_GOAL := ci

FAMILIES ?= all

.PHONY: help setup hooks tools ci coverage test-roms test-roms-all fetch-test-roms run-acid run-blargg run-daid run-hacktix run-mooneye run-mealybug

help:
	@echo "Available targets:"
	@echo "  make setup                Configure git hooks and install local cargo tools"
	@echo "  make hooks                Configure repository git hooks"
	@echo "  make tools                Install local cargo tools used by this repository"
	@echo "  make ci                   Run the local pre-push gate (fmt, clippy, test, typos, deny, coverage check)"
	@echo "  make coverage             Run the repository coverage gate and emit lcov output"
	@echo "  make test-roms            Fetch ROMs if needed and run the supported external DMG block"
	@echo "  make test-roms-all        Fetch ROMs if needed and run all external DMG blocks"
	@echo "  make fetch-test-roms      Materialize .roms/test from the pinned GBEmulatorShootout source using a temporary checkout"
	@echo "                           Set FAMILIES=all or FAMILIES=\"blargg acid\" to limit the fetch"
	@echo "  make run-blargg           Fetch and run the curated supported Blargg DMG family"
	@echo "  make run-acid             Fetch and run the curated supported Acid DMG family"
	@echo "  make run-daid             Fetch and run the exploratory Daid DMG family and update the report"
	@echo "  make run-hacktix          Fetch and run the exploratory Hacktix DMG family and update the report"
	@echo "  make run-mealybug         Fetch and run the exploratory Mealybug DMG family and update the report"
	@echo "  make run-mooneye          Fetch and run the exploratory Mooneye DMG family and update the report"

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
	cargo tests
	typos
	cargo deny-check
	cargo cov-check

coverage:
	cargo cov-lcov

test-roms:
	$(MAKE) run-blargg
	$(MAKE) run-acid

test-roms-all:
	$(MAKE) run-blargg
	$(MAKE) run-acid
	$(MAKE) run-daid
	$(MAKE) run-hacktix
	$(MAKE) run-mooneye
	$(MAKE) run-mealybug

fetch-test-roms:
	cargo run -q -p gb-test-runner --bin fetch_test_roms -- $(FAMILIES)

run-acid:
	$(MAKE) fetch-test-roms FAMILIES=acid
	cargo test --release -p gb-test-runner --test external acid_curated_suite_passes_from_repo_store -- --ignored --exact --nocapture

run-blargg:
	$(MAKE) fetch-test-roms FAMILIES=blargg
	cargo test --release -p gb-test-runner --test external blargg_curated_suite_passes_from_repo_store -- --ignored --exact --nocapture

run-daid:
	$(MAKE) fetch-test-roms FAMILIES=daid
	cargo test --release -p gb-test-runner --test external daid_curated_suite_updates_report_from_repo_store -- --ignored --exact --nocapture

run-hacktix:
	$(MAKE) fetch-test-roms FAMILIES=hacktix
	cargo test --release -p gb-test-runner --test external hacktix_curated_suite_updates_report_from_repo_store -- --ignored --exact --nocapture

run-mooneye:
	$(MAKE) fetch-test-roms FAMILIES=mooneye
	cargo test --release -p gb-test-runner --test external mooneye_curated_suite_updates_report_from_repo_store -- --ignored --exact --nocapture

run-mealybug:
	$(MAKE) fetch-test-roms FAMILIES=mealybug-tearoom-tests
	cargo test --release -p gb-test-runner --test external mealybug_curated_suite_updates_report_from_repo_store -- --ignored --exact --nocapture
