# Execution

## Standard workflow
1. Identify the subsystem and read its `AI/hardware/*.md` file.
2. Read `AI/ROADMAP.md` when the task maps to a roadmap phase, resumes prior work, or may leave deferred follow-up work.
3. Read matching primary references from `AI/REFERENCES.md`.
4. Read one or more `AI/research/*.md` files only if implementation examples are needed.
5. Define invariants before coding.
6. Implement the smallest correct change.
7. Validate with unit tests and ROM-based tests.
8. Cross-check against a trusted emulator when behavior is timing-sensitive.
9. Update docs when assumptions, sequencing, scope limits, or rules change.

## Change policy
- One behavioral change at a time.
- Avoid mixing refactors with timing fixes unless required.
- When in doubt, preserve debuggability over micro-optimizations.
- Use Conventional Commits for commit messages.
- Use Conventional Commits for pull request titles as well, so branch history and PR metadata follow the same naming contract.
- Before refactoring behavior-sensitive paths, verify coverage exists for the target behavior; add it first if missing.
- Keep bug fixes minimal and isolated from unrelated cleanup.
- New production code should normally land with automated unit tests or integration tests in the same change.
- If a code change cannot reasonably add automated tests yet, document the reason, the remaining validation gap, and the associated risk explicitly in the change report.

## Roadmap coordination policy

- `AI/ROADMAP.md` is a living document, not a one-time planning artifact.
- Use it to understand recommended implementation order, phase dependencies, and phase-level done criteria.
- When a task lands with known gaps, deferred fixes, incomplete validation, or partially unmet roadmap done criteria, add a TODO entry to the relevant roadmap section in the same change.
- If no single phase owns the remaining work cleanly, record the item under the roadmap's `Cross-phase` TODO section.
- Remove or rewrite roadmap TODOs when the underlying work is completed, invalidated, or superseded by a better plan.
- Do not record speculative ideas there; keep TODOs tied to concrete implemented or partially implemented work.

## When touching timing-sensitive code
- Document the expected ordering of events.
- State whether reasoning is based on Pan Docs, AntonioND, Gekkio, tests, or reference emulators.
- Prefer reproducible validation over intuition.
- Preserve the project's T-cycle timing foundation; do not introduce M-cycle-first scheduling shortcuts as a local convenience.

## When touching the global scheduler or cross-subsystem ordering

- Preserve the fixed per-T-cycle phase contract defined by `AI/ARCHITECTURE.md` and `AI/TIMING-AND-ACCURACY.md`.
- Update the matching hardware docs together when the observable ordering of DMA, PPU mode visibility, MMIO side effects, interrupt visibility, or CPU acceptance changes.
- Keep requester arbitration, MMIO commit, and interrupt aggregation explicit; do not replace them with ad hoc subsystem-to-subsystem calls just because one local test passes.
- Add or update focused cross-subsystem tests such as DMA-vs-CPU, delayed timer `IF`, serial completion plus IRQ, joypad visible-edge IRQ, `HALT` / IRQ priority, and `STAT`-versus-bus coherence.
- Prefer cycle traces over aggregate instruction summaries when validating scheduler work.

## Definition of done

- Behavior is implemented consistently with hardware semantics and project architecture.
- New code paths are covered by unit tests or integration tests unless a documented limitation makes that temporarily impossible.
- Unit, integration, and ROM-based validation are updated where applicable.
- Non-executed validation steps are reported explicitly with the remaining risk.
- If the change only partially satisfies the relevant roadmap phase or leaves concrete follow-up work, that remainder is recorded in `AI/ROADMAP.md`.
- Matching `AI/*` docs are updated when scope, limitations, workflow, or assumptions changed.
