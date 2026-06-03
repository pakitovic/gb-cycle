use gb_core::{CartridgeSlot, CompatibilityPolicy, Huc3RtcPersistentState, PersistentCartState};

pub(crate) fn load_cartridge(rom: Vec<u8>) -> CartridgeSlot {
    let report = CartridgeSlot::load(rom, &CompatibilityPolicy::strict())
        .expect("test cartridge should load");
    let (cartridge, _) = report.into_parts();
    cartridge
}

pub(crate) fn huc3_persistent_state(ram: Vec<u8>) -> PersistentCartState {
    PersistentCartState::Huc3 {
        ram,
        mcu_ram: [0; 256],
        rtc: Huc3RtcPersistentState {
            current_minutes_of_day: 0,
            current_days: 0,
            current_subminute_seconds: 0,
            event_minutes_of_day: 0,
            event_days: 0,
        },
        rom_bank: 0,
        ram_bank: 0,
        select_mode: 0,
        access_address: 0,
        mailbox_command: 0,
        mailbox_argument: 0,
        last_response_nybble: 0,
        semaphore_ready: true,
        ir_emitter_on: false,
        ir_light_detected: false,
        last_control_write: None,
        last_unsupported_command: None,
        last_unsupported_argument: None,
    }
}
