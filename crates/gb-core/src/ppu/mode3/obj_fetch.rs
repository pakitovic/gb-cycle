use super::*;

impl Ppu {
    pub(in crate::ppu) fn latch_object_fetch_hits(&mut self) {
        if !self.obj_enabled() {
            return;
        }

        let current_owner = self.current_obj_hit_ownership();
        for sprite_slot in 0..self.runtime.mode2_scan_state.selected_sprite_count() {
            if self.runtime.obj_pipeline_state.has_fetched(sprite_slot) {
                continue;
            }

            let Some(sprite) = self.runtime.mode2_scan_state.selected_sprite(sprite_slot) else {
                continue;
            };
            let Some(trigger_x) = sprite_trigger_x(sprite) else {
                continue;
            };

            if trigger_x == current_owner.match_x {
                let mode3_line_start_obj_height =
                    self.runtime.obj_pipeline_state.mode3_line_start_obj_height;
                self.runtime.obj_pipeline_state.queue_fetch_hit(
                    sprite_slot,
                    current_owner,
                    mode3_line_start_obj_height,
                );
            }
        }
    }

    pub(in crate::ppu) fn sync_pending_obj_hit_ownership(&mut self) {
        if !self.obj_enabled() {
            self.runtime.obj_pipeline_state.clear_pending_fetch_hits();
            return;
        }

        let current_owner = self.current_obj_hit_ownership();
        self.runtime
            .obj_pipeline_state
            .clear_pending_fetch_hits_if_stale(current_owner);
    }

    pub(in crate::ppu) fn try_start_object_fetch_from_current_dot(
        &mut self,
        start_source: ObjFetchStartSource,
    ) -> bool {
        if !self
            .current_dot_arbitration()
            .can_start_obj_fetch(start_source)
        {
            return false;
        }

        let Some((sprite_slot, selected_obj_height)) =
            self.runtime.obj_pipeline_state.pop_pending_fetch_hit()
        else {
            return false;
        };
        let Some(sprite) = self.runtime.mode2_scan_state.selected_sprite(sprite_slot) else {
            return false;
        };
        let current_obj_height = self.current_obj_height();

        self.runtime.obj_pipeline_state.start_fetch(
            sprite_slot,
            sprite,
            selected_obj_height,
            current_obj_height,
        );
        let match_transfer_x = self.runtime.bg_pipeline_state.current_transfer_x;
        let match_bg_x = u16::from(match_transfer_x)
            + u16::from(self.runtime.bg_pipeline_state.initial_scx_discard);
        let match_tile = match_bg_x / 8;
        let hidden_lane_fetch = match_transfer_x < 8;
        let match_phase = if hidden_lane_fetch && self.console_model.is_cgb_family() {
            match_transfer_x % 8
        } else {
            (match_bg_x % 8) as u8
        };
        if self.runtime.bg_pipeline_state.obj_alignment_paid_tile != Some(match_tile) {
            self.runtime.bg_pipeline_state.obj_alignment_paid_tile = Some(match_tile);
            self.runtime
                .obj_pipeline_state
                .fetch
                .alignment_stall_remaining = if sprite.x == 0 {
                OBJ_FETCH_MAX_ALIGNMENT_STALL_DOTS
            } else {
                OBJ_FETCH_MAX_ALIGNMENT_STALL_DOTS.saturating_sub(match_phase)
            };
        }
        if matches!(
            start_source,
            ObjFetchStartSource::FifoBackedTransfer | ObjFetchStartSource::PushCachedBgFetch
        ) {
            self.runtime
                .bg_pipeline_state
                .push
                .interrupt_for_object_fetch();
        }
        true
    }

    pub(in crate::ppu) fn current_obj_hit_ownership(&self) -> ObjHitOwnership {
        let phase = self
            .current_transfer()
            .map_or(ObjHitPhase::PreVisible, |transfer| {
                match transfer.context.lane {
                    Mode3TransferLane::PreVisible => ObjHitPhase::PreVisible,
                    Mode3TransferLane::Hidden => ObjHitPhase::Hidden,
                    Mode3TransferLane::Visible => ObjHitPhase::Visible,
                }
            });

        ObjHitOwnership {
            match_x: self.runtime.bg_pipeline_state.current_transfer_x,
            phase,
        }
    }

    pub(in crate::ppu) fn advance_object_fetch(
        &mut self,
        oam: &OamBusView<'_>,
        vram: &VramBusView<'_>,
        dma_oam_conflict: Option<PpuDmaOamConflict>,
    ) -> bool {
        if self.runtime.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle {
            return false;
        }

        if !self.obj_enabled() {
            self.runtime.obj_pipeline_state.fetch.cancelled = true;
        }

        self.runtime.bg_pipeline_state.extend_mode3_by_one_dot();
        if self
            .runtime
            .obj_pipeline_state
            .fetch
            .alignment_stall_remaining
            > 0
        {
            self.runtime
                .obj_pipeline_state
                .fetch
                .alignment_stall_remaining -= 1;
            return true;
        }

        let fetch = self.runtime.obj_pipeline_state.fetch;
        self.advance_object_fetch_stage(fetch, oam, vram, dma_oam_conflict);
        true
    }

    pub(in crate::ppu) fn object_fetch_in_alignment_stall(&self) -> bool {
        self.runtime.obj_pipeline_state.fetch.stage != PpuObjFetcherStage::Idle
            && self
                .runtime
                .obj_pipeline_state
                .fetch
                .alignment_stall_remaining
                > 0
    }

    fn advance_object_fetch_stage(
        &mut self,
        fetch: ObjFetchState,
        oam: &OamBusView<'_>,
        vram: &VramBusView<'_>,
        dma_oam_conflict: Option<PpuDmaOamConflict>,
    ) {
        match (fetch.stage, fetch.stage_dot) {
            (PpuObjFetcherStage::Startup, 0) => self.advance_object_fetch_startup_dot0(),
            (PpuObjFetcherStage::Startup, 1) => {
                self.advance_object_fetch_startup_dot1(fetch, oam, dma_oam_conflict)
            }
            (PpuObjFetcherStage::TileDataLow, 0) => self.advance_object_fetch_tile_data_low_dot0(),
            (PpuObjFetcherStage::TileDataLow, 1) => {
                self.advance_object_fetch_tile_data_low_dot1(fetch, vram)
            }
            (PpuObjFetcherStage::TileDataHigh, 0) => {
                self.advance_object_fetch_tile_data_high_dot0()
            }
            (PpuObjFetcherStage::TileDataHigh, 1) => {
                self.advance_object_fetch_tile_data_high_dot1(fetch, vram)
            }
            (PpuObjFetcherStage::Idle | PpuObjFetcherStage::Push, _) => unreachable!(
                "idle OBJ fetch must have returned before entering the explicit dot automaton"
            ),
            (_, other_dot) => unreachable!(
                "invalid OBJ fetcher stage_dot {other_dot} for stage {:?}",
                fetch.stage
            ),
        }
    }

    fn advance_object_fetch_startup_dot0(&mut self) {
        self.runtime.obj_pipeline_state.fetch.stage_dot = 1;
    }

    fn advance_object_fetch_startup_dot1(
        &mut self,
        fetch: ObjFetchState,
        oam: &OamBusView<'_>,
        dma_oam_conflict: Option<PpuDmaOamConflict>,
    ) {
        let resolved_sprite = fetch
            .sprite
            .map(|sprite| self.resolve_obj_fetch_sprite(oam, sprite, dma_oam_conflict));
        let resolved_tile = resolved_sprite.and_then(|sprite| {
            self.obj_tile_index_and_row_for_mode3_fetch(
                sprite,
                fetch.selected_obj_height,
                fetch.latched_obj_height,
            )
        });
        self.runtime.obj_pipeline_state.fetch.resolved_sprite = resolved_sprite;
        self.runtime.obj_pipeline_state.fetch.resolved_tile_index =
            resolved_tile.map(|(tile_index, _)| tile_index);
        self.runtime.obj_pipeline_state.fetch.resolved_tile_row =
            resolved_tile.map(|(_, tile_row)| tile_row);
        self.runtime.obj_pipeline_state.fetch.stage = PpuObjFetcherStage::TileDataLow;
        self.runtime.obj_pipeline_state.fetch.stage_dot = 0;
    }

    fn advance_object_fetch_tile_data_low_dot0(&mut self) {
        self.runtime.obj_pipeline_state.fetch.stage_dot = 1;
    }

    fn advance_object_fetch_tile_data_low_dot1(
        &mut self,
        fetch: ObjFetchState,
        vram: &VramBusView<'_>,
    ) {
        self.runtime.obj_pipeline_state.fetch.tile_low = fetch
            .resolved_tile_index
            .zip(fetch.resolved_tile_row)
            .zip(fetch.resolved_sprite)
            .map(|((tile_index, tile_row), sprite)| {
                self.read_obj_tile_data_byte_for_resolved_tile(
                    vram, sprite, tile_index, tile_row, 0,
                )
            })
            .unwrap_or(0);
        self.runtime.obj_pipeline_state.fetch.stage = PpuObjFetcherStage::TileDataHigh;
        self.runtime.obj_pipeline_state.fetch.stage_dot = 0;
    }

    fn advance_object_fetch_tile_data_high_dot0(&mut self) {
        self.runtime.obj_pipeline_state.fetch.stage_dot = 1;
    }

    fn advance_object_fetch_tile_data_high_dot1(
        &mut self,
        fetch: ObjFetchState,
        vram: &VramBusView<'_>,
    ) {
        let tile_high = fetch
            .resolved_tile_index
            .zip(fetch.resolved_tile_row)
            .zip(fetch.resolved_sprite)
            .map(|((tile_index, tile_row), sprite)| {
                self.read_obj_tile_data_byte_for_resolved_tile(
                    vram, sprite, tile_index, tile_row, 1,
                )
            })
            .unwrap_or(0);
        self.runtime.obj_pipeline_state.fetch.tile_high = tile_high;
        let resolved_sprite = fetch
            .resolved_sprite
            .expect("active OBJ fetch must keep resolved metadata until FIFO push");
        if !fetch.cancelled && self.obj_enabled() {
            let (tile_low, tile_high) = self.dmg_lcdc2_live_obj_size_push_bytes(
                resolved_sprite,
                fetch.tile_low,
                tile_high,
                vram,
            );
            self.push_obj_pixels(
                resolved_sprite,
                tile_low,
                tile_high,
                self.runtime.bg_pipeline_state.visible_pixels_output,
            );
            self.repaint_observed_startup_obj_prefix_overlap(resolved_sprite, tile_low, tile_high);
        }
        self.runtime
            .obj_pipeline_state
            .mark_fetched(fetch.sprite_slot);
        self.runtime.obj_pipeline_state.fetch = ObjFetchState::default();
        self.runtime
            .bg_pipeline_state
            .push
            .resume_after_object_fetch();
    }

    pub(in crate::ppu) fn resolve_obj_fetch_sprite(
        &mut self,
        oam: &OamBusView<'_>,
        sprite: PpuSelectedSprite,
        dma_oam_conflict: Option<PpuDmaOamConflict>,
    ) -> PpuSelectedSprite {
        let (tile_index, attributes) =
            read_obj_fetch_sprite_metadata(oam, sprite, dma_oam_conflict);
        self.runtime.obj_pipeline_state.late_metadata_word = Some((tile_index, attributes));

        PpuSelectedSprite {
            tile_index,
            attributes,
            ..sprite
        }
    }

    fn repaint_observed_startup_obj_prefix_overlap(
        &mut self,
        sprite: PpuSelectedSprite,
        tile_low: u8,
        tile_high: u8,
    ) {
        self.repaint_observed_obj_scanline_overlap(
            sprite,
            tile_low,
            tile_high,
            self.runtime.bg_pipeline_state.visible_pixels_output,
            true,
        );
    }

    pub(in crate::ppu) fn repaint_observed_obj_scanline_overlap(
        &mut self,
        sprite: PpuSelectedSprite,
        tile_low: u8,
        tile_high: u8,
        overlap_end_visible_x: u8,
        background_only: bool,
    ) {
        if !self.operating_mode.uses_dmg_software_contract()
            || self.runtime.panel.visible_output != PpuVisibleOutputState::Driving
            || overlap_end_visible_x == 0
        {
            return;
        }

        let sprite_screen_x = sprite_screen_x(sprite);
        let overlap_start = sprite_screen_x.max(0) as u8;
        let overlap_end = (sprite_screen_x + BG_TILE_WIDTH as i16)
            .min(i16::from(overlap_end_visible_x))
            .min(SCREEN_WIDTH as i16) as u8;
        if overlap_start >= overlap_end {
            return;
        }

        let bg_enabled = self.pixel_transfer_bg_enabled();
        for visible_x in overlap_start..overlap_end {
            let tile_pixel = i16::from(visible_x) - sprite_screen_x;
            if !(0..BG_TILE_WIDTH as i16).contains(&tile_pixel) {
                continue;
            }

            let candidate = self.obj_pixel_from_sprite(
                sprite,
                obj_tile_pixel_value(tile_low, tile_high, tile_pixel as u8, sprite.attributes),
            );
            if background_only && candidate.is_transparent() {
                continue;
            }

            let visible_x = visible_x as usize;
            if background_only
                && self.runtime.panel.current_scanline_mixed_pixels[visible_x].source
                    != MixedPixelSource::Background
            {
                continue;
            }

            let bg_pixel = self.runtime.panel.current_scanline_bg_pixels[visible_x];
            let cgb_bg_attrs =
                self.runtime.panel.current_scanline_mixed_pixels[visible_x].cgb_bg_attrs;
            let effective_bg_priority_pixel = if bg_enabled { bg_pixel } else { 0 };
            let output_pixel = if candidate.is_transparent() {
                MixedPixel::background_with_cgb_attrs(bg_pixel, cgb_bg_attrs)
            } else {
                self.mix_bg_and_obj(
                    bg_pixel,
                    cgb_bg_attrs,
                    effective_bg_priority_pixel,
                    candidate,
                )
            };
            let dmg_bg_forced_white =
                self.dmg_bg_panel_dot_is_forced_white(bg_enabled, output_pixel);
            let scanline_pixel = if self.runtime.panel.visible_output
                == PpuVisibleOutputState::Driving
                && !dmg_bg_forced_white
            {
                output_pixel.color
            } else {
                0
            };
            let panel_pixel = if dmg_bg_forced_white {
                0
            } else {
                self.map_mixed_pixel_to_panel_shade(output_pixel)
            };

            self.runtime.panel.current_scanline_mixed_pixels[visible_x] = output_pixel;
            self.runtime.panel.current_scanline_dmg_bg_forced_white[visible_x] =
                dmg_bg_forced_white;
            self.runtime.panel.current_scanline_pixels[visible_x] = scanline_pixel;
            self.write_framebuffer_pixel(
                self.ly as usize * SCREEN_WIDTH,
                visible_x,
                output_pixel,
                panel_pixel,
            );

            for dot in &mut self
                .runtime
                .panel
                .dmg_panel_live_write_state
                .recent_panel_dots
            {
                if usize::from(dot.visible_x) == visible_x {
                    dot.pixel = output_pixel;
                    dot.dmg_bg_forced_white = dmg_bg_forced_white;
                }
            }
        }
    }

    pub(in crate::ppu) fn fetched_same_x_obj_sprite_count_for_active_fetch(&self) -> usize {
        let Some(sprite) = self.runtime.obj_pipeline_state.fetch.sprite else {
            return 0;
        };
        let Some(trigger_x) = sprite_trigger_x(sprite) else {
            return 0;
        };

        self.fetched_same_x_obj_sprite_count_for_trigger_x(trigger_x)
    }

    pub(in crate::ppu) fn fetched_same_x_obj_sprite_count_for_pending_match_x(&self) -> usize {
        let Some(trigger_x) = self.runtime.obj_pipeline_state.pending_match_x else {
            return 0;
        };

        self.fetched_same_x_obj_sprite_count_for_trigger_x(trigger_x)
    }

    pub(in crate::ppu) fn fetched_same_x_obj_sprite_count_for_trigger_x(
        &self,
        trigger_x: u8,
    ) -> usize {
        let mut fetched_same_x_count = 0_usize;
        for sprite_slot in 0..self.runtime.mode2_scan_state.selected_sprite_count() {
            if !self.runtime.obj_pipeline_state.has_fetched(sprite_slot) {
                continue;
            }
            let Some(selected_sprite) = self.runtime.mode2_scan_state.selected_sprite(sprite_slot)
            else {
                continue;
            };
            if sprite_trigger_x(selected_sprite) == Some(trigger_x) {
                fetched_same_x_count += 1;
            }
        }
        fetched_same_x_count
    }
}
