.PHONY: help setup hooks tools typos check ci cov-check fetch-external-roms test-external-smoke test-external-cpu-instrs-full test-external-instr-timing test-external-halt-bug test-external-mem-timing test-external-mem-timing-individual test-external-oam-bug test-external-blargg-dmg

help:
	@echo "Available targets:"
	@echo "  make setup  - Configure project environment and install local cargo tools"
	@echo "  make hooks  - Configure git hooks path to .githooks"
	@echo "  make tools  - Install local cargo tools used by this repository"
	@echo "  make typos  - Run typos spellcheck"
	@echo "  make check  - Run the required local checks before push/PR (fmt, clippy, test, typos, deny, supported external Blargg DMG)"
	@echo "  make ci     - Run the full CI-like pipeline locally including coverage artifacts"
	@echo "  make cov-check - Enforce >=90% aggregate coverage on gb-core, gb-test-runner, and gb-persistence"
	@echo "  make fetch-external-roms - Download repo-managed external test ROM sources into .roms/external-test"
	@echo "  make test-external-smoke - Run the current external CPU smoke suite in release mode"
	@echo "  make test-external-cpu-instrs-full - Run the official blargg cpu_instrs multi-ROM in release mode"
	@echo "  make test-external-instr-timing - Run the official blargg instr_timing ROM in release mode"
	@echo "  make test-external-halt-bug - Run the official blargg halt_bug ROM in release mode"
	@echo "  make test-external-mem-timing - Run the official blargg mem_timing ROMs in release mode"
	@echo "  make test-external-mem-timing-individual - Run the official blargg mem_timing single-ROM set in release mode"
	@echo "  make test-external-oam-bug - Run the official blargg oam_bug ROMs in release mode"
	@echo "  make test-external-blargg-dmg - Run all currently supported non-APU, non-CGB Blargg DMG ROM suites in release mode"

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
	$(MAKE) fetch-external-roms
	$(MAKE) test-external-blargg-dmg

ci: check
	cargo cov-check
	cargo cov-lcov

cov-check:
	cargo cov-check

fetch-external-roms:
	cargo run -q -p gb-test-runner --bin fetch_external_roms --

test-external-smoke:
	cargo test --release -p gb-test-runner --test external retrio_blargg_cpu_smoke_suite_runs_against_real_external_assets -- --ignored --exact --nocapture

test-external-cpu-instrs-full:
	cargo test --release -p gb-test-runner --test external retrio_blargg_cpu_instrs_full_suite_runs_against_real_external_assets -- --ignored --exact --nocapture

test-external-instr-timing:
	cargo test --release -p gb-test-runner --test external retrio_blargg_instr_timing_suite_runs_against_real_external_assets -- --ignored --exact --nocapture

test-external-halt-bug:
	cargo test --release -p gb-test-runner --test external retrio_blargg_halt_bug_suite_runs_against_real_external_assets -- --ignored --exact --nocapture

test-external-mem-timing:
	cargo test --release -p gb-test-runner --test external retrio_blargg_mem_timing_suite_runs_against_real_external_assets -- --ignored --exact --nocapture

test-external-mem-timing-individual:
	cargo test --release -p gb-test-runner --test external retrio_blargg_mem_timing_individual_suite_runs_against_real_external_assets -- --ignored --exact --nocapture

test-external-oam-bug:
	cargo test --release -p gb-test-runner --test external retrio_blargg_oam_bug_suite_runs_against_real_external_assets -- --ignored --exact --nocapture

test-external-blargg-dmg:
	$(MAKE) test-external-smoke
	$(MAKE) test-external-cpu-instrs-full
	$(MAKE) test-external-instr-timing
	$(MAKE) test-external-halt-bug
	$(MAKE) test-external-mem-timing
	$(MAKE) test-external-mem-timing-individual
