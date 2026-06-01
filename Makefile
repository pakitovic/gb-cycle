.DEFAULT_GOAL := ci
ROM_PROFILE ?= release-max
LEGACY_REPORT := legacy
DOCBOY_REPORT := docboy
GBMICROTEST_REPORT := gbmicrotest
GB_EMULATOR_SHOOTOUT_REPORT := gb-emulator-shootout
DOCBOY_TEST_ROOT := test/$(DOCBOY_REPORT)
DOCBOY_ARTIFACT_ROOT := $(DOCBOY_TEST_ROOT)/.artifacts
GBMICROTEST_TEST_ROOT := test/$(GBMICROTEST_REPORT)
GBMICROTEST_ARTIFACT_ROOT := $(GBMICROTEST_TEST_ROOT)/.artifacts
GB_EMULATOR_SHOOTOUT_TEST_ROOT := test/$(GB_EMULATOR_SHOOTOUT_REPORT)
GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT := $(GB_EMULATOR_SHOOTOUT_TEST_ROOT)/.artifacts
LEGACY_TEST_ARTIFACT_ROOT := test/.artifacts
RUN_ROM_SUITE = cargo run --profile $(ROM_PROFILE) -q -p gb-test-runner --bin run_rom_suite --

.PHONY: help setup hooks tools ci coverage coverage-check test-roms test-roms-real-boot test-roms-extra test-roms-extra-real-boot test-roms-docboy test-roms-docboy-real-boot test-roms-gbmicrotest test-roms-gbmicrotest-real-boot test-roms-cgb-extra test-roms-cgb-extra-real-boot fetch-test-roms require-boot-rom-root run-acid run-ax6-dmg run-samesuite run-samesuite-dmg-extra run-samesuite-cgb run-mooneye-sgb-boot-regs run-magen-cgb run-little-things-gb run-little-things-gb-cgb run-gbmicrotest run-docboy-dmg run-docboy-cgb run-docboy-cgb-dmg run-docboy-cgb-dmg-ext run-blargg-cpu-instrs run-blargg-dmg-sound run-blargg-timing-memory-oam run-daid run-mooneye-acceptance run-mooneye-mbc1-mbc5 run-mooneye-mbc2 run-mooneye-cgb run-ashiepaws run-cpp run-mealybug run-mealybug-cgb run-cgb-boot-hwio run-blargg-cgb-sound run-samesuite-apu run-ax6-cgb

help:
	@echo "Available targets:"
	@echo "  make setup                Configure git hooks and install local cargo tools"
	@echo "  make hooks                Configure repository git hooks"
	@echo "  make tools                Install local cargo tools used by this repository"
	@echo "  make ci                   Run the local pre-push gate (fmt, clippy, typos, deny, workspace tests via coverage, per-crate coverage check)"
	@echo "  make coverage-check       Run one workspace coverage sweep, then enforce per-crate coverage gates"
	@echo "  make coverage             Run complete workspace coverage and emit the HTML report"
	@echo "  make test-roms            Fetch and run all local curated promoted ROM suites"
	@echo "  make test-roms-real-boot  Fetch and run local curated promoted RealBoot-compatible ROM suites through verified RealBoot"
	@echo "  make test-roms-extra      Fetch and run the exploratory/internal extra ROM suites"
	@echo "  make test-roms-extra-real-boot Fetch and run the exploratory/internal extra ROM suites through verified RealBoot"
	@echo "  make test-roms-docboy     Fetch and run all exploratory DocBoy single-machine ROM suites"
	@echo "  make test-roms-docboy-real-boot Fetch and run all exploratory DocBoy single-machine ROM suites through verified RealBoot"
	@echo "  make test-roms-gbmicrotest Fetch and run the gbmicrotest report suite"
	@echo "  make test-roms-gbmicrotest-real-boot Fetch and run the gbmicrotest report suite through verified RealBoot"
	@echo "  make test-roms-cgb-extra  Fetch and run the exploratory/internal CGB ROM suites"
	@echo "  make test-roms-cgb-extra-real-boot Fetch and run the exploratory/internal CGB ROM suites through verified RealBoot"
	@echo "  make fetch-test-roms      Materialize tests from the pinned upstream source(s) using temporary checkout(s)"
	@echo "                           Set REPORT=legacy FAMILIES=\"ax6 samesuite\"; direct fetches require an explicit report and one or more explicit families"
	@echo "                           Set REPORT=legacy for legacy extra families, REPORT=docboy for DocBoy single-machine families, REPORT=gbmicrotest for gbmicrotest, or REPORT=gb-emulator-shootout for promoted families"
	@echo "  make run-acid             Fetch and run the curated Acid suite"
	@echo "  make run-ax6-dmg          Fetch and run the extra AX6 DMG RTC suite"
	@echo "  make run-samesuite        Fetch and run the promoted consolidated SameSuite suite"
	@echo "  make run-samesuite-dmg-extra Fetch and run the extra SameSuite DMG suite"
	@echo "  make run-samesuite-cgb    Fetch and run the extra SameSuite CGB variant suite"
	@echo "  make run-mooneye-sgb-boot-regs Fetch and run the extra Mooneye SGB/SGB2 boot register suite"
	@echo "  make run-magen-cgb        Fetch and run the extra Magen CGB suite"
	@echo "  make run-little-things-gb Fetch and run the extra little-things-gb DMG suite"
	@echo "  make run-little-things-gb-cgb Fetch and run the extra little-things-gb CGB suite"
	@echo "  make run-gbmicrotest      Fetch and run the gbmicrotest report suite"
	@echo "  make run-docboy-dmg       Fetch and run the DocBoy docboy/* DMG suite"
	@echo "  make run-docboy-cgb       Fetch and run the DocBoy native CGB suite"
	@echo "  make run-docboy-cgb-dmg   Fetch and run the DocBoy CGB GB-compatible suite"
	@echo "  make run-docboy-cgb-dmg-ext Fetch and run the experimental DocBoy CGB DMG-ext suite"
	@echo "  make run-mealybug-cgb   Fetch and run the exploratory/internal Mealybug CGB suite"
	@echo "  make run-blargg-cpu-instrs Fetch and run the Blargg CPU instruction chunk"
	@echo "  make run-blargg-dmg-sound Fetch and run the Blargg DMG sound chunk"
	@echo "  make run-blargg-timing-memory-oam Fetch and run the Blargg timing/memory/OAM chunk"
	@echo "  make run-daid             Fetch and run the local Daid suite"
	@echo "  make run-mooneye-acceptance Fetch and run the Mooneye acceptance/manual chunk"
	@echo "  make run-mooneye-mbc1-mbc5 Fetch and run the Mooneye emulator-only MBC1/MBC5 chunk"
	@echo "  make run-mooneye-mbc2     Fetch and run the Mooneye emulator-only MBC2 chunk"
	@echo "  make run-mooneye-cgb      Fetch and run the exploratory/internal Mooneye CGB PPU suite"
	@echo "  make run-ashiepaws          Fetch and run the curated Ashiepaws suite"
	@echo "  make run-cpp              Fetch and run the curated cpp MBC3/SGB suite"
	@echo "  make run-mealybug         Fetch and run the local Mealybug DMG suite"
	@echo "  make run-cgb-boot-hwio    Fetch and run the exploratory/internal CGB boot HWIO suite"
	@echo "  make run-blargg-cgb-sound Fetch and run the curated Blargg CGB sound suite"
	@echo "  make run-samesuite-apu Fetch and run the exploratory SameSuite APU suite"
	@echo "  make run-ax6-cgb          Fetch and run the curated AX6 CGB MBC3 RTC suite"

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
	$(MAKE) run-blargg-cpu-instrs || status=$$?; \
	$(MAKE) run-blargg-dmg-sound || status=$$?; \
	$(MAKE) run-blargg-timing-memory-oam || status=$$?; \
	$(MAKE) run-daid || status=$$?; \
	$(MAKE) run-mooneye-acceptance || status=$$?; \
	$(MAKE) run-mooneye-mbc1-mbc5 || status=$$?; \
	$(MAKE) run-mooneye-mbc2 || status=$$?; \
	$(MAKE) run-ashiepaws || status=$$?; \
	$(MAKE) run-cpp || status=$$?; \
	$(MAKE) run-mealybug || status=$$?; \
	$(MAKE) run-samesuite || status=$$?; \
	$(MAKE) run-blargg-cgb-sound || status=$$?; \
	$(MAKE) run-samesuite-apu || status=$$?; \
	$(MAKE) run-ax6-cgb || status=$$?; \
	exit $$status

test-roms-real-boot: require-boot-rom-root
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-acid
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-blargg-cpu-instrs
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-blargg-dmg-sound
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-blargg-timing-memory-oam
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-daid
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-mooneye-acceptance
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-mooneye-mbc1-mbc5
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-mooneye-mbc2
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-ashiepaws
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-cpp
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-mealybug
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-blargg-cgb-sound
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-samesuite-apu
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-ax6-cgb

test-roms-extra:
	$(MAKE) run-ax6-dmg
	$(MAKE) run-samesuite-dmg-extra
	$(MAKE) run-mooneye-sgb-boot-regs
	$(MAKE) run-little-things-gb

test-roms-extra-real-boot: require-boot-rom-root
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-ax6-dmg
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-samesuite-dmg-extra
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-little-things-gb

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

test-roms-gbmicrotest:
	$(MAKE) run-gbmicrotest

test-roms-gbmicrotest-real-boot: require-boot-rom-root
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-gbmicrotest

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
	@if [ -z "$(strip $(REPORT))" ]; then echo "REPORT is required; use REPORT=$(LEGACY_REPORT), REPORT=$(DOCBOY_REPORT), REPORT=$(GBMICROTEST_REPORT), or REPORT=$(GB_EMULATOR_SHOOTOUT_REPORT)"; exit 2; fi
	cargo run --release -q -p gb-test-runner --bin fetch_test_roms -- $(REPORT) $(FAMILIES)

require-boot-rom-root:
	@if [ -z "$$GB_CYCLE_BOOT_ROM_ROOT" ]; then echo "GB_CYCLE_BOOT_ROM_ROOT must point at the directory containing verified boot ROM assets for RealBoot ROM test targets"; exit 2; fi

run-acid:
	$(MAKE) fetch-test-roms REPORT=$(GB_EMULATOR_SHOOTOUT_REPORT) FAMILIES=acid
	$(RUN_ROM_SUITE) --suite acid --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/acid

run-ax6-dmg:
	$(MAKE) fetch-test-roms REPORT=$(LEGACY_REPORT) FAMILIES=ax6
	$(RUN_ROM_SUITE) --suite ax6-dmg-extra --failure-artifact-root $(LEGACY_TEST_ARTIFACT_ROOT)/ax6

run-samesuite:
	$(MAKE) fetch-test-roms REPORT=$(GB_EMULATOR_SHOOTOUT_REPORT) FAMILIES=samesuite
	$(RUN_ROM_SUITE) --suite samesuite --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/samesuite

run-samesuite-dmg-extra:
	$(MAKE) fetch-test-roms REPORT=$(LEGACY_REPORT) FAMILIES=samesuite
	$(RUN_ROM_SUITE) --suite samesuite-dmg-extra --failure-artifact-root $(LEGACY_TEST_ARTIFACT_ROOT)/samesuite

run-samesuite-cgb:
	$(MAKE) fetch-test-roms REPORT=$(LEGACY_REPORT) FAMILIES=samesuite
	$(RUN_ROM_SUITE) --suite samesuite-cgb-extra --failure-artifact-root $(LEGACY_TEST_ARTIFACT_ROOT)/samesuite-cgb

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
	$(MAKE) fetch-test-roms REPORT=$(GBMICROTEST_REPORT) FAMILIES=gbmicrotest
	$(RUN_ROM_SUITE) --suite gbmicrotest --failure-artifact-root $(GBMICROTEST_ARTIFACT_ROOT)/gbmicrotest

run-docboy-dmg:
	$(MAKE) fetch-test-roms REPORT=$(DOCBOY_REPORT) FAMILIES=docboy-dmg
	$(RUN_ROM_SUITE) --suite docboy-dmg --failure-artifact-root $(DOCBOY_ARTIFACT_ROOT)/docboy-dmg

run-docboy-cgb:
	$(MAKE) fetch-test-roms REPORT=$(DOCBOY_REPORT) FAMILIES=docboy-cgb
	$(RUN_ROM_SUITE) --suite docboy-cgb --failure-artifact-root $(DOCBOY_ARTIFACT_ROOT)/docboy-cgb

run-docboy-cgb-dmg:
	$(MAKE) fetch-test-roms REPORT=$(DOCBOY_REPORT) FAMILIES=docboy-cgb-dmg
	$(RUN_ROM_SUITE) --suite docboy-cgb-dmg --failure-artifact-root $(DOCBOY_ARTIFACT_ROOT)/docboy-cgb-dmg

run-docboy-cgb-dmg-ext:
	$(MAKE) fetch-test-roms REPORT=$(DOCBOY_REPORT) FAMILIES=docboy-cgb-dmg-ext
	$(RUN_ROM_SUITE) --suite docboy-cgb-dmg-ext --failure-artifact-root $(DOCBOY_ARTIFACT_ROOT)/docboy-cgb-dmg-ext

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
	$(RUN_ROM_SUITE) --suite daid --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/daid

run-mooneye-acceptance:
	$(MAKE) fetch-test-roms REPORT=$(GB_EMULATOR_SHOOTOUT_REPORT) FAMILIES=mooneye
	$(RUN_ROM_SUITE) --suite mooneye-acceptance-manual-misc --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/mooneye-acceptance-manual-misc

run-mooneye-mbc1-mbc5:
	$(MAKE) fetch-test-roms REPORT=$(GB_EMULATOR_SHOOTOUT_REPORT) FAMILIES=mooneye
	$(RUN_ROM_SUITE) --suite mooneye-emulator-mbc1-mbc5 --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/mooneye-emulator-mbc1-mbc5

run-mooneye-mbc2:
	$(MAKE) fetch-test-roms REPORT=$(GB_EMULATOR_SHOOTOUT_REPORT) FAMILIES=mooneye
	$(RUN_ROM_SUITE) --suite mooneye-emulator-mbc2 --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/mooneye-emulator-mbc2

run-ashiepaws:
	$(MAKE) fetch-test-roms REPORT=$(GB_EMULATOR_SHOOTOUT_REPORT) FAMILIES=ashiepaws
	$(RUN_ROM_SUITE) --suite ashiepaws --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/ashiepaws

run-cpp:
	$(MAKE) fetch-test-roms REPORT=$(GB_EMULATOR_SHOOTOUT_REPORT) FAMILIES=cpp
	$(RUN_ROM_SUITE) --suite cpp --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/cpp

run-mealybug:
	$(MAKE) fetch-test-roms REPORT=$(GB_EMULATOR_SHOOTOUT_REPORT) FAMILIES=mealybug-tearoom-tests
	$(RUN_ROM_SUITE) --suite mealybug-tearoom-tests --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/mealybug-tearoom-tests

run-mealybug-cgb:
	$(MAKE) fetch-test-roms REPORT=$(LEGACY_REPORT) FAMILIES=mealybug-tearoom-tests
	$(RUN_ROM_SUITE) --suite mealybug-tearoom-cgb-extra --failure-artifact-root $(LEGACY_TEST_ARTIFACT_ROOT)/mealybug-cgb

run-cgb-boot-hwio:
	$(MAKE) fetch-test-roms REPORT=$(LEGACY_REPORT) FAMILIES=mooneye
	$(RUN_ROM_SUITE) --suite cgb-boot-hwio --failure-artifact-root $(LEGACY_TEST_ARTIFACT_ROOT)/cgb-boot-hwio

run-mooneye-cgb:
	$(MAKE) fetch-test-roms REPORT=$(LEGACY_REPORT) FAMILIES=mooneye
	$(RUN_ROM_SUITE) --suite mooneye-cgb-extra --failure-artifact-root $(LEGACY_TEST_ARTIFACT_ROOT)/mooneye-cgb

run-blargg-cgb-sound:
	$(MAKE) fetch-test-roms REPORT=$(GB_EMULATOR_SHOOTOUT_REPORT) FAMILIES=blargg
	$(RUN_ROM_SUITE) --suite blargg-cgb-sound --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/blargg-cgb-sound

run-samesuite-apu:
	$(MAKE) fetch-test-roms REPORT=$(GB_EMULATOR_SHOOTOUT_REPORT) FAMILIES=samesuite
	$(RUN_ROM_SUITE) --suite samesuite-apu --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/samesuite-apu

run-ax6-cgb:
	$(MAKE) fetch-test-roms REPORT=$(GB_EMULATOR_SHOOTOUT_REPORT) FAMILIES=ax6
	$(RUN_ROM_SUITE) --suite ax6 --failure-artifact-root $(GB_EMULATOR_SHOOTOUT_ARTIFACT_ROOT)/ax6
