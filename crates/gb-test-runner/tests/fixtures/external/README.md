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
make test-external-blargg-dmg
```

That command reads `crates/gb-test-runner/external-rom-sources.toml`, downloads
the pinned upstream revision, verifies the required file hashes, and populates
the gitignored `/.roms/external-test/` store that both local runs and CI use.

The ignored integration tests in
`crates/gb-test-runner/tests/external.rs` use that environment variable and
path contract directly. The ROM binaries remain external; the repo stores only
the typed suite metadata, the fetch manifest, and the harness behavior.

Current green official cases on top of the `cpu_instrs` individual block are:

- `retrio/blargg cpu_instrs/cpu_instrs.gb`
- `retrio/blargg instr_timing`
- `retrio/blargg halt_bug`
- `retrio/blargg mem_timing`
- `retrio/blargg mem_timing-2`
- `retrio/blargg mem_timing/individual 01..03`
- `retrio/blargg mem_timing-2/rom_singles 01..03`

Repository-gated external DMG block:

- `make local` fetches and runs the green non-APU, non-CGB Blargg DMG block
  as the full local pipeline
- GitHub runs the same block in the separate `external-roms` workflow
- that block currently includes:
  `cpu_instrs` smoke
  `cpu_instrs/cpu_instrs.gb`
  `instr_timing`
  `halt_bug`
  `mem_timing`
  `mem_timing/individual`
- `make check` intentionally stays lighter and does not fetch or execute the
  external ROM block
- it intentionally excludes `oam_bug` and the APU suites
  until those ROMs are green and intentionally promoted into the default
  external-ROM gate

Commercial or otherwise non-redistributable ROMs do not belong in this store.
Keep those local-only assets under the separate gitignored
`/.roms/local-commercial/` root and out of CI.
