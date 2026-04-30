.DEFAULT_GOAL := ci

FAMILIES ?= all

.PHONY: help setup hooks tools ci coverage coverage-check test-roms test-roms-cgb fetch-test-roms run-acid run-blargg run-blargg-cpu-instrs run-blargg-dmg-sound run-blargg-timing-memory-oam run-daid run-mooneye run-mooneye-acceptance run-mooneye-mbc1-mbc5 run-mooneye-mbc2 run-hacktix run-cpp run-mealybug run-cgb-smoke run-cgb-speed phase9-determinism-smoke phase9-determinism-local phase9-diff-cartridge phase9-sameboy-cartridge-oracles phase9-diff-acid phase9-sameboy-acid-oracles phase9-diff-mealybug phase9-sameboy-mealybug-oracles phase9-diff-hacktix phase9-sameboy-hacktix-oracles phase9-first-divergence-hacktix

help:
	@echo "Available targets:"
	@echo "  make setup                Configure git hooks and install local cargo tools"
	@echo "  make hooks                Configure repository git hooks"
	@echo "  make tools                Install local cargo tools used by this repository"
	@echo "  make ci                   Run the local pre-push gate (fmt, clippy, typos, deny, workspace tests via coverage, per-crate coverage check)"
	@echo "  make coverage-check       Run one workspace coverage sweep, then enforce per-crate coverage gates"
	@echo "  make coverage             Run complete workspace coverage and emit the HTML report"
	@echo "  make test-roms            Fetch and run all local curated DMG ROM suites"
	@echo "  make test-roms-cgb        Fetch and run all currently defined local curated CGB ROM suites"
	@echo "  make fetch-test-roms      Materialize .roms/test from the pinned GBEmulatorShootout source using a temporary checkout"
	@echo "                           Set FAMILIES=all or FAMILIES=\"blargg acid\" to limit the fetch"
	@echo "  make run-acid             Fetch and run the curated Acid DMG suite"
	@echo "  make run-blargg           Fetch and run the curated Blargg DMG suite"
	@echo "  make run-blargg-cpu-instrs Fetch and run the Blargg CPU instruction chunk"
	@echo "  make run-blargg-dmg-sound Fetch and run the Blargg DMG sound chunk"
	@echo "  make run-blargg-timing-memory-oam Fetch and run the Blargg timing/memory/OAM chunk"
	@echo "  make run-daid             Fetch and run the local Daid DMG suite"
	@echo "  make run-mooneye          Fetch and run the local Mooneye DMG suite"
	@echo "  make run-mooneye-acceptance Fetch and run the Mooneye acceptance/manual chunk"
	@echo "  make run-mooneye-mbc1-mbc5 Fetch and run the Mooneye emulator-only MBC1/MBC5 chunk"
	@echo "  make run-mooneye-mbc2     Fetch and run the Mooneye emulator-only MBC2 chunk"
	@echo "  make run-hacktix          Fetch and run the curated Hacktix DMG suite"
	@echo "  make run-cpp              Fetch and run the curated cpp MBC3 suite"
	@echo "  make run-mealybug         Fetch and run the local Mealybug DMG suite"
	@echo "  make run-cgb-smoke        Fetch and run the curated CGB smoke suite"
	@echo "  make run-cgb-speed        Fetch and run the curated CGB KEY1/speed suite"
	@echo "  make phase9-determinism-smoke Run Phase 9 replay/save-load smoke checks"
	@echo "  make phase9-determinism-local Run Phase 9 replay/save-load local closure sample"
	@echo "  make phase9-diff-cartridge    Compare Phase 6 cartridge oracle against SameBoy case-bundle artifacts"
	@echo "  make phase9-sameboy-cartridge-oracles Materialize SameBoy case-bundle artifacts for cartridge differential"
	@echo "  make phase9-diff-acid         Compare Acid framebuffer cases against LibSameBoy case-bundle artifacts"
	@echo "  make phase9-sameboy-acid-oracles Materialize LibSameBoy case-bundle artifacts for Acid"
	@echo "  make phase9-diff-mealybug     Compare SameBoy-PASS Mealybug framebuffer cases against LibSameBoy case-bundle artifacts"
	@echo "  make phase9-sameboy-mealybug-oracles Materialize LibSameBoy case-bundle artifacts for the SameBoy-PASS Mealybug subset"
	@echo "  make phase9-diff-hacktix      Compare Hacktix framebuffer cases against LibSameBoy case-bundle artifacts"
	@echo "  make phase9-sameboy-hacktix-oracles Materialize LibSameBoy case-bundle artifacts for Hacktix"
	@echo "  make phase9-first-divergence-hacktix Capture Hacktix local/LibSameBoy first-divergence probe windows"

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

test-roms-cgb:
	$(MAKE) run-cgb-smoke
	$(MAKE) run-cgb-speed

fetch-test-roms:
	cargo run -q -p gb-test-runner --bin fetch_test_roms -- $(FAMILIES)

run-acid:
	$(MAKE) fetch-test-roms FAMILIES=acid
	cargo test --release -p gb-test-runner --test external -- --ignored --exact acid_curated_suite_passes_from_repo_store --no-capture

run-blargg:
	$(MAKE) fetch-test-roms FAMILIES=blargg
	cargo test --release -p gb-test-runner --test external -- --ignored --exact blargg_cpu_instrs_chunk_passes_from_repo_store --no-capture
	cargo test --release -p gb-test-runner --test external -- --ignored --exact blargg_dmg_sound_chunk_passes_from_repo_store --no-capture
	cargo test --release -p gb-test-runner --test external -- --ignored --exact blargg_timing_memory_oam_chunk_passes_from_repo_store --no-capture

run-blargg-cpu-instrs:
	$(MAKE) fetch-test-roms FAMILIES=blargg
	cargo test --release -p gb-test-runner --test external -- --ignored --exact blargg_cpu_instrs_chunk_passes_from_repo_store --no-capture

run-blargg-dmg-sound:
	$(MAKE) fetch-test-roms FAMILIES=blargg
	cargo test --release -p gb-test-runner --test external -- --ignored --exact blargg_dmg_sound_chunk_passes_from_repo_store --no-capture

run-blargg-timing-memory-oam:
	$(MAKE) fetch-test-roms FAMILIES=blargg
	cargo test --release -p gb-test-runner --test external -- --ignored --exact blargg_timing_memory_oam_chunk_passes_from_repo_store --no-capture

run-daid:
	$(MAKE) fetch-test-roms FAMILIES=daid
	cargo test --release -p gb-test-runner --test external -- --ignored --exact daid_curated_suite_passes_from_repo_store --no-capture

run-mooneye:
	$(MAKE) fetch-test-roms FAMILIES=mooneye
	cargo test --release -p gb-test-runner --test external -- --ignored --exact mooneye_acceptance_chunk_passes_from_repo_store --no-capture
	cargo test --release -p gb-test-runner --test external -- --ignored --exact mooneye_mbc1_mbc5_chunk_passes_from_repo_store --no-capture
	cargo test --release -p gb-test-runner --test external -- --ignored --exact mooneye_mbc2_chunk_passes_from_repo_store --no-capture

run-mooneye-acceptance:
	$(MAKE) fetch-test-roms FAMILIES=mooneye
	cargo test --release -p gb-test-runner --test external -- --ignored --exact mooneye_acceptance_chunk_passes_from_repo_store --no-capture

run-mooneye-mbc1-mbc5:
	$(MAKE) fetch-test-roms FAMILIES=mooneye
	cargo test --release -p gb-test-runner --test external -- --ignored --exact mooneye_mbc1_mbc5_chunk_passes_from_repo_store --no-capture

run-mooneye-mbc2:
	$(MAKE) fetch-test-roms FAMILIES=mooneye
	cargo test --release -p gb-test-runner --test external -- --ignored --exact mooneye_mbc2_chunk_passes_from_repo_store --no-capture

run-hacktix:
	$(MAKE) fetch-test-roms FAMILIES=hacktix
	cargo test --release -p gb-test-runner --test external -- --ignored --exact hacktix_curated_suite_passes_from_repo_store --no-capture

run-cpp:
	$(MAKE) fetch-test-roms FAMILIES=cpp
	cargo test --release -p gb-test-runner --test external -- --ignored --exact cpp_curated_suite_passes_from_repo_store --no-capture

run-mealybug:
	$(MAKE) fetch-test-roms FAMILIES=mealybug-tearoom-tests
	cargo test --release -p gb-test-runner --test external -- --ignored --exact mealybug_curated_suite_passes_from_repo_store --no-capture

run-cgb-smoke:
	$(MAKE) fetch-test-roms FAMILIES="mooneye acid"
	cargo run -q -p gb-test-runner --bin run_rom_suite -- --suite cgb-smoke --failure-artifact-root .artifacts/cgb-smoke

run-cgb-speed:
	$(MAKE) fetch-test-roms FAMILIES="daid blargg"
	cargo run -q -p gb-test-runner --bin run_rom_suite -- --suite cgb-speed --failure-artifact-root .artifacts/cgb-speed

phase9-determinism-smoke:
	cargo run -q -p gb-test-runner --bin run_determinism -- --suite phase-2-cpu-timing
	cargo run -q -p gb-test-runner --bin run_determinism -- --suite phase-2-interrupt-timing
	cargo run -q -p gb-test-runner --bin run_determinism -- --suite phase-6-cartridge-oracle --save-at-tcycles 1024 --continuation-tcycles 1024

phase9-determinism-local: phase9-determinism-smoke
	$(MAKE) fetch-test-roms FAMILIES=mooneye
	$(MAKE) fetch-test-roms FAMILIES=acid
	$(MAKE) fetch-test-roms FAMILIES=mealybug-tearoom-tests
	$(MAKE) fetch-test-roms FAMILIES=blargg
	cargo run -q -p gb-test-runner --bin run_determinism -- --suite mooneye-acceptance-dmg-curated --case mooneye-timer-div-write --save-at-tcycles 1024 --continuation-tcycles 1024
	cargo run -q -p gb-test-runner --bin run_determinism -- --suite mooneye-acceptance-dmg-curated --case mooneye-oam-dma-basic --save-at-tcycles 1024 --continuation-tcycles 1024
	cargo run -q -p gb-test-runner --bin run_determinism -- --suite acid-dmg-curated --case dmg-acid2 --save-at-tcycles 1024 --continuation-tcycles 1024
	cargo run -q -p gb-test-runner --bin run_determinism -- --suite mealybug-tearoom-dmg-curated --case mealybug-m3-window-timing --save-at-tcycles 1024 --continuation-tcycles 1024
	cargo run -q -p gb-test-runner --bin run_determinism -- --suite blargg-dmg-curated --case blargg-dmg-sound-01-registers --save-at-tcycles 1024 --continuation-tcycles 1024

phase9-sameboy-cartridge-oracles:
	cargo run -p gb-test-runner --bin run_sameboy_case_bundle -- --suite phase-6-cartridge-oracle --build-if-missing

phase9-diff-cartridge:
	cargo run -p gb-test-runner --bin run_differential -- --oracle sameboy --oracle-layout case-bundle --suite phase-6-cartridge-oracle

phase9-sameboy-acid-oracles:
	$(MAKE) fetch-test-roms FAMILIES=acid
	cargo run -p gb-test-runner --bin run_sameboy_case_bundle -- --suite acid-dmg-curated --build-if-missing

phase9-diff-acid:
	$(MAKE) fetch-test-roms FAMILIES=acid
	cargo run -p gb-test-runner --bin run_differential -- --oracle sameboy --oracle-layout case-bundle --suite acid-dmg-curated

phase9-sameboy-mealybug-oracles:
	$(MAKE) fetch-test-roms FAMILIES=mealybug-tearoom-tests
	cargo run -p gb-test-runner --bin run_sameboy_case_bundle -- --suite mealybug-tearoom-dmg-sameboy-differential --build-if-missing

phase9-diff-mealybug:
	$(MAKE) fetch-test-roms FAMILIES=mealybug-tearoom-tests
	cargo run -p gb-test-runner --bin run_differential -- --oracle sameboy --oracle-layout case-bundle --suite mealybug-tearoom-dmg-sameboy-differential

phase9-sameboy-hacktix-oracles:
	$(MAKE) fetch-test-roms FAMILIES=hacktix
	cargo run -p gb-test-runner --bin run_sameboy_case_bundle -- --suite hacktix-dmg-curated --build-if-missing

phase9-diff-hacktix:
	$(MAKE) fetch-test-roms FAMILIES=hacktix
	cargo run -p gb-test-runner --bin run_differential -- --oracle sameboy --oracle-layout case-bundle --suite hacktix-dmg-curated

phase9-first-divergence-hacktix:
	$(MAKE) fetch-test-roms FAMILIES=hacktix
	cargo run -p gb-test-runner --bin run_first_divergence -- --oracle sameboy --suite hacktix-dmg-curated --probe-interval-tcycles 70224 --build-if-missing --allow-divergence
