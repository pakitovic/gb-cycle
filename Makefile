.DEFAULT_GOAL := ci
ROM_PROFILE ?= release-max
LEGACY_REPORT := legacy
DOCBOY_REPORT := docboy
GBMICROTEST_REPORT := gbmicrotest
GB_EMULATOR_SHOOTOUT_REPORT := gb-emulator-shootout
LEGACY_TEST_ARTIFACT_ROOT := test/.artifacts
RUN_ROM_SUITE = cargo run --profile $(ROM_PROFILE) -q -p gb-test-runner --bin run_rom_suite --

.PHONY: help setup hooks tools ci coverage coverage-check test-roms-extra test-roms-extra-real-boot test-roms-cgb-extra test-roms-cgb-extra-real-boot fetch-test-roms require-boot-rom-root run-ax6-dmg run-samesuite-dmg-extra run-samesuite-cgb run-mooneye-sgb-boot-regs run-magen-cgb run-little-things-gb run-little-things-gb-cgb run-mooneye-cgb run-mealybug-cgb run-cgb-boot-hwio

help:
	@echo "Available targets:"
	@echo "  make setup                Configure git hooks and install local cargo tools"
	@echo "  make hooks                Configure repository git hooks"
	@echo "  make tools                Install local cargo tools used by this repository"
	@echo "  make ci                   Run the local pre-push gate (fmt, clippy, typos, deny, workspace tests via coverage, per-crate coverage check)"
	@echo "  make coverage-check       Run one workspace coverage sweep, then enforce per-crate coverage gates"
	@echo "  make coverage             Run complete workspace coverage and emit the HTML report"
	@echo "  make test-roms-extra      Fetch and run the exploratory/internal extra ROM suites"
	@echo "  make test-roms-extra-real-boot Fetch and run the exploratory/internal extra ROM suites through verified RealBoot"
	@echo "  make test-roms-cgb-extra  Fetch and run the exploratory/internal CGB ROM suites"
	@echo "  make test-roms-cgb-extra-real-boot Fetch and run the exploratory/internal CGB ROM suites through verified RealBoot"
	@echo "  make fetch-test-roms      Materialize tests from the pinned upstream source(s) using temporary checkout(s)"
	@echo "                           Set REPORT=legacy FAMILIES=\"ax6 samesuite\"; direct fetches require an explicit report and one or more explicit families"
	@echo "                           Set REPORT=legacy for legacy extra families, REPORT=docboy for DocBoy single-machine families, REPORT=gbmicrotest for gbmicrotest, or REPORT=gb-emulator-shootout for promoted families"
	@echo "  make run-ax6-dmg          Fetch and run the extra AX6 DMG RTC suite"
	@echo "  make run-samesuite-dmg-extra Fetch and run the extra SameSuite DMG suite"
	@echo "  make run-samesuite-cgb    Fetch and run the extra SameSuite CGB variant suite"
	@echo "  make run-mooneye-sgb-boot-regs Fetch and run the extra Mooneye SGB/SGB2 boot register suite"
	@echo "  make run-magen-cgb        Fetch and run the extra Magen CGB suite"
	@echo "  make run-little-things-gb Fetch and run the extra little-things-gb DMG suite"
	@echo "  make run-little-things-gb-cgb Fetch and run the extra little-things-gb CGB suite"
	@echo "  make run-mealybug-cgb   Fetch and run the exploratory/internal Mealybug CGB suite"
	@echo "  make run-mooneye-cgb      Fetch and run the exploratory/internal Mooneye CGB PPU suite"
	@echo "  make run-cgb-boot-hwio    Fetch and run the exploratory/internal CGB boot HWIO suite"

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

test-roms-extra:
	$(MAKE) run-ax6-dmg
	$(MAKE) run-samesuite-dmg-extra
	$(MAKE) run-mooneye-sgb-boot-regs
	$(MAKE) run-little-things-gb

test-roms-extra-real-boot: require-boot-rom-root
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-ax6-dmg
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-samesuite-dmg-extra
	GB_CYCLE_TEST_ROM_STARTUP=real-boot $(MAKE) run-little-things-gb

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

run-ax6-dmg:
	$(MAKE) fetch-test-roms REPORT=$(LEGACY_REPORT) FAMILIES=ax6
	$(RUN_ROM_SUITE) --suite ax6-dmg-extra --failure-artifact-root $(LEGACY_TEST_ARTIFACT_ROOT)/ax6

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

run-mealybug-cgb:
	$(MAKE) fetch-test-roms REPORT=$(LEGACY_REPORT) FAMILIES=mealybug-tearoom-tests
	$(RUN_ROM_SUITE) --suite mealybug-tearoom-cgb-extra --failure-artifact-root $(LEGACY_TEST_ARTIFACT_ROOT)/mealybug-cgb

run-cgb-boot-hwio:
	$(MAKE) fetch-test-roms REPORT=$(LEGACY_REPORT) FAMILIES=mooneye
	$(RUN_ROM_SUITE) --suite cgb-boot-hwio --failure-artifact-root $(LEGACY_TEST_ARTIFACT_ROOT)/cgb-boot-hwio

run-mooneye-cgb:
	$(MAKE) fetch-test-roms REPORT=$(LEGACY_REPORT) FAMILIES=mooneye
	$(RUN_ROM_SUITE) --suite mooneye-cgb-extra --failure-artifact-root $(LEGACY_TEST_ARTIFACT_ROOT)/mooneye-cgb
