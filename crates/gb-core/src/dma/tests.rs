use super::*;

#[test]
fn oam_transfer_normalizes_the_source_range_destination_and_dmg_metadata() {
    let transfer = DmaTransfer::oam(0x12);

    assert_eq!(transfer.kind(), DmaTransferKind::Oam);
    assert_eq!(transfer.source_start(), 0x1200);
    assert_eq!(transfer.source_end_inclusive(), 0x129F);
    assert_eq!(transfer.destination_start(), 0xFE00);
    assert_eq!(transfer.destination_end_inclusive(), 0xFE9F);
    assert_eq!(transfer.total_bytes(), OAM_DMA_TRANSFER_BYTES);
    assert_eq!(transfer.timing().total_t_cycles(), OAM_DMA_TOTAL_T_CYCLES);
    assert_eq!(
        transfer.timing().first_byte_delay_t_cycles(),
        OAM_DMA_FIRST_BYTE_DELAY_T_CYCLES
    );
    assert_eq!(
        transfer.timing().cpu_bus_restriction_delay_t_cycles(),
        OAM_DMA_CPU_BUS_RESTRICTION_DELAY_T_CYCLES
    );
    assert_eq!(
        transfer.timing().t_cycles_per_byte(),
        OAM_DMA_T_CYCLES_PER_BYTE
    );
    assert_eq!(
        transfer.cpu_impact_policy(),
        DmaCpuImpactPolicy::NoCpuStallButBusRestriction
    );
    assert_eq!(transfer.memory_region_impact(), DmaMemoryRegionImpact::Oam);
}

#[test]
fn dmg_oam_dma_source_addresses_above_dfff_follow_the_echo_alias_path() {
    let transfer = DmaTransfer::oam(0xFE);

    assert_eq!(transfer.source_address_for_byte(0), 0xDE00);
    assert_eq!(transfer.source_address_for_byte(0x9F), 0xDE9F);

    let transfer = DmaTransfer::oam(0xFF);

    assert_eq!(transfer.source_address_for_byte(0), 0xDF00);
    assert_eq!(transfer.source_address_for_byte(0x9F), 0xDF9F);
}

#[test]
fn ff46_latches_the_source_page_and_builds_a_starting_oam_transfer_immediately() {
    let mut dma = DmaController::new(ConsoleModel::GameBoy);

    dma.write_ff46(0x12);

    assert_eq!(dma.read_ff46(), 0x12);
    assert_eq!(
        dma.transfer_state(),
        DmaTransferState::Starting(DmaTransferProgress::new(DmaTransfer::oam(0x12)))
    );
    assert_eq!(dma.current_transfer(), Some(DmaTransfer::oam(0x12)));
    assert_eq!(
        dma.transfer_progress(),
        Some(DmaTransferProgress::new(DmaTransfer::oam(0x12)))
    );
    assert_eq!(dma.bus_state(), DmaBusState::unrestricted());
    assert_eq!(dma.pending_restart, None);
}

#[test]
fn dma_tick_advances_starting_active_and_completed_lifecycle_over_t_cycles() {
    let mut dma = DmaController::new(ConsoleModel::GameBoy);
    let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

    dma.write_ff46(0x12);
    for expected_elapsed_t_cycles in 1..=4 {
        let transfer_work = dma.tick_t_cycle(&mut context);

        let starting_progress = match dma.transfer_state() {
            DmaTransferState::Starting(progress) => progress,
            state => panic!(
                "expected starting progress during the post-write machine cycle, got {state:?}"
            ),
        };
        assert_eq!(
            starting_progress.elapsed_t_cycles(),
            expected_elapsed_t_cycles
        );
        assert_eq!(starting_progress.completed_bytes(), 0);
        assert!(!starting_progress.is_cpu_bus_restriction_active());
        assert_eq!(dma.bus_state(), DmaBusState::unrestricted());
        assert_eq!(transfer_work, None);
    }

    let activation_work = dma.tick_t_cycle(&mut context);
    let active_progress = match dma.transfer_state() {
        DmaTransferState::Active(progress) => progress,
        state => panic!("expected active progress after fifth tick, got {state:?}"),
    };
    assert_eq!(active_progress.elapsed_t_cycles(), 5);
    assert_eq!(active_progress.completed_bytes(), 0);
    assert_eq!(active_progress.first_byte_delay_remaining_t_cycles(), 3);
    assert!(active_progress.is_cpu_bus_restriction_active());
    assert_eq!(activation_work, None);

    for expected_elapsed_t_cycles in 6..=7 {
        assert_eq!(dma.tick_t_cycle(&mut context), None);
        let active_progress = match dma.transfer_state() {
            DmaTransferState::Active(progress) => progress,
            state => panic!("expected active progress before the first byte, got {state:?}"),
        };
        assert_eq!(
            active_progress.elapsed_t_cycles(),
            expected_elapsed_t_cycles
        );
        assert_eq!(active_progress.completed_bytes(), 0);
    }

    let first_byte_work = dma
        .tick_t_cycle(&mut context)
        .expect("expected first DMA byte on the first active byte cycle");
    assert_eq!(first_byte_work.byte_index(), 0);

    for _ in 0..639 {
        dma.tick_t_cycle(&mut context);
    }

    let final_active_progress = match dma.transfer_state() {
        DmaTransferState::Active(progress) => progress,
        state => panic!("expected final active progress before completion, got {state:?}"),
    };
    assert_eq!(final_active_progress.elapsed_t_cycles(), 647);
    assert_eq!(final_active_progress.completed_bytes(), 160);
    assert_eq!(final_active_progress.remaining_bytes(), 0);

    dma.tick_t_cycle(&mut context);

    let completed_progress = match dma.transfer_state() {
        DmaTransferState::Completed(progress) => progress,
        state => panic!("expected completed transfer after final tick, got {state:?}"),
    };
    assert_eq!(completed_progress.elapsed_t_cycles(), 648);
    assert_eq!(dma.bus_state(), DmaBusState::unrestricted());
}

#[test]
fn restarting_active_oam_dma_keeps_the_current_transfer_alive_until_the_new_startup_seam_finishes()
{
    let mut dma = DmaController::new(ConsoleModel::GameBoy);
    let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

    dma.write_ff46(0x12);
    for _ in 0..4 {
        assert_eq!(dma.tick_t_cycle(&mut context), None);
    }
    assert_eq!(dma.tick_t_cycle(&mut context), None);
    assert!(matches!(
        dma.transfer_state(),
        DmaTransferState::Active(progress) if progress.elapsed_t_cycles() == 5
    ));

    dma.write_ff46(0x34);
    assert_eq!(
        dma.pending_restart,
        Some(DmaTransferProgress::new(DmaTransfer::oam(0x34)))
    );
    assert_eq!(
        dma.bus_state(),
        DmaBusState::external_bus_blocked(Some(DmaMemoryRegionImpact::Oam))
    );

    for expected_elapsed_t_cycles in 1..=4 {
        let transfer_work = dma.tick_t_cycle(&mut context);
        assert_eq!(
            dma.pending_restart,
            Some(DmaTransferProgress {
                transfer: DmaTransfer::oam(0x34),
                elapsed_t_cycles: expected_elapsed_t_cycles,
            })
        );
        assert_eq!(dma.current_transfer(), Some(DmaTransfer::oam(0x12)));

        match transfer_work {
            None => assert_ne!(expected_elapsed_t_cycles, 3),
            Some(work) => {
                assert_eq!(expected_elapsed_t_cycles, 3);
                assert_eq!(work.transfer(), DmaTransfer::oam(0x12));
                assert_eq!(work.byte_index(), 0);
            }
        }
    }

    let restart_work = dma.tick_t_cycle(&mut context);
    assert_eq!(restart_work, None);
    assert_eq!(dma.current_transfer(), Some(DmaTransfer::oam(0x34)));
    assert_eq!(dma.pending_restart, None);
}

#[test]
fn dma_tick_emits_the_first_oam_byte_after_the_first_active_dma_byte_cycle_and_then_every_four_t_cycles()
 {
    let mut dma = DmaController::new(ConsoleModel::GameBoy);
    let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

    dma.write_ff46(0x12);

    for _ in 0..7 {
        assert_eq!(dma.tick_t_cycle(&mut context), None);
    }

    let first_work = dma
        .tick_t_cycle(&mut context)
        .expect("expected first DMA byte after the visible start seam");
    assert_eq!(first_work.transfer(), DmaTransfer::oam(0x12));
    assert_eq!(first_work.byte_index(), 0);
    assert_eq!(first_work.source_address(), 0x1200);
    assert_eq!(first_work.destination_address(), 0xFE00);

    for _ in 0..3 {
        assert_eq!(dma.tick_t_cycle(&mut context), None);
    }

    let second_work = dma
        .tick_t_cycle(&mut context)
        .expect("expected second DMA byte four T-cycles later");
    assert_eq!(second_work.byte_index(), 1);
    assert_eq!(second_work.source_address(), 0x1201);
    assert_eq!(second_work.destination_address(), 0xFE01);
}

#[test]
fn transfer_progress_reports_the_oam_startup_seam_and_tail_without_losing_total_duration() {
    let transfer = DmaTransfer::oam(0x12);
    let warm_up_progress = DmaTransferProgress {
        transfer,
        elapsed_t_cycles: 4,
    };
    let active_no_byte_progress = DmaTransferProgress {
        transfer,
        elapsed_t_cycles: 5,
    };
    let first_byte_progress = DmaTransferProgress {
        transfer,
        elapsed_t_cycles: 8,
    };
    let completed_progress = DmaTransferProgress {
        transfer,
        elapsed_t_cycles: 648,
    };

    assert_eq!(warm_up_progress.first_byte_delay_remaining_t_cycles(), 4);
    assert_eq!(
        warm_up_progress.cpu_bus_restriction_delay_remaining_t_cycles(),
        1
    );
    assert_eq!(warm_up_progress.completed_bytes(), 0);
    assert_eq!(warm_up_progress.byte_phase_t_cycles(), 4);
    assert!(!warm_up_progress.is_cpu_bus_restriction_active());

    assert_eq!(
        active_no_byte_progress.first_byte_delay_remaining_t_cycles(),
        3
    );
    assert_eq!(
        active_no_byte_progress.cpu_bus_restriction_delay_remaining_t_cycles(),
        0
    );
    assert_eq!(active_no_byte_progress.completed_bytes(), 0);
    assert!(active_no_byte_progress.is_cpu_bus_restriction_active());

    assert_eq!(first_byte_progress.first_byte_delay_remaining_t_cycles(), 0);
    assert_eq!(
        first_byte_progress.cpu_bus_restriction_delay_remaining_t_cycles(),
        0
    );
    assert_eq!(first_byte_progress.completed_bytes(), 1);
    assert_eq!(first_byte_progress.remaining_bytes(), 159);
    assert_eq!(first_byte_progress.byte_phase_t_cycles(), 0);
    assert!(first_byte_progress.is_cpu_bus_restriction_active());

    assert_eq!(completed_progress.completed_bytes(), 160);
    assert_eq!(completed_progress.remaining_bytes(), 0);
    assert_eq!(completed_progress.byte_phase_t_cycles(), 0);
}

#[test]
fn startup_state_preserves_idle_dma_while_setting_visible_ff46() {
    let mut dma = DmaController::new(ConsoleModel::GameBoy);

    dma.apply_startup_state(DmaStartupState {
        source_page_latch: 0xFF,
    });

    assert_eq!(dma.read_ff46(), 0xFF);
    assert_eq!(dma.transfer_state(), DmaTransferState::Idle);
    assert_eq!(dma.current_transfer(), None);
}

#[test]
fn dma_transfer_contract_can_model_a_future_hblank_block_transfer_shape() {
    let transfer = DmaTransfer::from_spec(DmaTransferSpec {
        kind: DmaTransferKind::Hdma,
        source_start: 0x1230,
        destination_start: 0x8010,
        total_bytes: 0x40,
        block_size: 0x10,
        family: DmaTransferFamily::BlockWindowed,
        timing: DmaTransferTiming {
            total_t_cycles: 0x40,
            first_byte_delay_t_cycles: 1,
            cpu_bus_restriction_delay_t_cycles: 1,
            t_cycles_per_byte: 1,
        },
        cpu_impact_policy: DmaCpuImpactPolicy::CpuStalledPerBlock,
        memory_region_impact: DmaMemoryRegionImpact::Vram,
        advance_condition: DmaAdvanceCondition::HBlank,
    });
    let progress = DmaTransferProgress {
        transfer,
        elapsed_t_cycles: 0x20,
    };

    assert_eq!(transfer.kind(), DmaTransferKind::Hdma);
    assert_eq!(transfer.family(), DmaTransferFamily::BlockWindowed);
    assert_eq!(transfer.block_size(), 0x10);
    assert_eq!(transfer.total_blocks(), 4);
    assert_eq!(transfer.advance_condition(), DmaAdvanceCondition::HBlank);
    assert_eq!(
        transfer.cpu_impact_policy(),
        DmaCpuImpactPolicy::CpuStalledPerBlock
    );
    assert_eq!(transfer.memory_region_impact(), DmaMemoryRegionImpact::Vram);
    assert_eq!(progress.completed_bytes(), 0x20);
    assert_eq!(progress.completed_blocks(), 2);
    assert_eq!(progress.remaining_blocks(), 2);
}
