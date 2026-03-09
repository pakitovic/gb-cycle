# TIMER

## Scope

Own `DIV`, `TIMA`, `TMA`, `TAC`, their internal timing state, overflow behavior, and interrupt request generation.

## Hardware model

Model the timer as edge-sensitive hardware, not as a periodic software counter incremented every few instructions.

## Responsibilities

- track divider state
- implement timer enable/frequency selection behavior
- handle overflow, reload, and interrupt request ordering

## Registers / MMIO

- `DIV`
- `TIMA`
- `TMA`
- `TAC`

## Timing / accuracy requirements

- Explain edges, glitches, and event ordering explicitly.
- Do not reduce the model to "increment every X instructions" if finer timing matters.
- Preserve the interaction with interrupt timing and writes to timer registers.

## Dependencies

- interrupt controller
- scheduler or clock source
- bus/MMIO wiring

## Primary references

- Pan Docs timer sections
- AntonioND timing docs
- Gekkio research and Mooneye timer tests

## Open-source emulator references

Priority order:

1. SameBoy
2. binjgb
3. Mooneye GB
4. Danger Boy
5. GameRoy

## Tests

- Mooneye timer and DIV/TIMA tests
- focused write-order and overflow tests

## Implementation notes for this repo

- Keep timer state highly testable.
- Make the source of each timing decision visible in comments or docs.

## Known pitfalls

- incorrect edge detection
- incorrect reload timing
- mixing interrupt request timing with reload semantics

## Open questions

- which internal representation best exposes the divider edge logic
