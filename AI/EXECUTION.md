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
- Before refactoring behavior-sensitive paths, verify coverage exists for the target behavior; add it first if missing.
- Keep bug fixes minimal and isolated from unrelated cleanup.

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

## Definition of done

- Behavior is implemented consistently with hardware semantics and project architecture.
- Unit, integration, and ROM-based validation are updated where applicable.
- Non-executed validation steps are reported explicitly with the remaining risk.
- If the change only partially satisfies the relevant roadmap phase or leaves concrete follow-up work, that remainder is recorded in `AI/ROADMAP.md`.
- Matching `AI/*` docs are updated when scope, limitations, workflow, or assumptions changed.
