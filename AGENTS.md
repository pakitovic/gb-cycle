# Agent Guidelines for gb-cycle

Primary rule: consult the `AI/*` handbook files directly. Pick only the files relevant to the task; each file is authoritative for its own scope.

Quick map:
- `AI/ARCHITECTURE.md`
- `AI/EXECUTION.md`
- `AI/CODING-RULES.md`
- `AI/REFERENCES.md`
- `AI/TESTING.md`
- `AI/TIMING-AND-ACCURACY.md`
- `AI/hardware/*`
- `AI/research/*`

## Core rules (apply always)
- Treat hardware behavior as the source of truth, not another emulator's implementation.
- Use documentation and hardware research first; use open-source emulators as reference implementations and behavioral cross-checks.
- Prefer correctness over convenience in timing-sensitive code.
- Keep modules small, explicit, and portable.
- Avoid hidden global state.
- Optimize only after preserving correctness and observability.
- When changing behavior, update the matching `AI/*` file in the same change.

## Workflow checklist
- Identify the owning subsystem.
- Read the matching `AI/hardware/*.md` file first.
- Read the matching `AI/research/*.md` files only if implementation examples are needed.
- Define contracts and invariants before editing code.
- Add or update tests and ROM-based validation.
- Compare behavior against at least one trusted oracle when touching timing-sensitive logic.
- Update docs when architecture, rules, timing assumptions, or references change.
