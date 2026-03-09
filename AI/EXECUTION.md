# Execution

## Standard workflow
1. Identify the subsystem and read its `AI/hardware/*.md` file.
2. Read matching primary references from `AI/REFERENCES.md`.
3. Read one or more `AI/research/*.md` files only if implementation examples are needed.
4. Define invariants before coding.
5. Implement the smallest correct change.
6. Validate with unit tests and ROM-based tests.
7. Cross-check against a trusted emulator when behavior is timing-sensitive.
8. Update docs when assumptions or rules change.

## Change policy
- One behavioral change at a time.
- Avoid mixing refactors with timing fixes unless required.
- When in doubt, preserve debuggability over micro-optimizations.
- Before refactoring behavior-sensitive paths, verify coverage exists for the target behavior; add it first if missing.
- Keep bug fixes minimal and isolated from unrelated cleanup.

## When touching timing-sensitive code
- Document the expected ordering of events.
- State whether reasoning is based on Pan Docs, AntonioND, Gekkio, tests, or reference emulators.
- Prefer reproducible validation over intuition.

## Definition of done

- Behavior is implemented consistently with hardware semantics and project architecture.
- Unit, integration, and ROM-based validation are updated where applicable.
- Non-executed validation steps are reported explicitly with the remaining risk.
- Matching `AI/*` docs are updated when scope, limitations, workflow, or assumptions changed.
