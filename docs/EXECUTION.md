# Execution

This file owns the day-to-day implementation workflow: how to choose authority docs, keep changes scoped, select validation, and record follow-up work. It does not redefine hardware behavior, test-suite membership, or roadmap scope; those live in the linked owning documents.

## Working loop

1. Identify the owning subsystem or workflow and route through [`docs/index.md`](index.md) before editing.
2. Read only the relevant authority docs: hardware behavior in the matching `docs/hardware/` handbook, architecture and scheduler boundaries in [`docs/ARCHITECTURE.md`](ARCHITECTURE.md), timing vocabulary in [`docs/info/TIMING-AND-ACCURACY.md`](info/TIMING-AND-ACCURACY.md), model-axis guidance in [`docs/info/MODEL-AXES.md`](info/MODEL-AXES.md), frontend behavior in [`docs/info/CLI.md`](info/CLI.md) or [`docs/info/DESKTOP.md`](info/DESKTOP.md), validation policy in [`docs/TESTING.md`](TESTING.md), and ROM-suite mechanics in [`docs/info/ROM-SUITES.md`](info/ROM-SUITES.md).
3. Read [`docs/ROADMAP.md`](ROADMAP.md) when work maps to a phase, resumes an incomplete slice, changes sequencing, or leaves known follow-up work; read [`docs/TODO.md`](TODO.md) when adding, closing, or rewriting an active gap.
4. Read [`docs/REFERENCES.md`](REFERENCES.md) when hardware behavior, timing, oracle policy, or external-resource choice depends on evidence outside the repository.
5. State the contract, invariants, evidence source, and ownership boundary before changing behavior.
6. Implement the smallest correct change that preserves debuggability, determinism, and the T-cycle timing model.
7. Validate with the narrowest useful automated check first, then widen to integration tests, ROM suites, retained artifacts, or external cross-checks only when the behavior requires that evidence.
8. Update the owning docs in the same change when assumptions, scope, sequencing, limitations, validation policy, public commands, or authority boundaries change.

## Active resources

- [`docs/index.md`](index.md) is the map of authority boundaries; start there when ownership is unclear.
- [`docs/ARCHITECTURE.md`](ARCHITECTURE.md), [`docs/CODING-RULES.md`](CODING-RULES.md), and subsystem handbooks under `docs/hardware/` are the local source of design and hardware-model constraints.
- [`docs/TESTING.md`](TESTING.md) owns validation policy; [`docs/info/ROM-SUITES.md`](info/ROM-SUITES.md) owns external ROM materialization, report channels, and runner commands.
- [`docs/REFERENCES.md`](REFERENCES.md) owns the current consultation order for Pan Docs, AntonioND, Gekkio, GBEmulatorShootout, DocBoy, and other active references.
- `crates/gb-test-runner/data/reports.toml` and each report-local `sources.report.toml` own report fetch metadata for `cargo rom-fetch`, `cargo rom-suite`, and `cargo rom-suite-link`; update those files rather than documenting ad hoc source lists elsewhere. Local reports such as `linked` deliberately omit `sources.report.toml` because their assets are committed under `crates/gb-test-runner/data/<report>/`.
- `/test/gb-emulator-shootout/`, `/test/docboy/`, `/test/gbmicrotest/`, and standalone exploratory report roots such as `/test/mooneye/`, `/test/little-things-gb/`, `/test/turtle-tests/`, `/test/magen/`, `/test/mealybug-tearoom-tests/`, `/test/samesuite/`, `/test/wilbertpol/`, `/test/rtc3test/`, or `/test/mbc3-tester/` are generated/local evidence channels; keep before/after copies when the mandatory external-ROM regression workflow applies.

## Change policy

- Keep one behavioral change per patch when practical; group documentation-only cleanup only when it clarifies the same authority boundary or removes the same stale workflow.
- Do not mix refactors with timing fixes unless the refactor is required to make the timing fix explicit and testable.
- Before touching behavior-sensitive code, verify useful coverage exists or add focused coverage first.
- New production code normally lands with unit, integration, or ROM-backed coverage in the same change.
- If automated coverage cannot be added yet, report the reason, the remaining validation gap, and the risk explicitly, then record concrete follow-up in [`docs/TODO.md`](TODO.md) when the gap survives the change.
- Keep file moves separate from behavior changes when possible; if a move is required, update [`docs/index.md`](index.md), README links, and stale Markdown references in the same change.
- Preserve Conventional Commits for commits and pull request titles.
- Do not treat `Permissive` or `Experimental` ROM results as `Strict` accuracy evidence unless the owning docs explicitly frame the result as non-oracle exploratory data.

## Roadmap and TODO policy

- [`docs/ROADMAP.md`](ROADMAP.md) owns phase order, dependencies, scope summaries, and done criteria; roadmap order is the recommended implementation sequence, not a strict merge-order rule for independent work.
- [`docs/TODO.md`](TODO.md) owns concrete remaining work across phases; it should contain re-entry context, evidence already gathered, superseded directions not to retry first, and the highest-value next step.
- Add or update TODOs when a change leaves known gaps, deferred fixes, incomplete validation, or partially unmet phase criteria.
- Remove or rewrite TODOs when the underlying work is completed, invalidated, or superseded.
- Do not store speculative ideas in [`docs/TODO.md`](TODO.md); keep speculation in discussion until it becomes a concrete follow-up.

## Documentation policy

- Keep root `docs/` for top-level routing and project-wide policy, `docs/info/` for cross-cutting core/frontends/tooling guidance, `docs/hardware/` for subsystem behavior, and `docs/roadmap/` for phase plans.
- Link concrete Markdown file references so GitHub navigation works; keep local non-Markdown file paths as plain code unless there is a specific reason to link them.
- Do not hard-wrap Markdown prose. Keep each paragraph, bullet item, numbered item, and roadmap field on one physical line unless it is a fenced block, table, or intentional standalone formula.
- For documentation-only changes, run stale-reference/link checks plus `git diff --check`; run code tests only when docs change executable examples, commands, workflows, or behavior claims that need verification.

## Timing-sensitive work

- Preserve the project-wide T-cycle foundation and the fixed per-T-cycle scheduler contract from [`docs/ARCHITECTURE.md`](ARCHITECTURE.md) and [`docs/info/TIMING-AND-ACCURACY.md`](info/TIMING-AND-ACCURACY.md).
- Document expected event ordering, the evidence source, and whether the reasoning comes from Pan Docs, AntonioND, Gekkio, ROM tests, retained traces, or emulator cross-checks.
- Prefer reproducible validation over intuition; keep cycle traces, snapshots, or before/after artifacts when they are the practical evidence for a regression or fix.
- When global ordering changes, update matching hardware docs for observable DMA, PPU mode visibility, MMIO side effects, interrupt visibility, CPU acceptance, serial/link completion, joypad edges, or scheduler arbitration.
- Keep requester arbitration, MMIO commit, and interrupt aggregation explicit; do not replace them with ad hoc subsystem-to-subsystem calls just because one local test passes.
- Add or update focused cross-subsystem tests for the affected ordering path, such as DMA-vs-CPU, delayed timer `IF`, serial completion plus IRQ, joypad visible-edge IRQ, `HALT`/IRQ priority, or `STAT`-versus-bus coherence.

## Validation selection

- Start with formatting, typos, unit tests, or targeted cargo tests when they directly cover the change.
- Use the local pre-commit checks plus `make coverage` when the change affects code paths covered by the default repository gate.
- Use `cargo rom-suite` report commands from [`docs/info/ROM-SUITES.md`](info/ROM-SUITES.md) when behavior depends on external ROM evidence, and keep the generated report channel aligned with the suite being rerun.
- For already-known external ROM failures or timing-sensitive reruns, follow the before/after report workflow from [`docs/index.md`](index.md) and [`docs/TESTING.md`](TESTING.md) before deciding whether to keep the change.
- Use external emulator source or differential cross-checks only as corroborating evidence after primary documentation, hardware research, and executable tests have been considered.

## Definition of done

- Behavior matches the owning hardware semantics, architecture boundaries, and explicit model constraints.
- New or changed code paths have appropriate automated coverage, or the limitation and risk are documented.
- Relevant unit, integration, ROM-based, report, or artifact validation has been run, or skipped steps are listed with the remaining risk.
- Matching documentation has been updated for changed scope, assumptions, commands, limitations, workflow, or authority boundaries.
- Known remaining work is recorded or updated in [`docs/TODO.md`](TODO.md).
- Renamed or moved docs have updated routing entries, README links, and no stale Markdown references.
