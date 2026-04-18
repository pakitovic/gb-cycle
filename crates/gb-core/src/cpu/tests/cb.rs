use super::*;

#[test]
fn cb_prefix_register_and_hl_variants_keep_double_fetch_and_memory_timing_distinct() {
    let mut register_cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut register_bus = Bus::new(ConsoleModel::Dmg);
    let mut register_cartridge = build_test_cartridge(&[0xCB, 0x11]);
    register_cpu.apply_startup_state(CpuStartupState {
        c: 0x81,
        f: FLAG_C,
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });

    tick_cpu_n(
        &mut register_cpu,
        &mut register_bus,
        &mut register_cartridge,
        4,
    );

    assert_eq!(
        register_cpu.execution_state(),
        CpuExecutionState::Execute {
            step: 0,
            t_cycle: 0,
        }
    );

    tick_cpu_n(
        &mut register_cpu,
        &mut register_bus,
        &mut register_cartridge,
        4,
    );

    assert_eq!(register_cpu.registers().c, 0x03);
    assert_eq!(register_cpu.registers().f, FLAG_C);
    assert_eq!(register_cpu.registers().pc, 0x0102);
    assert_eq!(
        register_cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );

    let mut hl_cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut hl_bus = Bus::new(ConsoleModel::Dmg);
    let mut hl_cartridge = build_test_cartridge(&[0xCB, 0x06]);
    hl_cpu.apply_startup_state(CpuStartupState {
        h: 0xC0,
        l: 0x00,
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });
    hl_bus.write(0xC000, 0x81);

    tick_cpu_n(&mut hl_cpu, &mut hl_bus, &mut hl_cartridge, 8);

    assert_eq!(hl_bus.read(0xC000), 0x81);
    assert_eq!(
        hl_cpu.execution_state(),
        CpuExecutionState::Execute {
            step: 1,
            t_cycle: 0,
        }
    );

    tick_cpu_n(&mut hl_cpu, &mut hl_bus, &mut hl_cartridge, 4);

    assert_eq!(hl_bus.read(0xC000), 0x81);
    assert_eq!(
        hl_cpu.execution_state(),
        CpuExecutionState::Execute {
            step: 2,
            t_cycle: 0,
        }
    );

    tick_cpu_n(&mut hl_cpu, &mut hl_bus, &mut hl_cartridge, 4);

    assert_eq!(hl_bus.read(0xC000), 0x03);
    assert_eq!(hl_cpu.registers().f, FLAG_C);
    assert_eq!(hl_cpu.registers().pc, 0x0102);
    assert_eq!(
        hl_cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}

#[test]
fn cb_rr_and_srl_support_the_blargg_crc_runtime_path() {
    let mut rr_cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut rr_bus = Bus::new(ConsoleModel::Dmg);
    let mut rr_cartridge = build_test_cartridge(&[0xCB, 0x19]);
    rr_cpu.apply_startup_state(CpuStartupState {
        c: 0x80,
        f: FLAG_C,
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });

    tick_cpu_n(&mut rr_cpu, &mut rr_bus, &mut rr_cartridge, 8);

    assert_eq!(rr_cpu.registers().c, 0xC0);
    assert_eq!(rr_cpu.registers().f, 0x00);
    assert_eq!(rr_cpu.registers().pc, 0x0102);
    assert_eq!(
        rr_cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );

    let mut srl_cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut srl_bus = Bus::new(ConsoleModel::Dmg);
    let mut srl_cartridge = build_test_cartridge(&[0xCB, 0x3E]);
    srl_cpu.apply_startup_state(CpuStartupState {
        h: 0xC0,
        l: 0x00,
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });
    srl_bus.write(0xC000, 0x81);

    tick_cpu_n(&mut srl_cpu, &mut srl_bus, &mut srl_cartridge, 12);

    assert_eq!(srl_bus.read(0xC000), 0x81);
    assert_eq!(
        srl_cpu.execution_state(),
        CpuExecutionState::Execute {
            step: 2,
            t_cycle: 0,
        }
    );

    tick_cpu_n(&mut srl_cpu, &mut srl_bus, &mut srl_cartridge, 4);

    assert_eq!(srl_bus.read(0xC000), 0x40);
    assert_eq!(srl_cpu.registers().f, FLAG_C);
    assert_eq!(srl_cpu.registers().pc, 0x0102);
    assert_eq!(
        srl_cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}

#[test]
fn cb_rrc_register_and_hl_variants_support_the_external_bitop_paths() {
    let mut register_cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut register_bus = Bus::new(ConsoleModel::Dmg);
    let mut register_cartridge = build_test_cartridge(&[0xCB, 0x08]);
    register_cpu.apply_startup_state(CpuStartupState {
        b: 0x01,
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });

    tick_cpu_n(
        &mut register_cpu,
        &mut register_bus,
        &mut register_cartridge,
        8,
    );

    assert_eq!(register_cpu.registers().b, 0x80);
    assert_eq!(register_cpu.registers().f, FLAG_C);
    assert_eq!(register_cpu.registers().pc, 0x0102);
    assert_eq!(
        register_cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );

    let mut hl_cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut hl_bus = Bus::new(ConsoleModel::Dmg);
    let mut hl_cartridge = build_test_cartridge(&[0xCB, 0x0E]);
    hl_cpu.apply_startup_state(CpuStartupState {
        h: 0xC0,
        l: 0x00,
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });
    hl_bus.write(0xC000, 0x01);

    tick_cpu_n(&mut hl_cpu, &mut hl_bus, &mut hl_cartridge, 12);

    assert_eq!(hl_bus.read(0xC000), 0x01);
    assert_eq!(
        hl_cpu.execution_state(),
        CpuExecutionState::Execute {
            step: 2,
            t_cycle: 0,
        }
    );

    tick_cpu_n(&mut hl_cpu, &mut hl_bus, &mut hl_cartridge, 4);

    assert_eq!(hl_bus.read(0xC000), 0x80);
    assert_eq!(hl_cpu.registers().f, FLAG_C);
    assert_eq!(hl_cpu.registers().pc, 0x0102);
    assert_eq!(
        hl_cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}

#[test]
fn cb_sla_sra_and_swap_register_variants_update_flags_as_documented() {
    let mut sla_cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut sla_bus = Bus::new(ConsoleModel::Dmg);
    let mut sla_cartridge = build_test_cartridge(&[0xCB, 0x20]);
    sla_cpu.apply_startup_state(CpuStartupState {
        b: 0x81,
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });

    tick_cpu_n(&mut sla_cpu, &mut sla_bus, &mut sla_cartridge, 8);

    assert_eq!(sla_cpu.registers().b, 0x02);
    assert_eq!(sla_cpu.registers().f, FLAG_C);

    let mut sra_cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut sra_bus = Bus::new(ConsoleModel::Dmg);
    let mut sra_cartridge = build_test_cartridge(&[0xCB, 0x28]);
    sra_cpu.apply_startup_state(CpuStartupState {
        b: 0x81,
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });

    tick_cpu_n(&mut sra_cpu, &mut sra_bus, &mut sra_cartridge, 8);

    assert_eq!(sra_cpu.registers().b, 0xC0);
    assert_eq!(sra_cpu.registers().f, FLAG_C);

    let mut swap_cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut swap_bus = Bus::new(ConsoleModel::Dmg);
    let mut swap_cartridge = build_test_cartridge(&[0xCB, 0x30]);
    swap_cpu.apply_startup_state(CpuStartupState {
        b: 0xF0,
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });

    tick_cpu_n(&mut swap_cpu, &mut swap_bus, &mut swap_cartridge, 8);

    assert_eq!(swap_cpu.registers().b, 0x0F);
    assert_eq!(swap_cpu.registers().f, 0x00);
}

#[test]
fn cb_res_and_set_preserve_flags_for_register_and_hl_targets() {
    let mut register_cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut register_bus = Bus::new(ConsoleModel::Dmg);
    let mut register_cartridge = build_test_cartridge(&[0xCB, 0x80, 0xCB, 0xC0]);
    register_cpu.apply_startup_state(CpuStartupState {
        b: 0xFF,
        f: FLAG_Z | FLAG_C,
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });

    tick_cpu_n(
        &mut register_cpu,
        &mut register_bus,
        &mut register_cartridge,
        16,
    );

    assert_eq!(register_cpu.registers().b, 0xFF);
    assert_eq!(register_cpu.registers().f, FLAG_Z | FLAG_C);
    assert_eq!(register_cpu.registers().pc, 0x0104);

    let mut hl_cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut hl_bus = Bus::new(ConsoleModel::Dmg);
    let mut hl_cartridge = build_test_cartridge(&[0xCB, 0x86, 0xCB, 0xC6]);
    hl_cpu.apply_startup_state(CpuStartupState {
        h: 0xC0,
        l: 0x00,
        f: FLAG_Z | FLAG_C,
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });
    hl_bus.write(0xC000, 0xFF);

    tick_cpu_n(&mut hl_cpu, &mut hl_bus, &mut hl_cartridge, 32);

    assert_eq!(hl_bus.read(0xC000), 0xFF);
    assert_eq!(hl_cpu.registers().f, FLAG_Z | FLAG_C);
    assert_eq!(hl_cpu.registers().pc, 0x0104);
    assert_eq!(
        hl_cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}

#[test]
fn bit_cb_operation_preserves_carry_while_setting_half_carry() {
    let mut cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let mut cartridge = build_test_cartridge(&[0xCB, 0x7C]);

    cpu.apply_startup_state(CpuStartupState {
        h: 0x80,
        f: FLAG_C,
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });

    tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 8);

    assert_eq!(cpu.registers().f, FLAG_H | FLAG_C);
    assert_eq!(cpu.registers().pc, 0x0102);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}
