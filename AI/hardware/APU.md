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

## APU MMIO contract baseline

- The APU should expose register semantics by field, not as a generic byte bank that happens to live in `FF10-FF3F`.
- Registers documented as write-only, such as `NR13`, `NR23`, `NR31`, and `NR41`, should follow an explicit readback policy instead of echoing the last write by default.
- Mixed registers such as `NR14`, `NR24`, `NR34`, `NR44`, and especially `NR52` should keep their read-only and writable fields distinct.
- `NR52` should treat the power bit as writable control state and the channel-on flags as read-only live status.
- Powering the APU off through `NR52` should clear APU register state and make the other APU registers read-only until power is restored, while leaving wave RAM accessibility under the documented hardware policy.
- Trigger bits in the `NRx4` family should perform their channel-start side effects on the write itself.
- Write-only fields such as initial length-timer setup should remain write-only semantically, even if internal channel state later depends on them.

## Timing / accuracy requirements

- Keep channel and frame-sequencer timing visible.
- Do not mix backend sampling concerns with hardware state evolution.
- Keep internal APU sequencing compatible with the shared T-cycle timing model, even if audio output is resampled later.
- MMIO-triggered APU events such as `NR52` power transitions and `NRx4` triggers should remain visible on the shared T-cycle timeline.

## Dependencies

- bus/MMIO
- T-cycle scheduler or clock source
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
- direct-boot register-readback tests for the published post-boot audio snapshot when startup presets bypass firmware execution
- tests for write-only register readback policy
- tests for `NR52` mixed readback and power-gating behavior
- tests that `NRx4` trigger writes cause immediate channel-side effects

## Implementation notes for this repo

- Keep output backend decoupled from the emulation core.
- Favor correctness and clarity before micro-optimizations.
- Visible post-boot `NRxx` register values for `SkipBoot` should come from the centralized boot snapshot rather than ad hoc per-register reset literals spread through APU code.
- Wave RAM accessibility policy should stay explicit and separate from the ordinary `NRxx` register bank contract.

## Known pitfalls

- mixing host sample rate concerns into hardware timing
- hiding frame-sequencer behavior behind backend callbacks
- treating the APU MMIO range as a plain register array and losing write-only or mixed-field behavior

## Open questions

- what internal sampling interface best preserves determinism and portability
