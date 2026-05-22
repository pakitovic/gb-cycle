use super::*;

#[test]
#[ignore = "diagnostic case1 pre-read cpu-visible stat probe against the real mooneye ROM"]
fn cpu_stat_read_logs_case1_pre_read_state_against_real_rom() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    for _ in 0..10_000_000 {
        let cpu_before = machine.cpu().snapshot();
        if machine.read_bus(0xFF80) == 1
            && cpu_before.registers.pc == 0x0B9C
            && cpu_before.current_opcode == Some(0xF0)
            && matches!(
                cpu_before.execution_state,
                crate::CpuExecutionState::Execute { step: 2, .. }
            )
        {
            let ppu_before = machine.ppu().snapshot();
            let stat_before = machine
                .ppu()
                .read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation);
            machine.step_t_cycle();
            let cpu_after = machine.cpu().snapshot();
            let ppu_after = machine.ppu().snapshot();
            let activity = cpu_after
                .last_bus_activity
                .expect("the next t-cycle should perform the FF41 read");
            println!(
                "case1_pre_read_probe stat_before={:#04X} before_pc={:#06X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} before_x={} before_vpo={} after_value={:#04X} after_pc={:#06X} after_ly={} after_line_dot={} after_mode={:?} after_mode0_start_dot={} after_x={} after_vpo={}",
                stat_before,
                cpu_before.registers.pc,
                ppu_before.ly,
                ppu_before.line_dot,
                ppu_before.mode,
                ppu_before.mode0_start_dot,
                ppu_before.bg_current_transfer_x,
                ppu_before.visible_pixels_output,
                activity.value,
                cpu_after.registers.pc,
                ppu_after.ly,
                ppu_after.line_dot,
                ppu_after.mode,
                ppu_after.mode0_start_dot,
                ppu_after.bg_current_transfer_x,
                ppu_after.visible_pixels_output,
            );
            assert_eq!(activity.address, 0xFF41);
            return;
        }

        machine.step_t_cycle();
    }

    panic!("probe did not reach the testcase 1 pre-read state");
}

#[test]
#[ignore = "diagnostic helper conditions at the real first FF41 read for testcase 1"]
fn cpu_stat_read_logs_case1_first_read_helper_conditions_against_real_rom() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut saw_irq_for_case1 = false;

    for _ in 0..10_000_000 {
        machine.step_t_cycle();

        if machine.read_bus(0xFF80) != 1 {
            continue;
        }

        if !saw_irq_for_case1
            && matches!(
                machine.cpu().execution_state(),
                crate::CpuExecutionState::ServiceInterrupt {
                    source: crate::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq_for_case1 = true;
        }

        let cpu_snapshot = machine.cpu().snapshot();
        if saw_irq_for_case1
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == crate::CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            let ppu = machine.ppu();
            let published_mode = ppu.access_mode_for_line_dot(ppu.line_dot - 1);
            let current_mode = ppu.access_mode_for_line_dot(ppu.line_dot);
            let helper = ppu.terminal_visible_tail_should_publish_hblank_early();
            let current_transfer = ppu.current_transfer();
            let transfer_lane = current_transfer.map(|transfer| transfer.context.lane);
            let transfer_source_window =
                current_transfer.map(|transfer| transfer.context.source_window);
            println!(
                "case1_first_read_helper value={:#04X} pc={:#06X} line_dot={} ly={} published_mode={:?} current_mode={:?} current_mode0_start_dot={} helper={} blank_frame_active={} obj_stage={:?} pending_match_x={:?} pending_hit_len={} transfer_lane={:?} transfer_source_window={:?} current_transfer_x={} visible_pixels_output={} startup_fifo_placeholders={} fifo_len={} line_dot_plus_one_eq_mode0={} ly_visible={} obj_idle={} no_pending_match={} no_pending_hits={}",
                activity.value,
                cpu_snapshot.registers.pc,
                ppu.line_dot,
                ppu.ly,
                published_mode,
                current_mode,
                ppu.current_mode0_start_dot(),
                helper,
                ppu.blank_frame_active,
                ppu.obj_pipeline_state.fetch.stage,
                ppu.obj_pipeline_state.pending_match_x,
                ppu.obj_pipeline_state.pending_sprite_slots.len(),
                transfer_lane,
                transfer_source_window,
                ppu.bg_pipeline_state.current_transfer_x,
                ppu.bg_pipeline_state.visible_pixels_output,
                ppu.bg_pipeline_state.startup_fifo_placeholders,
                ppu.bg_pipeline_state.fifo.len(),
                ppu.line_dot + 1 == ppu.current_mode0_start_dot(),
                ppu.ly < VISIBLE_SCANLINES,
                ppu.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle,
                ppu.obj_pipeline_state.pending_match_x.is_none(),
                ppu.obj_pipeline_state.pending_sprite_slots.is_empty(),
            );
            return;
        }
    }

    panic!("probe did not reach the testcase 1 first FF41 read");
}

#[test]
#[ignore = "diagnostic probe for hacktix strikethrough line 68 DMA/OBJ overlap"]
fn sample_real_hacktix_strikethrough_line68_dma_obj_overlap() {
    for target_ly in 64..=72 {
        let (selected_sprites, events, segment, framebuffer_segment) =
            sample_hacktix_strikethrough_line(target_ly, 64);

        println!("ly={target_ly} selected_sprites={selected_sprites:#?}");
        println!("ly={target_ly} line_pixels_71_79={segment:?}");
        println!("ly={target_ly} framebuffer_71_79={framebuffer_segment:?}");
        for event in &events {
            println!("ly={target_ly} {event:?}");
        }
    }

    let (selected_sprites, events, _, _) = sample_hacktix_strikethrough_line(68, 64);
    assert!(!selected_sprites.is_empty() || !events.is_empty());
}
