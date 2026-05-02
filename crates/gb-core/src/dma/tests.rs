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
fn cgb_external_source_oam_dma_publishes_external_bus_only_cpu_policy() {
    let mut dma = DmaController::new(ConsoleModel::GameBoyColor);
    let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

    dma.write_ff46(0x12);
    for _ in 0..5 {
        dma.tick_t_cycle(&mut context);
    }

    assert_eq!(
        dma.bus_state(),
        DmaBusState::external_bus_only_blocked(Some(DmaMemoryRegionImpact::Oam))
    );

    for _ in 0..3 {
        dma.tick_t_cycle(&mut context);
    }

    assert_eq!(
        dma.bus_state(),
        DmaBusState::external_bus_only_blocked(Some(DmaMemoryRegionImpact::Oam))
            .with_cpu_conflict_source_address(Some(0x1200))
    );
}

#[test]
fn cgb_wram_source_oam_dma_publishes_wram_bus_only_cpu_policy() {
    let mut dma = DmaController::new(ConsoleModel::GameBoyColor);
    let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

    dma.write_ff46(0xC1);
    for _ in 0..5 {
        dma.tick_t_cycle(&mut context);
    }

    assert_eq!(
        dma.bus_state(),
        DmaBusState::wram_bus_blocked(Some(DmaMemoryRegionImpact::Oam))
    );

    for _ in 0..3 {
        dma.tick_t_cycle(&mut context);
    }

    assert_eq!(
        dma.bus_state(),
        DmaBusState::wram_bus_blocked(Some(DmaMemoryRegionImpact::Oam))
            .with_cpu_conflict_source_address(Some(0xC100))
    );
}

#[test]
fn cgb_echo_source_oam_dma_uses_the_wram_bus_policy_after_source_normalization() {
    let mut dma = DmaController::new(ConsoleModel::GameBoyColor);
    let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

    dma.write_ff46(0xFE);
    for _ in 0..8 {
        dma.tick_t_cycle(&mut context);
    }

    assert_eq!(
        dma.bus_state(),
        DmaBusState::wram_bus_blocked(Some(DmaMemoryRegionImpact::Oam))
            .with_cpu_conflict_source_address(Some(0xDE00))
    );
}

#[test]
fn dmg_external_source_oam_dma_keeps_the_legacy_external_bus_policy() {
    let mut dma = DmaController::new(ConsoleModel::GameBoy);
    let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

    dma.write_ff46(0x12);
    for _ in 0..8 {
        dma.tick_t_cycle(&mut context);
    }

    assert_eq!(
        dma.bus_state(),
        DmaBusState::external_bus_blocked(Some(DmaMemoryRegionImpact::Oam))
            .with_cpu_conflict_source_address(Some(0x1200))
    );
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
fn hdma1_4_mask_source_destination_registers_into_vram_dma_endpoints() {
    let mut dma = DmaController::new(ConsoleModel::GameBoyColor);

    dma.write_hdma1(0x12);
    dma.write_hdma2(0x3F);
    dma.write_hdma3(0xE1);
    dma.write_hdma4(0x4F);

    let registers = dma.vram_dma_registers();
    assert_eq!(registers.source_high(), 0x12);
    assert_eq!(registers.source_low(), 0x30);
    assert_eq!(registers.source_start(), 0x1230);
    assert_eq!(registers.destination_high(), 0x01);
    assert_eq!(registers.destination_low(), 0x40);
    assert_eq!(registers.destination_start(), 0x8140);
}

#[test]
fn hdma5_hblank_start_latches_mode_length_and_addresses_without_copying_yet() {
    let mut dma = DmaController::new(ConsoleModel::GameBoyColor);

    dma.write_hdma1(0x12);
    dma.write_hdma2(0x3F);
    dma.write_hdma3(0x9A);
    dma.write_hdma4(0xBC);
    dma.write_hdma5(0x82);

    let transfer = match dma.vram_dma_state() {
        VramDmaState::HBlankActive(transfer) => transfer,
        state => panic!("expected active HBlank VRAM DMA, got {state:?}"),
    };
    assert_eq!(transfer.mode(), VramDmaMode::HBlank);
    assert_eq!(transfer.source_start(), 0x1230);
    assert_eq!(transfer.destination_start(), 0x9AB0);
    assert_eq!(transfer.total_blocks(), 3);
    assert_eq!(transfer.total_bytes(), 0x30);
    assert_eq!(transfer.remaining_blocks(), 3);
    assert_eq!(dma.read_hdma5(), 0x02);

    dma.write_hdma1(0x44);
    dma.write_hdma2(0x55);
    dma.write_hdma3(0x66);
    dma.write_hdma4(0x77);

    assert_eq!(dma.vram_dma_registers().source_start(), 0x4450);
    assert_eq!(dma.vram_dma_registers().destination_start(), 0x8670);
    assert_eq!(
        dma.vram_dma_state(),
        VramDmaState::HBlankActive(transfer),
        "active HDMA must keep the latched endpoints even if HDMA1-4 are rewritten"
    );
}

#[test]
fn hdma5_general_dma_start_records_the_request_as_completed_until_transfer_timing_is_wired() {
    let mut dma = DmaController::new(ConsoleModel::GameBoyColor);

    dma.write_hdma1(0xA1);
    dma.write_hdma2(0x2B);
    dma.write_hdma3(0x0E);
    dma.write_hdma4(0xF9);
    dma.write_hdma5(0x03);

    let transfer = match dma.vram_dma_state() {
        VramDmaState::GeneralPurposeComplete(transfer) => transfer,
        state => panic!("expected completed GDMA request, got {state:?}"),
    };
    assert_eq!(transfer.mode(), VramDmaMode::GeneralPurpose);
    assert_eq!(transfer.source_start(), 0xA120);
    assert_eq!(transfer.destination_start(), 0x8EF0);
    assert_eq!(transfer.total_blocks(), 4);
    assert_eq!(transfer.total_bytes(), 0x40);
    assert_eq!(dma.read_hdma5(), 0xFF);
}

#[test]
fn hdma5_cancel_stops_active_hblank_dma_and_preserves_hdma1_4_latches() {
    let mut dma = DmaController::new(ConsoleModel::GameBoyColor);

    dma.write_hdma1(0x12);
    dma.write_hdma2(0x34);
    dma.write_hdma3(0x1F);
    dma.write_hdma4(0xF0);
    dma.write_hdma5(0x84);
    let registers_before_cancel = dma.vram_dma_registers();

    dma.write_hdma5(0x00);

    assert_eq!(
        dma.vram_dma_state(),
        VramDmaState::Inactive {
            hdma5_read_low: 0x04
        }
    );
    assert_eq!(dma.read_hdma5(), 0x84);
    assert_eq!(dma.vram_dma_registers(), registers_before_cancel);
}

#[test]
fn hdma5_bit7_set_while_hblank_dma_is_active_does_not_restart_the_latched_transfer_yet() {
    let mut dma = DmaController::new(ConsoleModel::GameBoyColor);

    dma.write_hdma1(0x12);
    dma.write_hdma2(0x30);
    dma.write_hdma5(0x81);
    let active_state = dma.vram_dma_state();

    dma.write_hdma1(0x44);
    dma.write_hdma2(0x50);
    dma.write_hdma5(0x87);

    assert_eq!(dma.vram_dma_state(), active_state);
    assert_eq!(dma.read_hdma5(), 0x01);
}

#[test]
fn dma_save_state_defaults_missing_vram_dma_fields_for_same_version_compatibility() {
    let dma = DmaController::new(ConsoleModel::GameBoyColor);
    let mut serialized = serde_json::to_value(dma.capture_save_state())
        .expect("DMA save state should serialize to JSON for compatibility checks");
    let fields = serialized
        .as_object_mut()
        .expect("DMA save state should serialize as a JSON object");
    fields.remove("vram_dma_registers");
    fields.remove("vram_dma_state");

    let restored: DmaSaveState = serde_json::from_value(serialized)
        .expect("missing additive VRAM-DMA fields should use defaults");

    assert_eq!(restored.vram_dma_registers, VramDmaRegisters::default());
    assert_eq!(restored.vram_dma_state, VramDmaState::default());
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
