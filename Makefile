.DEFAULT_GOAL := ci

FAMILIES ?= all
ROM_PROFILE ?= release-max

.PHONY: help setup hooks tools ci coverage coverage-check test-roms test-roms-real-boot test-roms-extra test-roms-extra-real-boot test-roms-cgb test-roms-cgb-real-boot test-roms-cgb-extra test-roms-cgb-extra-real-boot fetch-test-roms require-boot-rom-root run-acid run-ax6 run-samesuite run-little-things-gb run-gbmicrotest run-docboy run-blargg run-blargg-cpu-instrs run-blargg-dmg-sound run-blargg-timing-memory-oam run-daid run-mooneye run-mooneye-acceptance run-mooneye-mbc1-mbc5 run-mooneye-mbc2 run-hacktix run-cpp run-mealybug run-cgb-smoke run-cgb-boot-div run-cgb-boot-hwio run-cgb-speed run-cgb-ppu-basic run-cgb-ppu-hard run-cgb-dma run-cgb-audio-blargg run-cgb-audio-samesuite run-cgb-rtc run-mbc6-oracle phase9-determinism-smoke phase9-determinism-local phase9-diff-cartridge phase9-sameboy-cartridge-oracles phase9-diff-acid phase9-sameboy-acid-oracles phase9-diff-mealybug phase9-sameboy-mealybug-oracles phase9-diff-hacktix phase9-sameboy-hacktix-oracles phase9-first-divergence-hacktix

help:
	@echo "Available targets:"
	@echo "  make setup                Configure git hooks and install local cargo tools"
	@echo "  make hooks                Configure repository git hooks"
	@echo "  make tools                Install local cargo tools used by this repository"
	@echo "  make ci                   Run the local pre-push gate (fmt, clippy, typos, deny, workspace tests via coverage, per-crate coverage check)"
	@echo "  make coverage-check       Run one workspace coverage sweep, then enforce per-crate coverage gates"
	@echo "  make coverage             Run complete workspace coverage and emit the HTML report"
	@echo "  make test-roms            Fetch and run all local curated DMG ROM suites"
	@echo "  make test-roms-real-boot  Fetch and run all local curated DMG ROM suites through verified RealBoot"
	@echo "  make test-roms-extra      Fetch and run the exploratory/internal extra ROM suites"
	@echo "  make test-roms-extra-real-boot Fetch and run the exploratory/internal extra ROM suites through verified RealBoot"
	@echo "  make test-roms-cgb        Fetch and run the promoted green local curated CGB ROM suites"
	@echo "  make test-roms-cgb-real-boot Fetch and run the promoted green local curated CGB ROM suites through verified RealBoot"
	@echo "  make test-roms-cgb-extra  Fetch and run the exploratory/internal CGB ROM suites"
	@echo "  make test-roms-cgb-extra-real-boot Fetch and run the exploratory/internal CGB ROM suites through verified RealBoot"
	@echo "  make fetch-test-roms      Materialize .roms/test from the pinned upstream source(s) using temporary checkout(s)"
	@echo "                           Set FAMILIES=all or FAMILIES=\"blargg acid\" to limit the fetch"
	@echo "  make run-acid             Fetch and run the curated Acid DMG suite"
	@echo "  make run-ax6              Fetch and run the extra AX6 DMG RTC suite"
	@echo "  make run-samesuite        Fetch and run the extra SameSuite DMG suite"
	@echo "  make run-little-things-gb Fetch and run the extra little-things-gb DMG suite"
	@echo "  make run-gbmicrotest      Fetch and run the extra DocBoy gbmicrotest DMG suite"
	@echo "  make run-docboy           Fetch and run the extra DocBoy docboy/* DMG suite"
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
	@echo "  make run-cgb-boot-div     Fetch and run the curated CGB boot DIV suite"
	@echo "  make run-cgb-boot-hwio    Fetch and run the exploratory/internal CGB boot HWIO suite"
	@echo "  make run-cgb-speed        Fetch and run the curated CGB KEY1/speed suite"
	@echo "  make run-cgb-ppu-basic    Fetch and run the curated CGB PPU baseline suite"
	@echo "  make run-cgb-ppu-hard     Fetch and run the curated hard CGB PPU suite"
	@echo "  make run-cgb-dma          Fetch and run the curated CGB DMA/GDMA/HDMA suite"
	@echo "  make run-cgb-audio-blargg Fetch and run the curated CGB Blargg sound suite"
	@echo "  make run-cgb-audio-samesuite Fetch and run the exploratory CGB SameSuite APU suite"
	@echo "  make run-cgb-rtc          Fetch and run the curated CGB MBC3 RTC suite"
	@echo "  make run-mbc6-oracle      Run the built-in synthetic MBC6 split-window/flash oracle"
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

test-roms-real-boot: require-boot-rom-root
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-acid
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-blargg
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-daid
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-mooneye
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-hacktix
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-cpp
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-mealybug

test-roms-extra:
	$(MAKE) run-ax6
	$(MAKE) run-samesuite
	$(MAKE) run-little-things-gb
	$(MAKE) run-gbmicrotest
	$(MAKE) run-docboy

test-roms-extra-real-boot: require-boot-rom-root
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-ax6
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-samesuite
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-little-things-gb
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-gbmicrotest
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-docboy

test-roms-cgb:
	$(MAKE) run-cgb-smoke
	$(MAKE) run-cgb-boot-div
	$(MAKE) run-cgb-speed
	$(MAKE) run-cgb-ppu-basic
	$(MAKE) run-cgb-ppu-hard
	$(MAKE) run-cgb-dma
	$(MAKE) run-cgb-audio-blargg
	$(MAKE) run-cgb-audio-samesuite
	$(MAKE) run-cgb-rtc

test-roms-cgb-real-boot: require-boot-rom-root
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-cgb-smoke
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-cgb-boot-div
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-cgb-speed
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-cgb-ppu-basic
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-cgb-ppu-hard
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-cgb-dma
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-cgb-audio-blargg
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-cgb-audio-samesuite
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-cgb-rtc

test-roms-cgb-extra:
	$(MAKE) run-cgb-boot-hwio

test-roms-cgb-extra-real-boot: require-boot-rom-root
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-cgb-boot-hwio

fetch-test-roms:
	cargo run --release -q -p gb-test-runner --bin fetch_test_roms -- $(FAMILIES)

require-boot-rom-root:
	@if [ -z "$$GB_CYCLE_BOOT_ROM_ROOT" ]; then echo "GB_CYCLE_BOOT_ROM_ROOT must point at the directory containing verified boot ROM assets for RealBoot ROM test targets"; exit 2; fi

run-acid:
	$(MAKE) fetch-test-roms FAMILIES=acid
	cargo test --release -p gb-test-runner --test external -- --ignored --exact acid_curated_suite_passes_from_repo_store --no-capture

run-ax6:
	$(MAKE) fetch-test-roms FAMILIES=ax6
	cargo run --profile $(ROM_PROFILE) -q -p gb-test-runner --bin run_rom_suite -- --suite ax6-dmg-extra --failure-artifact-root .artifacts/ax6

run-samesuite:
	$(MAKE) fetch-test-roms FAMILIES=samesuite
	cargo run --profile $(ROM_PROFILE) -q -p gb-test-runner --bin run_rom_suite -- --suite samesuite-dmg-extra --failure-artifact-root .artifacts/samesuite

run-little-things-gb:
	$(MAKE) fetch-test-roms FAMILIES=little-things-gb
	cargo run --profile $(ROM_PROFILE) -q -p gb-test-runner --bin run_rom_suite -- --suite little-things-gb-dmg-extra --failure-artifact-root .artifacts/little-things-gb

run-gbmicrotest:
	$(MAKE) fetch-test-roms FAMILIES=gbmicrotest
	cargo run --profile $(ROM_PROFILE) -q -p gb-test-runner --bin run_rom_suite -- --suite gbmicrotest-dmg-extra --failure-artifact-root .artifacts/gbmicrotest

run-docboy:
	$(MAKE) fetch-test-roms FAMILIES=docboy
	cargo run --profile $(ROM_PROFILE) -q -p gb-test-runner --bin run_rom_suite -- --suite docboy-dmg-extra --failure-artifact-root .artifacts/docboy
	cargo run --profile $(ROM_PROFILE) -q -p gb-test-runner --bin run_linked_session -- --suite docboy-dmg-linked-extra --failure-artifact-root .artifacts/docboy-linked

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
	cargo run --release -q -p gb-test-runner --bin run_rom_suite -- --suite cgb-smoke --failure-artifact-root .artifacts/cgb-smoke

run-cgb-boot-div:
	$(MAKE) fetch-test-roms FAMILIES=mooneye
	cargo run --release -q -p gb-test-runner --bin run_rom_suite -- --suite cgb-boot-div --failure-artifact-root .artifacts/cgb-boot-div

run-cgb-boot-hwio:
	$(MAKE) fetch-test-roms FAMILIES=mooneye
	cargo run --profile $(ROM_PROFILE) -q -p gb-test-runner --bin run_rom_suite -- --suite cgb-boot-hwio --failure-artifact-root .artifacts/cgb-boot-hwio

run-cgb-speed:
	$(MAKE) fetch-test-roms FAMILIES="daid blargg"
	cargo run --release -q -p gb-test-runner --bin run_rom_suite -- --suite cgb-speed --failure-artifact-root .artifacts/cgb-speed

run-cgb-ppu-basic:
	$(MAKE) fetch-test-roms FAMILIES="samesuite daid acid hacktix"
	cargo run --release -q -p gb-test-runner --bin run_rom_suite -- --suite cgb-ppu-basic --failure-artifact-root .artifacts/cgb-ppu-basic

run-cgb-ppu-hard:
	$(MAKE) fetch-test-roms FAMILIES=acid
	cargo run --release -q -p gb-test-runner --bin run_rom_suite -- --suite cgb-ppu-hard --failure-artifact-root .artifacts/cgb-ppu-hard

run-cgb-dma:
	$(MAKE) fetch-test-roms FAMILIES=samesuite
	cargo run --release -q -p gb-test-runner --bin run_rom_suite -- --suite cgb-dma --failure-artifact-root .artifacts/cgb-dma

run-cgb-audio-blargg:
	$(MAKE) fetch-test-roms FAMILIES=blargg
	cargo run --release -q -p gb-test-runner --bin run_rom_suite -- --suite cgb-audio-blargg --failure-artifact-root .artifacts/cgb-audio-blargg

run-cgb-audio-samesuite:
	$(MAKE) fetch-test-roms FAMILIES=samesuite
	cargo run --release -q -p gb-test-runner --bin run_rom_suite -- --suite cgb-audio-samesuite --failure-artifact-root .artifacts/cgb-audio-samesuite

run-cgb-rtc:
	$(MAKE) fetch-test-roms FAMILIES=ax6
	cargo run --release -q -p gb-test-runner --bin run_rom_suite -- --suite cgb-rtc --failure-artifact-root .artifacts/cgb-rtc

run-mbc6-oracle:
	cargo run --release -q -p gb-test-runner --bin run_rom_suite -- --suite phase-6-mbc6-oracle --failure-artifact-root .artifacts/phase-6-mbc6-oracle

phase9-determinism-smoke:
	cargo run --release -q -p gb-test-runner --bin run_determinism -- --suite phase-2-cpu-timing
	cargo run --release -q -p gb-test-runner --bin run_determinism -- --suite phase-2-interrupt-timing
	cargo run --release -q -p gb-test-runner --bin run_determinism -- --suite phase-6-cartridge-oracle --save-at-tcycles 1024 --continuation-tcycles 1024

phase9-determinism-local: phase9-determinism-smoke
	$(MAKE) fetch-test-roms FAMILIES="mooneye acid mealybug-tearoom-tests blargg"
	cargo run --release -q -p gb-test-runner --bin run_determinism -- --suite mooneye-acceptance-dmg-curated --case mooneye-timer-div-write --save-at-tcycles 1024 --continuation-tcycles 1024
	cargo run --release -q -p gb-test-runner --bin run_determinism -- --suite mooneye-acceptance-dmg-curated --case mooneye-oam-dma-basic --save-at-tcycles 1024 --continuation-tcycles 1024
	cargo run --release -q -p gb-test-runner --bin run_determinism -- --suite acid-dmg-curated --case dmg-acid2 --save-at-tcycles 1024 --continuation-tcycles 1024
	cargo run --release -q -p gb-test-runner --bin run_determinism -- --suite mealybug-tearoom-dmg-curated --case mealybug-m3-window-timing --save-at-tcycles 1024 --continuation-tcycles 1024
	cargo run --release -q -p gb-test-runner --bin run_determinism -- --suite blargg-dmg-curated --case blargg-dmg-sound-01-registers --save-at-tcycles 1024 --continuation-tcycles 1024

phase9-sameboy-cartridge-oracles:
	cargo run --release -p gb-test-runner --bin run_sameboy_case_bundle -- --suite phase-6-cartridge-oracle --build-if-missing

phase9-diff-cartridge:
	cargo run --release -p gb-test-runner --bin run_differential -- --oracle sameboy --oracle-layout case-bundle --suite phase-6-cartridge-oracle

phase9-sameboy-acid-oracles:
	$(MAKE) fetch-test-roms FAMILIES=acid
	cargo run --release -p gb-test-runner --bin run_sameboy_case_bundle -- --suite acid-dmg-curated --build-if-missing

phase9-diff-acid:
	$(MAKE) fetch-test-roms FAMILIES=acid
	cargo run --release -p gb-test-runner --bin run_differential -- --oracle sameboy --oracle-layout case-bundle --suite acid-dmg-curated

phase9-sameboy-mealybug-oracles:
	$(MAKE) fetch-test-roms FAMILIES=mealybug-tearoom-tests
	cargo run --release -p gb-test-runner --bin run_sameboy_case_bundle -- --suite mealybug-tearoom-dmg-sameboy-differential --build-if-missing

phase9-diff-mealybug:
	$(MAKE) fetch-test-roms FAMILIES=mealybug-tearoom-tests
	cargo run --release -p gb-test-runner --bin run_differential -- --oracle sameboy --oracle-layout case-bundle --suite mealybug-tearoom-dmg-sameboy-differential

phase9-sameboy-hacktix-oracles:
	$(MAKE) fetch-test-roms FAMILIES=hacktix
	cargo run --release -p gb-test-runner --bin run_sameboy_case_bundle -- --suite hacktix-dmg-curated --build-if-missing

phase9-diff-hacktix:
	$(MAKE) fetch-test-roms FAMILIES=hacktix
	cargo run --release -p gb-test-runner --bin run_differential -- --oracle sameboy --oracle-layout case-bundle --suite hacktix-dmg-curated

phase9-first-divergence-hacktix:
	$(MAKE) fetch-test-roms FAMILIES=hacktix
	cargo run --release -p gb-test-runner --bin run_first_divergence -- --oracle sameboy --suite hacktix-dmg-curated --probe-interval-tcycles 70224 --build-if-missing --allow-divergence
