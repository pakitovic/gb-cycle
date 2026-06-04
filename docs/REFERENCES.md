# References

Consult primary documentation and hardware research before emulator source code. Emulator implementations and executable ROM suites are comparison tools; they do not override hardware evidence or subsystem handbooks.

## Project reference order

1. Real hardware evidence, official/manual documentation, and hardware research.
2. The owning local handbook under `docs/hardware/*.md`, plus project policy in [`ARCHITECTURE.md`](ARCHITECTURE.md), [`TESTING.md`](TESTING.md), and [`info/ROM-SUITES.md`](info/ROM-SUITES.md).
3. Executable ROM suites and retained artifacts that expose the behavior under test.
4. Open-source emulator source as implementation cross-checks, never as code to copy blindly.

## Primary hardware documentation

- [Pan Docs](https://gbdev.io/pandocs/) — first stop for documented hardware behavior, registers, memory map, boot flow, cartridge types, audio, serial/link, and CGB features.
- [The Cycle-Accurate Game Boy Docs](https://github.com/AntonioND/giibiiadvance/blob/master/docs/TCAGBD.pdf) — timing-oriented reference for CPU, PPU, DMA, and bus behavior.
- [Game Boy: Complete Technical Reference](https://gekkio.fi/files/gb-docs/gbctr.pdf) — detailed register, timing, revision, and hardware-behavior reference.

## Hardware research and databases

- [Gekkio research hub](https://gekkio.fi/) and [gb-research](https://github.com/Gekkio/gb-research) — hardware measurements, edge cases, and supporting research notes.
- [gb-schematics](https://github.com/Gekkio/gb-schematics) — circuit-level context when behavior needs board/schematic support.
- [Game Boy Hardware Database](https://gbhwdb.gekkio.fi/) and its [cartridge catalog](https://gbhwdb.gekkio.fi/cartridges/gb.html) — hardware evidence for boards, mapper labels, ROM IDs, PCB photos, and cartridge-family research; do not use it as a replacement for documented header-compatibility policy.
- [Dan Docs](https://shonumi.github.io/dandocs.html) — strongest broad written reference for obscure accessories and link/cartridge-adjacent hardware such as DMG-07, CGB infrared, Barcode Boy, Pocket Sonar, Mobile Adapter GB, and sewing-machine adapters.
- [ProjectPokemon Mystery Gift IR reverse-engineering post](https://projectpokemon.org/home/forums/topic/43930-mystery-gift-reverse-engineering-of-ir-protocol/#comment-232992) — reference for Generation 2 IR Mystery Gift and Pokémon Pikachu 2 GS behavior.

## Active executable references and ROM-suite sources

- [GBEmulatorShootout dashboard](https://gbdev.io/GBEmulatorShootout/) and [gbdev/GBEmulatorShootout repository](https://github.com/gbdev/GBEmulatorShootout) — primary catalog and maturity signal for curated ROM-suite selection; not a hardware-behavior authority by itself.
- [gb-cycle GBEmulatorShootout fork dashboard](https://pakitovic.github.io/GBEmulatorShootout/) — project-facing summary of the current gb-cycle counted rows.
- [blargg test ROMs](https://github.com/retrio/gb-test-roms) — CPU, timing, memory, OAM, and DMG/CGB sound ROMs, currently consumed through the curated GBEmulatorShootout source path rather than a direct retrio checkout.
- [Mooneye GB](https://github.com/Gekkio/mooneye-gb) — acceptance, boot, CPU/interrupt/timer/DMA, serial, and model-specific executable tests plus documentary reasoning.
- [dmg-acid2](https://github.com/mattcurrie/dmg-acid2), [cgb-acid2](https://github.com/mattcurrie/cgb-acid2), and Acid-family CGB hardening ROMs — framebuffer-oracle visual PPU references.
- [mealybug-tearoom-tests](https://github.com/mattcurrie/mealybug-tearoom-tests) — DMG/CGB PPU timing and LCD pipeline framebuffer-oracle rows.
- [SameSuite](https://github.com/LIJI32/SameSuite) — CGB APU, DMA, palette/PPU, and SGB command/multiplayer executable tests.
- [docboy-test-suite](https://github.com/Docheinstein/docboy-test-suite/) and [docboy repository](https://github.com/Docheinstein/docboy) — large high-precision DMG/CGB/linked-session ROM corpus and a structural implementation cross-check for timing-sensitive behavior.
- GBEmulatorShootout/DocBoy-sourced family names actively used by manifests include `acid`, `ashiepaws`, `ax6`, `blargg`, `cpp`, `daid`, `gbmicrotest`, `little-things-gb`, `magen`, `mealybug-tearoom-tests`, `mooneye`, `samesuite`, `docboy-dmg`, `docboy-cgb`, `docboy-cgb-dmg`, and `docboy-cgb-dmg-ext`; pinned source revisions and file hashes for report-local cargo lanes live in `crates/gb-test-runner/data/reports.toml` plus each report-local `sources.report.toml`; linked-session manifests use the report-local `*.link.suite.toml` model and share the same report registry.

## Open-source emulator consultation tier

Use this default order only when implementation examples or behavioral cross-checks are needed and no subsystem handbook names a stronger oracle. The order reflects sources the project has actually used for current DMG/CGB/SGB work, not a universal accuracy ranking.

1. [SameBoy](https://github.com/LIJI32/SameBoy) — first implementation cross-check when emulator source is useful, especially for DMG/CGB/SGB behavior that already has primary evidence or executable-test pressure.
2. [docboy](https://github.com/Docheinstein/docboy) — high-precision structural cross-check for PPU, bus domains, startup/boot residue, linked-session rows, and large DocBoy suite behavior.
3. [GameRoy](https://github.com/Rodrigodd/gameroy) — Rust-oriented structure and corroborating CPU/bus/PPU/APU reference.
4. [Mooneye GB](https://github.com/Gekkio/mooneye-gb) — implementation plus documentary reasoning for CPU, timer, interrupts, boot, DMA, and model edge cases.
5. [GBE+](https://github.com/shonumi/gbe-plus) — specialized cross-check for obscure peripherals, accessories, and link/cartridge-adjacent flows after primary documentation and Dan Docs.
6. [mGBA](https://github.com/mgba-emu/mgba) — specialized reference for external `.sav` compatibility shapes, not a broad Game Boy oracle for this project.

## Usage notes

- Prefer subsystem-specific references over this generic list when a hardware handbook narrows the oracle order.
- Treat GBEmulatorShootout as catalog/maturity signal and executable source inventory, not as hardware truth.
- Use docboy-test-suite and DocBoy rows as high-value executable references, but keep experimental/extra DocBoy rows report-isolated until intentionally promoted.
- Use GBE+ and Dan Docs for obscure accessories and IR/peripheral research, then lock project behavior with explicit tests or manifests.
- Record any intentional source conflict in the owning hardware doc or [`TODO.md`](TODO.md), including the exact ROM, mode, oracle, and evidence needed to resolve it.
