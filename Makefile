.DEFAULT_GOAL := ci

FAMILIES ?= all

.PHONY: help setup hooks tools ci coverage coverage-check test-roms fetch-test-roms run-acid run-blargg run-daid run-mooneye run-hacktix run-cpp run-mealybug

help:
	@echo "Available targets:"
	@echo "  make setup                Configure git hooks and install local cargo tools"
	@echo "  make hooks                Configure repository git hooks"
	@echo "  make tools                Install local cargo tools used by this repository"
	@echo "  make ci                   Run the local pre-push gate (fmt, clippy, typos, deny, workspace tests via coverage, per-crate coverage check)"
	@echo "  make coverage-check       Run one workspace coverage sweep, then enforce per-crate coverage gates"
	@echo "  make coverage             Run complete workspace coverage and emit the HTML report"
	@echo "  make test-roms            Fetch and run all local curated DMG ROM suites"
	@echo "  make fetch-test-roms      Materialize .roms/test from the pinned GBEmulatorShootout source using a temporary checkout"
	@echo "                           Set FAMILIES=all or FAMILIES=\"blargg acid\" to limit the fetch"
	@echo "  make run-acid             Fetch and run the curated Acid DMG suite"
	@echo "  make run-blargg           Fetch and run the curated Blargg DMG suite"
	@echo "  make run-daid             Fetch and run the local Daid DMG suite"
	@echo "  make run-mooneye          Fetch and run the local Mooneye DMG suite"
	@echo "  make run-hacktix          Fetch and run the curated Hacktix DMG suite"
	@echo "  make run-cpp              Fetch and run the curated cpp MBC3 suite"
	@echo "  make run-mealybug         Fetch and run the local Mealybug DMG suite"

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
	cargo test --release -p gb-test-runner --test external -- --ignored --exact acid_curated_suite_passes_from_repo_store --no-capture

run-blargg:
	$(MAKE) fetch-test-roms FAMILIES=blargg
	cargo test --release -p gb-test-runner --test external -- --ignored --exact blargg_curated_suite_passes_from_repo_store --no-capture

run-daid:
	$(MAKE) fetch-test-roms FAMILIES=daid
	cargo test --release -p gb-test-runner --test external -- --ignored --exact daid_curated_suite_updates_report_from_repo_store --no-capture

run-mooneye:
	$(MAKE) fetch-test-roms FAMILIES=mooneye
	cargo test --release -p gb-test-runner --test external -- --ignored --exact mooneye_curated_suite_updates_report_from_repo_store --no-capture

run-hacktix:
	$(MAKE) fetch-test-roms FAMILIES=hacktix
	cargo test --release -p gb-test-runner --test external -- --ignored --exact hacktix_curated_suite_updates_report_from_repo_store --no-capture

run-cpp:
	$(MAKE) fetch-test-roms FAMILIES=cpp
	cargo test --release -p gb-test-runner --test external -- --ignored --exact cpp_curated_suite_updates_report_from_repo_store --no-capture

run-mealybug:
	$(MAKE) fetch-test-roms FAMILIES=mealybug-tearoom-tests
	cargo test --release -p gb-test-runner --test external -- --ignored --exact mealybug_curated_suite_updates_report_from_repo_store --no-capture
