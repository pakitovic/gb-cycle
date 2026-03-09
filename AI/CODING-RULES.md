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

## Timing-sensitive code
- Avoid clever shortcuts that hide ordering.
- Prefer code that mirrors hardware phases when accuracy matters.
- Comment non-obvious hardware quirks with source references.

## Error handling
- Use explicit error types at boundaries.
- Avoid panics in normal emulator runtime paths.
- Reserve assertions for invariant violations in development and tests.

## Performance
- Correctness first.
- Benchmark before optimizing.
- Optimize with local reasoning and measurable effect.
