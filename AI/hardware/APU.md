# APU

## Scope

Own internal sound hardware state: channels, registers, frame sequencer, and hardware mixing inputs. Do not own the host audio backend.

## Hardware model

Keep channel behavior and frame-sequencer timing explicit. Separate internal audio generation from output sampling and playback.

## Responsibilities

- channel state and register semantics
- frame sequencer behavior
- internal sample generation and mixing inputs

## Registers / MMIO

- `NR10`-`NR52`
- wave RAM ownership and access rules

## Timing / accuracy requirements

- Keep channel and frame-sequencer timing visible.
- Do not mix backend sampling concerns with hardware state evolution.

## Dependencies

- bus/MMIO
- scheduler or clock source
- model/revision configuration

## Primary references

- Pan Docs APU sections
- gbdev audio references
- subsystem-specific hardware research where needed

## Open-source emulator references

Priority order:

1. SameBoy
2. Gambatte
3. binjgb
4. GameRoy

## Tests

- audio-focused ROMs where available
- register semantics tests
- frame-sequencer timing tests

## Implementation notes for this repo

- Keep output backend decoupled from the emulation core.
- Favor correctness and clarity before micro-optimizations.

## Known pitfalls

- mixing host sample rate concerns into hardware timing
- hiding frame-sequencer behavior behind backend callbacks

## Open questions

- what internal sampling interface best preserves determinism and portability
