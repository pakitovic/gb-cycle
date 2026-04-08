use gb_core::{
    Bus, BusArbitrationState, BusRequester, ConsoleModel, DmaAdvanceCondition, DmaBusState,
    DmaCpuAccessPolicy, DmaCpuImpactPolicy, DmaMemoryRegionImpact, DmaTransferFamily,
    DmaTransferKind, DmaTransferLifecycle, DmaTransferState, Machine, MachineConfig, StartupMode,
};

fn read_cartridgeless_bus_harness(bus: &mut Bus, address: u16) -> u8 {
    let state = BusArbitrationState::default();
    bus.read_with_cartridge(address, BusRequester::Cpu, &state, None)
}

#[test]
fn ff46_write_builds_a_starting_oam_transfer_with_normalized_dmg_metadata() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF46, 0x12);

    let progress = match machine.dma().transfer_state() {
        DmaTransferState::Starting(progress) => progress,
        state => panic!("expected starting DMA transfer, got {state:?}"),
    };
    let transfer = progress.transfer();

    assert_eq!(machine.read_bus(0xFF46), 0x12);
    assert_eq!(transfer.kind(), DmaTransferKind::Oam);
    assert_eq!(transfer.source_start(), 0x1200);
    assert_eq!(transfer.source_end_inclusive(), 0x129F);
    assert_eq!(transfer.destination_start(), 0xFE00);
    assert_eq!(transfer.destination_end_inclusive(), 0xFE9F);
    assert_eq!(transfer.total_bytes(), 160);
    assert_eq!(transfer.block_size(), 1);
    assert_eq!(transfer.total_blocks(), 160);
    assert_eq!(transfer.family(), DmaTransferFamily::FullBurst);
    assert_eq!(
        transfer.advance_condition(),
        DmaAdvanceCondition::EveryTCycle
    );
    assert_eq!(progress.remaining_bytes(), 160);
    assert_eq!(transfer.timing().total_t_cycles(), 648);
    assert_eq!(transfer.timing().first_byte_delay_t_cycles(), 8);
    assert_eq!(transfer.timing().cpu_bus_restriction_delay_t_cycles(), 5);
    assert_eq!(transfer.timing().t_cycles_per_byte(), 4);
    assert_eq!(
        transfer.cpu_impact_policy(),
        DmaCpuImpactPolicy::NoCpuStallButBusRestriction
    );
    assert_eq!(transfer.memory_region_impact(), DmaMemoryRegionImpact::Oam);
}

#[test]
fn skip_boot_keeps_ff46_visible_but_leaves_dma_idle_until_software_starts_it() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    assert_eq!(machine.read_bus(0xFF46), 0xFF);
    assert_eq!(machine.dma().transfer_state(), DmaTransferState::Idle);
    assert_eq!(machine.dma().current_transfer(), None);
}

#[test]
fn scheduler_ticks_move_oam_dma_from_starting_to_active_and_then_completed() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF46, 0x12);
    for expected_elapsed_t_cycles in 1..=4 {
        machine.step_t_cycle();

        let starting_progress = match machine.dma().transfer_state() {
            DmaTransferState::Starting(progress) => progress,
            state => panic!(
                "expected starting transfer during the post-write machine cycle, got {state:?}"
            ),
        };
        assert_eq!(
            starting_progress.elapsed_t_cycles(),
            expected_elapsed_t_cycles
        );
        assert_eq!(starting_progress.completed_bytes(), 0);
        assert_eq!(starting_progress.remaining_bytes(), 160);
        assert_eq!(machine.dma().bus_state(), DmaBusState::unrestricted());
    }

    machine.step_t_cycle();

    let active_progress = match machine.dma().transfer_state() {
        DmaTransferState::Active(progress) => progress,
        state => panic!("expected active transfer after fifth tick, got {state:?}"),
    };
    assert_eq!(active_progress.elapsed_t_cycles(), 5);
    assert_eq!(active_progress.completed_bytes(), 0);
    assert_eq!(active_progress.first_byte_delay_remaining_t_cycles(), 3);
    assert_eq!(
        machine.dma().bus_state(),
        DmaBusState::external_bus_blocked(Some(DmaMemoryRegionImpact::Oam))
    );

    for _ in 0..2 {
        machine.step_t_cycle();
    }

    let first_byte_progress = match machine.dma().transfer_state() {
        DmaTransferState::Active(progress) => progress,
        state => panic!("expected active transfer before the first byte, got {state:?}"),
    };
    assert_eq!(first_byte_progress.elapsed_t_cycles(), 7);
    assert_eq!(first_byte_progress.completed_bytes(), 0);

    machine.step_t_cycle();

    let first_byte_progress = match machine.dma().transfer_state() {
        DmaTransferState::Active(progress) => progress,
        state => panic!("expected the first DMA byte on the eighth tick, got {state:?}"),
    };
    assert_eq!(first_byte_progress.elapsed_t_cycles(), 8);
    assert_eq!(first_byte_progress.completed_bytes(), 1);

    for _ in 0..639 {
        machine.step_t_cycle();
    }

    let final_active_progress = match machine.dma().transfer_state() {
        DmaTransferState::Active(progress) => progress,
        state => panic!("expected active transfer one tick before completion, got {state:?}"),
    };
    assert_eq!(final_active_progress.elapsed_t_cycles(), 647);
    assert_eq!(final_active_progress.completed_bytes(), 160);
    assert_eq!(final_active_progress.remaining_bytes(), 0);

    machine.step_t_cycle();

    let completed_progress = match machine.dma().transfer_state() {
        DmaTransferState::Completed(progress) => progress,
        state => panic!("expected completed transfer after the visible DMA window, got {state:?}"),
    };
    assert_eq!(completed_progress.elapsed_t_cycles(), 648);
    assert_eq!(completed_progress.completed_bytes(), 160);
    assert_eq!(completed_progress.remaining_bytes(), 0);
    assert_eq!(machine.dma().bus_state(), DmaBusState::unrestricted());
}

#[test]
fn dma_trace_shows_start_and_completion_points_with_progress_metadata() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF46, 0x12);

    for _ in 0..649 {
        machine.step_t_cycle();
    }

    let trace = machine.tracer().sink().render_text();

    assert!(trace.contains(
        "subsystem=dma level=trace message=\"t_cycle=0 phase=autonomous_peripheral_ticks console_model=Dmg status=Ready transfer_state=Starting transfer_kind=Oam transfer_family=FullBurst block_size=1 advance_condition=EveryTCycle first_byte_delay_t_cycles=8 first_byte_delay_remaining_t_cycles=7 cpu_bus_restriction_delay_t_cycles=5 cpu_bus_restriction_delay_remaining_t_cycles=4 cpu_bus_restriction_active=false elapsed_t_cycles=1 completed_bytes=0 remaining_bytes=160 completed_blocks=0 remaining_blocks=160"
    ));
    assert!(trace.contains(
        "subsystem=dma level=trace message=\"t_cycle=648 phase=autonomous_peripheral_ticks console_model=Dmg status=Ready transfer_state=Completed transfer_kind=Oam transfer_family=FullBurst block_size=1 advance_condition=EveryTCycle first_byte_delay_t_cycles=8 first_byte_delay_remaining_t_cycles=0 cpu_bus_restriction_delay_t_cycles=5 cpu_bus_restriction_delay_remaining_t_cycles=0 cpu_bus_restriction_active=true elapsed_t_cycles=648 completed_bytes=160 remaining_bytes=0 completed_blocks=160 remaining_blocks=0"
    ));
    assert!(trace.contains(&format!(
        "cpu_access_policy={:?} active_region={:?}",
        DmaCpuAccessPolicy::ExternalBusBlocked,
        Some(DmaMemoryRegionImpact::Oam)
    )));
}

#[test]
fn external_bus_dma_restricts_machine_bus_access_to_hram_and_vram_only_from_live_state() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xC000, 0x34);
    machine.write_bus(0x8000, 0x78);
    machine.write_bus(0xFF80, 0x12);
    machine.write_bus(0xFF46, 0x12);
    for _ in 0..4 {
        machine.step_t_cycle();
    }

    assert_eq!(machine.dma().bus_state(), DmaBusState::unrestricted());
    assert_eq!(machine.read_bus(0xC000), 0x34);
    assert_eq!(machine.read_bus(0x8000), 0x78);
    assert_eq!(machine.read_bus(0xFF46), 0x12);
    assert_eq!(machine.dma().source_page_latch(), 0x12);
    assert_eq!(machine.read_bus(0xFF80), 0x12);

    machine.step_t_cycle();

    assert_eq!(
        machine.dma().bus_state(),
        DmaBusState::external_bus_blocked(Some(DmaMemoryRegionImpact::Oam))
    );
    assert_eq!(machine.read_bus(0xC000), 0xFF);
    assert_eq!(machine.read_bus(0x8000), 0x78);
    assert_eq!(machine.read_bus(0xFF46), 0x12);

    machine.write_bus(0xC000, 0x99);
    machine.write_bus(0x8000, 0xBC);
    machine.write_bus(0xFF80, 0x56);

    assert_eq!(machine.read_bus(0xC000), 0xFF);
    assert_eq!(machine.read_bus(0x8000), 0xBC);
    assert_eq!(machine.read_bus(0xFF80), 0x56);

    for _ in 0..649 {
        machine.step_t_cycle();
    }

    assert_eq!(machine.dma().bus_state(), DmaBusState::unrestricted());
    assert_eq!(machine.read_bus(0xC000), 0x34);
}

#[test]
fn external_bus_dma_redirects_cpu_reads_to_the_most_recent_source_byte_after_the_first_copy() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    seed_dma_source_page(&mut machine, 0xC1, 0x33);
    machine.write_bus(0xC200, 0xAA);
    machine.write_bus(0xFF46, 0xC1);
    for _ in 0..8 {
        machine.step_t_cycle();
    }

    assert_eq!(
        machine.dma().bus_state(),
        DmaBusState::external_bus_blocked(Some(DmaMemoryRegionImpact::Oam))
            .with_cpu_conflict_source_address(Some(0xC100))
    );
    assert_eq!(machine.read_bus(0xC200), dma_source_byte(0x33, 0));
}

#[test]
fn external_bus_dma_redirects_cpu_writes_to_the_most_recent_source_byte_after_the_first_copy() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    seed_dma_source_page(&mut machine, 0xC1, 0x47);
    machine.write_bus(0xC200, 0x55);
    machine.write_bus(0xFF46, 0xC1);
    for _ in 0..8 {
        machine.step_t_cycle();
    }

    machine.write_bus(0xC200, 0x99);

    for _ in 0..640 {
        machine.step_t_cycle();
    }

    assert_eq!(machine.dma().bus_state(), DmaBusState::unrestricted());
    assert_eq!(machine.read_bus(0xC100), 0x99);
    assert_eq!(machine.read_bus(0xC200), 0x55);
}

#[test]
fn video_bus_dma_leaves_wram_and_echo_accessible_while_blocking_vram_and_oam() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xC000, 0x34);
    machine.write_bus(0xFDFF, 0x7A);
    machine.write_bus(0x8000, 0x56);
    machine.write_bus(0xFE00, 0x89);
    machine.write_bus(0xFF80, 0x12);
    machine.write_bus(0xFF46, 0x80);
    for _ in 0..5 {
        machine.step_t_cycle();
    }

    assert_eq!(
        machine.dma().bus_state(),
        DmaBusState::video_bus_blocked(Some(DmaMemoryRegionImpact::Oam))
    );
    assert_eq!(machine.read_bus(0xC000), 0x34);
    assert_eq!(machine.read_bus(0xFDFF), 0x7A);
    assert_eq!(machine.read_bus(0x8000), 0xFF);
    assert_eq!(machine.read_bus(0xFE00), 0xFF);
    assert_eq!(machine.read_bus(0xFF80), 0x12);
    assert_eq!(machine.read_bus(0xFF46), 0x80);

    machine.write_bus(0xC000, 0x91);
    machine.write_bus(0xFDFF, 0x42);
    machine.write_bus(0x8000, 0xAB);
    machine.write_bus(0xFE00, 0xCD);

    assert_eq!(machine.read_bus(0xC000), 0x91);
    assert_eq!(machine.read_bus(0xFDFF), 0x42);
    assert_eq!(machine.read_bus(0x8000), 0xFF);
    assert_eq!(machine.read_bus(0xFE00), 0xFF);
}

#[test]
fn dma_and_bus_traces_show_the_same_cycle_arbitration_constraints() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF46, 0x12);
    machine.step_t_cycle();

    let trace = machine.tracer().sink().render_text();

    assert!(trace.contains(
        "subsystem=dma level=trace message=\"t_cycle=0 phase=autonomous_peripheral_ticks console_model=Dmg status=Ready transfer_state=Starting transfer_kind=Oam transfer_family=FullBurst block_size=1 advance_condition=EveryTCycle first_byte_delay_t_cycles=8 first_byte_delay_remaining_t_cycles=7 cpu_bus_restriction_delay_t_cycles=5 cpu_bus_restriction_delay_remaining_t_cycles=4 cpu_bus_restriction_active=false elapsed_t_cycles=1 completed_bytes=0 remaining_bytes=160 completed_blocks=0 remaining_blocks=160"
    ));
    assert!(trace.contains(
        "subsystem=bus level=trace message=\"t_cycle=0 phase=bus_arbitration console_model=Dmg status=Ready boot_low_window_mapped=false boot_cgb_upper_window_mapped=false ppu_lcd_enabled=true ppu_mode=OamScan dma_cpu_access_policy=Unrestricted dma_active_region=None dma_cpu_conflict_source_address=None\""
    ));
}

#[test]
fn oam_dma_copies_the_latched_source_page_contents_into_oam_after_completion() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    seed_dma_source_page(&mut machine, 0xC1, 0x19);
    seed_dma_source_page(&mut machine, 0xC2, 0x5C);

    machine.write_bus(0xFF46, 0xC2);

    for _ in 0..649 {
        machine.step_t_cycle();
    }

    let completed_progress = match machine.dma().transfer_state() {
        DmaTransferState::Completed(progress) => progress,
        state => panic!("expected completed DMA transfer, got {state:?}"),
    };
    assert_eq!(completed_progress.completed_bytes(), 160);
    assert_eq!(completed_progress.remaining_bytes(), 0);

    let mut bus = machine.bus().clone();

    for byte_index in 0..160u16 {
        assert_eq!(
            read_cartridgeless_bus_harness(&mut bus, 0xFE00 + byte_index),
            dma_source_byte(0x5C, byte_index)
        );
        assert_ne!(
            read_cartridgeless_bus_harness(&mut bus, 0xFE00 + byte_index),
            dma_source_byte(0x19, byte_index)
        );
    }
}

#[test]
fn dma_status_view_reports_lifecycle_progress_and_bus_impact_without_ff46_readback() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    let idle_status = machine.dma().transfer_status();
    assert_eq!(idle_status.lifecycle(), DmaTransferLifecycle::Idle);
    assert!(!idle_status.is_in_flight());
    assert_eq!(idle_status.transfer(), None);
    assert_eq!(idle_status.progress(), None);
    assert_eq!(idle_status.bus_state(), DmaBusState::unrestricted());
    assert_eq!(
        machine.dma().transfer_lifecycle(),
        DmaTransferLifecycle::Idle
    );
    assert!(!machine.dma().has_in_flight_transfer());

    machine.write_bus(0xFF46, 0x12);

    let armed_status = machine.dma().transfer_status();
    assert_eq!(armed_status.lifecycle(), DmaTransferLifecycle::Starting);
    assert!(armed_status.is_in_flight());
    assert_eq!(armed_status.bus_state(), DmaBusState::unrestricted());
    assert_eq!(
        armed_status.transfer().map(|transfer| transfer.kind()),
        Some(DmaTransferKind::Oam)
    );
    assert_eq!(
        armed_status
            .progress()
            .map(|progress| progress.elapsed_t_cycles()),
        Some(0)
    );
    assert!(machine.dma().has_in_flight_transfer());

    for _ in 0..4 {
        machine.step_t_cycle();
    }
    let warm_up_status = machine.dma().transfer_status();
    assert_eq!(warm_up_status.lifecycle(), DmaTransferLifecycle::Starting);
    assert_eq!(warm_up_status.bus_state(), DmaBusState::unrestricted());

    machine.step_t_cycle();
    let active_bus_impact = machine.dma().transfer_status();
    assert_eq!(active_bus_impact.lifecycle(), DmaTransferLifecycle::Active);
    assert_eq!(
        active_bus_impact.bus_state(),
        DmaBusState::external_bus_blocked(Some(DmaMemoryRegionImpact::Oam))
    );

    for _ in 0..643 {
        machine.step_t_cycle();
    }

    let completed_status = machine.dma().transfer_status();
    assert_eq!(
        completed_status.lifecycle(),
        DmaTransferLifecycle::Completed
    );
    assert!(!completed_status.is_in_flight());
    assert_eq!(completed_status.bus_state(), DmaBusState::unrestricted());
    assert_eq!(
        completed_status
            .progress()
            .map(|progress| progress.completed_bytes()),
        Some(160)
    );
}

#[test]
fn oam_dma_progress_and_partial_oam_contents_remain_observable_before_completion() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    seed_dma_source_page(&mut machine, 0xC1, 0x33);
    machine.write_bus(0xFE00, 0xE0);
    machine.write_bus(0xFE01, 0xE1);
    machine.write_bus(0xFE02, 0xE2);
    machine.write_bus(0xFF46, 0xC1);

    machine.step_t_cycle();
    for _ in 0..3 {
        machine.step_t_cycle();
    }

    let warm_up_progress = match machine.dma().transfer_state() {
        DmaTransferState::Starting(progress) => progress,
        state => panic!("expected DMA warm-up through the post-write machine cycle, got {state:?}"),
    };
    assert_eq!(warm_up_progress.elapsed_t_cycles(), 4);
    assert_eq!(warm_up_progress.completed_bytes(), 0);
    assert_eq!(warm_up_progress.first_byte_delay_remaining_t_cycles(), 4);
    assert_eq!(machine.dma().bus_state(), DmaBusState::unrestricted());

    let mut warm_up_bus = machine.bus().clone();
    assert_eq!(
        read_cartridgeless_bus_harness(&mut warm_up_bus, 0xFE00),
        0xE0
    );
    assert_eq!(
        read_cartridgeless_bus_harness(&mut warm_up_bus, 0xFE01),
        0xE1
    );

    machine.step_t_cycle();

    let active_progress = match machine.dma().transfer_state() {
        DmaTransferState::Active(progress) => progress,
        state => {
            panic!("expected active DMA transfer after the post-write machine cycle, got {state:?}")
        }
    };
    assert_eq!(active_progress.elapsed_t_cycles(), 5);
    assert_eq!(active_progress.completed_bytes(), 0);
    assert_eq!(active_progress.remaining_bytes(), 160);
    assert_eq!(
        machine.dma().bus_state(),
        DmaBusState::external_bus_blocked(Some(DmaMemoryRegionImpact::Oam))
    );

    let mut active_bus = machine.bus().clone();
    assert_eq!(
        read_cartridgeless_bus_harness(&mut active_bus, 0xFE00),
        0xE0
    );
    assert_eq!(
        read_cartridgeless_bus_harness(&mut active_bus, 0xFE01),
        0xE1
    );

    for _ in 0..3 {
        machine.step_t_cycle();
    }

    let first_byte_progress = match machine.dma().transfer_state() {
        DmaTransferState::Active(progress) => progress,
        state => {
            panic!("expected the first DMA byte after the first active byte cycle, got {state:?}")
        }
    };
    assert_eq!(first_byte_progress.elapsed_t_cycles(), 8);
    assert_eq!(first_byte_progress.completed_bytes(), 1);
    assert_eq!(first_byte_progress.remaining_bytes(), 159);

    let mut bus = machine.bus().clone();
    assert_eq!(
        read_cartridgeless_bus_harness(&mut bus, 0xFE00),
        dma_source_byte(0x33, 0)
    );
    assert_eq!(read_cartridgeless_bus_harness(&mut bus, 0xFE01), 0xE1);
    assert_eq!(read_cartridgeless_bus_harness(&mut bus, 0xFE02), 0xE2);
}

#[test]
fn oam_dma_completion_happens_after_the_last_active_transfer_t_cycle() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    seed_dma_source_page(&mut machine, 0xC1, 0x47);
    machine.write_bus(0xFF46, 0xC1);

    for _ in 0..647 {
        machine.step_t_cycle();
    }

    let final_active_progress = match machine.dma().transfer_state() {
        DmaTransferState::Active(progress) => progress,
        state => {
            panic!("expected final active DMA progress one tick before completion, got {state:?}")
        }
    };
    assert_eq!(final_active_progress.elapsed_t_cycles(), 647);
    assert_eq!(final_active_progress.completed_bytes(), 160);
    assert_eq!(final_active_progress.byte_phase_t_cycles(), 3);

    let mut active_bus = machine.bus().clone();
    assert_eq!(
        read_cartridgeless_bus_harness(&mut active_bus, 0xFE9F),
        dma_source_byte(0x47, 159)
    );

    machine.step_t_cycle();

    let completed_progress = match machine.dma().transfer_state() {
        DmaTransferState::Completed(progress) => progress,
        state => {
            panic!("expected completed DMA transfer after the last active T-cycle, got {state:?}")
        }
    };
    assert_eq!(completed_progress.elapsed_t_cycles(), 648);
    assert_eq!(machine.dma().bus_state(), DmaBusState::unrestricted());

    let mut completed_bus = machine.bus().clone();
    assert_eq!(
        read_cartridgeless_bus_harness(&mut completed_bus, 0xFE9F),
        dma_source_byte(0x47, 159)
    );
}

fn seed_dma_source_page(machine: &mut Machine, source_page: u8, seed: u8) {
    let source_start = (source_page as u16) << 8;

    for byte_index in 0..160u16 {
        machine.write_bus(source_start + byte_index, dma_source_byte(seed, byte_index));
    }
}

fn dma_source_byte(seed: u8, byte_index: u16) -> u8 {
    seed.wrapping_mul(17)
        .wrapping_add(byte_index as u8)
        .rotate_left(1)
}
