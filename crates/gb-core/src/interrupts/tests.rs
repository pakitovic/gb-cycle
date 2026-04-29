use super::*;

#[test]
fn if_forces_unused_upper_bits_high() {
    let mut interrupts = InterruptController::new(ConsoleModel::GameBoy);

    interrupts.write_if(0x04);

    assert_eq!(interrupts.read_if(), 0xE4);
}

#[test]
fn request_and_pending_selection_follow_dmg_priority() {
    let mut interrupts = InterruptController::new(ConsoleModel::GameBoy);

    interrupts.write_ie(0x1F);
    interrupts.request(InterruptSource::Joypad);
    interrupts.request(InterruptSource::Timer);
    interrupts.request(InterruptSource::VBlank);

    assert_eq!(interrupts.pending_mask(), 0x15);
    assert_eq!(interrupts.highest_pending(), Some(InterruptSource::VBlank));
}

#[test]
fn startup_state_keeps_if_upper_bits_forced_high_on_readback() {
    let mut interrupts = InterruptController::new(ConsoleModel::GameBoy);

    interrupts.apply_startup_state(InterruptStartupState {
        interrupt_flags: 0x01,
        interrupt_enable: 0x00,
    });

    assert_eq!(interrupts.read_if(), 0xE1);
    assert_eq!(interrupts.read_ie(), 0x00);
}
