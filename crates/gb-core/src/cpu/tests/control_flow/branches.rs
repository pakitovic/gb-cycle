use super::*;

#[test]
fn jr_nz_taken_and_untaken_use_different_temporal_sequences() {
    let startup_taken = CpuStartupState {
        f: 0x00,
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    };
    let startup_untaken = CpuStartupState {
        f: FLAG_Z,
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    };

    let mut taken_cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut taken_bus = Bus::new(ConsoleModel::Dmg);
    let mut taken_cartridge = build_test_cartridge(&[0x20, 0x02]);
    taken_cpu.apply_startup_state(startup_taken);

    tick_cpu_n(&mut taken_cpu, &mut taken_bus, &mut taken_cartridge, 8);

    assert_eq!(taken_cpu.registers().pc, 0x0102);
    assert_eq!(
        taken_cpu.execution_state(),
        CpuExecutionState::Execute {
            step: 1,
            t_cycle: 0,
        }
    );

    tick_cpu_n(&mut taken_cpu, &mut taken_bus, &mut taken_cartridge, 4);

    assert_eq!(taken_cpu.registers().pc, 0x0104);
    assert_eq!(
        taken_cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );

    let mut untaken_cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut untaken_bus = Bus::new(ConsoleModel::Dmg);
    let mut untaken_cartridge = build_test_cartridge(&[0x20, 0x02]);
    untaken_cpu.apply_startup_state(startup_untaken);

    tick_cpu_n(
        &mut untaken_cpu,
        &mut untaken_bus,
        &mut untaken_cartridge,
        8,
    );

    assert_eq!(untaken_cpu.registers().pc, 0x0102);
    assert_eq!(
        untaken_cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}

#[test]
fn jr_positive_negative_and_jp_hl_match_blargg_special_control_flow_cases() {
    let mut jr_negative_cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut jr_negative_bus = Bus::new(ConsoleModel::Dmg);
    let mut jr_negative_cartridge = build_test_cartridge(&[0x18, 0xFE]);

    jr_negative_cpu.apply_startup_state(CpuStartupState {
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });

    tick_cpu_n(
        &mut jr_negative_cpu,
        &mut jr_negative_bus,
        &mut jr_negative_cartridge,
        12,
    );

    assert_eq!(jr_negative_cpu.registers().pc, 0x0100);
    assert_eq!(
        jr_negative_cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );

    let mut jr_positive_cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut jr_positive_bus = Bus::new(ConsoleModel::Dmg);
    let mut jr_positive_cartridge = build_test_cartridge(&[0x18, 0x01, 0x00, 0x00]);

    jr_positive_cpu.apply_startup_state(CpuStartupState {
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });

    tick_cpu_n(
        &mut jr_positive_cpu,
        &mut jr_positive_bus,
        &mut jr_positive_cartridge,
        12,
    );

    assert_eq!(jr_positive_cpu.registers().pc, 0x0103);
    assert_eq!(
        jr_positive_cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );

    let mut jp_hl_cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut jp_hl_bus = Bus::new(ConsoleModel::Dmg);
    let mut jp_hl_cartridge = build_test_cartridge(&[0xE9]);

    jp_hl_cpu.apply_startup_state(CpuStartupState {
        h: 0xC1,
        l: 0x23,
        f: FLAG_Z | FLAG_C,
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });

    tick_cpu_n(&mut jp_hl_cpu, &mut jp_hl_bus, &mut jp_hl_cartridge, 4);

    assert_eq!(jp_hl_cpu.registers().pc, 0xC123);
    assert_eq!(jp_hl_cpu.registers().f, FLAG_Z | FLAG_C);
    assert_eq!(
        jp_hl_cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}

#[test]
fn jp_nz_taken_and_untaken_use_different_temporal_sequences() {
    let mut taken_cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut taken_bus = Bus::new(ConsoleModel::Dmg);
    let mut taken_cartridge = build_test_cartridge(&[0xC2, 0x34, 0x12]);

    taken_cpu.apply_startup_state(CpuStartupState {
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });

    tick_cpu_n(&mut taken_cpu, &mut taken_bus, &mut taken_cartridge, 12);

    assert_eq!(taken_cpu.registers().pc, 0x0103);
    assert_eq!(
        taken_cpu.execution_state(),
        CpuExecutionState::Execute {
            step: 2,
            t_cycle: 0,
        }
    );

    tick_cpu_n(&mut taken_cpu, &mut taken_bus, &mut taken_cartridge, 4);

    assert_eq!(taken_cpu.registers().pc, 0x1234);
    assert_eq!(
        taken_cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );

    let mut untaken_cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut untaken_bus = Bus::new(ConsoleModel::Dmg);
    let mut untaken_cartridge = build_test_cartridge(&[0xC2, 0x34, 0x12]);

    untaken_cpu.apply_startup_state(CpuStartupState {
        f: FLAG_Z,
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });

    tick_cpu_n(
        &mut untaken_cpu,
        &mut untaken_bus,
        &mut untaken_cartridge,
        12,
    );

    assert_eq!(untaken_cpu.registers().pc, 0x0103);
    assert_eq!(
        untaken_cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}

#[test]
fn call_nz_taken_and_untaken_split_before_stack_traffic() {
    let mut taken_cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut taken_bus = Bus::new(ConsoleModel::Dmg);
    let mut taken_cartridge = build_test_cartridge(&[0xC4, 0x08, 0x01]);

    taken_cpu.apply_startup_state(CpuStartupState {
        sp: 0xFFFE,
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });

    tick_cpu_n(&mut taken_cpu, &mut taken_bus, &mut taken_cartridge, 12);

    assert_eq!(taken_cpu.registers().pc, 0x0103);
    assert_eq!(taken_cpu.registers().sp, 0xFFFE);
    assert_eq!(
        taken_cpu.execution_state(),
        CpuExecutionState::Execute {
            step: 2,
            t_cycle: 0,
        }
    );

    tick_cpu_n(&mut taken_cpu, &mut taken_bus, &mut taken_cartridge, 4);

    assert_eq!(taken_cpu.registers().sp, 0xFFFD);
    assert_eq!(taken_bus.read(0xFFFD), 0x00);
    assert_eq!(
        taken_cpu.execution_state(),
        CpuExecutionState::Execute {
            step: 3,
            t_cycle: 0,
        }
    );

    let mut untaken_cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut untaken_bus = Bus::new(ConsoleModel::Dmg);
    let mut untaken_cartridge = build_test_cartridge(&[0xC4, 0x08, 0x01]);

    untaken_cpu.apply_startup_state(CpuStartupState {
        f: FLAG_Z,
        sp: 0xFFFE,
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });

    tick_cpu_n(
        &mut untaken_cpu,
        &mut untaken_bus,
        &mut untaken_cartridge,
        12,
    );

    assert_eq!(untaken_cpu.registers().pc, 0x0103);
    assert_eq!(untaken_cpu.registers().sp, 0xFFFE);
    assert_eq!(
        untaken_cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}

#[test]
fn ret_nz_taken_and_untaken_split_before_stack_reads() {
    let mut taken_cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut taken_bus = Bus::new(ConsoleModel::Dmg);
    let mut taken_cartridge = build_test_cartridge(&[0xC0]);

    taken_cpu.apply_startup_state(CpuStartupState {
        sp: 0xFFFC,
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });
    taken_bus.write(0xFFFC, 0x34);
    taken_bus.write(0xFFFD, 0x12);

    tick_cpu_n(&mut taken_cpu, &mut taken_bus, &mut taken_cartridge, 8);

    assert_eq!(taken_cpu.registers().sp, 0xFFFC);
    assert_eq!(
        taken_cpu.execution_state(),
        CpuExecutionState::Execute {
            step: 1,
            t_cycle: 0,
        }
    );

    tick_cpu_n(&mut taken_cpu, &mut taken_bus, &mut taken_cartridge, 4);

    assert_eq!(taken_cpu.registers().sp, 0xFFFD);
    assert_eq!(
        taken_cpu.execution_state(),
        CpuExecutionState::Execute {
            step: 2,
            t_cycle: 0,
        }
    );
    assert_eq!(
        taken_cpu.last_address_event(),
        Some(CpuAddressEvent {
            kind: CpuAddressEventKind::ReadWithIncDec,
            access_address: Some(0xFFFC),
            idu_address: Some(0xFFFD),
            update_direction: Some(CpuAddressUpdateDirection::Increment),
        })
    );

    let mut untaken_cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut untaken_bus = Bus::new(ConsoleModel::Dmg);
    let mut untaken_cartridge = build_test_cartridge(&[0xC0]);

    untaken_cpu.apply_startup_state(CpuStartupState {
        f: FLAG_Z,
        sp: 0xFFFC,
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });

    tick_cpu_n(
        &mut untaken_cpu,
        &mut untaken_bus,
        &mut untaken_cartridge,
        8,
    );

    assert_eq!(untaken_cpu.registers().pc, 0x0101);
    assert_eq!(untaken_cpu.registers().sp, 0xFFFC);
    assert_eq!(
        untaken_cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}
