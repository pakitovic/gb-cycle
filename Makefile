.DEFAULT_GOAL := check

.PHONY: help setup hooks tools check local fetch-external-roms test-external-blargg-dmg

help:
	@echo "Available targets:"
	@echo "  make setup                Configure git hooks and install local cargo tools"
	@echo "  make hooks                Configure repository git hooks"
	@echo "  make tools                Install local cargo tools used by this repository"
	@echo "  make check                Run the local pre-push gate (fmt, clippy, test, typos, deny)"
	@echo "  make local                Run check plus external Blargg DMG suites and coverage"
	@echo "  make fetch-external-roms  Populate .roms/external-test from the pinned manifest"
	@echo "  make test-external-blargg-dmg  Run the supported external Blargg DMG block"

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
	cargo tests
	typos
	cargo deny-check

local: check
	$(MAKE) fetch-external-roms
	$(MAKE) test-external-blargg-dmg
	cargo cov-check
	cargo cov-lcov

fetch-external-roms:
	cargo run -q -p gb-test-runner --bin fetch_external_roms --

test-external-blargg-dmg:
	cargo test --release -p gb-test-runner --test external retrio_blargg_cpu_smoke_suite_runs_against_real_external_assets -- --ignored --exact --nocapture
	cargo test --release -p gb-test-runner --test external retrio_blargg_cpu_instrs_full_suite_runs_against_real_external_assets -- --ignored --exact --nocapture
	cargo test --release -p gb-test-runner --test external retrio_blargg_instr_timing_suite_runs_against_real_external_assets -- --ignored --exact --nocapture
	cargo test --release -p gb-test-runner --test external retrio_blargg_halt_bug_suite_runs_against_real_external_assets -- --ignored --exact --nocapture
	cargo test --release -p gb-test-runner --test external retrio_blargg_mem_timing_suite_runs_against_real_external_assets -- --ignored --exact --nocapture
	cargo test --release -p gb-test-runner --test external retrio_blargg_mem_timing_individual_suite_runs_against_real_external_assets -- --ignored --exact --nocapture
	cargo test --release -p gb-test-runner --test external retrio_blargg_oam_bug_suite_runs_against_real_external_assets -- --ignored --exact --nocapture
