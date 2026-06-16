use super::*;

impl Ppu {
    pub(in crate::ppu) fn execute_transfer_service_plan(
        &mut self,
        plan: Mode3TransferServicePlan,
        vram: &VramBusView<'_>,
    ) -> Mode3TransferDot {
        if self.console_model.is_dmg_family()
            || self.console_model.is_cgb_family()
                && self.operating_mode.uses_dmg_software_contract()
        {
            self.apply_pending_dmg_window_lcdc4_output_repaint(vram);
        }
        let pixel = self.take_transfer_service_bg_pixel(plan);
        self.begin_transfer_service_execution(plan);
        self.execute_transfer_service_execution(plan, pixel, vram)
    }

    fn take_transfer_service_bg_pixel(&mut self, plan: Mode3TransferServicePlan) -> Option<u8> {
        if matches!(
            plan.execution,
            Mode3TransferServiceExecution::EmitVisiblePixel
        ) {
            // The visible pixel is popped (with its cached metadata) in execute_transfer_visible_pixel.
            None
        } else {
            // SCX-discard / pre-visible / hidden dots each pop one real BG FIFO entry (the leading
            // startup junk pixels are real FIFO entries that drain the same way).
            self.runtime.bg_pipeline_state.pop_real_fifo_pixel()
        }
    }

    fn begin_transfer_service_execution(&mut self, plan: Mode3TransferServicePlan) {
        self.runtime.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
        if !matches!(
            plan.execution,
            Mode3TransferServiceExecution::ConsumeScxDiscard
                | Mode3TransferServiceExecution::EmitVisiblePixel
        ) {
            self.runtime
                .bg_pipeline_state
                .consume_startup_pre_visible_transfer_dot();
        }
    }

    fn execute_transfer_service_execution(
        &mut self,
        plan: Mode3TransferServicePlan,
        pixel: Option<u8>,
        vram: &VramBusView<'_>,
    ) -> Mode3TransferDot {
        match plan.execution {
            Mode3TransferServiceExecution::ConsumeScxDiscard => {
                self.execute_transfer_scx_discard(plan, pixel)
            }
            Mode3TransferServiceExecution::AdvancePreVisibleWithBgPop => {
                self.execute_transfer_previsible_bg_pop(plan, pixel)
            }
            Mode3TransferServiceExecution::AdvanceHiddenWithBgAndObjPop => {
                self.execute_transfer_hidden_bg_and_obj_pop(plan, pixel)
            }
            Mode3TransferServiceExecution::EmitVisiblePixel => {
                self.execute_transfer_visible_pixel(plan, vram)
            }
        }
    }

    fn execute_transfer_scx_discard(
        &mut self,
        plan: Mode3TransferServicePlan,
        pixel: Option<u8>,
    ) -> Mode3TransferDot {
        let _ = pixel
            .expect("startup scx discard must consume one effective BG FIFO slot before output");
        self.runtime.bg_pipeline_state.scx_discard_remaining -= 1;
        Mode3TransferDot::served(plan.result_kind, true)
    }

    fn execute_transfer_previsible_bg_pop(
        &mut self,
        plan: Mode3TransferServicePlan,
        pixel: Option<u8>,
    ) -> Mode3TransferDot {
        let _ =
            pixel.expect("pre-visible startup transfer must consume one effective BG FIFO slot");
        self.runtime.bg_pipeline_state.current_transfer_x += 1;
        Mode3TransferDot::served(plan.result_kind, false)
    }

    fn execute_transfer_hidden_bg_and_obj_pop(
        &mut self,
        plan: Mode3TransferServicePlan,
        pixel: Option<u8>,
    ) -> Mode3TransferDot {
        let _ = pixel.expect("hidden transfer must consume one effective BG FIFO slot");
        self.runtime.bg_pipeline_state.current_transfer_x += 1;
        let _ = self.pop_obj_fifo_pixel();
        Mode3TransferDot::served(plan.result_kind, false)
    }

    fn execute_transfer_visible_pixel(
        &mut self,
        plan: Mode3TransferServicePlan,
        vram: &VramBusView<'_>,
    ) -> Mode3TransferDot {
        let bg_pixel = self
            .pop_visible_bg_fifo_pixel(vram)
            .expect("visible transfer plans must carry a BG pixel");
        let bg_enabled = self.pixel_transfer_bg_enabled();
        let visible_x = self.runtime.bg_pipeline_state.visible_pixels_output;
        let dmg_family = self.console_model.is_dmg_family();
        let dmg_software_contract = self.operating_mode.uses_dmg_software_contract();
        let effective_bg_priority_pixel = if bg_enabled { bg_pixel.color } else { 0 };
        let obj_pixel = self.pop_obj_fifo_pixel();
        let obj_pixel = if dmg_software_contract {
            self.apply_dmg_lcdc2_live_obj_size_output_override(obj_pixel, visible_x, vram)
        } else {
            obj_pixel
        };
        let output_pixel = self.mix_bg_and_obj(
            bg_pixel.color,
            bg_pixel.cgb_bg_attrs,
            effective_bg_priority_pixel,
            obj_pixel,
        );
        let dmg_bg_forced_white = self.dmg_bg_panel_dot_is_forced_white(bg_enabled, output_pixel);
        let panel_pixel = if self.runtime.panel.visible_output == PpuVisibleOutputState::Driving {
            if dmg_bg_forced_white {
                0
            } else {
                self.map_mixed_pixel_to_panel_shade(output_pixel)
            }
        } else {
            0
        };
        let scanline_pixel = if self.runtime.panel.visible_output == PpuVisibleOutputState::Driving
            && !dmg_bg_forced_white
        {
            output_pixel.color
        } else {
            0
        };
        let visible_x_index = visible_x as usize;
        self.runtime.panel.current_scanline_bg_pixels[visible_x_index] = bg_pixel.color;
        self.write_bgwin_framebuffer_pixel(
            self.ly as usize * SCREEN_WIDTH,
            visible_x_index,
            bg_pixel.color,
            bg_enabled,
        );
        self.runtime.panel.current_scanline_mixed_pixels[visible_x_index] = output_pixel;
        self.runtime.panel.current_scanline_dmg_bg_forced_white[visible_x_index] =
            dmg_bg_forced_white;
        self.runtime.panel.current_scanline_pixels[visible_x_index] = scanline_pixel;
        self.write_framebuffer_pixel(
            self.ly as usize * SCREEN_WIDTH,
            visible_x_index,
            output_pixel,
            panel_pixel,
        );
        if self.uses_dmg_palette_live_write_model() {
            self.record_dmg_recent_panel_dot(
                visible_x_index as u8,
                output_pixel,
                dmg_bg_forced_white,
            );
            self.consume_dmg_bgp_cpu_commit_bg_visible_hold(output_pixel);
        }
        if dmg_family
            || self.console_model.is_cgb_family()
                && self.operating_mode.uses_dmg_software_contract()
        {
            self.apply_dmg_wx0_window_disable_prefix_override(visible_x_index, bg_pixel.color);
        }
        if dmg_family
            || self.console_model.is_cgb_family()
                && self.operating_mode.uses_dmg_software_contract()
        {
            self.apply_dmg_late_window_enable_override_repaint_up_to(visible_x_index + 1, vram);
        }
        if self.operating_mode.uses_dmg_software_contract() {
            self.consume_dmg_lcdc0_bg_enable_visible_hold();
        }
        if self.operating_mode.uses_dmg_software_contract() {
            self.consume_dmg_lcdc1_obj_enable_visible_hold();
        }
        self.runtime.bg_pipeline_state.current_transfer_x = self
            .runtime
            .bg_pipeline_state
            .current_transfer_x
            .saturating_add(1);
        self.runtime.bg_pipeline_state.visible_pixels_output += 1;
        Mode3TransferDot::served(plan.result_kind, false)
    }

    pub(in crate::ppu) fn pop_visible_bg_fifo_pixel(
        &mut self,
        vram: &VramBusView<'_>,
    ) -> Option<BgOutputPixel> {
        let visible_x = self.runtime.bg_pipeline_state.visible_pixels_output as usize;
        let mut pixel = self.runtime.bg_pipeline_state.pop_visible_fifo_pixel()?;
        let mut cgb_bg_attrs = pixel.cgb_bg_attrs();
        if self
            .runtime
            .bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_cancel_background_override_onset_x
            .is_some_and(|onset_x| self.runtime.bg_pipeline_state.visible_pixels_output >= onset_x)
        {
            self.runtime.panel.current_scanline_bg_dot_contexts[visible_x] = None;
            return Some(BgOutputPixel::new(
                self.dmg_bg_color_for_panel_shade(0),
                None,
            ));
        }
        let Some(cached) = pixel.cached.as_mut() else {
            self.runtime.panel.current_scanline_bg_dot_contexts[visible_x] = None;
            return Some(BgOutputPixel::new(pixel.color, cgb_bg_attrs));
        };
        cgb_bg_attrs = cached.cached.cgb_bg_attrs;
        let window_activation_tilemap_override = self.compute_window_activation_tilemap_override(
            cached.cached,
            cached.pixel_index,
            vram,
        );
        let window_tiledata_selector_override = self
            .compute_window_lcdc4_tiledata_selector_override(
                cached.cached,
                cached.pixel_index,
                vram,
            );
        let Some(recomputed) = recompute_live_background_cached_slice(
            cached.cached,
            vram,
            self.current_mode3_live_background_refetch_context(),
        ) else {
            self.runtime.panel.current_scanline_bg_dot_contexts[visible_x] =
                Some(PpuRecentBgDotContext {
                    source: cached.cached.source,
                    fetch_x: cached.cached.fetch_x,
                    pixel_index: cached.pixel_index,
                    tile_index: cached.cached.tile_index,
                });
            let color = window_activation_tilemap_override
                .or(window_tiledata_selector_override)
                .unwrap_or(pixel.color);
            return Some(BgOutputPixel::new(color, cgb_bg_attrs));
        };

        cached.cached = recomputed;
        cgb_bg_attrs = cached.cached.cgb_bg_attrs;
        self.runtime.panel.current_scanline_bg_dot_contexts[visible_x] =
            Some(PpuRecentBgDotContext {
                source: cached.cached.source,
                fetch_x: cached.cached.fetch_x,
                pixel_index: cached.pixel_index,
                tile_index: cached.cached.tile_index,
            });
        pixel.color = window_activation_tilemap_override
            .or(window_tiledata_selector_override)
            .unwrap_or_else(|| recomputed.pixel_value(cached.pixel_index));
        Some(BgOutputPixel::new(pixel.color, cgb_bg_attrs))
    }

    pub(in crate::ppu) fn current_transfer_selected_sprite_x(&self) -> Option<u8> {
        let current_transfer_x = self.runtime.bg_pipeline_state.current_transfer_x;
        (0..self.runtime.mode2_scan_state.selected_sprite_count())
            .filter(|&slot| !self.runtime.obj_pipeline_state.has_fetched(slot))
            .filter_map(|slot| self.runtime.mode2_scan_state.selected_sprite(slot))
            .find(|sprite| sprite_trigger_x(*sprite) == Some(current_transfer_x))
            .map(|sprite| sprite.x)
    }

    pub(in crate::ppu) fn startup_line_lead_sprite_x(&self) -> Option<u8> {
        (0..self.runtime.mode2_scan_state.selected_sprite_count())
            .filter_map(|slot| self.runtime.mode2_scan_state.selected_sprite(slot))
            .min_by_key(|sprite| sprite.x)
            .map(|sprite| sprite.x)
    }

    pub(in crate::ppu) fn scy_startup_line_lead_owner_window_open(&self) -> bool {
        self.current_transfer().is_some()
    }

    pub(in crate::ppu) fn scy_obj_phase_owner(&self) -> Option<PpuMode3ScyObjPhaseOwner> {
        if self.current_dot_has_pending_obj_hit() {
            return Some(PpuMode3ScyObjPhaseOwner::PendingHit {
                match_x: self.runtime.bg_pipeline_state.current_transfer_x,
            });
        }

        if self.obj_enabled()
            && self.runtime.obj_pipeline_state.fetch.stage != PpuObjFetcherStage::Idle
        {
            let sprite = self.runtime.obj_pipeline_state.fetch.sprite?;
            return Some(PpuMode3ScyObjPhaseOwner::ActiveFetch { sprite_x: sprite.x });
        }

        self.current_transfer_selected_sprite_x()
            .map(|sprite_x| PpuMode3ScyObjPhaseOwner::CurrentTransferSprite { sprite_x })
            .or_else(|| {
                if !self.scy_startup_line_lead_owner_window_open() {
                    return None;
                }
                self.startup_line_lead_sprite_x()
                    .map(|sprite_x| PpuMode3ScyObjPhaseOwner::StartupLineLead { sprite_x })
            })
    }

    pub(in crate::ppu) fn scy_obj_phase_policy(&self) -> Option<PpuMode3ScyObjPhasePolicy> {
        let phase_owner = self.scy_obj_phase_owner()?;
        let context = PpuMode3ScyObjPhaseContext {
            phase_owner,
            current_transfer_x: self.runtime.bg_pipeline_state.current_transfer_x,
            current_transfer: self.current_transfer(),
            bg_fetcher_stage: self.runtime.bg_pipeline_state.fetcher.stage,
            bg_fetcher_stage_dot: self.runtime.bg_pipeline_state.fetcher.stage_dot,
            bg_fifo_len: self.runtime.bg_pipeline_state.fifo.len(),
            obj_fetcher_stage: self.runtime.obj_pipeline_state.fetch.stage,
            obj_fetcher_stage_dot: self.runtime.obj_pipeline_state.fetch.stage_dot,
        };

        Some(PpuMode3ScyObjPhasePolicy::new(context))
    }

    pub(in crate::ppu) fn obj_enabled(&self) -> bool {
        self.mode3_register_latches().visible().obj_enabled()
    }
}
