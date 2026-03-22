# External ROM Fixtures

This directory reserves stable names for external ROM-suite inputs that are not
checked into the repository.

Current external harness contract:

- root environment variable: `GB_CYCLE_RETRIO_GB_TEST_ROMS_ROOT`
- default repo-managed root when the environment variable is unset:
  `/.roms/external-test/retrio-gb-test-roms/`
- first reserved suite: `retrio-blargg-cpu-smoke`
- current reserved ROM paths under that root:
  the full `cpu_instrs/individual/` block
  `cpu_instrs/cpu_instrs.gb`
  `01-special.gb`
  `02-interrupts.gb`
  `03-op sp,hl.gb`
  `04-op r,imm.gb`
  `05-op rp.gb`
  `06-ld r,r.gb`
  `07-jr,jp,call,ret,rst.gb`
  `08-misc instrs.gb`
  `09-op r,r.gb`
  `10-bit ops.gb`
  `11-op a,(hl).gb`
  plus `instr_timing/instr_timing.gb`
  plus `halt_bug.gb`
  plus `mem_timing/mem_timing.gb`
  plus `mem_timing-2/mem_timing.gb`

Repo-managed external test assets are fetched with:

```bash
make fetch-external-roms
make test-external-dmg
```

Those commands read `crates/gb-test-runner/external-rom-sources.toml`, download
the pinned upstream revision, verify the required file hashes, and populate
the gitignored `/.roms/external-test/` store that both local runs and CI use.

The ignored integration tests in
`crates/gb-test-runner/tests/external.rs` use that environment variable and
path contract directly. The ROM binaries remain external; the repo stores only
the typed suite metadata, the fetch manifest, and the harness behavior.

To inspect the current built-in suite catalog together with oracle channel,
capture plan, and retained-artifact policy, run:

```bash
cargo run -p gb-test-runner --bin run_rom_suite -- --list-detailed
```

To inspect the current early hardening checklist by subsystem, run:

```bash
cargo run -p gb-test-runner --bin run_rom_suite -- --early-checklist
```

To compare one built-in suite against imported SameBoy artifacts,
run:

```bash
cargo run -p gb-test-runner --bin run_differential -- \
  --oracle sameboy \
  --oracle-layout sameboy-tester \
  --suite gbdev-dmg-acid2
```

If `--oracle-artifact-root` is omitted, the default repo-local root is
`/.oracles/<oracle>/<layout>/`, so this example reads from
`/.oracles/sameboy/sameboy-tester/`.

The default `case-bundle` layout expects one subdirectory per case id, using
the same filenames that the local runner already emits for retained artifacts:
`serial.txt`, `memory_text_output.txt`, `blargg_console.txt`,
`framebuffer.pgm`, `trace.txt`, and optional archived context such as
`snapshot.txt`.

The `sameboy-tester` layout is currently framebuffer-only and expects SameBoy
Tester image artifacts mirrored by ROM-relative path, for example
`testroms/acid/dmg-acid2.bmp` or `.tga` under the oracle root.

To materialize those artifacts with SameBoy's internal `tester` target, run:

```bash
cargo run -p gb-test-runner --bin run_sameboy_tester -- \
  --sameboy-root /path/to/SameBoy \
  --suite gbdev-dmg-acid2 \
  --image-format bmp \
  --build-if-missing
```

This command stages the ROMs under the default repo-local oracle root
`/.oracles/sameboy/sameboy-tester/`, runs SameBoy Tester, and leaves `.bmp` /
`.tga` plus `.log` files in the same tree. The current path is intentionally
limited to framebuffer-oracle suites. Because SameBoy Tester always boots
through a boot ROM, use it as end-of-test framebuffer evidence, not as proof
of startup-path equivalence for local `SkipBoot` runs.
The wrapper intentionally does not override SameBoy's own boot-ROM path. If an
oracle run needs a specific SameBoy firmware choice, make that choice in the
SameBoy checkout or build itself rather than through `gb-test-runner`.

Current green official cases on top of the `cpu_instrs` individual block are:

- `retrio/blargg cpu_instrs/cpu_instrs.gb`
- `retrio/blargg instr_timing`
- `retrio/blargg halt_bug`
- `retrio/blargg mem_timing`
- `retrio/blargg mem_timing-2`
- `retrio/blargg mem_timing/individual 01..03`
- `retrio/blargg mem_timing-2/rom_singles 01..03`
- `retrio/blargg oam_bug/rom_singles 1..6,8`

Current repo-gated external PPU suite:

- `gbdev/GBEmulatorShootout acid/dmg-acid2.gb`
- oracle channel: framebuffer fixture derived from the upstream
  `testroms/acid/dmg-acid2.png` reference
- current built-in suite name: `gbdev-dmg-acid2`
- current source env var: `GB_CYCLE_GBEMU_SHOOTOUT_ROOT`
- this suite is part of `make test-external-dmg`, `make local`, and the
  `external-roms` workflow

Repository-gated external DMG block:

- `make local` fetches and runs the green non-APU, non-CGB external DMG block
  as the full local pipeline
- GitHub runs the same block in the separate `external-roms` workflow
- that block currently includes:
  `cpu_instrs` smoke
  `cpu_instrs/cpu_instrs.gb`
  `instr_timing`
  `halt_bug`
  `mem_timing`
  `mem_timing/individual`
  `oam_bug/rom_singles 1..6,8`
  `gbdev-dmg-acid2`
- `make check` intentionally stays lighter and does not fetch or execute the
  external ROM block
- it still excludes the upstream `oam_bug.gb`, `7-timing_effect.gb`, and the
  APU suites from the default external-ROM gate

Commercial or otherwise non-redistributable ROMs do not belong in this store.
Keep those local-only assets under the separate gitignored
`/.roms/local-commercial/` root and out of CI.
