# CGB IR linked-session fixtures

This directory stores repo-owned synthetic native-CGB ROMs for the internal `linked-cgb-ir-smoke` suite.

Both ROMs are direct-start `NoMBC` CGB-capable fixtures with `0x0143 = 0x80`, cartridge type `0x00`, ROM size `0x00`, RAM size `0x00`, and a deterministic program beginning at `0x0100`.

`emitter.gb` executes `LD A,$C1; LDH ($56),A; JP $0104`, which enables `RP` readback and keeps the IR emitter latch on.

`receiver.gb` executes `LD A,$C0; LDH ($56),A; LDH A,($56); BIT 1,A; JR NZ,$0104; LD A,$B2; LDH ($01),A; LD A,$81; LDH ($02),A; JP $0112`, which enables `RP` readback, waits until the linked CGB IR sensor reports a signal, emits serial byte `$B2`, and then idles.

These fixtures validate only the core CGB-to-CGB optical topology and read-enabled `RP` sensor path; the Pokémon Pikachu 2 and custom GSC Mystery Gift accessory protocols are validated separately, while Pocket Sakura, TV remotes, lamps, Chee Chai Alien, HuC1/HuC3-to-CGB IR, and other title-specific external protocols remain outside this fixture contract.

## Audit rule

`crates/gb-test-runner/tests/linked_fixture_roms.rs` reconstructs these ROMs from the byte sequences above and verifies that the committed `.gb` files match exactly. If either ROM is intentionally changed, update both this README and that test in the same change.
