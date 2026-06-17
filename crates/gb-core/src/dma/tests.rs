use super::*;

fn vram_dma_block_body_t_cycles(speed_mode: CgbSpeedMode) -> u16 {
    vram_dma_body_t_cycles(VRAM_DMA_BLOCK_BYTES, speed_mode)
}

fn vram_dma_cpu_release_t_cycles() -> u16 {
    VRAM_DMA_CPU_RELEASE_T_CYCLES as u16
}

fn vram_dma_publication_tail_t_cycles() -> u16 {
    (VRAM_DMA_PUBLICATION_T_CYCLES - VRAM_DMA_CPU_RELEASE_T_CYCLES) as u16
}

fn tick_gdma_cpu_release(dma: &mut DmaController, context: &mut CycleContext) {
    for _ in 0..vram_dma_cpu_release_t_cycles() {
        assert_eq!(dma.tick_t_cycle(context), None);
    }
}

fn tick_gdma_publication(dma: &mut DmaController, context: &mut CycleContext) {
    for _ in 0..vram_dma_publication_tail_t_cycles() {
        assert_eq!(dma.tick_t_cycle(context), None);
    }
}

fn tick_hdma_cpu_release(
    dma: &mut DmaController,
    context: &mut CycleContext,
    runtime: VramDmaRuntimeContext,
) {
    for _ in 0..vram_dma_cpu_release_t_cycles() {
        assert_eq!(
            dma.tick_t_cycle_with_vram_dma_context(context, runtime),
            None
        );
    }
}

fn tick_hdma_publication(
    dma: &mut DmaController,
    context: &mut CycleContext,
    runtime: VramDmaRuntimeContext,
) {
    for _ in 0..vram_dma_publication_tail_t_cycles() {
        assert_eq!(
            dma.tick_t_cycle_with_vram_dma_context(context, runtime),
            None
        );
    }
}

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
    assert_eq!(transfer.oam_speed_mode(), CgbSpeedMode::Normal);
    assert_eq!(transfer.lcd_domain_duration_dots(), OAM_DMA_TOTAL_T_CYCLES);
}

#[test]
fn cgb_oam_dma_latches_double_speed_without_changing_the_cpu_m_cycle_duration() {
    let normal_speed = DmaTransfer::oam_for_speed(0x12, CgbSpeedMode::Normal);
    let double_speed = DmaTransfer::oam_for_speed(0x12, CgbSpeedMode::Double);

    assert_eq!(normal_speed.timing(), double_speed.timing());
    assert_eq!(double_speed.oam_speed_mode(), CgbSpeedMode::Double);
    assert_eq!(
        double_speed.timing().total_t_cycles(),
        OAM_DMA_TOTAL_T_CYCLES
    );
    assert_eq!(
        double_speed.timing().first_byte_delay_t_cycles(),
        OAM_DMA_FIRST_BYTE_DELAY_T_CYCLES
    );
    assert_eq!(
        double_speed.timing().cpu_bus_restriction_delay_t_cycles(),
        OAM_DMA_CPU_BUS_RESTRICTION_DELAY_T_CYCLES
    );
    assert_eq!(
        double_speed.timing().t_cycles_per_byte(),
        OAM_DMA_T_CYCLES_PER_BYTE
    );
    assert_eq!(
        normal_speed.lcd_domain_duration_dots(),
        OAM_DMA_TOTAL_T_CYCLES
    );
    assert_eq!(
        double_speed.lcd_domain_duration_dots(),
        OAM_DMA_TOTAL_T_CYCLES.div_ceil(2)
    );
}

#[test]
fn ff46_latches_cgb_oam_dma_speed_profile_only_on_cgb_family_hardware() {
    let mut cgb = DmaController::new(ConsoleModel::GameBoyColor);
    cgb.write_ff46_for_speed(0x12, CgbSpeedMode::Double);
    assert_eq!(
        cgb.current_transfer()
            .expect("CGB OAM DMA should start")
            .oam_speed_mode(),
        CgbSpeedMode::Double
    );

    let mut dmg = DmaController::new(ConsoleModel::GameBoy);
    dmg.write_ff46_for_speed(0x12, CgbSpeedMode::Double);
    assert_eq!(
        dmg.current_transfer()
            .expect("DMG OAM DMA should start")
            .oam_speed_mode(),
        CgbSpeedMode::Normal
    );
}

#[test]
fn oam_dma_source_addresses_above_dfff_follow_the_common_echo_alias_path() {
    for (source_page, first_source, last_source) in [
        (0xE0, 0xC000, 0xC09F),
        (0xFD, 0xDD00, 0xDD9F),
        (0xFE, 0xDE00, 0xDE9F),
        (0xFF, 0xDF00, 0xDF9F),
    ] {
        let transfer = DmaTransfer::oam(source_page);

        assert_eq!(
            transfer.source_address_for_byte(0),
            first_source,
            "source page {source_page:02X} should use the WRAM echo alias for the first OAM DMA byte"
        );
        assert_eq!(
            transfer.source_address_for_byte(0x9F),
            last_source,
            "source page {source_page:02X} should use the WRAM echo alias for the last OAM DMA byte"
        );
    }
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

    dma.write_ff46_for_speed(0xC1, CgbSpeedMode::Double);
    for _ in 0..5 {
        dma.tick_t_cycle(&mut context);
    }

    assert_eq!(
        dma.bus_state(),
        DmaBusState::wram_bus_blocked(Some(DmaMemoryRegionImpact::Oam))
    );
    assert_eq!(
        dma.current_transfer()
            .expect("CGB OAM DMA should remain active")
            .oam_speed_mode(),
        CgbSpeedMode::Double
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
fn cgb_video_source_oam_dma_publishes_video_bus_cpu_policy() {
    let mut dma = DmaController::new(ConsoleModel::GameBoyColor);
    let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

    dma.write_ff46(0x80);
    for _ in 0..5 {
        dma.tick_t_cycle(&mut context);
    }

    assert_eq!(
        dma.bus_state(),
        DmaBusState::video_bus_blocked(Some(DmaMemoryRegionImpact::Oam))
    );

    for _ in 0..3 {
        dma.tick_t_cycle(&mut context);
    }

    assert_eq!(
        dma.bus_state(),
        DmaBusState::video_bus_blocked(Some(DmaMemoryRegionImpact::Oam))
            .with_cpu_conflict_source_address(Some(0x8000))
    );
}

#[test]
fn cgb_edge_source_oam_dma_uses_the_wram_bus_policy_after_source_normalization() {
    for (source_page, first_conflict_source) in [
        (0xE0, 0xC000),
        (0xFD, 0xDD00),
        (0xFE, 0xDE00),
        (0xFF, 0xDF00),
    ] {
        let mut dma = DmaController::new(ConsoleModel::GameBoyColor);
        let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

        dma.write_ff46(source_page);
        for _ in 0..8 {
            dma.tick_t_cycle(&mut context);
        }

        assert_eq!(
            dma.bus_state(),
            DmaBusState::wram_bus_blocked(Some(DmaMemoryRegionImpact::Oam))
                .with_cpu_conflict_source_address(Some(first_conflict_source)),
            "CGB OAM DMA page {source_page:02X} should publish the WRAM-bus policy after the common source alias is applied"
        );
    }
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
fn restarting_active_cgb_oam_dma_preserves_the_pending_restart_speed_profile() {
    let mut dma = DmaController::new(ConsoleModel::GameBoyColor);
    let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

    dma.write_ff46_for_speed(0x12, CgbSpeedMode::Normal);
    for _ in 0..5 {
        dma.tick_t_cycle(&mut context);
    }
    assert_eq!(
        dma.current_transfer()
            .expect("initial transfer should be active")
            .oam_speed_mode(),
        CgbSpeedMode::Normal
    );

    dma.write_ff46_for_speed(0x34, CgbSpeedMode::Double);
    assert_eq!(
        dma.pending_restart
            .expect("restart should be pending")
            .transfer()
            .oam_speed_mode(),
        CgbSpeedMode::Double
    );

    for _ in 0..5 {
        dma.tick_t_cycle(&mut context);
    }

    assert_eq!(
        dma.current_transfer()
            .expect("restarted transfer should take over")
            .oam_speed_mode(),
        CgbSpeedMode::Double
    );
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
fn hdma5_general_dma_start_records_an_active_full_burst_transfer() {
    let mut dma = DmaController::new(ConsoleModel::GameBoyColor);

    dma.write_hdma1(0xA1);
    dma.write_hdma2(0x2B);
    dma.write_hdma3(0x0E);
    dma.write_hdma4(0xF9);
    dma.write_hdma5(0x03);

    let transfer = match dma.vram_dma_state() {
        VramDmaState::GeneralPurposeActive(transfer) => transfer,
        state => panic!("expected active GDMA request, got {state:?}"),
    };
    assert_eq!(transfer.mode(), VramDmaMode::GeneralPurpose);
    assert_eq!(transfer.source_start(), 0xA120);
    assert_eq!(transfer.destination_start(), 0x8EF0);
    assert_eq!(transfer.total_blocks(), 4);
    assert_eq!(transfer.total_bytes(), 0x40);
    assert_eq!(
        dma.current_transfer().map(DmaTransfer::kind),
        Some(DmaTransferKind::Gdma)
    );
    assert!(dma.cpu_stall_active());
    assert_eq!(
        dma.bus_state(),
        DmaBusState::video_bus_blocked(Some(DmaMemoryRegionImpact::Vram))
    );
    assert_eq!(dma.read_hdma5(), 0x03);
}

#[test]
fn hdma5_cancel_stops_active_hblank_dma_and_preserves_hdma1_4_latches() {
    let mut dma = DmaController::new(ConsoleModel::GameBoyColor);

    dma.write_hdma1(0x12);
    dma.write_hdma2(0x34);
    dma.write_hdma3(0x08);
    dma.write_hdma4(0x00);
    dma.write_hdma5(0x84);
    let registers_before_cancel = dma.vram_dma_registers();

    dma.write_hdma5(0x00);

    assert_eq!(
        dma.vram_dma_state(),
        VramDmaState::Inactive {
            hdma5_read_low: 0x00
        }
    );
    assert_eq!(dma.read_hdma5(), 0x80);
    assert_eq!(dma.vram_dma_registers(), registers_before_cancel);
}

#[test]
fn hdma5_cancel_readback_uses_the_cancel_write_low_bits_not_the_remaining_count() {
    let mut dma = DmaController::new(ConsoleModel::GameBoyColor);

    dma.write_hdma1(0x12);
    dma.write_hdma2(0x30);
    dma.write_hdma3(0x08);
    dma.write_hdma4(0x00);
    dma.write_hdma5(0x86);

    dma.write_hdma5(0x15);

    assert_eq!(
        dma.vram_dma_state(),
        VramDmaState::Inactive {
            hdma5_read_low: 0x15
        }
    );
    assert_eq!(dma.read_hdma5(), 0x95);
}

#[test]
fn hdma5_bit7_set_while_hblank_dma_is_active_does_not_restart_the_latched_transfer_yet() {
    let mut dma = DmaController::new(ConsoleModel::GameBoyColor);

    dma.write_hdma1(0x12);
    dma.write_hdma2(0x30);
    dma.write_hdma3(0x08);
    dma.write_hdma4(0x00);
    dma.write_hdma5(0x81);
    let active_state = dma.vram_dma_state();

    dma.write_hdma1(0x44);
    dma.write_hdma2(0x50);
    dma.write_hdma5(0x87);

    assert_eq!(dma.vram_dma_state(), active_state);
    assert_eq!(dma.read_hdma5(), 0x01);
}

#[test]
fn gdma_copies_all_blocks_updates_hdma_latches_and_returns_completed_readback() {
    let mut dma = DmaController::new(ConsoleModel::GameBoyColor);
    let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

    dma.write_hdma1(0xC1);
    dma.write_hdma2(0x20);
    dma.write_hdma3(0x08);
    dma.write_hdma4(0x00);
    dma.write_hdma5(0x01);

    let mut work = Vec::new();
    while !matches!(
        dma.vram_dma_state(),
        VramDmaState::GeneralPurposeComplete(_)
    ) {
        if let Some(transfer_work) = dma.tick_t_cycle(&mut context) {
            work.push((
                transfer_work.source_address(),
                transfer_work.destination_address(),
            ));
        }
    }

    assert_eq!(work.len(), 0x20);
    assert_eq!(work[0], (0xC120, 0x8800));
    assert_eq!(work[0x1F], (0xC13F, 0x881F));
    assert_eq!(dma.vram_dma_registers().source_start(), 0xC140);
    assert_eq!(dma.vram_dma_registers().destination_start(), 0x8820);
    assert_eq!(dma.read_hdma5(), 0xFF);
    assert!(!dma.cpu_stall_active());
}

#[test]
fn gdma_double_speed_keeps_the_vram_dma_body_in_the_lcd_time_domain() {
    let mut dma = DmaController::new(ConsoleModel::GameBoyColor);
    let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

    dma.write_hdma1(0xC1);
    dma.write_hdma2(0x20);
    dma.write_hdma3(0x08);
    dma.write_hdma4(0x00);
    dma.write_hdma5_for_speed(0x00, CgbSpeedMode::Double);

    let transfer = dma
        .current_transfer()
        .expect("GDMA should start immediately");
    assert_eq!(transfer.oam_speed_mode(), CgbSpeedMode::Double);
    assert_eq!(transfer.timing().first_byte_delay_t_cycles(), 4);
    assert_eq!(transfer.timing().t_cycles_per_byte(), 4);
    assert_eq!(
        transfer.timing().total_t_cycles(),
        vram_dma_block_body_t_cycles(CgbSpeedMode::Double) + vram_dma_cpu_release_t_cycles()
    );

    let mut copied = 0;
    for _ in 0..vram_dma_block_body_t_cycles(CgbSpeedMode::Normal) {
        copied += dma.tick_t_cycle(&mut context).is_some() as u16;
    }

    assert_eq!(
        copied, 8,
        "double-speed GDMA must not finish a 16-byte LCD-domain block after only 32 fast T-cycles"
    );
    assert!(dma.cpu_stall_active());
    assert_eq!(dma.read_hdma5(), 0x00);

    for _ in vram_dma_block_body_t_cycles(CgbSpeedMode::Normal)
        ..vram_dma_block_body_t_cycles(CgbSpeedMode::Double)
    {
        copied += dma.tick_t_cycle(&mut context).is_some() as u16;
    }

    assert_eq!(copied, VRAM_DMA_BLOCK_BYTES);
    assert!(dma.cpu_stall_active());
    assert_eq!(dma.read_hdma5(), 0x00);

    tick_gdma_cpu_release(&mut dma, &mut context);
    assert!(!dma.cpu_stall_active());
    assert_eq!(dma.read_hdma5(), 0x00);

    tick_gdma_publication(&mut dma, &mut context);
    assert_eq!(dma.read_hdma5(), 0xFF);
    assert!(!dma.cpu_stall_active());
}

#[test]
fn hdma_starts_one_block_immediately_for_each_visible_hblank_window() {
    let mut dma = DmaController::new(ConsoleModel::GameBoyColor);
    let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

    dma.write_hdma1(0xC1);
    dma.write_hdma2(0x20);
    dma.write_hdma3(0x08);
    dma.write_hdma4(0x00);
    dma.write_hdma5(0x81);

    let hblank0 =
        VramDmaRuntimeContext::new(PpuBusState::lcd_enabled(PpuAccessMode::HBlank), 0, false);
    let hblank1 =
        VramDmaRuntimeContext::new(PpuBusState::lcd_enabled(PpuAccessMode::HBlank), 1, false);

    let mut first_block_work = 0;
    for _ in 0..vram_dma_block_body_t_cycles(CgbSpeedMode::Normal) {
        first_block_work += dma
            .tick_t_cycle_with_vram_dma_context(&mut context, hblank0)
            .is_some() as u16;
    }
    assert_eq!(first_block_work, VRAM_DMA_BLOCK_BYTES);
    assert_eq!(dma.read_hdma5(), 0x01);

    tick_hdma_cpu_release(&mut dma, &mut context, hblank0);
    assert_eq!(dma.read_hdma5(), 0x01);

    tick_hdma_publication(&mut dma, &mut context, hblank0);
    assert_eq!(dma.read_hdma5(), 0x00);
    assert_eq!(dma.vram_dma_registers().source_start(), 0xC130);
    assert_eq!(dma.vram_dma_registers().destination_start(), 0x8810);

    for _ in 0..64 {
        assert_eq!(
            dma.tick_t_cycle_with_vram_dma_context(&mut context, hblank0),
            None,
            "the same HBlank window must not copy a second block"
        );
    }

    let mut second_block_work = 0;
    for _ in 0..vram_dma_block_body_t_cycles(CgbSpeedMode::Normal) {
        second_block_work += dma
            .tick_t_cycle_with_vram_dma_context(&mut context, hblank1)
            .is_some() as u16;
    }
    assert_eq!(second_block_work, VRAM_DMA_BLOCK_BYTES);
    assert_eq!(dma.read_hdma5(), 0x00);

    tick_hdma_cpu_release(&mut dma, &mut context, hblank1);
    assert_eq!(dma.read_hdma5(), 0x00);

    tick_hdma_publication(&mut dma, &mut context, hblank1);
    assert_eq!(
        dma.vram_dma_state(),
        VramDmaState::Inactive {
            hdma5_read_low: HDMA5_TRANSFER_LENGTH_MASK
        }
    );
    assert_eq!(dma.read_hdma5(), 0xFF);
}

#[test]
fn hdma_waits_until_the_visible_hblank_start_seam_is_past() {
    let mut dma = DmaController::new(ConsoleModel::GameBoyColor);
    let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

    dma.write_hdma1(0xC1);
    dma.write_hdma2(0x20);
    dma.write_hdma3(0x08);
    dma.write_hdma4(0x00);
    dma.write_hdma5(0x80);

    let mode0_start_dot = 100;
    let early_hblank = VramDmaRuntimeContext::new_for_speed_at_dot(
        PpuBusState::lcd_enabled(PpuAccessMode::HBlank),
        0,
        mode0_start_dot + 2,
        mode0_start_dot,
        false,
        CgbSpeedMode::Normal,
    );
    assert_eq!(
        dma.tick_t_cycle_with_vram_dma_context(&mut context, early_hblank),
        None
    );
    assert_eq!(dma.current_transfer(), None);
    assert_eq!(dma.read_hdma5(), 0x00);

    let eligible_hblank = VramDmaRuntimeContext::new_for_speed_at_dot(
        PpuBusState::lcd_enabled(PpuAccessMode::HBlank),
        0,
        mode0_start_dot + 3,
        mode0_start_dot,
        false,
        CgbSpeedMode::Normal,
    );
    assert_eq!(
        dma.tick_t_cycle_with_vram_dma_context(&mut context, eligible_hblank),
        None
    );
    assert_eq!(
        dma.current_transfer().map(DmaTransfer::kind),
        Some(DmaTransferKind::Hdma)
    );
    assert!(dma.cpu_stall_active());
}

#[test]
fn hdma_lcd_off_window_transfers_only_one_block_until_a_new_window_appears() {
    let mut dma = DmaController::new(ConsoleModel::GameBoyColor);
    let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

    dma.write_hdma1(0xC1);
    dma.write_hdma2(0x20);
    dma.write_hdma3(0x08);
    dma.write_hdma4(0x00);
    dma.write_hdma5(0x83);

    let lcd_disabled = VramDmaRuntimeContext::new(PpuBusState::lcd_disabled(), 0, false);
    let mut copied = 0;
    for _ in 0..vram_dma_block_body_t_cycles(CgbSpeedMode::Normal) {
        copied += dma
            .tick_t_cycle_with_vram_dma_context(&mut context, lcd_disabled)
            .is_some() as u16;
    }

    assert_eq!(copied, VRAM_DMA_BLOCK_BYTES);
    assert_eq!(dma.read_hdma5(), 0x03);
    tick_hdma_cpu_release(&mut dma, &mut context, lcd_disabled);
    assert_eq!(dma.read_hdma5(), 0x03);

    tick_hdma_publication(&mut dma, &mut context, lcd_disabled);
    assert_eq!(dma.read_hdma5(), 0x02);
    for _ in 0..64 {
        assert_eq!(
            dma.tick_t_cycle_with_vram_dma_context(&mut context, lcd_disabled),
            None
        );
    }
    assert_eq!(dma.read_hdma5(), 0x02);
}

#[test]
fn hdma_block_started_in_hblank_completes_across_the_exit_seam_without_rearming_the_same_line() {
    let mut dma = DmaController::new(ConsoleModel::GameBoyColor);
    let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

    dma.write_hdma1(0xC1);
    dma.write_hdma2(0x20);
    dma.write_hdma3(0x08);
    dma.write_hdma4(0x00);
    dma.write_hdma5(0x81);

    let hblank0 =
        VramDmaRuntimeContext::new(PpuBusState::lcd_enabled(PpuAccessMode::HBlank), 0, false);
    let drawing0 =
        VramDmaRuntimeContext::new(PpuBusState::lcd_enabled(PpuAccessMode::Drawing), 0, false);

    let mut copied = 0;
    copied += dma
        .tick_t_cycle_with_vram_dma_context(&mut context, hblank0)
        .is_some() as u16;
    for _ in 1..vram_dma_block_body_t_cycles(CgbSpeedMode::Normal) {
        copied += dma
            .tick_t_cycle_with_vram_dma_context(&mut context, drawing0)
            .is_some() as u16;
    }

    assert_eq!(copied, VRAM_DMA_BLOCK_BYTES);
    assert_eq!(dma.read_hdma5(), 0x01);
    tick_hdma_cpu_release(&mut dma, &mut context, drawing0);
    assert_eq!(dma.read_hdma5(), 0x01);

    tick_hdma_publication(&mut dma, &mut context, drawing0);
    assert_eq!(dma.read_hdma5(), 0x00);
    assert_eq!(dma.vram_dma_registers().source_start(), 0xC130);
    assert_eq!(dma.vram_dma_registers().destination_start(), 0x8810);

    for _ in 0..64 {
        assert_eq!(
            dma.tick_t_cycle_with_vram_dma_context(&mut context, hblank0),
            None,
            "returning to the same visible HBlank line must not rearm a second seam block"
        );
    }
    assert_eq!(dma.read_hdma5(), 0x00);
}

#[test]
fn hdma_ignores_mode0_like_windows_outside_visible_lines() {
    let mut dma = DmaController::new(ConsoleModel::GameBoyColor);
    let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

    dma.write_hdma1(0xC1);
    dma.write_hdma2(0x20);
    dma.write_hdma3(0x08);
    dma.write_hdma4(0x00);
    dma.write_hdma5(0x80);

    let vblank_line_hblank =
        VramDmaRuntimeContext::new(PpuBusState::lcd_enabled(PpuAccessMode::HBlank), 144, false);
    for _ in 0..64 {
        assert_eq!(
            dma.tick_t_cycle_with_vram_dma_context(&mut context, vblank_line_hblank),
            None
        );
    }

    assert_eq!(dma.read_hdma5(), 0x00);
    assert_eq!(dma.vram_dma_registers().destination_start(), 0x8800);
}

#[test]
fn hdma_does_not_advance_during_vblank_oam_scan_or_drawing() {
    let mut dma = DmaController::new(ConsoleModel::GameBoyColor);
    let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

    dma.write_hdma1(0xC1);
    dma.write_hdma2(0x20);
    dma.write_hdma3(0x08);
    dma.write_hdma4(0x00);
    dma.write_hdma5(0x80);

    for ppu_mode in [
        PpuAccessMode::VBlank,
        PpuAccessMode::OamScan,
        PpuAccessMode::Drawing,
    ] {
        let runtime = VramDmaRuntimeContext::new(PpuBusState::lcd_enabled(ppu_mode), 42, false);
        for _ in 0..64 {
            assert_eq!(
                dma.tick_t_cycle_with_vram_dma_context(&mut context, runtime),
                None
            );
        }
    }

    assert_eq!(dma.read_hdma5(), 0x00);
    assert_eq!(dma.vram_dma_registers().destination_start(), 0x8800);
}

#[test]
fn hdma_block_publishes_cpu_stall_and_video_bus_occupation_until_complete() {
    let mut dma = DmaController::new(ConsoleModel::GameBoyColor);
    let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

    dma.write_hdma1(0xC1);
    dma.write_hdma2(0x20);
    dma.write_hdma3(0x08);
    dma.write_hdma4(0x00);
    dma.write_hdma5(0x80);

    let runtime =
        VramDmaRuntimeContext::new(PpuBusState::lcd_enabled(PpuAccessMode::HBlank), 0, false);
    assert_eq!(
        dma.tick_t_cycle_with_vram_dma_context(&mut context, runtime),
        None
    );
    assert!(dma.cpu_stall_active());
    assert_eq!(
        dma.bus_state(),
        DmaBusState::video_bus_blocked(Some(DmaMemoryRegionImpact::Vram))
    );

    for _ in 1..vram_dma_block_body_t_cycles(CgbSpeedMode::Normal) {
        dma.tick_t_cycle_with_vram_dma_context(&mut context, runtime);
    }

    assert!(dma.cpu_stall_active());
    assert_eq!(
        dma.bus_state(),
        DmaBusState::video_bus_blocked(Some(DmaMemoryRegionImpact::Vram))
    );
    assert_eq!(dma.read_hdma5(), 0x00);

    tick_hdma_cpu_release(&mut dma, &mut context, runtime);
    assert!(!dma.cpu_stall_active());
    assert_eq!(dma.bus_state(), DmaBusState::unrestricted());
    assert_eq!(dma.read_hdma5(), 0x00);

    tick_hdma_publication(&mut dma, &mut context, runtime);
    assert_eq!(dma.read_hdma5(), 0xFF);
}

#[test]
fn hdma_double_speed_latches_the_speed_profile_at_block_start() {
    let mut dma = DmaController::new(ConsoleModel::GameBoyColor);
    let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

    dma.write_hdma1(0xC1);
    dma.write_hdma2(0x20);
    dma.write_hdma3(0x08);
    dma.write_hdma4(0x00);
    dma.write_hdma5(0x80);

    let runtime = VramDmaRuntimeContext::new_for_speed(
        PpuBusState::lcd_enabled(PpuAccessMode::HBlank),
        0,
        false,
        CgbSpeedMode::Double,
    );
    let mut copied = 0;
    for _ in 0..vram_dma_block_body_t_cycles(CgbSpeedMode::Normal) {
        copied += dma
            .tick_t_cycle_with_vram_dma_context(&mut context, runtime)
            .is_some() as u16;
    }

    assert_eq!(
        copied, 8,
        "double-speed HDMA should keep the same LCD-domain body duration as normal-speed HDMA"
    );
    assert!(dma.cpu_stall_active());
    assert_eq!(dma.read_hdma5(), 0x00);

    for _ in vram_dma_block_body_t_cycles(CgbSpeedMode::Normal)
        ..vram_dma_block_body_t_cycles(CgbSpeedMode::Double)
    {
        copied += dma
            .tick_t_cycle_with_vram_dma_context(&mut context, runtime)
            .is_some() as u16;
    }

    assert_eq!(copied, VRAM_DMA_BLOCK_BYTES);
    assert!(dma.cpu_stall_active());
    assert_eq!(dma.read_hdma5(), 0x00);

    tick_hdma_cpu_release(&mut dma, &mut context, runtime);
    assert!(!dma.cpu_stall_active());
    assert_eq!(dma.read_hdma5(), 0x00);

    tick_hdma_publication(&mut dma, &mut context, runtime);
    assert_eq!(dma.read_hdma5(), 0xFF);
    assert!(!dma.cpu_stall_active());
}

#[test]
fn hdma_pauses_while_cpu_is_halted_and_resumes_after_halt_wake() {
    let mut dma = DmaController::new(ConsoleModel::GameBoyColor);
    let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

    dma.write_hdma1(0xC1);
    dma.write_hdma2(0x20);
    dma.write_hdma3(0x08);
    dma.write_hdma4(0x00);
    dma.write_hdma5(0x80);

    let halted_hblank =
        VramDmaRuntimeContext::new(PpuBusState::lcd_enabled(PpuAccessMode::HBlank), 0, true);
    for _ in 0..64 {
        assert_eq!(
            dma.tick_t_cycle_with_vram_dma_context(&mut context, halted_hblank),
            None
        );
    }
    assert_eq!(dma.read_hdma5(), 0x00);

    let running_hblank =
        VramDmaRuntimeContext::new(PpuBusState::lcd_enabled(PpuAccessMode::HBlank), 0, false);
    let mut copied = 0;
    for _ in 0..vram_dma_block_body_t_cycles(CgbSpeedMode::Normal) {
        copied += dma
            .tick_t_cycle_with_vram_dma_context(&mut context, running_hblank)
            .is_some() as u16;
    }
    assert_eq!(copied, VRAM_DMA_BLOCK_BYTES);
    assert_eq!(dma.read_hdma5(), 0x00);
    tick_hdma_cpu_release(&mut dma, &mut context, running_hblank);
    assert!(!dma.cpu_stall_active());
    assert_eq!(dma.read_hdma5(), 0x00);

    tick_hdma_publication(&mut dma, &mut context, running_hblank);
    assert_eq!(dma.read_hdma5(), 0xFF);
}

#[test]
fn vram_dma_destination_overflow_stops_the_transfer_at_the_end_of_vram() {
    let mut dma = DmaController::new(ConsoleModel::GameBoyColor);
    let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

    dma.write_hdma1(0xC1);
    dma.write_hdma2(0x20);
    dma.write_hdma3(0x1F);
    dma.write_hdma4(0xF0);
    dma.write_hdma5(0x03);

    let mut work = Vec::new();
    while !matches!(
        dma.vram_dma_state(),
        VramDmaState::GeneralPurposeComplete(_)
    ) {
        if let Some(transfer_work) = dma.tick_t_cycle(&mut context) {
            work.push(transfer_work.destination_address());
        }
    }

    assert_eq!(work.len(), VRAM_DMA_BLOCK_BYTES as usize);
    assert_eq!(work[0], 0x9FF0);
    assert_eq!(work[15], 0x9FFF);
    assert_eq!(dma.read_hdma5(), 0xFF);
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
    fields.remove("vram_dma_last_served_window");

    let restored: DmaSaveState = serde_json::from_value(serialized)
        .expect("missing additive VRAM-DMA fields should use defaults");

    assert_eq!(restored.vram_dma_registers, VramDmaRegisters::default());
    assert_eq!(restored.vram_dma_state, VramDmaState::default());
    assert_eq!(
        restored.vram_dma_last_served_window,
        VramDmaHBlankWindow::default()
    );
}

#[test]
fn dma_save_state_defaults_missing_oam_speed_mode_for_active_transfer_compatibility() {
    let mut dma = DmaController::new(ConsoleModel::GameBoyColor);
    dma.write_ff46_for_speed(0xC0, CgbSpeedMode::Double);

    let mut serialized = serde_json::to_value(dma.capture_save_state())
        .expect("active DMA save state should serialize to JSON for compatibility checks");
    let transfer = serialized
        .get_mut("transfer_state")
        .and_then(|state| state.get_mut("Starting"))
        .and_then(|progress| progress.get_mut("transfer"))
        .and_then(serde_json::Value::as_object_mut)
        .expect("active DMA transfer should be serialized as a struct");
    transfer.remove("oam_speed_mode");

    let restored_state: DmaSaveState = serde_json::from_value(serialized)
        .expect("missing additive OAM speed field should default to normal speed");
    let mut restored = DmaController::new(ConsoleModel::GameBoyColor);
    restored.restore_save_state(&restored_state);

    assert_eq!(
        restored
            .current_transfer()
            .expect("restored transfer should remain active")
            .oam_speed_mode(),
        CgbSpeedMode::Normal
    );
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
        oam_speed_mode: CgbSpeedMode::Normal,
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
