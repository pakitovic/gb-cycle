use super::*;

pub(super) const TEST_VRAM_BYTES: usize = 0x2000;
pub(super) const DMG_BOOT_LOGO_TILE_VRAM_START: u16 = 0x8010;
pub(super) const DMG_BOOT_LOGO_MAP_VRAM_START: u16 = 0x9904;
pub(super) const DMG_BOOT_LOGO_TILE_BYTES: [u8; 200] = [
    0xF0, 0xF0, 0xFC, 0xFC, 0xFC, 0xFC, 0xF3, 0xF3, 0x3C, 0x3C, 0x3C, 0x3C, 0x3C, 0x3C, 0x3C, 0x3C,
    0xF0, 0xF0, 0xF0, 0xF0, 0x00, 0x00, 0xF3, 0xF3, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xCF, 0xCF,
    0x00, 0x00, 0x0F, 0x0F, 0x3F, 0x3F, 0x0F, 0x0F, 0x00, 0x00, 0x00, 0x00, 0xC0, 0xC0, 0x0F, 0x0F,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xF0, 0xF0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xF3, 0xF3,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0xC0, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0xFF, 0xFF,
    0xC0, 0xC0, 0xC0, 0xC0, 0xC0, 0xC0, 0xC3, 0xC3, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFC, 0xFC,
    0xF3, 0xF3, 0xF0, 0xF0, 0xF0, 0xF0, 0xF0, 0xF0, 0x3C, 0x3C, 0xFC, 0xFC, 0xFC, 0xFC, 0x3C, 0x3C,
    0xF3, 0xF3, 0xF3, 0xF3, 0xF3, 0xF3, 0xF3, 0xF3, 0xF3, 0xF3, 0xC3, 0xC3, 0xC3, 0xC3, 0xC3, 0xC3,
    0xCF, 0xCF, 0xCF, 0xCF, 0xCF, 0xCF, 0xCF, 0xCF, 0x3C, 0x3C, 0x3F, 0x3F, 0x3C, 0x3C, 0x0F, 0x0F,
    0x3C, 0x3C, 0xFC, 0xFC, 0x00, 0x00, 0xFC, 0xFC, 0xFC, 0xFC, 0xF0, 0xF0, 0xF0, 0xF0, 0xF0, 0xF0,
    0xF3, 0xF3, 0xF3, 0xF3, 0xF3, 0xF3, 0xF0, 0xF0, 0xC3, 0xC3, 0xC3, 0xC3, 0xC3, 0xC3, 0xFF, 0xFF,
    0xCF, 0xCF, 0xCF, 0xCF, 0xCF, 0xCF, 0xC3, 0xC3, 0x0F, 0x0F, 0x0F, 0x0F, 0x0F, 0x0F, 0xFC, 0xFC,
    0x3C, 0x42, 0xB9, 0xA5, 0xB9, 0xA5, 0x42, 0x3C,
];
pub(super) const DMG_BOOT_LOGO_MAP_BYTES: [u8; 44] = [
    0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x19, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18,
];

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct HacktixStrikethroughLine68Observation {
    t_cycle: u64,
    line_dot: u16,
    visible_pixels_output: u8,
    mode0_start_dot: u16,
    current_transfer_x: u8,
    current_transfer_lane: Option<PpuMode3TransferLaneSnapshot>,
    obj_fetcher_stage: PpuObjFetcherStage,
    obj_fetcher_stage_dot: u8,
    fetch_sprite_slot: u8,
    fetch_sprite_oam_index: Option<u8>,
    fetch_sprite_x: Option<u8>,
    resolved_tile_index: Option<u8>,
    resolved_attributes: Option<u8>,
    late_metadata_word: Option<(u8, u8)>,
    dma_byte_destination_address: Option<u16>,
}

pub(super) struct PpuTestRig {
    pub(super) ppu: Ppu,
    pub(super) oam_bytes: [u8; 160],
    pub(super) vram_bytes: [u8; TEST_VRAM_BYTES],
    pub(super) dma_oam_active: bool,
    pub(super) dma_oam_conflict: Option<PpuDmaOamConflict>,
    pub(super) t_cycle: u64,
}

impl PpuTestRig {
    pub(super) fn with_model(model: ConsoleModel) -> Self {
        Self {
            ppu: Ppu::new(model),
            oam_bytes: [0; 160],
            vram_bytes: [0; TEST_VRAM_BYTES],
            dma_oam_active: false,
            dma_oam_conflict: None,
            t_cycle: 0,
        }
    }

    pub(super) fn dmg() -> Self {
        Self::with_model(ConsoleModel::GameBoy)
    }

    pub(super) fn with_startup_state(mut self, startup_state: PpuStartupState) -> Self {
        self.ppu.apply_startup_state(startup_state);
        self
    }

    pub(super) fn with_oam(mut self, oam_bytes: [u8; 160]) -> Self {
        self.oam_bytes = oam_bytes;
        self
    }

    pub(super) fn with_vram(mut self, vram_bytes: [u8; TEST_VRAM_BYTES]) -> Self {
        self.vram_bytes = vram_bytes;
        self
    }

    pub(super) fn with_dma_active(mut self, dma_oam_active: bool) -> Self {
        self.dma_oam_active = dma_oam_active;
        self
    }

    pub(super) fn with_dma_conflict(mut self, dma_oam_conflict: Option<PpuDmaOamConflict>) -> Self {
        self.dma_oam_conflict = dma_oam_conflict;
        self
    }

    pub(super) fn set_dma_active(&mut self, dma_oam_active: bool) {
        self.dma_oam_active = dma_oam_active;
    }

    pub(super) fn tick(&mut self) -> CycleContext {
        let context = tick_ppu_with_vram_and_dma(
            &mut self.ppu,
            self.t_cycle,
            &self.oam_bytes,
            &self.vram_bytes,
            self.dma_oam_active,
            self.dma_oam_conflict,
        );
        self.t_cycle += 1;
        context
    }

    pub(super) fn tick_n(&mut self, count: u64) {
        for _ in 0..count {
            self.tick();
        }
    }

    pub(super) fn advance_until_hblank(&mut self) {
        let start_t_cycle = self.t_cycle;
        while self.snapshot().mode != PpuAccessMode::HBlank {
            self.tick();
            assert!(self.t_cycle - start_t_cycle < 2 * DOTS_PER_SCANLINE as u64);
        }
    }

    pub(super) fn advance_until_line_start(&mut self, target_ly: u8) {
        let start_t_cycle = self.t_cycle;
        while !(self.snapshot().ly == target_ly && self.snapshot().line_dot == 0) {
            self.tick();
            assert!(self.t_cycle - start_t_cycle < 20 * DOTS_PER_SCANLINE as u64);
        }
    }

    pub(super) fn advance_until_next_frame_start(&mut self) {
        let start_t_cycle = self.t_cycle;
        while !(self.t_cycle > 0 && self.snapshot().ly == 0 && self.snapshot().line_dot == 0) {
            self.tick();
            assert!(
                self.t_cycle - start_t_cycle
                    < 2 * TOTAL_SCANLINES as u64 * DOTS_PER_SCANLINE as u64
            );
        }
    }

    pub(super) fn advance_until_tile_sel_replay_position(
        &mut self,
        target_ly: u8,
        target_line_dot: u16,
    ) {
        let start_t_cycle = self.t_cycle;
        while !(self.snapshot().ly == target_ly && self.snapshot().line_dot == target_line_dot) {
            apply_tile_sel_line_write_replay(&mut self.ppu);
            self.tick();
            assert!(
                self.t_cycle - start_t_cycle
                    < 40 * TOTAL_SCANLINES as u64 * DOTS_PER_SCANLINE as u64
            );
        }
    }

    pub(super) fn write_oam_entry(&mut self, index: u8, y: u8, x: u8, tile_index: u8) {
        write_oam_entry(&mut self.oam_bytes, index, y, x, tile_index);
    }

    pub(super) fn write_oam_entry_with_attributes(
        &mut self,
        index: u8,
        y: u8,
        x: u8,
        tile_index: u8,
        attributes: u8,
    ) {
        write_oam_entry_with_attributes(&mut self.oam_bytes, index, y, x, tile_index, attributes);
    }

    pub(super) fn write_bg_tile_row(&mut self, tile_index: u8, row: u8, low: u8, high: u8) {
        write_bg_tile_row(&mut self.vram_bytes, tile_index, row, low, high);
    }

    pub(super) fn write_bg_tilemap_entry(&mut self, x: u8, y: u8, tile_index: u8) {
        write_bg_tilemap_entry(&mut self.vram_bytes, x, y, tile_index);
    }

    pub(super) fn write_window_tilemap_entry(&mut self, x: u8, y: u8, tile_index: u8) {
        write_window_tilemap_entry(&mut self.vram_bytes, x, y, tile_index);
    }

    pub(super) fn with_ppu_vram<R>(
        &mut self,
        f: impl FnOnce(&mut Self, &VramBusView<'_>) -> R,
    ) -> R {
        let mut vram = crate::bus::VramDomain::from_bytes(&self.vram_bytes);
        vram.set_acquired(BusMaster::Ppu, true);
        f(self, &VramBusView::new(BusMaster::Ppu, &mut vram))
    }

    pub(super) fn with_ppu_video_buses<R>(
        &mut self,
        f: impl FnOnce(&mut Self, &OamBusView<'_>, &VramBusView<'_>) -> R,
    ) -> R {
        let mut oam = crate::bus::OamDomain::from_bytes(&self.oam_bytes);
        let mut vram = crate::bus::VramDomain::from_bytes(&self.vram_bytes);
        oam.set_acquired(BusMaster::Ppu, true);
        vram.set_acquired(BusMaster::Ppu, true);
        let oam = OamBusView::new(BusMaster::Ppu, &mut oam);
        let vram = VramBusView::new(BusMaster::Ppu, &mut vram);
        f(self, &oam, &vram)
    }

    pub(super) fn maybe_recompute_pending_background_fill_with_ppu_vram(&mut self) {
        self.with_ppu_vram(|ppu, vram| ppu.maybe_recompute_pending_background_fill(vram));
    }

    pub(super) fn advance_mode3_output_phase_with_ppu_vram(&mut self) -> Mode3TransferDot {
        self.with_ppu_vram(|ppu, vram| ppu.advance_mode3_output_phase_with_vram(vram))
    }

    pub(super) fn advance_bg_fetcher_with_ppu_vram(&mut self) -> bool {
        self.with_ppu_vram(|ppu, vram| ppu.advance_bg_fetcher(vram))
    }

    pub(super) fn advance_object_fetch_with_ppu_video(
        &mut self,
        dma_oam_conflict: Option<PpuDmaOamConflict>,
    ) -> bool {
        self.with_ppu_video_buses(|ppu, oam, vram| {
            ppu.advance_object_fetch(oam, vram, dma_oam_conflict)
        })
    }

    pub(super) fn advance_mode3_object_phase_with_ppu_video(
        &mut self,
        dma_oam_conflict: Option<PpuDmaOamConflict>,
    ) -> bool {
        self.with_ppu_video_buses(|ppu, oam, vram| {
            ppu.advance_mode3_object_phase(oam, vram, dma_oam_conflict)
        })
    }
}

impl std::ops::Deref for PpuTestRig {
    type Target = Ppu;

    fn deref(&self) -> &Self::Target {
        &self.ppu
    }
}

impl std::ops::DerefMut for PpuTestRig {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ppu
    }
}

pub(super) fn sync_test_video_ownership(
    ppu: &Ppu,
    oam: &mut crate::bus::OamDomain,
    vram: &mut crate::bus::VramDomain,
    dma_oam_active: bool,
) {
    let bus_state = ppu.owner_bus_state();
    let ppu_vram = bus_state.is_lcd_enabled() && bus_state.mode() == PpuAccessMode::Drawing;
    let ppu_oam = bus_state.is_lcd_enabled()
        && matches!(
            bus_state.mode(),
            PpuAccessMode::OamScan | PpuAccessMode::Drawing
        );

    oam.set_acquired(BusMaster::Ppu, ppu_oam);
    vram.set_acquired(BusMaster::Ppu, ppu_vram);
    oam.set_acquired(BusMaster::Dma, dma_oam_active);
    vram.set_acquired(BusMaster::Dma, false);
}

pub(super) fn tick_ppu_with_vram_and_dma(
    ppu: &mut Ppu,
    t_cycle: u64,
    oam_bytes: &[u8],
    vram_bytes: &[u8],
    dma_oam_active: bool,
    dma_oam_conflict: Option<PpuDmaOamConflict>,
) -> CycleContext {
    let mut context = CycleContext::for_cycle(TCycle::new(t_cycle));
    let mut oam = crate::bus::OamDomain::from_bytes(oam_bytes);
    let mut vram = crate::bus::VramDomain::from_bytes(vram_bytes);
    sync_test_video_ownership(ppu, &mut oam, &mut vram, dma_oam_active);
    ppu.tick_t_cycle(
        &mut context,
        OamBusView::new(BusMaster::Ppu, &mut oam),
        VramBusView::new(BusMaster::Ppu, &mut vram),
        dma_oam_active,
        dma_oam_conflict,
    );
    context
}

pub(super) fn drain_ppu_interrupts(ppu: &mut Ppu) -> Vec<InterruptSource> {
    let mut requests = Vec::with_capacity(2);
    let mask = ppu.take_pending_interrupt_request_mask();
    if mask & 0x01 != 0 {
        requests.push(InterruptSource::VBlank);
    }
    if mask & 0x02 != 0 {
        requests.push(InterruptSource::LcdStat);
    }
    requests
}

pub(super) fn seed_hacktix_dmg_boot_logo_vram(machine: &mut Machine<TraceSummaryBuffer>) {
    for (index, byte) in DMG_BOOT_LOGO_TILE_BYTES.iter().copied().enumerate() {
        machine.write_bus(DMG_BOOT_LOGO_TILE_VRAM_START + (index as u16 * 2), byte);
    }
    for (index, byte) in DMG_BOOT_LOGO_MAP_BYTES.iter().copied().enumerate() {
        machine.write_bus(DMG_BOOT_LOGO_MAP_VRAM_START + index as u16, byte);
    }
}

pub(super) fn load_hacktix_strikethrough_machine() -> Machine<TraceSummaryBuffer> {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../test/hacktix/strikethrough.gb");
    let rom = std::fs::read(&rom_path).expect("hacktix strikethrough ROM should be present");
    let mut machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(rom)
        .expect("hacktix ROM should load");
    seed_hacktix_dmg_boot_logo_vram(&mut machine);
    machine
}

pub(super) fn sample_hacktix_strikethrough_line(
    target_ly: u8,
    max_events: usize,
) -> (
    Vec<PpuSelectedSprite>,
    Vec<HacktixStrikethroughLine68Observation>,
    [u8; 8],
    [u8; 8],
) {
    let mut machine = load_hacktix_strikethrough_machine();
    let mut current_selected_sprites = Vec::new();
    let mut current_events = Vec::with_capacity(max_events);
    let mut last_completed_line68 = None;

    for _ in 0..3_000_000 {
        machine.step_t_cycle();

        let ppu = machine.ppu();
        if ppu.ly != target_ly {
            if machine.cpu().execution_state() == crate::CpuExecutionState::Halted
                && let Some(line68) = last_completed_line68
            {
                return line68;
            }
            continue;
        }

        if ppu.line_dot == MODE2_DOTS {
            current_selected_sprites = ppu.mode2_scan_state.selected_sprites_snapshot();
            current_events.clear();
        }

        let dma_progress = machine.dma().transfer_progress();
        let dma_byte_destination_address = dma_progress
            .filter(|progress| {
                progress.completed_bytes() > 0 && progress.byte_phase_t_cycles() == 0
            })
            .map(|progress| {
                progress
                    .transfer()
                    .destination_address_for_byte(progress.completed_bytes() - 1)
            });

        if (ppu.obj_pipeline_state.fetch.stage != PpuObjFetcherStage::Idle
            || dma_byte_destination_address.is_some())
            && current_events.len() < max_events
        {
            current_events.push(HacktixStrikethroughLine68Observation {
                t_cycle: machine.next_t_cycle().get().saturating_sub(1),
                line_dot: ppu.line_dot,
                visible_pixels_output: ppu.bg_pipeline_state.visible_pixels_output,
                mode0_start_dot: ppu.current_mode0_start_dot(),
                current_transfer_x: ppu.bg_pipeline_state.current_transfer_x,
                current_transfer_lane: ppu
                    .current_transfer()
                    .map(|transfer| snapshot_bg_transfer_lane(transfer.context.lane)),
                obj_fetcher_stage: ppu.obj_pipeline_state.fetch.stage,
                obj_fetcher_stage_dot: ppu.obj_pipeline_state.fetch.stage_dot,
                fetch_sprite_slot: ppu.obj_pipeline_state.fetch.sprite_slot,
                fetch_sprite_oam_index: ppu
                    .obj_pipeline_state
                    .fetch
                    .sprite
                    .map(|sprite| sprite.oam_index),
                fetch_sprite_x: ppu.obj_pipeline_state.fetch.sprite.map(|sprite| sprite.x),
                resolved_tile_index: ppu
                    .obj_pipeline_state
                    .fetch
                    .resolved_sprite
                    .map(|sprite| sprite.tile_index),
                resolved_attributes: ppu
                    .obj_pipeline_state
                    .fetch
                    .resolved_sprite
                    .map(|sprite| sprite.attributes),
                late_metadata_word: ppu.obj_pipeline_state.late_metadata_word,
                dma_byte_destination_address,
            });
        }

        if ppu.ly == target_ly && ppu.current_access_mode() == PpuAccessMode::HBlank {
            let mut segment = [0_u8; 8];
            segment.copy_from_slice(&ppu.current_scanline_pixels[71..79]);
            let mut framebuffer_segment = [0_u8; 8];
            let framebuffer_start = target_ly as usize * SCREEN_WIDTH + 71;
            framebuffer_segment
                .copy_from_slice(&ppu.framebuffer[framebuffer_start..framebuffer_start + 8]);
            last_completed_line68 = Some((
                current_selected_sprites.clone(),
                current_events.clone(),
                segment,
                framebuffer_segment,
            ));
        }
    }

    if let Some(line68) = last_completed_line68 {
        return line68;
    }

    panic!(
        "hacktix strikethrough line sample did not reach the halted framebuffer; target_ly={} pc={:#06X} state={:?} ly={} line_dot={} mode={:?}",
        target_ly,
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().ly,
        machine.ppu().line_dot,
        machine.ppu().current_access_mode()
    );
}

pub(super) fn write_oam_entry(oam_bytes: &mut [u8; 160], index: u8, y: u8, x: u8, tile_index: u8) {
    write_oam_entry_with_attributes(oam_bytes, index, y, x, tile_index, 0);
}

pub(super) fn write_oam_entry_with_attributes(
    oam_bytes: &mut [u8; 160],
    index: u8,
    y: u8,
    x: u8,
    tile_index: u8,
    attributes: u8,
) {
    let entry_start = index as usize * OAM_ENTRY_BYTES;
    oam_bytes[entry_start] = y;
    oam_bytes[entry_start + 1] = x;
    oam_bytes[entry_start + 2] = tile_index;
    oam_bytes[entry_start + 3] = attributes;
}

pub(super) fn write_oam_corruption_row(oam_bytes: &mut [u8; 160], row: u8, words: [u16; 4]) {
    for (word_index, value) in words.into_iter().enumerate() {
        write_oam_word(oam_bytes, row, word_index, value);
    }
}

pub(super) fn write_bg_tile_row(
    vram_bytes: &mut [u8; TEST_VRAM_BYTES],
    tile_index: u8,
    row: u8,
    low: u8,
    high: u8,
) {
    let tile_address =
        tile_index as usize * TILE_BYTES as usize + row as usize * TILE_ROW_BYTES as usize;
    vram_bytes[tile_address] = low;
    vram_bytes[tile_address + 1] = high;
}

pub(super) fn write_bg_tilemap_entry(
    vram_bytes: &mut [u8; TEST_VRAM_BYTES],
    x: u8,
    y: u8,
    tile_index: u8,
) {
    let tile_map_address = 0x1800 + y as usize * BG_TILE_MAP_WIDTH as usize + x as usize;
    vram_bytes[tile_map_address] = tile_index;
}

pub(super) fn write_window_tilemap_entry(
    vram_bytes: &mut [u8; TEST_VRAM_BYTES],
    x: u8,
    y: u8,
    tile_index: u8,
) {
    let tile_map_address = 0x1C00 + y as usize * BG_TILE_MAP_WIDTH as usize + x as usize;
    vram_bytes[tile_map_address] = tile_index;
}

pub(super) fn apply_tile_sel_line_write_replay(ppu: &mut Ppu) {
    let snapshot = ppu.snapshot();
    if snapshot.mode != PpuAccessMode::Drawing {
        return;
    }

    if snapshot.line_dot == 104 {
        ppu.write_register(0xFF40, 0x93);
    } else if snapshot.line_dot == 112 {
        ppu.write_register(0xFF40, 0x83);
    }
}
