use super::*;

#[test]
fn skip_boot_exposes_the_published_post_boot_audio_readback() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    let cases = [
        (0xFF10, 0x80),
        (0xFF11, 0xBF),
        (0xFF12, 0xF3),
        (0xFF13, 0xFF),
        (0xFF14, 0xBF),
        (0xFF16, 0x3F),
        (0xFF17, 0x00),
        (0xFF18, 0xFF),
        (0xFF19, 0xBF),
        (0xFF1A, 0x7F),
        (0xFF1B, 0xFF),
        (0xFF1C, 0x9F),
        (0xFF1D, 0xFF),
        (0xFF1E, 0xBF),
        (0xFF20, 0xFF),
        (0xFF21, 0x00),
        (0xFF22, 0x00),
        (0xFF23, 0xBF),
        (0xFF24, 0x77),
        (0xFF25, 0xF3),
        (0xFF26, 0xF1),
        (0xFF30, 0x00),
    ];

    for (address, expected) in cases {
        assert_eq!(
            machine.read_bus(address),
            expected,
            "unexpected value at {address:#06X}"
        );
    }

    let snapshot = machine.apu().snapshot();
    assert!(snapshot.powered);
    assert_eq!(snapshot.channel_active_mask, 0x01);
    assert_eq!(snapshot.channel_dac_mask, 0x01);
    assert_eq!(snapshot.div_apu, 0x05);
    assert_eq!(
        snapshot.wave_ram_startup_policy,
        WaveRamStartupPolicy::DeterministicZeroed
    );
}

#[test]
fn nr52_power_off_clears_audio_registers_but_preserves_wave_ram_through_the_machine_bus() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF30, 0x12);
    machine.write_bus(0xFF31, 0x34);
    machine.write_bus(0xFF12, 0xF3);
    machine.write_bus(0xFF14, 0x80);
    machine.write_bus(0xFF24, 0x77);
    machine.write_bus(0xFF25, 0xF3);

    machine.write_bus(0xFF26, 0x00);

    assert_eq!(machine.read_bus(0xFF26), 0x70);
    assert_eq!(machine.read_bus(0xFF12), 0x00);
    assert_eq!(machine.read_bus(0xFF14), 0xBF);
    assert_eq!(machine.read_bus(0xFF24), 0x00);
    assert_eq!(machine.read_bus(0xFF25), 0x00);
    assert_eq!(machine.read_bus(0xFF30), 0x12);
    assert_eq!(machine.read_bus(0xFF31), 0x34);

    machine.write_bus(0xFF12, 0xF3);
    machine.write_bus(0xFF24, 0x77);
    machine.write_bus(0xFF25, 0xF3);
    assert_eq!(machine.read_bus(0xFF12), 0x00);
    assert_eq!(machine.read_bus(0xFF24), 0x00);
    assert_eq!(machine.read_bus(0xFF25), 0x00);

    machine.write_bus(0xFF26, 0x80);
    machine.write_bus(0xFF12, 0xF3);
    assert_eq!(machine.read_bus(0xFF12), 0xF3);
}

#[test]
fn channel_3_wave_ram_survives_nr52_power_off_after_the_channel_has_been_started() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF30, 0x12);
    machine.write_bus(0xFF31, 0x34);
    machine.write_bus(0xFF1A, 0x80);
    machine.write_bus(0xFF1C, 0x20);
    machine.write_bus(0xFF1E, 0x80);

    assert_eq!(machine.read_bus(0xFF26) & 0x04, 0x04);

    machine.write_bus(0xFF26, 0x00);

    assert_eq!(machine.read_bus(0xFF1A), 0x7F);
    assert_eq!(machine.read_bus(0xFF1C), 0x9F);
    assert_eq!(machine.read_bus(0xFF1E), 0xBF);
    assert_eq!(machine.read_bus(0xFF26) & 0x04, 0x00);
    assert_eq!(machine.read_bus(0xFF30), 0x12);
    assert_eq!(machine.read_bus(0xFF31), 0x34);
}

#[test]
fn nr52_channel_status_follows_trigger_and_dac_disable_through_the_machine_bus() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF26, 0x00);
    machine.write_bus(0xFF26, 0x80);

    assert_eq!(machine.read_bus(0xFF26), 0xF0);
    assert_eq!(machine.apu().snapshot().channel_dac_mask, 0x00);

    machine.write_bus(0xFF12, 0xF3);
    assert_eq!(machine.read_bus(0xFF26), 0xF0);
    assert_eq!(machine.apu().snapshot().channel_dac_mask, 0x01);
    assert_eq!(machine.apu().snapshot().channel_active_mask, 0x00);

    machine.write_bus(0xFF14, 0x80);
    assert_eq!(machine.read_bus(0xFF26), 0xF1);
    assert_eq!(machine.apu().snapshot().channel_dac_mask, 0x01);
    assert_eq!(machine.apu().snapshot().channel_active_mask, 0x01);

    machine.write_bus(0xFF12, 0x00);
    assert_eq!(machine.read_bus(0xFF26), 0xF0);
    assert_eq!(machine.apu().snapshot().channel_dac_mask, 0x00);
    assert_eq!(machine.apu().snapshot().channel_active_mask, 0x00);
}
