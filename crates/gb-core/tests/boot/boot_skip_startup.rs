use super::*;

#[test]
fn skip_boot_uses_the_centralized_synthetic_startup_state() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    assert_eq!(machine.boot().revision(), HardwareRevision::DmgCpuC);
    assert!(!machine.boot().is_boot_rom_mapped());
    assert_eq!(machine.cpu().startup_state().pc, 0x0100);
    assert_eq!(machine.cpu().startup_state().a, 0x01);
    assert_eq!(machine.cpu().startup_state().f, 0xB0);
    assert_eq!(
        machine.boot().startup_memory_policy(),
        StartupMemoryPolicy::DmgBootLogoVram
    );

    assert_eq!(machine.read_bus(0xFF00), 0xCF);
    assert_eq!(machine.read_bus(0xFF01), 0x00);
    assert_eq!(machine.read_bus(0xFF02), 0x7E);
    assert_eq!(machine.read_bus(0xFF04), 0xAB);
    assert_eq!(machine.read_bus(0xFF05), 0x00);
    assert_eq!(machine.read_bus(0xFF06), 0x00);
    assert_eq!(machine.read_bus(0xFF07), 0xF8);
    assert_eq!(machine.read_bus(0xFF0F), 0xE1);
    assert_eq!(machine.read_bus(0xFF40), 0x91);
    assert_eq!(machine.read_bus(0xFF41), 0x85);
    assert_eq!(machine.read_bus(0xFF42), 0x00);
    assert_eq!(machine.read_bus(0xFF43), 0x00);
    assert_eq!(machine.read_bus(0xFF44), 0x00);
    assert_eq!(machine.read_bus(0xFF45), 0x00);
    assert_eq!(machine.read_bus(0xFF46), 0xFF);
    assert_eq!(machine.read_bus(0xFF47), 0xFC);
    assert_eq!(machine.read_bus(0xFF4A), 0x00);
    assert_eq!(machine.read_bus(0xFF4B), 0x00);
    assert_eq!(machine.read_bus(0xFFFF), 0x00);
    let wram_seed = machine.read_bus(0xC000);
    let hram_seed = machine.read_bus(0xFF80);
    assert_ne!(wram_seed, 0x00);
    assert_ne!(hram_seed, 0x00);

    let mut second_machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    assert_eq!(
        second_machine.boot().startup_memory_policy(),
        StartupMemoryPolicy::DmgBootLogoVram
    );
    assert_eq!(second_machine.read_bus(0xC000), wram_seed);
    assert_eq!(second_machine.read_bus(0xFF80), hram_seed);
}

#[test]
fn dmg0_direct_skip_and_custom_boot_share_the_verified_startup_contract() {
    let boot = BootController::new(
        ConsoleModel::GameBoy,
        HardwareRevision::DmgCpu0,
        StartupMode::SkipBoot,
        BootRomAssets::none(),
    );
    let direct_boot = boot
        .direct_boot_state(None)
        .expect("DMG0 SkipBoot should expose a direct-boot state");

    assert_eq!(direct_boot.cpu.a, 0x01);
    assert_eq!(direct_boot.cpu.f, 0x00);
    assert_eq!(direct_boot.cpu.b, 0xFF);
    assert_eq!(direct_boot.cpu.c, 0x13);
    assert_eq!(direct_boot.cpu.d, 0x00);
    assert_eq!(direct_boot.cpu.e, 0xC1);
    assert_eq!(direct_boot.cpu.h, 0x84);
    assert_eq!(direct_boot.cpu.l, 0x03);
    assert_eq!(direct_boot.cpu.sp, 0xFFFE);
    assert_eq!(direct_boot.cpu.pc, 0x0100);
    assert_eq!(direct_boot.io.div, 0x18);
    assert_eq!(direct_boot.io.stat, 0x83);
    assert_eq!(direct_boot.io.ly, 0x01);
    assert_eq!(direct_boot.timer.system_counter, 0x182C);
    assert_eq!(direct_boot.apu.div_apu, 0x00);
    assert_eq!(
        direct_boot.startup_memory_policy,
        StartupMemoryPolicy::DmgBootLogoVram
    );

    for startup_mode in [StartupMode::SkipBoot, StartupMode::CustomBoot] {
        let mut machine = Machine::new(
            MachineConfig::new(ConsoleModel::GameBoy)
                .with_revision(HardwareRevision::DmgCpu0)
                .with_startup_mode(startup_mode),
        );
        let snapshot = machine.snapshot();

        assert_eq!(machine.boot().revision(), HardwareRevision::DmgCpu0);
        assert_eq!(machine.cpu().startup_state(), direct_boot.cpu);
        assert_eq!(
            snapshot.timer.system_counter,
            direct_boot.timer.system_counter
        );
        assert_eq!(snapshot.apu.div_apu, direct_boot.apu.div_apu);
        assert_eq!(snapshot.ppu.ly, direct_boot.ppu.ly);
        assert_eq!(snapshot.ppu.lcdc, direct_boot.ppu.lcdc);
        assert_eq!(
            machine.boot().startup_memory_policy(),
            direct_boot.startup_memory_policy
        );
        assert_eq!(machine.read_bus(0xFF04), direct_boot.io.div);
        assert_eq!(machine.read_bus(0xFF41), direct_boot.io.stat);
        assert_eq!(machine.read_bus(0xFF44), direct_boot.io.ly);
    }
}

#[test]
fn boot_asset_metadata_keeps_dmg0_and_sgb_profiles_distinct() {
    assert_eq!(
        BootRomAssetKind::from(HardwareRevision::DmgCpu0),
        BootRomAssetKind::Dmg0
    );
    assert_eq!(
        BootRomAssetKind::from(HardwareRevision::DmgCpuC),
        BootRomAssetKind::Dmg
    );
    assert_eq!(
        BootRomAssetKind::from(SgbHostProfile::SgbNtsc),
        BootRomAssetKind::Sgb
    );
    assert_eq!(
        BootRomAssetKind::from(SgbHostProfile::Sgb2Ntsc),
        BootRomAssetKind::Sgb2
    );
    assert_eq!(BootRomAssetKind::Dmg0.filename(), "dmg0_boot.bin");
    assert_eq!(BootRomAssetKind::Dmg.filename(), "dmg_boot.bin");
    assert_eq!(BootRomAssetKind::Dmg0.expected_size(), BOOT_ROM_LEN);
    assert_eq!(BootRomAssetKind::Dmg.expected_size(), BOOT_ROM_LEN);
    assert_eq!(
        BootRomAssetKind::Dmg0.expected_sha256(),
        "26e71cf01e301e5dc40e987cd2ecbf6d0276245890ac829db2a25323da86818e"
    );
    assert_eq!(
        BootRomAssetKind::Dmg.expected_sha256(),
        "cf053eccb4ccafff9e67339d4e78e98dce7d1ed59be819d2a1ba2232c6fce1c7"
    );
}

#[test]
fn skip_boot_recomputes_the_checksum_derived_f_register_when_a_cartridge_is_loaded() {
    let mut zero_checksum_machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    let mut non_zero_checksum_machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    zero_checksum_machine
        .load_cartridge(build_test_rom(0x00))
        .expect("supported NoMBC image should load");
    non_zero_checksum_machine
        .load_cartridge(build_test_rom(0x7F))
        .expect("supported NoMBC image should load");

    assert_eq!(zero_checksum_machine.cpu().startup_state().f, 0x80);
    assert_eq!(non_zero_checksum_machine.cpu().startup_state().f, 0xB0);
}

#[test]
fn sgb_skip_boot_uses_profile_specific_io_and_header_timed_div_phase() {
    let mut boot_div_rom = build_test_rom(0x2D);
    boot_div_rom[CGB_FLAG_ADDRESS] = 0x00;
    boot_div_rom[SGB_FLAG_ADDRESS] = 0x00;
    boot_div_rom[GLOBAL_CHECKSUM_START..GLOBAL_CHECKSUM_START + 2].copy_from_slice(&[0x34, 0x12]);

    let mut boot_div2_rom = boot_div_rom.clone();
    boot_div2_rom[GLOBAL_CHECKSUM_START..GLOBAL_CHECKSUM_START + 2].copy_from_slice(&[0x96, 0xA7]);

    let mut boot_div_machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy)
            .with_sgb_profile(SgbHostProfile::SgbNtsc)
            .with_startup_mode(StartupMode::SkipBoot),
    );
    let mut boot_div2_machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy)
            .with_sgb_profile(SgbHostProfile::SgbNtsc)
            .with_startup_mode(StartupMode::SkipBoot),
    );

    boot_div_machine
        .load_cartridge(boot_div_rom)
        .expect("supported SGB NoMBC image should load");
    boot_div2_machine
        .load_cartridge(boot_div2_rom)
        .expect("supported SGB NoMBC image should load");

    assert_eq!(boot_div_machine.cpu().startup_state().c, 0x14);
    assert_eq!(boot_div_machine.read_bus(0xFF00), 0xFF);
    assert_eq!(boot_div_machine.read_bus(0xFF02), 0x7E);
    assert_eq!(boot_div_machine.read_bus(0xFF07), 0xF8);
    assert_eq!(boot_div_machine.read_bus(0xFF0F), 0xE1);
    assert_eq!(boot_div_machine.read_bus(0xFF26), 0xF0);
    assert_eq!(boot_div_machine.read_bus(0xFF42), 0x00);
    assert_eq!(boot_div_machine.read_bus(0xFF43), 0x00);
    assert_eq!(boot_div_machine.read_bus(0xFF45), 0x00);
    assert_eq!(boot_div_machine.read_bus(0xFF47), 0xFC);
    assert_eq!(boot_div_machine.read_bus(0xFF4A), 0x00);
    assert_eq!(boot_div_machine.read_bus(0xFF4B), 0x00);
    assert_eq!(boot_div_machine.read_bus(0xFFFF), 0x00);

    let boot_div_counter = boot_div_machine.snapshot().timer.system_counter;
    let boot_div2_counter = boot_div2_machine.snapshot().timer.system_counter;
    assert_eq!(boot_div_counter.wrapping_sub(boot_div2_counter), 16);
}
