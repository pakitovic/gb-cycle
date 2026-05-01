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
        overlap_current_dot: bool,
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
        let pending_nonterminal_same_x_cluster_pays_startup_dot =
            self.pending_nonterminal_same_x_cluster_pays_startup_dot();
        let overlap_current_dot =
            overlap_current_dot && !pending_nonterminal_same_x_cluster_pays_startup_dot;
        if overlap_current_dot {
            if matches!(
                start_source,
                ObjFetchStartSource::FifoBackedTransfer | ObjFetchStartSource::PushCachedBgFetch
            ) {
                self.runtime
                    .bg_pipeline_state
                    .push
                    .interrupt_for_object_fetch();
            }
            self.runtime.obj_pipeline_state.fetch.stage_dot = 1;
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

    pub(in crate::ppu) fn bg_fetcher_ready_for_fifo_backed_obj_start(&self) -> bool {
        let allow_same_x_cluster_tileindex_overlap = self
            .current_transfer_x_supports_early_same_x_obj_start()
            && self.runtime.obj_pipeline_state.pending_sprite_slots.len() >= 2
            && self.runtime.obj_pipeline_state.pending_match_x
                == Some(self.runtime.bg_pipeline_state.current_transfer_x);
        if self.runtime.bg_pipeline_state.current_transfer_x < 8 {
            allow_same_x_cluster_tileindex_overlap
                || !matches!(
                    self.runtime.bg_pipeline_state.fetcher.stage,
                    PpuBgFetcherStage::TileIndex
                )
        } else {
            !matches!(
                self.runtime.bg_pipeline_state.fetcher.stage,
                PpuBgFetcherStage::TileIndex | PpuBgFetcherStage::TileDataLow
            )
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

        if self.runtime.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Startup
            && !self.obj_fetch_startup_ready()
        {
            return false;
        }

        if !self.obj_enabled() {
            self.runtime.obj_pipeline_state.fetch.cancelled = true;
        }

        let fetch = self.runtime.obj_pipeline_state.fetch;
        self.extend_mode3_for_object_fetch_if_needed(fetch, dma_oam_conflict);
        self.advance_object_fetch_stage(fetch, oam, vram, dma_oam_conflict);
        true
    }

    fn extend_mode3_for_object_fetch_if_needed(
        &mut self,
        fetch: ObjFetchState,
        dma_oam_conflict: Option<PpuDmaOamConflict>,
    ) {
        if self.object_fetch_startup_dot_is_shared(fetch)
            || self.object_fetch_push_dot_is_shared(fetch, dma_oam_conflict)
        {
            return;
        }

        self.runtime.bg_pipeline_state.extend_mode3_by_one_dot();
    }

    fn object_fetch_startup_dot_is_shared(&self, fetch: ObjFetchState) -> bool {
        matches!(
            (fetch.stage, fetch.stage_dot),
            (PpuObjFetcherStage::Startup, 1)
        ) && !fetch.count_terminal_push_dot
    }

    fn object_fetch_push_dot_is_shared(
        &self,
        fetch: ObjFetchState,
        dma_oam_conflict: Option<PpuDmaOamConflict>,
    ) -> bool {
        let hidden_left_edge_same_x_chain_pays_push_dot =
            self.hidden_left_edge_same_x_chain_pays_push_dot();
        let visible_left_edge_same_x_chain_shares_push_dot =
            dma_oam_conflict.is_none() && self.visible_left_edge_same_x_chain_shares_push_dot();
        let terminal_right_edge_same_x_chain_shares_push_dot =
            self.terminal_right_edge_same_x_chain_shares_push_dot();

        matches!(
            (fetch.stage, fetch.stage_dot),
            (PpuObjFetcherStage::Push, 1)
        ) && !fetch.count_terminal_push_dot
            && (self.runtime.bg_pipeline_state.current_transfer_x < 8
                || visible_left_edge_same_x_chain_shares_push_dot
                || terminal_right_edge_same_x_chain_shares_push_dot)
            && !hidden_left_edge_same_x_chain_pays_push_dot
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
            (PpuObjFetcherStage::Push, 0) => self.advance_object_fetch_push_dot0(),
            (PpuObjFetcherStage::Push, 1) => {
                self.advance_object_fetch_push_dot1(fetch, oam, vram, dma_oam_conflict)
            }
            (PpuObjFetcherStage::Idle, _) => unreachable!(
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
        let first_hidden_same_x_cluster_fetch_skips_obj_tile_data_low_byte =
            self.first_hidden_same_x_cluster_fetch_skips_obj_tile_data_low_byte();
        let terminal_right_edge_same_x_chain_skips_to_tile_data_high_half_step =
            self.terminal_right_edge_same_x_chain_skips_to_tile_data_high_half_step();
        let first_fast_same_x_cluster_fetch_skips_first_tile_data_low_half_step =
            !first_hidden_same_x_cluster_fetch_skips_obj_tile_data_low_byte
                && !terminal_right_edge_same_x_chain_skips_to_tile_data_high_half_step
                && self.initial_nonterminal_same_x_cluster_skips_first_low_half_step()
                && !self
                    .runtime
                    .obj_pipeline_state
                    .pending_sprite_slots
                    .is_empty()
                && self.fetched_same_x_obj_sprite_count_for_active_fetch() == 0;
        self.runtime.obj_pipeline_state.fetch.resolved_sprite = resolved_sprite;
        self.runtime.obj_pipeline_state.fetch.resolved_tile_index =
            resolved_tile.map(|(tile_index, _)| tile_index);
        self.runtime.obj_pipeline_state.fetch.resolved_tile_row =
            resolved_tile.map(|(_, tile_row)| tile_row);
        if first_hidden_same_x_cluster_fetch_skips_obj_tile_data_low_byte {
            self.runtime.obj_pipeline_state.fetch.stage = PpuObjFetcherStage::TileDataHigh;
            self.runtime.obj_pipeline_state.fetch.stage_dot = 0;
        } else if terminal_right_edge_same_x_chain_skips_to_tile_data_high_half_step {
            self.runtime.obj_pipeline_state.fetch.stage = PpuObjFetcherStage::TileDataHigh;
            self.runtime.obj_pipeline_state.fetch.stage_dot = 1;
        } else {
            self.runtime.obj_pipeline_state.fetch.stage = PpuObjFetcherStage::TileDataLow;
            self.runtime.obj_pipeline_state.fetch.stage_dot =
                u8::from(first_fast_same_x_cluster_fetch_skips_first_tile_data_low_half_step);
        }
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
        self.runtime.obj_pipeline_state.fetch.tile_high = fetch
            .resolved_tile_index
            .zip(fetch.resolved_tile_row)
            .zip(fetch.resolved_sprite)
            .map(|((tile_index, tile_row), sprite)| {
                self.read_obj_tile_data_byte_for_resolved_tile(
                    vram, sprite, tile_index, tile_row, 1,
                )
            })
            .unwrap_or(0);
        self.runtime.obj_pipeline_state.fetch.stage = PpuObjFetcherStage::Push;
        self.runtime.obj_pipeline_state.fetch.stage_dot = 0;
    }

    fn advance_object_fetch_push_dot0(&mut self) {
        self.runtime.obj_pipeline_state.fetch.stage_dot = 1;
    }

    fn advance_object_fetch_push_dot1(
        &mut self,
        fetch: ObjFetchState,
        oam: &OamBusView<'_>,
        vram: &VramBusView<'_>,
        dma_oam_conflict: Option<PpuDmaOamConflict>,
    ) {
        let resolved_sprite = fetch
            .resolved_sprite
            .expect("active OBJ fetch must keep resolved metadata until FIFO push");
        if !fetch.cancelled && self.obj_enabled() {
            let (tile_low, tile_high) = self.dmg_lcdc2_live_obj_size_push_bytes(
                resolved_sprite,
                fetch.tile_low,
                fetch.tile_high,
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
        if self.runtime.bg_pipeline_state.current_transfer_x < 8
            || self.right_edge_visible_same_x_cluster_continues_after_push()
        {
            let _ = self.continue_same_x_obj_chain_after_push(oam, dma_oam_conflict);
        } else if self.first_late_visible_push_backed_same_x_cluster_chains_after_push() {
            let _ = self.try_start_object_fetch_from_current_dot(
                ObjFetchStartSource::FifoBackedTransfer,
                true,
            );
        }
    }

    pub(in crate::ppu) fn obj_fetch_startup_ready(&self) -> bool {
        let fifo_ready = !self.runtime.bg_pipeline_state.fifo.is_empty();
        let Some(sprite) = self.runtime.obj_pipeline_state.fetch.sprite else {
            return fifo_ready;
        };

        if sprite.x >= 8 {
            return fifo_ready;
        }

        fifo_ready
            && !matches!(
                self.runtime.bg_pipeline_state.fetcher.stage,
                PpuBgFetcherStage::TileIndex
            )
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
        if !self.console_model.is_dmg_family()
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
            let effective_bg_priority_pixel = if bg_enabled { bg_pixel } else { 0 };
            let output_pixel = if candidate.is_transparent() {
                MixedPixel::background(bg_pixel)
            } else {
                self.mix_bg_and_obj(bg_pixel, effective_bg_priority_pixel, candidate)
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

    pub(in crate::ppu) fn chained_same_x_obj_fetch_skips_first_tile_data_low_half_step(
        &self,
    ) -> bool {
        let fetched_same_x_count = self.fetched_same_x_obj_sprite_count_for_active_fetch();
        let first_hidden_x4_same_x_restart_pays_low_half_step = fetched_same_x_count == 1
            && self.runtime.bg_pipeline_state.current_transfer_x == 4
            && matches!(
                self.current_transfer(),
                Some(Mode3CurrentTransfer {
                    context: Mode3TransferContext {
                        lane: Mode3TransferLane::Hidden,
                        source_window: Mode3TransferSourceWindow::FifoBacked,
                    },
                    ..
                })
            );
        (fetched_same_x_count >= 2 && fetched_same_x_count.is_multiple_of(2))
            || (matches!(fetched_same_x_count, 1 | 3)
                && !first_hidden_x4_same_x_restart_pays_low_half_step
                && self.nonterminal_same_x_cluster_restart_skips_first_low_half_step())
            || (fetched_same_x_count >= 5
                && fetched_same_x_count % 2 == 1
                && (self.hidden_same_x_cluster_restart_skips_first_low_half_step()
                    || self.visible_periodic_same_x_cluster_restart_skips_first_low_half_step()))
            || self.terminal_previsible_same_x_chain_skips_first_low_half_step()
    }

    pub(in crate::ppu) fn chained_same_x_obj_fetch_uses_long_tail_restart(&self) -> bool {
        let fetched_same_x_count = self.fetched_same_x_obj_sprite_count_for_active_fetch();
        fetched_same_x_count >= 5
            && fetched_same_x_count % 2 == 1
            && !self.current_transfer_x_supports_early_same_x_obj_start()
    }

    pub(in crate::ppu) fn first_hidden_same_x_cluster_fetch_skips_obj_tile_data_low_byte(
        &self,
    ) -> bool {
        self.runtime.bg_pipeline_state.current_transfer_x < 167
            && (self.runtime.bg_pipeline_state.current_transfer_x & 0x07) < 6
            && self.current_transfer_x_supports_early_same_x_obj_start()
            && !self
                .runtime
                .obj_pipeline_state
                .pending_sprite_slots
                .is_empty()
            && self.fetched_same_x_obj_sprite_count_for_active_fetch() == 0
            && matches!(
                (
                    self.runtime.bg_pipeline_state.fetcher.stage,
                    self.runtime.bg_pipeline_state.fetcher.stage_dot,
                ),
                (PpuBgFetcherStage::TileDataHigh, 1)
            )
    }

    pub(in crate::ppu) fn initial_nonterminal_same_x_cluster_skips_first_low_half_step(
        &self,
    ) -> bool {
        self.runtime.bg_pipeline_state.current_transfer_x < 167
            && (self.runtime.bg_pipeline_state.current_transfer_x & 0x07) < 7
            && self.current_transfer_x_supports_early_same_x_obj_start()
    }

    pub(in crate::ppu) fn nonterminal_same_x_cluster_restart_skips_first_low_half_step(
        &self,
    ) -> bool {
        self.runtime.bg_pipeline_state.current_transfer_x < 167
            && (self.runtime.bg_pipeline_state.current_transfer_x & 0x07) < 7
            && self.current_transfer_x_supports_early_same_x_obj_start()
    }

    pub(in crate::ppu) fn hidden_same_x_cluster_restart_skips_first_low_half_step(&self) -> bool {
        matches!(
            self.current_transfer(),
            Some(Mode3CurrentTransfer {
                context: Mode3TransferContext {
                    lane: Mode3TransferLane::Hidden,
                    source_window: Mode3TransferSourceWindow::FifoBacked,
                },
                ..
            })
        ) && matches!(
            (
                self.runtime.bg_pipeline_state.fetcher.stage,
                self.runtime.bg_pipeline_state.fetcher.stage_dot,
            ),
            (PpuBgFetcherStage::TileDataHigh, 1)
        )
    }

    pub(in crate::ppu) fn visible_periodic_same_x_cluster_restart_skips_first_low_half_step(
        &self,
    ) -> bool {
        self.runtime.bg_pipeline_state.visible_pixels_output >= 24
            && self.current_transfer_x_supports_early_same_x_obj_start()
            && matches!(
                self.current_transfer(),
                Some(Mode3CurrentTransfer {
                    context: Mode3TransferContext {
                        lane: Mode3TransferLane::Visible,
                        source_window: Mode3TransferSourceWindow::FifoBacked,
                    },
                    ..
                })
            )
            && matches!(
                (
                    self.runtime.bg_pipeline_state.fetcher.stage,
                    self.runtime.bg_pipeline_state.fetcher.stage_dot,
                ),
                (PpuBgFetcherStage::TileDataHigh, 1)
            )
    }

    pub(in crate::ppu) fn hidden_left_edge_same_x_chain_pays_push_dot(&self) -> bool {
        let fetched_same_x_count = self.fetched_same_x_obj_sprite_count_for_active_fetch();
        (4..=7).contains(&self.runtime.bg_pipeline_state.current_transfer_x)
            && matches!(
                self.current_transfer(),
                Some(Mode3CurrentTransfer {
                    context: Mode3TransferContext {
                        lane: Mode3TransferLane::Hidden,
                        source_window: Mode3TransferSourceWindow::FifoBacked,
                    },
                    ..
                })
            )
            && (1..=4).contains(&fetched_same_x_count)
    }

    pub(in crate::ppu) fn visible_left_edge_same_x_chain_shares_push_dot(&self) -> bool {
        let fetched_same_x_count = self.fetched_same_x_obj_sprite_count_for_active_fetch();
        let visible_output = self.runtime.bg_pipeline_state.visible_pixels_output;
        let current_transfer_x = self.runtime.bg_pipeline_state.current_transfer_x;
        let repeated_visible_boundary_fetched_count_supports_push_dot_share =
            (1..=4).contains(&fetched_same_x_count);
        let late_visible_same_x_cluster_shares_push_dot = (visible_output & 0x1f) >= 24
            && self.current_transfer_x_supports_early_same_x_obj_start()
            && fetched_same_x_count >= 5;
        let same_x_screen_edge_supports_push_dot_share = (visible_output < 8
            && self.current_transfer_x_supports_early_same_x_obj_start())
            || late_visible_same_x_cluster_shares_push_dot
            || ((visible_output & 0x0f) == 8
                && (current_transfer_x & 0x07) == 0
                && repeated_visible_boundary_fetched_count_supports_push_dot_share);
        same_x_screen_edge_supports_push_dot_share
            && matches!(
                self.current_transfer(),
                Some(Mode3CurrentTransfer {
                    context: Mode3TransferContext {
                        lane: Mode3TransferLane::Visible,
                        source_window: Mode3TransferSourceWindow::FifoBacked,
                    },
                    ..
                })
            )
            && fetched_same_x_count > 0
    }

    pub(in crate::ppu) fn first_late_visible_push_backed_same_x_cluster_chains_after_push(
        &self,
    ) -> bool {
        self.runtime.bg_pipeline_state.startup_fifo_placeholders == 0
            && self.runtime.bg_pipeline_state.fifo.len() == 2
            && self.current_transfer_x_supports_early_same_x_obj_start()
            && (self.runtime.bg_pipeline_state.current_transfer_x & 0x07) == 2
            && matches!(
                self.current_transfer(),
                Some(Mode3CurrentTransfer {
                    context: Mode3TransferContext {
                        lane: Mode3TransferLane::Visible,
                        source_window: Mode3TransferSourceWindow::FifoBacked,
                    },
                    readiness: Mode3TransferReadiness::Ready(_),
                })
            )
            && matches!(
                (
                    self.runtime.bg_pipeline_state.fetcher.stage,
                    self.runtime.bg_pipeline_state.fetcher.stage_dot,
                ),
                (PpuBgFetcherStage::Push, 0)
            )
            && self.runtime.obj_pipeline_state.pending_match_x
                == Some(self.runtime.bg_pipeline_state.current_transfer_x)
            && !self
                .runtime
                .obj_pipeline_state
                .pending_sprite_slots
                .is_empty()
            && self.fetched_same_x_obj_sprite_count_for_pending_match_x() > 0
    }

    pub(in crate::ppu) fn right_edge_visible_same_x_cluster_continues_after_push(&self) -> bool {
        self.runtime.bg_pipeline_state.current_transfer_x >= 160
            && matches!(
                self.current_transfer(),
                Some(Mode3CurrentTransfer {
                    context: Mode3TransferContext {
                        lane: Mode3TransferLane::Visible,
                        source_window: Mode3TransferSourceWindow::FifoBacked,
                    },
                    readiness: Mode3TransferReadiness::Ready(_),
                })
            )
            && self.runtime.obj_pipeline_state.pending_match_x
                == Some(self.runtime.bg_pipeline_state.current_transfer_x)
            && self.runtime.obj_pipeline_state.pending_sprite_slots.len() >= 2
            && self.fetched_same_x_obj_sprite_count_for_pending_match_x() > 0
    }

    pub(in crate::ppu) fn continue_same_x_obj_chain_after_push(
        &mut self,
        oam: &OamBusView<'_>,
        dma_oam_conflict: Option<PpuDmaOamConflict>,
    ) -> bool {
        let pending_nonterminal_same_x_cluster_pays_startup_dot =
            self.pending_nonterminal_same_x_cluster_pays_startup_dot();
        let right_edge_visible_same_x_cluster_pays_startup_dot =
            self.right_edge_visible_same_x_cluster_pays_startup_dot();
        let right_edge_visible_same_x_cluster = right_edge_visible_same_x_cluster_pays_startup_dot
            || self.right_edge_visible_same_x_cluster_continues_after_push();
        let started = self
            .try_start_object_fetch_from_current_dot(ObjFetchStartSource::FifoBackedTransfer, true);
        if !started || !self.obj_fetch_startup_ready() {
            return started;
        }
        if right_edge_visible_same_x_cluster {
            self.runtime
                .bg_pipeline_state
                .saw_right_edge_visible_same_x_cluster_this_line = true;
        }

        let long_same_x_tail_restart = self.chained_same_x_obj_fetch_uses_long_tail_restart();
        if long_same_x_tail_restart {
            self.runtime.bg_pipeline_state.extend_mode3_by_one_dot();
        }
        let sprite = self
            .runtime
            .obj_pipeline_state
            .fetch
            .sprite
            .expect("chained OBJ fetch must keep sprite metadata");
        let resolved_sprite = self.resolve_obj_fetch_sprite(oam, sprite, dma_oam_conflict);
        self.runtime.obj_pipeline_state.fetch.resolved_sprite = Some(resolved_sprite);
        if long_same_x_tail_restart {
            self.runtime.obj_pipeline_state.fetch.stage = PpuObjFetcherStage::Startup;
            self.runtime.obj_pipeline_state.fetch.stage_dot = 0;
            self.runtime
                .obj_pipeline_state
                .fetch
                .count_terminal_push_dot = true;
        } else {
            if pending_nonterminal_same_x_cluster_pays_startup_dot
                || right_edge_visible_same_x_cluster_pays_startup_dot
            {
                self.runtime.bg_pipeline_state.extend_mode3_by_one_dot();
            }
            if self.terminal_previsible_same_x_chain_skips_obj_tile_data_low_byte() {
                self.runtime.obj_pipeline_state.fetch.stage = PpuObjFetcherStage::TileDataHigh;
                self.runtime.obj_pipeline_state.fetch.stage_dot = 0;
            } else {
                self.runtime.obj_pipeline_state.fetch.stage = PpuObjFetcherStage::TileDataLow;
                self.runtime.obj_pipeline_state.fetch.stage_dot =
                    u8::from(self.chained_same_x_obj_fetch_skips_first_tile_data_low_half_step());
            }
        }

        true
    }

    pub(in crate::ppu) fn right_edge_visible_same_x_cluster_pays_startup_dot(&self) -> bool {
        self.runtime.bg_pipeline_state.current_transfer_x >= 160
            && matches!(
                self.current_transfer(),
                Some(Mode3CurrentTransfer {
                    context: Mode3TransferContext {
                        lane: Mode3TransferLane::Visible,
                        source_window: Mode3TransferSourceWindow::FifoBacked,
                    },
                    readiness: Mode3TransferReadiness::Ready(_),
                })
            )
            && self.runtime.obj_pipeline_state.pending_match_x
                == Some(self.runtime.bg_pipeline_state.current_transfer_x)
            && self.fetched_same_x_obj_sprite_count_for_pending_match_x() >= 5
    }

    pub(in crate::ppu) fn saturated_placeholder_backed_terminal_bg_tail_can_hold_one_post_push_dot(
        &self,
    ) -> bool {
        self.runtime.bg_pipeline_state.mode3_started
            && self.runtime.bg_pipeline_state.visible_pixels_output as usize >= SCREEN_WIDTH
            && self.runtime.bg_pipeline_state.current_transfer_x >= 168
            && (160..=161).any(|sprite_x| {
                (0..self.runtime.mode2_scan_state.selected_sprite_count())
                    .filter(|&slot| {
                        self.runtime
                            .mode2_scan_state
                            .selected_sprite(slot)
                            .is_some_and(|sprite| sprite.x == sprite_x)
                    })
                    .count()
                    >= 5
            })
            && usize::from(self.runtime.mode2_scan_state.selected_sprite_count())
                == MAX_SELECTED_SPRITES_PER_LINE
            && self.runtime.bg_pipeline_state.startup_fifo_placeholders == 4
            && self.runtime.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.runtime.obj_pipeline_state.pending_match_x.is_none()
            && self
                .runtime
                .obj_pipeline_state
                .pending_sprite_slots
                .is_empty()
            && self.runtime.bg_pipeline_state.fetcher.stage == PpuBgFetcherStage::Push
    }

    pub(in crate::ppu) fn terminal_previsible_same_x_chain_can_start_obj_fetch(&self) -> bool {
        self.runtime.bg_pipeline_state.current_transfer_x < 8
            && self.runtime.obj_pipeline_state.pending_match_x
                == Some(self.runtime.bg_pipeline_state.current_transfer_x)
            && self.runtime.obj_pipeline_state.pending_sprite_slots.len() == 1
            && self.fetched_same_x_obj_sprite_count_for_pending_match_x() > 0
    }

    pub(in crate::ppu) fn terminal_previsible_same_x_chain_skips_first_low_half_step(
        &self,
    ) -> bool {
        self.runtime.bg_pipeline_state.current_transfer_x < 8
            && self.current_transfer_x_supports_early_same_x_obj_start()
            && self.runtime.obj_pipeline_state.pending_match_x.is_none()
            && self
                .runtime
                .obj_pipeline_state
                .pending_sprite_slots
                .is_empty()
            && self.fetched_same_x_obj_sprite_count_for_active_fetch() > 0
    }

    pub(in crate::ppu) fn terminal_previsible_same_x_chain_skips_obj_tile_data_low_byte(
        &self,
    ) -> bool {
        self.terminal_previsible_same_x_chain_skips_first_low_half_step()
            && self.fetched_same_x_obj_sprite_count_for_active_fetch() >= 9
    }

    pub(in crate::ppu) fn terminal_right_edge_same_x_chain_skips_to_tile_data_high_half_step(
        &self,
    ) -> bool {
        self.runtime.bg_pipeline_state.current_transfer_x >= 160
            && matches!(
                self.current_transfer(),
                Some(Mode3CurrentTransfer {
                    context: Mode3TransferContext {
                        lane: Mode3TransferLane::Visible,
                        source_window: Mode3TransferSourceWindow::FifoBacked,
                    },
                    readiness: Mode3TransferReadiness::Ready(_),
                })
            )
            && self.runtime.obj_pipeline_state.pending_match_x.is_none()
            && self
                .runtime
                .obj_pipeline_state
                .pending_sprite_slots
                .is_empty()
            && self.fetched_same_x_obj_sprite_count_for_active_fetch() > 0
    }

    pub(in crate::ppu) fn terminal_right_edge_same_x_chain_shares_push_dot(&self) -> bool {
        self.runtime.bg_pipeline_state.current_transfer_x >= 160
            && matches!(
                self.current_transfer(),
                Some(Mode3CurrentTransfer {
                    context: Mode3TransferContext {
                        lane: Mode3TransferLane::Visible,
                        source_window: Mode3TransferSourceWindow::FifoBacked,
                    },
                    readiness: Mode3TransferReadiness::Ready(_),
                })
            )
            && self.runtime.obj_pipeline_state.pending_match_x.is_none()
            && self
                .runtime
                .obj_pipeline_state
                .pending_sprite_slots
                .is_empty()
            && self.fetched_same_x_obj_sprite_count_for_active_fetch() > 0
    }

    pub(in crate::ppu) fn current_transfer_x_supports_early_same_x_obj_start(&self) -> bool {
        matches!(
            self.runtime.bg_pipeline_state.current_transfer_x & 0x07,
            2..=7
        )
    }

    pub(in crate::ppu) fn terminal_mode3_dot_started_shared_obj_fetch(&self) -> bool {
        matches!(
            (
                self.runtime.obj_pipeline_state.fetch.stage,
                self.runtime.obj_pipeline_state.fetch.stage_dot,
            ),
            (PpuObjFetcherStage::Startup, 1)
        ) && self.line_dot.saturating_add(1) == self.current_mode0_start_dot()
    }

    pub(in crate::ppu) fn pending_nonterminal_same_x_cluster_pays_startup_dot(&self) -> bool {
        self.runtime.bg_pipeline_state.current_transfer_x < 167
            && self.current_transfer_x_supports_early_same_x_obj_start()
            && self.runtime.obj_pipeline_state.pending_match_x
                == Some(self.runtime.bg_pipeline_state.current_transfer_x)
            && self.runtime.obj_pipeline_state.pending_sprite_slots.len() >= 2
            && !self.first_late_visible_push_backed_same_x_cluster_chains_after_push()
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
