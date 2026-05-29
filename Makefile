.DEFAULT_GOAL := ci
ROM_PROFILE ?= release-max
LEGACY_REPORT := legacy
GB_EMULATOR_SHOOTOUT_REPORT := gb-emulator-shootout
GB_EMULATOR_SHOOTOUT_TEST_ROOT := test/$(GB_EMULATOR_SHOOTOUT_REPORT)
GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT := $(GB_EMULATOR_SHOOTOUT_TEST_ROOT)/.artifacts
LEGACY_TEST_ARTIFACT_ROOT := test/.artifacts
RUN_ROM_SUITE = cargo run --profile $(ROM_PROFILE) -q -p gb-test-runner --bin run_rom_suite --
RUN_LINKED_SESSION = cargo run --profile $(ROM_PROFILE) -q -p gb-test-runner --bin run_linked_session --

.PHONY: help setup hooks tools ci coverage coverage-check test-roms test-roms-real-boot test-roms-extra test-roms-extra-real-boot test-roms-docboy test-roms-docboy-real-boot test-roms-cgb test-roms-cgb-real-boot test-roms-cgb-extra test-roms-cgb-extra-real-boot fetch-test-roms require-boot-rom-root run-acid run-ax6 run-samesuite run-samesuite-cgb run-samesuite-sgb run-mooneye-sgb-boot-regs run-magen-cgb run-little-things-gb run-little-things-gb-cgb run-gbmicrotest run-docboy-dmg run-docboy-cgb run-docboy-cgb-dmg run-docboy-cgb-dmg-ext run-blargg run-blargg-cpu-instrs run-blargg-dmg-sound run-blargg-timing-memory-oam run-daid run-mooneye run-mooneye-acceptance run-mooneye-mbc1-mbc5 run-mooneye-mbc2 run-mooneye-cgb run-ashiepaws run-cpp run-cpp-sgb run-mealybug run-mealybug-cgb run-cgb-smoke run-cgb-boot-div run-cgb-boot-hwio run-cgb-speed run-cgb-ppu-basic run-cgb-ppu-hard run-cgb-dma run-cgb-audio-blargg run-cgb-audio-samesuite run-cgb-rtc

help:
	@echo "Available targets:"
	@echo "  make setup                Configure git hooks and install local cargo tools"
	@echo "  make hooks                Configure repository git hooks"
	@echo "  make tools                Install local cargo tools used by this repository"
	@echo "  make ci                   Run the local pre-push gate (fmt, clippy, typos, deny, workspace tests via coverage, per-crate coverage check)"
	@echo "  make coverage-check       Run one workspace coverage sweep, then enforce per-crate coverage gates"
	@echo "  make coverage             Run complete workspace coverage and emit the HTML report"
	@echo "  make test-roms            Fetch and run all local curated DMG/SGB ROM suites"
	@echo "  make test-roms-real-boot  Fetch and run all local curated DMG ROM suites through verified RealBoot"
	@echo "  make test-roms-extra      Fetch and run the exploratory/internal extra ROM suites"
	@echo "  make test-roms-extra-real-boot Fetch and run the exploratory/internal extra ROM suites through verified RealBoot"
	@echo "  make test-roms-docboy     Fetch and run all exploratory DocBoy single-machine ROM suites"
	@echo "  make test-roms-docboy-real-boot Fetch and run all exploratory DocBoy single-machine ROM suites through verified RealBoot"
	@echo "  make test-roms-cgb        Fetch and run the promoted green local curated CGB ROM suites"
	@echo "  make test-roms-cgb-real-boot Fetch and run the promoted green local curated CGB ROM suites through verified RealBoot"
	@echo "  make test-roms-cgb-extra  Fetch and run the exploratory/internal CGB ROM suites"
	@echo "  make test-roms-cgb-extra-real-boot Fetch and run the exploratory/internal CGB ROM suites through verified RealBoot"
	@echo "  make fetch-test-roms      Materialize tests from the pinned upstream source(s) using temporary checkout(s)"
	@echo "                           Set REPORT=legacy FAMILIES=\"ax6 samesuite\"; direct fetches require an explicit report and one or more explicit families"
	@echo "                           Set REPORT=legacy for legacy extra/DocBoy families or REPORT=gb-emulator-shootout for promoted families"
	@echo "  make run-acid             Fetch and run the curated Acid DMG suite"
	@echo "  make run-ax6              Fetch and run the extra AX6 DMG RTC suite"
	@echo "  make run-samesuite        Fetch and run the extra SameSuite DMG suite"
	@echo "  make run-samesuite-cgb    Fetch and run the extra SameSuite CGB variant suite"
	@echo "  make run-samesuite-sgb    Fetch and run the fixture-backed SameSuite SGB suite"
	@echo "  make run-mooneye-sgb-boot-regs Fetch and run the extra Mooneye SGB/SGB2 boot register suite"
	@echo "  make run-magen-cgb        Fetch and run the extra Magen CGB suite"
	@echo "  make run-little-things-gb Fetch and run the extra little-things-gb DMG suite"
	@echo "  make run-little-things-gb-cgb Fetch and run the extra little-things-gb CGB suite"
	@echo "  make run-gbmicrotest      Fetch and run the extra DocBoy gbmicrotest DMG suite"
	@echo "  make run-docboy-dmg       Fetch and run the DocBoy docboy/* DMG suite"
	@echo "  make run-docboy-cgb       Fetch and run the DocBoy native CGB suite"
	@echo "  make run-docboy-cgb-dmg   Fetch and run the DocBoy CGB GB-compatible suite"
	@echo "  make run-docboy-cgb-dmg-ext Fetch and run the experimental DocBoy CGB DMG-ext suite"
	@echo "  make run-mealybug-cgb   Fetch and run the exploratory/internal Mealybug CGB suite"
	@echo "  make run-blargg           Fetch and run the curated Blargg DMG suite"
	@echo "  make run-blargg-cpu-instrs Fetch and run the Blargg CPU instruction chunk"
	@echo "  make run-blargg-dmg-sound Fetch and run the Blargg DMG sound chunk"
	@echo "  make run-blargg-timing-memory-oam Fetch and run the Blargg timing/memory/OAM chunk"
	@echo "  make run-daid             Fetch and run the local Daid DMG suite"
	@echo "  make run-mooneye          Fetch and run the local Mooneye DMG suite"
	@echo "  make run-mooneye-acceptance Fetch and run the Mooneye acceptance/manual chunk"
	@echo "  make run-mooneye-mbc1-mbc5 Fetch and run the Mooneye emulator-only MBC1/MBC5 chunk"
	@echo "  make run-mooneye-mbc2     Fetch and run the Mooneye emulator-only MBC2 chunk"
	@echo "  make run-mooneye-cgb      Fetch and run the exploratory/internal Mooneye CGB PPU suite"
	@echo "  make run-ashiepaws          Fetch and run the curated Ashiepaws DMG suite"
	@echo "  make run-cpp              Fetch and run the curated cpp MBC3 suite"
	@echo "  make run-cpp-sgb          Fetch and run the informational cpp SGB suite"
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
	@status=0; \
	$(MAKE) run-acid || status=$$?; \
	$(MAKE) run-blargg || status=$$?; \
	$(MAKE) run-daid || status=$$?; \
	$(MAKE) run-mooneye || status=$$?; \
	$(MAKE) run-ashiepaws || status=$$?; \
	$(MAKE) run-cpp || status=$$?; \
	$(MAKE) run-cpp-sgb || status=$$?; \
	$(MAKE) run-mealybug || status=$$?; \
	$(MAKE) run-samesuite-sgb || status=$$?; \
	exit $$status

test-roms-real-boot: require-boot-rom-root
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-acid
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-blargg
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-daid
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-mooneye
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-ashiepaws
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-cpp
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-mealybug

test-roms-extra:
	$(MAKE) run-ax6
	$(MAKE) run-samesuite
	$(MAKE) run-mooneye-sgb-boot-regs
	$(MAKE) run-little-things-gb
	$(MAKE) run-gbmicrotest

test-roms-extra-real-boot: require-boot-rom-root
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-ax6
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-samesuite
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-little-things-gb
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-gbmicrotest

test-roms-docboy:
	@status=0; \
	$(MAKE) run-docboy-dmg || status=$$?; \
	$(MAKE) run-docboy-cgb || status=$$?; \
	$(MAKE) run-docboy-cgb-dmg || status=$$?; \
	$(MAKE) run-docboy-cgb-dmg-ext || status=$$?; \
	exit $$status

test-roms-docboy-real-boot: require-boot-rom-root
	@status=0; \
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-docboy-dmg || status=$$?; \
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-docboy-cgb || status=$$?; \
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-docboy-cgb-dmg || status=$$?; \
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-docboy-cgb-dmg-ext || status=$$?; \
	exit $$status

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
	@status=0; \
	$(MAKE) run-cgb-boot-hwio || status=$$?; \
	$(MAKE) run-mooneye-cgb || status=$$?; \
	$(MAKE) run-samesuite-cgb || status=$$?; \
	$(MAKE) run-magen-cgb || status=$$?; \
	$(MAKE) run-mealybug-cgb || status=$$?; \
	$(MAKE) run-little-things-gb-cgb || status=$$?; \
	exit $$status

test-roms-cgb-extra-real-boot: require-boot-rom-root
	@status=0; \
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-cgb-boot-hwio || status=$$?; \
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-mooneye-cgb || status=$$?; \
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-samesuite-cgb || status=$$?; \
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-magen-cgb || status=$$?; \
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-mealybug-cgb || status=$$?; \
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-little-things-gb-cgb || status=$$?; \
	exit $$status

fetch-test-roms:
	@if [ -z "$(strip $(REPORT))" ]; then echo "REPORT is required; use REPORT=$(LEGACY_REPORT) or REPORT=$(GB_EMULATOR_SHOOTOUT_REPORT)"; exit 2; fi
	cargo run --release -q -p gb-test-runner --bin fetch_test_roms -- $(REPORT) $(FAMILIES)

require-boot-rom-root:
	@if [ -z "$$GB_CYCLE_BOOT_ROM_ROOT" ]; then echo "GB_CYCLE_BOOT_ROM_ROOT must point at the directory containing verified boot ROM assets for RealBoot ROM test targets"; exit 2; fi

run-acid:
	$(MAKE) fetch-test-roms REPORT=$(GB_EMULATOR_SHOOTOUT_REPORT) FAMILIES=acid
	$(RUN_ROM_SUITE) --suite acid-dmg-curated --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/acid-dmg-curated

run-ax6:
	$(MAKE) fetch-test-roms REPORT=$(LEGACY_REPORT) FAMILIES=ax6
	$(RUN_ROM_SUITE) --suite ax6-dmg-extra --failure-artifact-root $(LEGACY_TEST_ARTIFACT_ROOT)/ax6

run-samesuite:
	$(MAKE) fetch-test-roms REPORT=$(LEGACY_REPORT) FAMILIES=samesuite
	$(RUN_ROM_SUITE) --suite samesuite-dmg-extra --failure-artifact-root $(LEGACY_TEST_ARTIFACT_ROOT)/samesuite

run-samesuite-cgb:
	$(MAKE) fetch-test-roms REPORT=$(LEGACY_REPORT) FAMILIES=samesuite
	$(RUN_ROM_SUITE) --suite samesuite-cgb-extra --failure-artifact-root $(LEGACY_TEST_ARTIFACT_ROOT)/samesuite-cgb

run-samesuite-sgb:
	$(MAKE) fetch-test-roms REPORT=$(GB_EMULATOR_SHOOTOUT_REPORT) FAMILIES=samesuite
	$(RUN_ROM_SUITE) --suite samesuite-sgb --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/samesuite-sgb

run-mooneye-sgb-boot-regs:
	$(MAKE) fetch-test-roms REPORT=$(LEGACY_REPORT) FAMILIES=mooneye
	$(RUN_ROM_SUITE) --suite mooneye-sgb-boot-regs-extra --failure-artifact-root $(LEGACY_TEST_ARTIFACT_ROOT)/mooneye-sgb-boot-regs

run-magen-cgb:
	$(MAKE) fetch-test-roms REPORT=$(LEGACY_REPORT) FAMILIES=magen
	$(RUN_ROM_SUITE) --suite magen-cgb-extra --failure-artifact-root $(LEGACY_TEST_ARTIFACT_ROOT)/magen-cgb

run-little-things-gb:
	$(MAKE) fetch-test-roms REPORT=$(LEGACY_REPORT) FAMILIES=little-things-gb
	$(RUN_ROM_SUITE) --suite little-things-gb-dmg-extra --failure-artifact-root $(LEGACY_TEST_ARTIFACT_ROOT)/little-things-gb

run-little-things-gb-cgb:
	$(MAKE) fetch-test-roms REPORT=$(LEGACY_REPORT) FAMILIES=little-things-gb
	$(RUN_ROM_SUITE) --suite little-things-gb-cgb-extra --failure-artifact-root $(LEGACY_TEST_ARTIFACT_ROOT)/little-things-gb-cgb

run-gbmicrotest:
	$(MAKE) fetch-test-roms REPORT=$(LEGACY_REPORT) FAMILIES=gbmicrotest
	$(RUN_ROM_SUITE) --suite gbmicrotest-dmg-extra --failure-artifact-root $(LEGACY_TEST_ARTIFACT_ROOT)/gbmicrotest

run-docboy-dmg:
	$(MAKE) fetch-test-roms REPORT=$(LEGACY_REPORT) FAMILIES=docboy-dmg
	@status=0; \
	$(RUN_ROM_SUITE) --suite docboy-dmg-extra --failure-artifact-root $(LEGACY_TEST_ARTIFACT_ROOT)/docboy-dmg || status=$$?; \
	$(RUN_LINKED_SESSION) --suite docboy-dmg-linked-extra --failure-artifact-root $(LEGACY_TEST_ARTIFACT_ROOT)/docboy-dmg-linked || status=$$?; \
	exit $$status

run-docboy-cgb:
	$(MAKE) fetch-test-roms REPORT=$(LEGACY_REPORT) FAMILIES=docboy-cgb
	$(RUN_ROM_SUITE) --suite docboy-cgb-extra --failure-artifact-root $(LEGACY_TEST_ARTIFACT_ROOT)/docboy-cgb

run-docboy-cgb-dmg:
	$(MAKE) fetch-test-roms REPORT=$(LEGACY_REPORT) FAMILIES=docboy-cgb-dmg
	$(RUN_ROM_SUITE) --suite docboy-cgb-dmg-extra --failure-artifact-root $(LEGACY_TEST_ARTIFACT_ROOT)/docboy-cgb-dmg

run-docboy-cgb-dmg-ext:
	$(MAKE) fetch-test-roms REPORT=$(LEGACY_REPORT) FAMILIES=docboy-cgb-dmg-ext
	$(RUN_ROM_SUITE) --suite docboy-cgb-dmg-ext-extra --failure-artifact-root $(LEGACY_TEST_ARTIFACT_ROOT)/docboy-cgb-dmg-ext

run-blargg:
	$(MAKE) fetch-test-roms REPORT=$(GB_EMULATOR_SHOOTOUT_REPORT) FAMILIES=blargg
	@status=0; \
	$(RUN_ROM_SUITE) --suite blargg-cpu-instrs --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/blargg-cpu-instrs || status=$$?; \
	$(RUN_ROM_SUITE) --suite blargg-dmg-sound --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/blargg-dmg-sound || status=$$?; \
	$(RUN_ROM_SUITE) --suite blargg-timing-memory-oam --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/blargg-timing-memory-oam || status=$$?; \
	exit $$status

run-blargg-cpu-instrs:
	$(MAKE) fetch-test-roms REPORT=$(GB_EMULATOR_SHOOTOUT_REPORT) FAMILIES=blargg
	$(RUN_ROM_SUITE) --suite blargg-cpu-instrs --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/blargg-cpu-instrs

run-blargg-dmg-sound:
	$(MAKE) fetch-test-roms REPORT=$(GB_EMULATOR_SHOOTOUT_REPORT) FAMILIES=blargg
	$(RUN_ROM_SUITE) --suite blargg-dmg-sound --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/blargg-dmg-sound

run-blargg-timing-memory-oam:
	$(MAKE) fetch-test-roms REPORT=$(GB_EMULATOR_SHOOTOUT_REPORT) FAMILIES=blargg
	$(RUN_ROM_SUITE) --suite blargg-timing-memory-oam --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/blargg-timing-memory-oam

run-daid:
	$(MAKE) fetch-test-roms REPORT=$(GB_EMULATOR_SHOOTOUT_REPORT) FAMILIES=daid
	$(RUN_ROM_SUITE) --suite daid-dmg-curated --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/daid-dmg-curated

run-mooneye:
	$(MAKE) fetch-test-roms REPORT=$(GB_EMULATOR_SHOOTOUT_REPORT) FAMILIES=mooneye
	@status=0; \
	$(RUN_ROM_SUITE) --suite mooneye-acceptance-manual --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/mooneye-acceptance-manual || status=$$?; \
	$(RUN_ROM_SUITE) --suite mooneye-emulator-mbc1-mbc5 --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/mooneye-emulator-mbc1-mbc5 || status=$$?; \
	$(RUN_ROM_SUITE) --suite mooneye-emulator-mbc2 --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/mooneye-emulator-mbc2 || status=$$?; \
	exit $$status

run-mooneye-acceptance:
	$(MAKE) fetch-test-roms REPORT=$(GB_EMULATOR_SHOOTOUT_REPORT) FAMILIES=mooneye
	$(RUN_ROM_SUITE) --suite mooneye-acceptance-manual --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/mooneye-acceptance-manual

run-mooneye-mbc1-mbc5:
	$(MAKE) fetch-test-roms REPORT=$(GB_EMULATOR_SHOOTOUT_REPORT) FAMILIES=mooneye
	$(RUN_ROM_SUITE) --suite mooneye-emulator-mbc1-mbc5 --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/mooneye-emulator-mbc1-mbc5

run-mooneye-mbc2:
	$(MAKE) fetch-test-roms REPORT=$(GB_EMULATOR_SHOOTOUT_REPORT) FAMILIES=mooneye
	$(RUN_ROM_SUITE) --suite mooneye-emulator-mbc2 --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/mooneye-emulator-mbc2

run-ashiepaws:
	$(MAKE) fetch-test-roms REPORT=$(GB_EMULATOR_SHOOTOUT_REPORT) FAMILIES=ashiepaws
	$(RUN_ROM_SUITE) --suite ashiepaws-dmg-curated --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/ashiepaws-dmg-curated

run-cpp:
	$(MAKE) fetch-test-roms REPORT=$(GB_EMULATOR_SHOOTOUT_REPORT) FAMILIES=cpp
	$(RUN_ROM_SUITE) --suite cpp-dmg-curated --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/cpp-dmg-curated

run-cpp-sgb:
	$(MAKE) fetch-test-roms REPORT=$(GB_EMULATOR_SHOOTOUT_REPORT) FAMILIES=cpp
	$(RUN_ROM_SUITE) --suite cpp-sgb --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/cpp-sgb

run-mealybug:
	$(MAKE) fetch-test-roms REPORT=$(GB_EMULATOR_SHOOTOUT_REPORT) FAMILIES=mealybug-tearoom-tests
	$(RUN_ROM_SUITE) --suite mealybug-tearoom-dmg-curated --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/mealybug-tearoom-dmg-curated

run-mealybug-cgb:
	$(MAKE) fetch-test-roms REPORT=$(LEGACY_REPORT) FAMILIES=mealybug-tearoom-tests
	$(RUN_ROM_SUITE) --suite mealybug-tearoom-cgb-extra --failure-artifact-root $(LEGACY_TEST_ARTIFACT_ROOT)/mealybug-cgb

run-cgb-smoke:
	$(MAKE) fetch-test-roms REPORT=$(GB_EMULATOR_SHOOTOUT_REPORT) FAMILIES="mooneye acid"
	$(RUN_ROM_SUITE) --suite cgb-smoke --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/cgb-smoke

run-cgb-boot-div:
	$(MAKE) fetch-test-roms REPORT=$(GB_EMULATOR_SHOOTOUT_REPORT) FAMILIES=mooneye
	$(RUN_ROM_SUITE) --suite cgb-boot-div --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/cgb-boot-div

run-cgb-boot-hwio:
	$(MAKE) fetch-test-roms REPORT=$(LEGACY_REPORT) FAMILIES=mooneye
	$(RUN_ROM_SUITE) --suite cgb-boot-hwio --failure-artifact-root $(LEGACY_TEST_ARTIFACT_ROOT)/cgb-boot-hwio

run-mooneye-cgb:
	$(MAKE) fetch-test-roms REPORT=$(LEGACY_REPORT) FAMILIES=mooneye
	$(RUN_ROM_SUITE) --suite mooneye-cgb-extra --failure-artifact-root $(LEGACY_TEST_ARTIFACT_ROOT)/mooneye-cgb

run-cgb-speed:
	$(MAKE) fetch-test-roms REPORT=$(GB_EMULATOR_SHOOTOUT_REPORT) FAMILIES="daid blargg"
	$(RUN_ROM_SUITE) --suite cgb-speed --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/cgb-speed

run-cgb-ppu-basic:
	$(MAKE) fetch-test-roms REPORT=$(GB_EMULATOR_SHOOTOUT_REPORT) FAMILIES="samesuite daid acid ashiepaws"
	$(RUN_ROM_SUITE) --suite cgb-ppu-basic --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/cgb-ppu-basic

run-cgb-ppu-hard:
	$(MAKE) fetch-test-roms REPORT=$(GB_EMULATOR_SHOOTOUT_REPORT) FAMILIES=acid
	$(RUN_ROM_SUITE) --suite cgb-ppu-hard --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/cgb-ppu-hard

run-cgb-dma:
	$(MAKE) fetch-test-roms REPORT=$(GB_EMULATOR_SHOOTOUT_REPORT) FAMILIES=samesuite
	$(RUN_ROM_SUITE) --suite cgb-dma --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/cgb-dma

run-cgb-audio-blargg:
	$(MAKE) fetch-test-roms REPORT=$(GB_EMULATOR_SHOOTOUT_REPORT) FAMILIES=blargg
	$(RUN_ROM_SUITE) --suite cgb-audio-blargg --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/cgb-audio-blargg

run-cgb-audio-samesuite:
	$(MAKE) fetch-test-roms REPORT=$(GB_EMULATOR_SHOOTOUT_REPORT) FAMILIES=samesuite
	$(RUN_ROM_SUITE) --suite cgb-audio-samesuite --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/cgb-audio-samesuite

run-cgb-rtc:
	$(MAKE) fetch-test-roms REPORT=$(GB_EMULATOR_SHOOTOUT_REPORT) FAMILIES=ax6
	$(RUN_ROM_SUITE) --suite cgb-rtc --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/cgb-rtc
