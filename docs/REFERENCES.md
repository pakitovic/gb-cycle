# References

Primary documentation should be consulted before using emulator source code as guidance.

## Primary documentation

- Pan Docs — https://gbdev.io/pandocs/
- The Cycle-Accurate Game Boy Docs (AntonioND) — https://github.com/AntonioND/giibiiadvance/blob/master/docs/TCAGBD.pdf
- Game Boy: Complete Technical Reference (Gekkio) — https://gekkio.fi/files/gb-docs/gbctr.pdf

## Hardware research

- Gekkio research hub — https://gekkio.fi/
- gb-research — https://github.com/Gekkio/gb-research
- gb-schematics — https://github.com/Gekkio/gb-schematics
- Game Boy Hardware Database — https://gbhwdb.gekkio.fi/
- Game Boy Hardware Database cartridge catalog — https://gbhwdb.gekkio.fi/cartridges/gb.html
- Dan Docs — https://shonumi.github.io/dandocs.html

## Test ROMs and executable references

- blargg test ROMs
- retrio/gb-test-roms — https://github.com/retrio/gb-test-roms
- GB Emulator Shootout test ROM catalog and results — https://gbdev.io/GBEmulatorShootout/
- dmg-acid2 — https://github.com/mattcurrie/dmg-acid2
- cgb-acid2 — https://github.com/mattcurrie/cgb-acid2
- mealybug-tearoom-tests — https://github.com/mattcurrie/mealybug-tearoom-tests
- Mooneye GB test suite — https://github.com/Gekkio/mooneye-gb
- SameSuite — https://github.com/LIJI32/SameSuite
- docboy-test-suite — https://github.com/Docheinstein/docboy-test-suite/
- GB Accuracy Tests — see the awesome-gbdev testing section below
- 144p Test Suite — see the awesome-gbdev testing section below
- MBC3 RTC test ROMs — see the awesome-gbdev testing section below

## Community indexes

- gbdev resources — https://gbdev.io/resources/
- awesome-gbdev — https://github.com/gbdev/awesome-gbdev
- use the awesome-gbdev testing section to discover and cross-check broader external DMG-closure suites that are not listed above directly

## Reference usage notes

- Treat the Game Boy Hardware Database cartridge catalog as hardware evidence for cartridge boards, mapper labels, ROM IDs, and submitted PCB photos; use it to support cartridge-family research, not as a replacement for the header parser's documented compatibility rules.
- Treat Dan Docs as the strongest broad reference for obscure accessories and special hardware such as the Barcode Boy, DMG-07 4-Player Adapter, GBC infrared, Pocket Sonar, Mobile Adapter GB, sewing-machine adapters, and other link-port or cartridge-adjacent devices not covered well by Pan Docs.
- Treat GB Emulator Shootout as the project-wide external ROM catalog and maturity signal for curated suite selection; do not use it as a hardware behavior source by itself.
- Treat docboy-test-suite as a high-precision executable reference for T-cycle-sensitive DMG/GBC behavior after the ordinary Blargg and Mooneye acceptance coverage is already mostly green.

## Audio references

- Gameboy sound hardware — https://gbdev.gg8.se/wiki/articles/Gameboy_sound_hardware
- GBSOUND.txt — https://github.com/gbdev/awesome-gbdev/blob/master/src/audio/GBSOUND.txt

## Opcode references

- gbdev opcode table — https://gbdev.io/gb-opcodes/optables/
- Pastraiser opcode table — https://www.pastraiser.com/cpu/gameboy/gameboy_opcodes.html

## Open-source emulator consultation tier

Use this order when implementation examples or behavioral cross-checks are needed and no subsystem-specific handbook names a stronger oracle. This default order balances current `GBEmulatorShootout` maturity signals, implementation coverage, maintainability, source readability, and the oracle workflows this repo actually supports; it is not a literal static accuracy ranking for every subsystem.

1. SameBoy — https://github.com/LIJI32/SameBoy
2. docboy — https://github.com/Docheinstein/docboy
3. binjgb — https://github.com/binji/binjgb
4. GameRoy — https://github.com/Rodrigodd/gameroy
5. accurateboy — https://github.com/Atem2069/accurateboy
6. Mooneye GB — https://github.com/Gekkio/mooneye-gb
7. Danger Boy — https://github.com/austinthresher/dangerboy
8. Gambatte — https://github.com/gb-archive/gambatte
9. GBE+ — https://github.com/shonumi/gbe-plus

## How to use the consultation tier

- Do not copy code blindly.
- Use `GBEmulatorShootout` first for the current broad test-ROM maturity picture, then use this tier plus subsystem handbooks to choose which implementation is worth reading for the concrete behavior under study.
- If references disagree, prefer the best documented behavior first.
- Prefer the strongest subsystem-specific reference, then explain the choice.
- Treat emulator code as a comparison aid, not as absolute truth.

## Practical specialization by reference

- SameBoy: best general reference and the practical ceiling for DMG/CGB behavior, LCD timing, audio, and compatibility study; use it as the DMG differential oracle when comparable traces or observables are available.
- docboy: approved high-precision DMG oracle for PPU pixel FIFO, window timing, LCD restart behavior, and bus-domain/view architecture; use it as the first structural cross-check when studying DMG PPU or video-bus behavior and as a second oracle when SameBoy alone is not enough to localize a divergence.
- binjgb: strong reference for timing, CPU/bus behavior, and compact architecture.
- GameRoy: useful reference for idiomatic Rust-oriented structure.
- accurateboy: especially valuable for PPU, fetcher, and pixel FIFO behavior.
- Mooneye GB: strong documentary and edge-case reasoning reference.
- Danger Boy: useful smaller codebase for DMG timing study.
- Gambatte: historical high-accuracy reference and a useful corroborating implementation perspective for practical accuracy and corner cases, but not a repo-supported automated differential oracle path in the current project.
- GBE+: valuable implementation and UX inspiration for obscure peripherals, accessories, and link-adjacent hardware such as printer/camera/adapter-style flows; use it as a specialized cross-check after primary documentation, Dan Docs, and subsystem-specific evidence rather than as a broad accuracy oracle.

## Retained source-level cross-check notes

- SameBoy source-level timing cross-checks previously used by this repo remain useful around `Core/timing.c`, `Core/memory.c`, and `Core/sm83_cpu.c`: timer bit selection and falling-edge TIMA behavior, explicit `DIV` / `TAC` glitch handling, delayed timer interrupt visibility through a reload state, `FF50` boot-overlay disable behavior, immediate `DI`, delayed `EI`, immediate `RETI` re-enable, and bytewise stack operations for call/return/interrupt paths.
- SameBoy's public SDL frontend should not be treated as a documented headless oracle runner. The supported repo-local oracle workflows are the imported `sameboy-tester` framebuffer layout and the `case-bundle` layout documented in `docs/testing/ROM-SUITES.md`.
