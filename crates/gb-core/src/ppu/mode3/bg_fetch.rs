use super::*;

impl Ppu {
    pub(in crate::ppu) fn advance_bg_fetcher(&mut self, vram: &VramBusView<'_>) -> bool {
        self.prepare_bg_fetcher_dot(vram);
        let fetch_policy = self.mode3_bgwin_fetch_policy();

        if let Some(handed_off) = self.advance_bg_fetcher_special_stage() {
            return handed_off;
        }

        if self.consume_bg_fetcher_post_alignment_restart_delay_dot() {
            return false;
        }

        self.advance_bg_fetcher_automaton_step(fetch_policy, vram)
    }

    fn prepare_bg_fetcher_dot(&mut self, vram: &VramBusView<'_>) {
        self.maybe_apply_dmg_previsible_wx_retarget(vram);
        self.maybe_abort_window_fetcher_to_background(vram);
        self.maybe_recompute_pending_background_push(vram);
    }

    fn advance_bg_fetcher_special_stage(&mut self) -> Option<bool> {
        match (
            self.runtime.bg_pipeline_state.fetcher.stage,
            self.runtime.bg_pipeline_state.fetcher.stage_dot,
        ) {
            (PpuBgFetcherStage::Idle, _) => {
                self.runtime.bg_pipeline_state.fetcher.start_background();
                Some(false)
            }
            (PpuBgFetcherStage::WindowActivating, _) => {
                self.runtime.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileIndex;
                self.runtime.bg_pipeline_state.fetcher.stage_dot = 0;
                Some(false)
            }
            (PpuBgFetcherStage::Push, _) => Some(Self::bg_push_handed_off_to_object_fetch(
                self.advance_bg_push_stage(),
            )),
            _ => None,
        }
    }

    fn consume_bg_fetcher_post_alignment_restart_delay_dot(&mut self) -> bool {
        if self
            .runtime
            .bg_pipeline_state
            .fetcher
            .post_alignment_fetch_restart_delay_dots
            == 0
        {
            return false;
        }

        self.runtime
            .bg_pipeline_state
            .fetcher
            .post_alignment_fetch_restart_delay_dots -= 1;
        true
    }

    fn advance_bg_fetcher_automaton_step(
        &mut self,
        fetch_policy: PpuMode3BgWinFetchPolicy,
        vram: &VramBusView<'_>,
    ) -> bool {
        let fetcher = self.runtime.bg_pipeline_state.fetcher;

        match (fetcher.stage, fetcher.stage_dot) {
            (PpuBgFetcherStage::TileIndex, 0) => {
                self.advance_bg_fetcher_tile_index_dot0(fetcher, fetch_policy, vram);
                false
            }
            (PpuBgFetcherStage::TileIndex, 1) => {
                self.advance_bg_fetcher_tile_index_dot1(fetcher, fetch_policy, vram);
                false
            }
            (PpuBgFetcherStage::TileDataLow, 0) => {
                self.advance_bg_fetcher_tile_data_low_dot0(fetcher, vram);
                false
            }
            (PpuBgFetcherStage::TileDataLow, 1) => {
                self.advance_bg_fetcher_tile_data_low_dot1(fetcher, vram);
                false
            }
            (PpuBgFetcherStage::TileDataHigh, 0) => {
                self.advance_bg_fetcher_tile_data_high_dot0(fetcher, vram);
                false
            }
            (PpuBgFetcherStage::TileDataHigh, 1) => {
                self.advance_bg_fetcher_tile_data_high_dot1(fetcher, vram)
            }
            (PpuBgFetcherStage::Idle, _)
            | (PpuBgFetcherStage::WindowActivating, _)
            | (PpuBgFetcherStage::Push, _) => unreachable!(
                "special BG fetcher stages are handled before the explicit dot-stage automaton"
            ),
            (_, other_dot) => unreachable!(
                "invalid BG fetcher stage_dot {other_dot} for non-push stage {:?}",
                fetcher.stage
            ),
        }
    }

    fn advance_bg_fetcher_tile_index_dot0(
        &mut self,
        fetcher: BgFetcherState,
        fetch_policy: PpuMode3BgWinFetchPolicy,
        vram: &VramBusView<'_>,
    ) {
        if fetcher.source == PpuBgFetcherSource::Background {
            self.runtime.bg_pipeline_state.fetcher.cached_origin = self
                .runtime
                .bg_pipeline_state
                .peek_startup_background_fetch_origin();
            self.runtime
                .bg_pipeline_state
                .fetcher
                .needs_live_tilemap_refetch_on_push = false;
            self.runtime
                .bg_pipeline_state
                .fetcher
                .needs_live_tilemap_full_refetch_on_push = false;
            self.runtime
                .bg_pipeline_state
                .fetcher
                .needs_live_tile_data_refetch_on_push = false;
            self.runtime
                .bg_pipeline_state
                .fetcher
                .needs_live_tile_data_current_row_refetch_on_push = false;
            self.runtime
                .bg_pipeline_state
                .fetcher
                .needs_live_tile_low_current_row_refetch_on_push = false;
            self.runtime
                .bg_pipeline_state
                .fetcher
                .needs_live_tile_high_current_row_refetch_on_push = false;
            self.runtime
                .bg_pipeline_state
                .fetcher
                .cgb_dmg_scy_high_plane_uses_low_row = false;
        }

        let tile_map_address =
            self.compute_fetch_tile_index_address(fetcher.source, fetcher.fetch_x);
        self.runtime.bg_pipeline_state.fetcher.tile_map_address = tile_map_address;
        if !fetch_policy.should_delay_background_tileindex_read(fetcher.source) {
            self.runtime.bg_pipeline_state.fetcher.tile_index =
                vram.read(tile_map_address as usize).unwrap_or(0);
            self.runtime.bg_pipeline_state.fetcher.cgb_bg_attrs =
                self.read_cgb_bg_tile_attributes(vram, tile_map_address);
        } else {
            self.runtime.bg_pipeline_state.fetcher.cgb_bg_attrs = None;
        }
        if fetcher.source == PpuBgFetcherSource::Window {
            self.runtime.bg_pipeline_state.fetcher.window_tilemap_x = self
                .runtime
                .bg_pipeline_state
                .fetcher
                .window_tilemap_x
                .wrapping_add(1);
        }
        if self
            .runtime
            .bg_pipeline_state
            .fetcher
            .rewind_bg_resume_after_first_tile_index_dot
        {
            self.runtime.bg_pipeline_state.fetcher.bg_resume_fetch_pixel = self
                .runtime
                .bg_pipeline_state
                .fetcher
                .bg_resume_fetch_pixel
                .wrapping_sub(BG_TILE_WIDTH as u16);
            self.runtime
                .bg_pipeline_state
                .fetcher
                .rewind_bg_resume_after_first_tile_index_dot = false;
        }
        self.runtime
            .bg_pipeline_state
            .fetcher
            .same_cycle_window_tilemap_lcdc_hold = false;
        self.runtime.bg_pipeline_state.fetcher.stage_dot = 1;
    }

    fn advance_bg_fetcher_tile_index_dot1(
        &mut self,
        fetcher: BgFetcherState,
        fetch_policy: PpuMode3BgWinFetchPolicy,
        vram: &VramBusView<'_>,
    ) {
        if fetch_policy.should_delay_background_tileindex_read(fetcher.source) {
            self.runtime.bg_pipeline_state.fetcher.tile_index = vram
                .read(self.runtime.bg_pipeline_state.fetcher.tile_map_address as usize)
                .unwrap_or(0);
            self.runtime.bg_pipeline_state.fetcher.cgb_bg_attrs = self.read_cgb_bg_tile_attributes(
                vram,
                self.runtime.bg_pipeline_state.fetcher.tile_map_address,
            );
        }
        self.maybe_apply_bgwin_tilemap_selector_glitch(vram, fetcher.source);
        self.runtime.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataLow;
        self.runtime.bg_pipeline_state.fetcher.stage_dot = 0;
    }

    fn advance_bg_fetcher_tile_data_low_dot0(
        &mut self,
        fetcher: BgFetcherState,
        vram: &VramBusView<'_>,
    ) {
        let tile_data_address = if fetcher.source == PpuBgFetcherSource::Window {
            self.runtime
                .bg_pipeline_state
                .fetcher
                .dmg_lcdc4_previous_tiledata_select_on_next_low
                .take()
                .map_or_else(
                    || {
                        self.compute_fetch_tile_data_address_with_attributes(
                            fetcher.source,
                            fetcher.fetch_x,
                            fetcher.tile_index,
                            0,
                            fetcher.cgb_bg_attrs,
                        )
                    },
                    |selector| {
                        self.compute_window_fetch_tile_data_address_with_selector_and_attributes(
                            fetcher.tile_index,
                            0,
                            selector,
                            fetcher.cgb_bg_attrs,
                        )
                    },
                )
        } else {
            self.compute_fetch_tile_data_address_with_attributes(
                fetcher.source,
                fetcher.fetch_x,
                fetcher.tile_index,
                0,
                fetcher.cgb_bg_attrs,
            )
        };
        self.runtime.bg_pipeline_state.fetcher.tile_data_address = tile_data_address;
        self.runtime.bg_pipeline_state.fetcher.tile_low_address = tile_data_address;
        let tile_data = self.read_bg_tile_data_byte(vram, fetcher.cgb_bg_attrs, tile_data_address);
        self.runtime.bg_pipeline_state.fetcher.tile_low = tile_data;
        self.maybe_cache_unsigned_bgwin_tile_data_fetch(
            fetcher.source,
            fetcher.fetch_x,
            0,
            tile_data,
        );
        self.runtime.bg_pipeline_state.fetcher.stage_dot = 1;
    }

    fn advance_bg_fetcher_tile_data_low_dot1(
        &mut self,
        fetcher: BgFetcherState,
        vram: &VramBusView<'_>,
    ) {
        self.maybe_apply_bgwin_tile_data_selector_glitch(vram, fetcher.source, 0);
        self.runtime.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::TileDataHigh;
        self.runtime.bg_pipeline_state.fetcher.stage_dot = 0;
    }

    fn advance_bg_fetcher_tile_data_high_dot0(
        &mut self,
        fetcher: BgFetcherState,
        vram: &VramBusView<'_>,
    ) {
        let tile_data_address = self.compute_fetch_tile_data_address_with_attributes(
            fetcher.source,
            fetcher.fetch_x,
            fetcher.tile_index,
            1,
            fetcher.cgb_bg_attrs,
        );
        let tile_data_address = if fetcher.cgb_dmg_scy_high_plane_uses_low_row {
            fetcher.tile_low_address | 1
        } else {
            tile_data_address
        };
        self.runtime
            .bg_pipeline_state
            .fetcher
            .cgb_dmg_scy_high_plane_uses_low_row = false;
        self.runtime.bg_pipeline_state.fetcher.tile_data_address = tile_data_address;
        self.runtime.bg_pipeline_state.fetcher.tile_high_address = tile_data_address;
        let tile_data = self.read_bg_tile_data_byte(vram, fetcher.cgb_bg_attrs, tile_data_address);
        self.runtime.bg_pipeline_state.fetcher.tile_high = tile_data;
        self.maybe_cache_unsigned_bgwin_tile_data_fetch(
            fetcher.source,
            fetcher.fetch_x,
            1,
            tile_data,
        );
        self.runtime.bg_pipeline_state.fetcher.stage_dot = 1;
    }

    fn advance_bg_fetcher_tile_data_high_dot1(
        &mut self,
        fetcher: BgFetcherState,
        vram: &VramBusView<'_>,
    ) -> bool {
        self.maybe_apply_bgwin_tile_data_selector_glitch(vram, fetcher.source, 1);
        if self
            .runtime
            .bg_pipeline_state
            .startup_alignment_seed_pending()
        {
            return self.queue_bg_startup_alignment_seed_from_fetcher();
        }

        self.queue_bg_push_from_fetcher(fetcher)
    }

    fn queue_bg_startup_alignment_seed_from_fetcher(&mut self) -> bool {
        let fetcher_state = self.runtime.bg_pipeline_state.fetcher;
        self.runtime
            .bg_pipeline_state
            .push
            .queue_startup_alignment_seed_from_fetcher(fetcher_state);
        self.runtime
            .bg_pipeline_state
            .fetcher
            .startup_visible_tile3_scx_boundary_full_refetch_next_tile = false;
        self.runtime
            .bg_pipeline_state
            .fetcher
            .first_window_tile_after_activation = false;
        self.runtime.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
        self.runtime.bg_pipeline_state.fetcher.stage_dot = 0;
        Self::bg_push_handed_off_to_object_fetch(self.advance_bg_push_stage())
    }

    fn queue_bg_push_from_fetcher(&mut self, fetcher: BgFetcherState) -> bool {
        self.runtime
            .bg_pipeline_state
            .maybe_attach_startup_visible_tile3_scx_boundary_next_slice_to_fetcher();
        let fetcher_state = self.runtime.bg_pipeline_state.fetcher;
        self.runtime
            .bg_pipeline_state
            .push
            .queue_from_fetcher(fetcher_state);
        self.runtime
            .bg_pipeline_state
            .maybe_apply_dmg_lcdc3_startup_continuation_tilemap_select_override_to_push();
        self.runtime
            .bg_pipeline_state
            .maybe_apply_latched_dmg_lcdc4_startup_tiledata_select_override_to_push();
        if self.console_model.is_cgb_family()
            && self.operating_mode.uses_dmg_software_contract()
            && fetcher_state.source == PpuBgFetcherSource::Window
            && fetcher_state.dmg_lcdc4_previous_tiledata_select_for_output_override
                == Some(BgTileDataSelect::Signed8800)
        {
            self.runtime
                .bg_pipeline_state
                .fetcher
                .dmg_lcdc4_previous_tiledata_select_for_output_override = None;
        }
        self.runtime
            .bg_pipeline_state
            .fetcher
            .startup_visible_tile3_scx_boundary_full_refetch_next_tile = false;
        self.runtime
            .bg_pipeline_state
            .fetcher
            .clear_startup_visible_tile3_scx_boundary_old_pixel_window();
        if fetcher.source == PpuBgFetcherSource::Background {
            self.runtime
                .bg_pipeline_state
                .advance_startup_background_fetch_tile();
        }

        let mut advance_push_immediately = false;
        if self
            .runtime
            .bg_pipeline_state
            .take_startup_first_real_push_skip_entry_delay()
        {
            self.runtime.bg_pipeline_state.push.entry_delay_remaining = 0;
            advance_push_immediately = true;
        }
        self.runtime
            .bg_pipeline_state
            .fetcher
            .first_window_tile_after_activation = false;
        self.runtime.bg_pipeline_state.fetcher.stage = PpuBgFetcherStage::Push;
        self.runtime.bg_pipeline_state.fetcher.stage_dot = 0;
        if advance_push_immediately {
            return Self::bg_push_handed_off_to_object_fetch(self.advance_bg_push_stage());
        }

        false
    }

    fn bg_push_handed_off_to_object_fetch(result: BgPushDotResult) -> bool {
        matches!(
            result,
            BgPushDotResult::HandedOffToObjectFetch
                | BgPushDotResult::QueuedFillAndHandedOffToObjectFetch
        )
    }
}
