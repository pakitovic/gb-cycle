use super::*;

#[test]
fn decimal_adjust_accumulator_crc_matches_blargg_01_special_reference() {
    let mut crc = 0xFFFF_FFFF_u32;

    for flags in (0_u8..=0xF0).step_by(0x10) {
        for a in u8::MIN..=u8::MAX {
            let mut cpu = CpuCore::new(ConsoleModel::GameBoy);
            let mut bus = Bus::new(ConsoleModel::GameBoy);
            let mut cartridge = build_test_cartridge(&[0x27]);

            cpu.apply_startup_state(CpuStartupState {
                a,
                f: flags,
                pc: 0x0100,
                ..CpuStartupState::power_on_reset()
            });

            tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 4);

            crc = crc32_iso_hdlc(crc, cpu.registers().a);
            crc = crc32_iso_hdlc(crc, cpu.registers().f);
        }
    }

    assert_eq!(crc ^ 0xFFFF_FFFF, 0x6A9F_8D8A);
}
