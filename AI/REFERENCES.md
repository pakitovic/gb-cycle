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

## Test ROMs and executable references

- blargg test ROMs
- dmg-acid2 — https://github.com/mattcurrie/dmg-acid2
- cgb-acid2 — https://github.com/mattcurrie/cgb-acid2
- mealybug-tearoom-tests — https://github.com/mattcurrie/mealybug-tearoom-tests
- Mooneye GB test suite — https://github.com/Gekkio/mooneye-gb

## Community indexes

- gbdev resources — https://gbdev.io/resources/
- awesome-gbdev — https://github.com/gbdev/awesome-gbdev

## Audio references

- Gameboy sound hardware — https://gbdev.gg8.se/wiki/articles/Gameboy_sound_hardware
- GBSOUND.txt — https://github.com/gbdev/awesome-gbdev/blob/master/src/audio/GBSOUND.txt

## Opcode references

- gbdev opcode table — https://gbdev.io/gb-opcodes/optables/
- Pastraiser opcode table — https://www.pastraiser.com/cpu/gameboy/gameboy_opcodes.html

## Open-source emulator consultation tier

Use this order when implementation examples or behavioral cross-checks are needed and no subsystem-specific handbook names a stronger oracle.
This default order balances coverage, maintainability, and accessibility; it is not a literal accuracy ranking for every subsystem.

1. SameBoy — https://github.com/LIJI32/SameBoy
2. binjgb — https://github.com/binji/binjgb
3. GameRoy — https://github.com/Rodrigodd/gameroy
4. accurateboy — https://github.com/Atem2069/accurateboy
5. Mooneye GB — https://github.com/Gekkio/mooneye-gb
6. Danger Boy — https://github.com/austinthresher/dangerboy
7. Gambatte — https://github.com/gb-archive/gambatte

## How to use the consultation tier

- Do not copy code blindly.
- Use this order as the consultation priority when several approaches exist.
- If references disagree, prefer the best documented behavior first.
- Prefer the strongest subsystem-specific reference, then explain the choice.
- Treat emulator code as a comparison aid, not as absolute truth.

## Practical specialization by reference

- SameBoy: best general reference and the practical ceiling for DMG/CGB behavior, LCD timing, audio, and compatibility study.
- binjgb: strong reference for timing, CPU/bus behavior, and compact architecture.
- GameRoy: useful reference for idiomatic Rust-oriented structure.
- accurateboy: especially valuable for PPU, fetcher, and pixel FIFO behavior.
- Mooneye GB: strong documentary and edge-case reasoning reference.
- Danger Boy: useful smaller codebase for DMG timing study.
- Gambatte: historical high-accuracy reference and a strong corroborating oracle for practical accuracy and corner cases.
