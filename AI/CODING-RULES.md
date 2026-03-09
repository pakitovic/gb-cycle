# Coding Rules

## Rust style goals

- Prefer clarity and explicitness.
- Keep modules cohesive and small.
- Avoid over-engineering.
- Make invariants visible in types where practical.

## Recommended practices

- Use descriptive names tied to hardware meaning.
- Prefer enums and small structs over magic numbers.
- Keep `pub` visibility narrow.
- Separate state mutation from derived calculations when possible.
- Keep debug logging and tracing easy to enable in development.
- Prefer pure APIs when reasonable, especially around decode, derived state, and reusable helpers.
- Make facts, inferences, and design choices easy to distinguish in code comments and docs.

## Architecture discipline

- Separate core, platform/frontend, persistence, debugger/tooling, and tests.
- Avoid coupling CPU, PPU, APU, timer, or bus code to UI and I/O details.
- Avoid circular dependencies between subsystems.
- Avoid implicit temporal logic hidden inside convenience APIs.
- Every hardware quirk should have an explicit home in code.

## Timing-sensitive code

- Avoid clever shortcuts that hide ordering.
- Prefer code that mirrors hardware phases when accuracy matters.
- Comment non-obvious hardware quirks with source references.
- Do not replace fine-grained timing with "every N instructions" models unless the subsystem documentation explicitly justifies it.

## Error handling

- Use explicit error types at boundaries.
- Avoid panics in normal emulator runtime paths.
- Reserve assertions for invariant violations in development and tests.

## Performance

- Correctness first.
- Benchmark before optimizing.
- Optimize with local reasoning and measurable effect.

Only introduce an optimization when all of this is true:

1. A correct and testable implementation exists first.
2. The optimization has a measurable hypothesis.
3. It preserves the clarity of the hardware model.
4. Its cost in maintainability and portability is understood.

Usually welcome:

- dispatch tables when they simplify CPU hot paths
- cache-friendly layouts in measured bottlenecks
- branch reduction in known hot paths
- optional instrumentation or tracing modes

Avoid without evidence:

- merging unrelated hardware modules
- obscuring real clocking behind broad abstractions
- `unsafe` without a strong reason
- parallelism that harms determinism
