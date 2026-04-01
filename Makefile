.DEFAULT_GOAL := ci

FAMILIES ?= all

.PHONY: help setup hooks tools ci coverage test-roms fetch-test-roms run-acid run-blargg run-daid run-mooneye run-hacktix run-cpp run-mealybug

help:
	@echo "Available targets:"
	@echo "  make setup                Configure git hooks and install local cargo tools"
	@echo "  make hooks                Configure repository git hooks"
	@echo "  make tools                Install local cargo tools used by this repository"
	@echo "  make ci                   Run the local pre-push gate (fmt, clippy, test, typos, deny, coverage check)"
	@echo "  make coverage             Run complete workspace coverage and emit HTML output"
	@echo "  make test-roms            Run all DMG ROMs tests"
	@echo "  make fetch-test-roms      Materialize .roms/test from the pinned GBEmulatorShootout source using a temporary checkout"
	@echo "                           Set FAMILIES=all or FAMILIES=\"blargg acid\" to limit the fetch"
	@echo "  make run-acid             Fetch and run Acid DMG ROMs tests"
	@echo "  make run-blargg           Fetch and run Blargg DMG ROMs tests"
	@echo "  make run-daid             Fetch and run Daid DMG ROMs tests"
	@echo "  make run-mooneye          Fetch and run Mooneye DMG ROMs tests"
	@echo "  make run-hacktix          Fetch and run Hacktix DMG ROMs tests"
	@echo "  make run-cpp              Fetch and run cpp MBC3 ROMs tests"
	@echo "  make run-mealybug         Fetch and run Mealybug DMG ROMs tests"

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
	cargo cov-html

test-roms:
	$(MAKE) run-acid
	$(MAKE) run-blargg
	$(MAKE) run-daid
	$(MAKE) run-mooneye
	$(MAKE) run-hacktix
	$(MAKE) run-cpp
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

run-mooneye:
	$(MAKE) fetch-test-roms FAMILIES=mooneye
	cargo test --release -p gb-test-runner --test external mooneye_curated_suite_updates_report_from_repo_store -- --ignored --exact --nocapture

run-hacktix:
	$(MAKE) fetch-test-roms FAMILIES=hacktix
	cargo test --release -p gb-test-runner --test external hacktix_curated_suite_updates_report_from_repo_store -- --ignored --exact --nocapture

run-cpp:
	$(MAKE) fetch-test-roms FAMILIES=cpp
	cargo test --release -p gb-test-runner --test external cpp_curated_suite_updates_report_from_repo_store -- --ignored --exact --nocapture

run-mealybug:
	$(MAKE) fetch-test-roms FAMILIES=mealybug-tearoom-tests
	cargo test --release -p gb-test-runner --test external mealybug_curated_suite_updates_report_from_repo_store -- --ignored --exact --nocapture
