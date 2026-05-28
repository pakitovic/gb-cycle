# Execution

## Standard workflow

1. Identify the owning subsystem or workflow and use [`docs/index.md`](index.md) to route to the authoritative document.
2. For hardware behavior, read the matching `docs/hardware/*.md`; for cross-cutting core design, read [`docs/ARCHITECTURE.md`](ARCHITECTURE.md) plus the relevant `docs/info/*.md`; for frontend behavior, read [`docs/info/CLI.md`](info/CLI.md) or [`docs/info/DESKTOP.md`](info/DESKTOP.md); for validation workflow, read [`docs/TESTING.md`](TESTING.md) or [`docs/info/ROM-SUITES.md`](info/ROM-SUITES.md).
3. Read [`docs/ROADMAP.md`](ROADMAP.md) when the task maps to a roadmap phase, resumes prior work, or may leave deferred follow-up work; read [`docs/TODO.md`](TODO.md) when closing, rewriting, or adding an active gap.
4. Read matching primary references from [`docs/REFERENCES.md`](REFERENCES.md) when hardware behavior, timing, or oracle policy depends on external evidence.
5. Use the open-source emulator consultation notes in [`docs/REFERENCES.md`](REFERENCES.md) only if implementation examples or source-level emulator cross-checks are needed.
6. Define contracts, invariants, and ownership boundaries before coding.
7. Implement the smallest correct change.
8. Validate with the narrowest useful automated tests first, then ROM-based tests or oracle comparison when the behavior requires them.
9. Cross-check against a trusted emulator or retained oracle when behavior is timing-sensitive or when a compatibility decision depends on non-obvious evidence.
10. Update docs when assumptions, sequencing, scope limits, ownership boundaries, validation policy, or rules change.

## Change policy

- One behavioral change at a time.
- Documentation-only cleanups may group related moves or stale-text removal when they preserve behavior and make authority boundaries clearer.
- Avoid mixing refactors with timing fixes unless required.
- When in doubt, preserve debuggability over micro-optimizations.
- Use Conventional Commits for commit messages.
- Use Conventional Commits for pull request titles as well, so branch history and PR metadata follow the same naming contract.
- Before refactoring behavior-sensitive paths, verify coverage exists for the target behavior; add it first if missing.
- Keep bug fixes minimal and isolated from unrelated cleanup.
- Keep file moves separate from behavior changes when possible; if a move is required, update [`docs/index.md`](index.md), README links, and stale path references in the same change.
- New production code should normally land with automated unit tests or integration tests in the same change.
- If a code change cannot reasonably add automated tests yet, document the reason, the remaining validation gap, and the associated risk explicitly in the change report.
- Do not use `Permissive` or `Experimental` results as evidence for `Strict` accuracy claims unless the docs explicitly frame them as non-oracle exploratory data.

## Roadmap coordination policy

- [`docs/ROADMAP.md`](ROADMAP.md) is a living document, not a one-time planning artifact.
- Use it to understand recommended implementation order, phase dependencies, and phase-level done criteria.
- Treat roadmap phase order as the recommended implementation order, not as a strict merge-order constraint when parallel work can land cleanly without violating subsystem authority docs.
- [`docs/TODO.md`](TODO.md) owns the active TODO ledger. When a task lands with known gaps, deferred fixes, incomplete validation, or partially unmet roadmap done criteria, add or update the concrete TODO there in the same change.
- Remove or rewrite TODO entries when the underlying work is completed, invalidated, or superseded by a better plan.
- Keep TODOs lean in status noise and rich in re-entry context: exact remaining behavior, evidence already in hand, superseded directions not to retry first, and the highest-value next step.
- Do not record speculative ideas there; keep TODOs tied to concrete implemented or partially implemented work.
- Update [`docs/ROADMAP.md`](ROADMAP.md) or `docs/roadmap/*.md` only when phase structure, sequencing, scope, or done criteria changes.

## Documentation workflow

- Keep root `docs/` for top-level routing and project-wide policy; place cross-cutting core design, frontend usage, and validation tooling workflows under `docs/info/`, hardware behavior under `docs/hardware/`, and phase plans under `docs/roadmap/`.
- When moving or renaming docs, update [`docs/index.md`](index.md) and any README or cross-doc links in the same change.
- Do not hard-wrap Markdown prose. Keep each paragraph, bullet item, numbered item, and roadmap field on one physical line unless it is a fenced block, table, or intentional standalone formula.
- For documentation-only changes, validate with stale-reference searches and `git diff --check`; run code tests only when the docs change executable examples, commands, workflows, or behavior claims that need verification.

## When touching timing-sensitive code

- Document the expected ordering of events.
- State whether reasoning is based on Pan Docs, AntonioND, Gekkio, tests, or reference emulators.
- Prefer reproducible validation over intuition.
- Preserve the project's T-cycle timing foundation; do not introduce M-cycle-first scheduling shortcuts as a local convenience.
- Preserve retained traces, snapshots, or before/after artifacts when they are the only practical way to explain a regression.
- When rerunning curated external ROM suites for an existing failure or timing-sensitive change, follow the mandatory matching `/test/test-report.md`, `/test/test-report-extra.md`, or `/test/test-report-docboy.md` before/after workflow from [`docs/index.md`](index.md) and [`docs/TESTING.md`](TESTING.md).

## When touching the global scheduler or cross-subsystem ordering

- Preserve the fixed per-T-cycle phase contract defined by [`docs/ARCHITECTURE.md`](ARCHITECTURE.md) and [`docs/info/TIMING-AND-ACCURACY.md`](info/TIMING-AND-ACCURACY.md).
- Update the matching hardware docs together when the observable ordering of DMA, PPU mode visibility, MMIO side effects, interrupt visibility, or CPU acceptance changes.
- Keep requester arbitration, MMIO commit, and interrupt aggregation explicit; do not replace them with ad hoc subsystem-to-subsystem calls just because one local test passes.
- Add or update focused cross-subsystem tests such as DMA-vs-CPU, delayed timer `IF`, serial completion plus IRQ, joypad visible-edge IRQ, `HALT` / IRQ priority, and `STAT`-versus-bus coherence.
- Prefer cycle traces over aggregate instruction summaries when validating scheduler work.

## Definition of done

- Behavior is implemented consistently with hardware semantics and project architecture.
- New code paths are covered by unit tests or integration tests unless a documented limitation makes that temporarily impossible.
- Unit, integration, and ROM-based validation are updated where applicable.
- Non-executed validation steps are reported explicitly with the remaining risk.
- If the change only partially satisfies the relevant roadmap phase or leaves concrete follow-up work, that remainder is recorded or updated in [`docs/TODO.md`](TODO.md).
- Matching `docs/*` docs are updated when scope, limitations, workflow, or assumptions changed.
- Renamed or moved docs have updated routing entries and no stale path references.
