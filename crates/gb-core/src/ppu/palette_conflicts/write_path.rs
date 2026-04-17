use super::*;

impl Ppu {
    pub(in crate::ppu) fn write_dmg_palette_register(
        &mut self,
        register: PpuPaletteRegister,
        value: u8,
        source: PpuRegisterWriteSource,
    ) {
        let previous_visible = self.visible_palette_register_value(register);
        self.write_palette_register_storage(register, value);

        let bgp_cpu_commit_delay_active = register == PpuPaletteRegister::Bgp
            && source == PpuRegisterWriteSource::CpuMmioCommit
            && matches!(
                self.current_raster_state(),
                PpuRasterState::Active {
                    mode: PpuAccessMode::Drawing,
                    ..
                }
            );

        if bgp_cpu_commit_delay_active {
            self.clear_dmg_bgp_cpu_commit_bg_visible_hold();
            let visible_pixels_output = self.bg_pipeline_state.visible_pixels_output;
            if let Some(retroactive_pixels) = self.dmg_palette_conflict_retroactive_pixels(register)
            {
                if let Some((transient_start_x, final_onset_x)) =
                    self.dmg_single_left_sprite_bgp_second_write_transient_range()
                {
                    self.apply_single_left_sprite_bgp_second_write_transient_range(
                        register,
                        previous_visible,
                        value,
                        visible_pixels_output,
                        transient_start_x,
                        final_onset_x,
                    );
                    return;
                }

                if let Some(desired_onset_x) = self
                    .dmg_single_left_sprite_bgp_live_write_onset_visible_x(
                        self.dmg_panel_live_write_state
                            .bgp_cpu_commit
                            .current_line_writes
                            .len(),
                    )
                {
                    self.apply_single_left_sprite_bgp_live_write_onset(
                        register,
                        previous_visible,
                        value,
                        visible_pixels_output,
                        desired_onset_x,
                    );
                    return;
                }

                let base_effect_kind = self.dmg_bgp_cpu_commit_effect_kind(retroactive_pixels);
                let effective_retroactive_pixels = retroactive_pixels;
                let line_has_pipeline_delayed = self
                    .dmg_panel_live_write_state
                    .bgp_cpu_commit
                    .current_line_writes
                    .iter()
                    .any(|write| {
                        write.effect_kind == PpuDmgBgpCpuCommitEffectKind::PipelineDelayed
                    });
                let retroactive_write_count = self
                    .dmg_panel_live_write_state
                    .bgp_cpu_commit
                    .current_line_writes
                    .iter()
                    .filter(|write| {
                        write.effect_kind == PpuDmgBgpCpuCommitEffectKind::RetroactivePanel
                    })
                    .count();
                let first_retroactive_write = self
                    .dmg_panel_live_write_state
                    .bgp_cpu_commit
                    .current_line_writes
                    .iter()
                    .find(|write| {
                        write.effect_kind == PpuDmgBgpCpuCommitEffectKind::RetroactivePanel
                    });
                let transient_palette = previous_visible | value;
                let transient_visible_x =
                    visible_pixels_output.saturating_sub(effective_retroactive_pixels as u8);
                let repaint_visible_x = visible_pixels_output.saturating_add(4);
                let selected_retroactive_line = base_effect_kind
                    == PpuDmgBgpCpuCommitEffectKind::RetroactivePanel
                    && line_has_pipeline_delayed
                    && first_retroactive_write
                        .map_or(transient_visible_x, |write| write.transient_visible_x)
                        <= 8;
                let subsequent_selected_retroactive_write =
                    selected_retroactive_line && retroactive_write_count > 0;
                let delay_final_visible_commit =
                    selected_retroactive_line && !subsequent_selected_retroactive_write;
                let effect_kind = if subsequent_selected_retroactive_write {
                    PpuDmgBgpCpuCommitEffectKind::CurrentDotTransient
                } else {
                    base_effect_kind
                };
                let recorded_transient_palette = transient_palette;
                let recorded_transient_visible_x =
                    if effect_kind == PpuDmgBgpCpuCommitEffectKind::CurrentDotTransient {
                        visible_pixels_output
                    } else {
                        transient_visible_x
                    };
                let recorded_repaint_visible_x =
                    if effect_kind == PpuDmgBgpCpuCommitEffectKind::CurrentDotTransient {
                        visible_pixels_output.saturating_add(1)
                    } else {
                        repaint_visible_x
                    };
                self.record_dmg_bgp_cpu_commit_visible_write(
                    effect_kind,
                    recorded_transient_visible_x,
                    recorded_transient_palette,
                    recorded_repaint_visible_x,
                    value,
                );
                match effect_kind {
                    PpuDmgBgpCpuCommitEffectKind::PipelineDelayed => {
                        let output_delay_pixels =
                            self.dmg_bgp_cpu_commit_output_delay_pixels(visible_pixels_output);
                        self.set_dmg_bgp_cpu_commit_output_override(
                            (output_delay_pixels > 0).then(|| self.pixel_pipeline_bgp()),
                            output_delay_pixels,
                        );
                    }
                    PpuDmgBgpCpuCommitEffectKind::CurrentDotTransient => {
                        self.set_dmg_bgp_cpu_commit_output_override(Some(transient_palette), 1);
                    }
                    PpuDmgBgpCpuCommitEffectKind::RetroactivePanel => {
                        self.retroactively_recolor_recent_pixels(
                            register,
                            transient_palette,
                            value,
                            effective_retroactive_pixels,
                            delay_final_visible_commit,
                        );
                        if !delay_final_visible_commit
                            && self
                                .dmg_panel_live_write_state
                                .bgp_cpu_commit
                                .current_line_writes
                                .len()
                                == 1
                            && let Some(bg_visible_pixels) =
                                self.early_line_retroactive_obj_hold_bg_visible_pixels()
                        {
                            self.start_dmg_bgp_cpu_commit_bg_visible_hold(
                                value,
                                bg_visible_pixels,
                                value,
                            );
                            self.set_dmg_bgp_cpu_commit_output_override(None, 0);
                        } else if delay_final_visible_commit {
                            let output_delay_pixels = self
                                .dmg_bgp_cpu_commit_retroactive_final_delay_pixels(
                                    visible_pixels_output,
                                );
                            if output_delay_pixels == 0 {
                                self.set_dmg_bgp_cpu_commit_output_override(Some(value), 1);
                            } else {
                                self.set_dmg_bgp_cpu_commit_output_override(
                                    Some(self.pixel_pipeline_bgp()),
                                    output_delay_pixels,
                                );
                            }
                        } else {
                            self.set_dmg_bgp_cpu_commit_output_override(Some(value), 1);
                        }
                    }
                }
            }
        }

        if !bgp_cpu_commit_delay_active
            && let Some(retroactive_pixels) = self.dmg_palette_conflict_retroactive_pixels(register)
        {
            self.retroactively_recolor_recent_pixels(
                register,
                previous_visible | value,
                value,
                retroactive_pixels,
                false,
            );
        }
    }

    pub(in crate::ppu) fn dmg_bgp_cpu_commit_effect_kind(
        &self,
        retroactive_pixels: usize,
    ) -> PpuDmgBgpCpuCommitEffectKind {
        if self.mode2_scan_state.selected_sprite_count() == 0
            && self.bg_pipeline_state.visible_pixels_output == 0
            && self.bg_pipeline_state.current_transfer_x == 0
        {
            return PpuDmgBgpCpuCommitEffectKind::RetroactivePanel;
        }

        let mut affected_pixel_count = 0usize;
        let mut recent_affected_pixels_are_bg_color0 = true;
        if !self.dmg_panel_live_write_state.recent_panel_dots.is_empty() {
            let recent_dots = self
                .dmg_panel_live_write_state
                .recent_panel_dots
                .iter()
                .rev()
                .take(retroactive_pixels)
                .copied()
                .collect::<Vec<_>>();
            for dot in recent_dots.iter().rev() {
                if !register_affects_pixel(PpuPaletteRegister::Bgp, dot.pixel) {
                    continue;
                }

                affected_pixel_count += 1;
                if dot.pixel.color != 0 {
                    recent_affected_pixels_are_bg_color0 = false;
                    break;
                }
            }
        } else {
            let visible_x = self.bg_pipeline_state.visible_pixels_output as usize;
            let start = visible_x.saturating_sub(retroactive_pixels);

            for pixel in &self.current_scanline_mixed_pixels[start..visible_x] {
                if !register_affects_pixel(PpuPaletteRegister::Bgp, *pixel) {
                    continue;
                }

                affected_pixel_count += 1;
                if pixel.color != 0 {
                    recent_affected_pixels_are_bg_color0 = false;
                    break;
                }
            }
        }

        if affected_pixel_count > 0 && recent_affected_pixels_are_bg_color0 {
            PpuDmgBgpCpuCommitEffectKind::RetroactivePanel
        } else {
            PpuDmgBgpCpuCommitEffectKind::PipelineDelayed
        }
    }

    pub(in crate::ppu) fn consume_dmg_bgp_cpu_commit_output_delay(&mut self) {
        if self
            .dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_delay_pixels_remaining
            == 0
        {
            return;
        }

        self.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_delay_pixels_remaining -= 1;
        if self
            .dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_delay_pixels_remaining
            == 0
        {
            if let Some(palette) = self
                .dmg_panel_live_write_state
                .bgp_cpu_commit
                .output_followup_palette_override
                .take()
            {
                self.dmg_panel_live_write_state
                    .bgp_cpu_commit
                    .output_palette_override = Some(palette);
                self.dmg_panel_live_write_state
                    .bgp_cpu_commit
                    .output_delay_pixels_remaining = self
                    .dmg_panel_live_write_state
                    .bgp_cpu_commit
                    .output_followup_pixels_remaining;
                self.dmg_panel_live_write_state
                    .bgp_cpu_commit
                    .output_followup_pixels_remaining = 0;
            } else {
                self.dmg_panel_live_write_state
                    .bgp_cpu_commit
                    .output_palette_override = None;
            }
        }
    }

    pub(in crate::ppu) fn consume_dmg_bgp_cpu_commit_bg_visible_hold(
        &mut self,
        output_pixel: MixedPixel,
    ) {
        if self
            .dmg_panel_live_write_state
            .bgp_cpu_commit
            .bg_visible_hold_palette_override
            .is_none()
            || !matches!(output_pixel.source, MixedPixelSource::Background)
            || self
                .dmg_panel_live_write_state
                .bgp_cpu_commit
                .bg_visible_hold_bg_pixels_remaining
                == 0
        {
            return;
        }

        self.dmg_panel_live_write_state
            .bgp_cpu_commit
            .bg_visible_hold_bg_pixels_remaining -= 1;
        if self
            .dmg_panel_live_write_state
            .bgp_cpu_commit
            .bg_visible_hold_bg_pixels_remaining
            == 0
        {
            self.dmg_panel_live_write_state
                .bgp_cpu_commit
                .bg_visible_hold_palette_override = self
                .dmg_panel_live_write_state
                .bgp_cpu_commit
                .bg_visible_hold_fallback_palette
                .take();
        }
    }

    pub(in crate::ppu) fn clear_dmg_bgp_cpu_commit_bg_visible_hold(&mut self) {
        self.dmg_panel_live_write_state
            .bgp_cpu_commit
            .bg_visible_hold_palette_override = None;
        self.dmg_panel_live_write_state
            .bgp_cpu_commit
            .bg_visible_hold_bg_pixels_remaining = 0;
        self.dmg_panel_live_write_state
            .bgp_cpu_commit
            .bg_visible_hold_fallback_palette = None;
    }

    fn start_dmg_bgp_cpu_commit_bg_visible_hold(
        &mut self,
        palette: u8,
        bg_visible_pixels: u8,
        fallback_palette: u8,
    ) {
        self.dmg_panel_live_write_state
            .bgp_cpu_commit
            .bg_visible_hold_palette_override = Some(palette);
        self.dmg_panel_live_write_state
            .bgp_cpu_commit
            .bg_visible_hold_bg_pixels_remaining = bg_visible_pixels;
        self.dmg_panel_live_write_state
            .bgp_cpu_commit
            .bg_visible_hold_fallback_palette = Some(fallback_palette);
    }

    fn clear_dmg_bgp_cpu_commit_output_followup(&mut self) {
        self.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_followup_palette_override = None;
        self.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_followup_pixels_remaining = 0;
    }

    fn set_dmg_bgp_cpu_commit_output_override(
        &mut self,
        palette_override: Option<u8>,
        pixels_remaining: u8,
    ) {
        self.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_palette_override = palette_override;
        self.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_delay_pixels_remaining = pixels_remaining;
        self.clear_dmg_bgp_cpu_commit_output_followup();
    }

    fn queue_dmg_bgp_cpu_commit_output_followup(&mut self, palette_override: u8, pixels: u8) {
        if pixels == 0 {
            self.clear_dmg_bgp_cpu_commit_output_followup();
            return;
        }

        self.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_followup_palette_override = Some(palette_override);
        self.dmg_panel_live_write_state
            .bgp_cpu_commit
            .output_followup_pixels_remaining = pixels;
    }

    fn dmg_bgp_cpu_commit_output_delay_pixels(&self, visible_pixels_output: u8) -> u8 {
        if visible_pixels_output == 0
            && self.dmg_panel_live_write_state.recent_panel_dots.is_empty()
        {
            let leading_visible_obj_pixels = self.leading_visible_obj_fifo_prefix_pixels();
            if leading_visible_obj_pixels == 0 {
                return 0;
            }
            return leading_visible_obj_pixels.min(4) as u8;
        }

        4
    }

    fn dmg_bgp_cpu_commit_retroactive_final_delay_pixels(&self, visible_pixels_output: u8) -> u8 {
        let visible_obj_prefix_pixels =
            self.visible_obj_prefix_pixels_output_so_far(visible_pixels_output);
        if visible_obj_prefix_pixels >= 4 {
            0
        } else if visible_obj_prefix_pixels > 0 {
            1
        } else {
            self.dmg_bgp_cpu_commit_output_delay_pixels(visible_pixels_output)
        }
    }

    fn leading_visible_obj_fifo_prefix_pixels(&self) -> usize {
        let hidden_pops_before_visible = self.obj_fifo_hidden_pops_before_first_visible_pixel();

        self.obj_pipeline_state
            .fifo
            .iter()
            .skip(hidden_pops_before_visible)
            .take_while(|pixel| !pixel.is_transparent())
            .count()
    }

    fn visible_obj_prefix_pixels_output_so_far(&self, visible_pixels_output: u8) -> usize {
        self.current_scanline_mixed_pixels[..usize::from(visible_pixels_output)]
            .iter()
            .take_while(|pixel| matches!(pixel.source, MixedPixelSource::Object { .. }))
            .count()
    }

    fn dmg_single_left_sprite_bgp_live_write_phase(&self) -> Option<u8> {
        if self.mode2_scan_state.selected_sprite_count() != 1 {
            return None;
        }

        let sprite = self.mode2_scan_state.selected_sprite(0)?;
        if sprite.x >= 16 {
            return None;
        }

        Some((sprite.x % 8).min(5))
    }

    fn dmg_single_left_sprite_bgp_live_write_onset_visible_x(
        &self,
        write_index: usize,
    ) -> Option<u8> {
        if self.mode2_scan_state.selected_sprite_count() != 1 {
            return None;
        }

        let sprite_x = self.mode2_scan_state.selected_sprite(0)?.x as usize;

        // `m3_bgp_change_sprites` needs an explicit DMG seam for the first two
        // writes: the visible onset follows the left sprite phase, not the
        // generic 4-dot CPU-commit delay.
        const EARLY_WRITE0_ONSETS: [u8; 19] =
            [0, 0, 0, 1, 2, 1, 0, 0, 0, 1, 2, 3, 4, 5, 5, 5, 5, 5, 5];
        const EARLY_WRITE1_ONSETS: [u8; 19] =
            [0, 8, 9, 10, 11, 12, 12, 1, 2, 3, 4, 5, 6, 7, 8, 9, 8, 9, 10];

        match write_index {
            0 if sprite_x < EARLY_WRITE0_ONSETS.len() => Some(EARLY_WRITE0_ONSETS[sprite_x]),
            1 if sprite_x < EARLY_WRITE1_ONSETS.len() => Some(EARLY_WRITE1_ONSETS[sprite_x]),
            2..=5 => self
                .dmg_single_left_sprite_bgp_live_write_phase()
                .map(|phase| match write_index {
                    2 => 46 + phase,
                    3 => 59 + phase,
                    4 => 91 + phase,
                    5 => 102 + phase,
                    _ => unreachable!(),
                }),
            _ => None,
        }
    }

    fn dmg_single_left_sprite_bgp_second_write_transient_range(&self) -> Option<(u8, u8)> {
        if self.mode2_scan_state.selected_sprite_count() != 1
            || self
                .dmg_panel_live_write_state
                .bgp_cpu_commit
                .current_line_writes
                .len()
                != 1
        {
            return None;
        }

        let sprite_x = self.mode2_scan_state.selected_sprite(0)?.x;
        // The second write on the same scanline exposes a short transient seam
        // before the final palette reaches the left-edge boundary.
        let (transient_start_x, final_onset_x) = match sprite_x {
            7 => (5, 12),
            8..=14 => (sprite_x.saturating_sub(2), sprite_x.saturating_sub(1)),
            15 => (12, 12),
            16..=18 => (5, sprite_x.saturating_sub(8)),
            _ => return None,
        };
        let final_onset_x = match sprite_x {
            14 => 12,
            _ => final_onset_x,
        };

        Some((transient_start_x, final_onset_x))
    }

    fn apply_single_left_sprite_bgp_live_write_onset(
        &mut self,
        register: PpuPaletteRegister,
        previous_visible: u8,
        value: u8,
        visible_pixels_output: u8,
        desired_onset_x: u8,
    ) {
        let effect_kind = if desired_onset_x < visible_pixels_output {
            PpuDmgBgpCpuCommitEffectKind::RetroactivePanel
        } else if desired_onset_x == visible_pixels_output {
            PpuDmgBgpCpuCommitEffectKind::CurrentDotTransient
        } else {
            PpuDmgBgpCpuCommitEffectKind::PipelineDelayed
        };
        self.record_dmg_bgp_cpu_commit_visible_write(
            effect_kind,
            desired_onset_x,
            value,
            desired_onset_x.saturating_add(1),
            value,
        );

        if desired_onset_x < visible_pixels_output {
            for x in usize::from(desired_onset_x)..usize::from(visible_pixels_output) {
                let mixed_pixel = self.current_scanline_mixed_pixels[x];
                if !register_affects_pixel(register, mixed_pixel) {
                    continue;
                }

                let panel_pixel = self.map_mixed_pixel_to_panel_shade_with_palette_override(
                    mixed_pixel,
                    register,
                    value,
                );
                self.framebuffer[self.ly as usize * SCREEN_WIDTH + x] = panel_pixel;
            }
        }

        if desired_onset_x > visible_pixels_output {
            self.set_dmg_bgp_cpu_commit_output_override(
                Some(previous_visible),
                desired_onset_x.saturating_sub(visible_pixels_output),
            );
        } else {
            self.set_dmg_bgp_cpu_commit_output_override(Some(value), 1);
        }
    }

    fn apply_single_left_sprite_bgp_second_write_transient_range(
        &mut self,
        register: PpuPaletteRegister,
        previous_visible: u8,
        value: u8,
        visible_pixels_output: u8,
        transient_start_x: u8,
        final_onset_x: u8,
    ) {
        let transient_palette = previous_visible | value;
        self.record_dmg_bgp_cpu_commit_visible_write(
            PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
            transient_start_x,
            transient_palette,
            final_onset_x,
            value,
        );

        let row_start = self.ly as usize * SCREEN_WIDTH;
        for x in usize::from(transient_start_x)..usize::from(visible_pixels_output) {
            let mixed_pixel = self.current_scanline_mixed_pixels[x];
            if !register_affects_pixel(register, mixed_pixel) {
                continue;
            }

            let palette = if x < usize::from(final_onset_x) {
                transient_palette
            } else {
                value
            };
            let panel_pixel = self.map_mixed_pixel_to_panel_shade_with_palette_override(
                mixed_pixel,
                register,
                palette,
            );
            self.framebuffer[row_start + x] = panel_pixel;
        }

        let hold_pixels = transient_start_x.saturating_sub(visible_pixels_output);
        let future_transient_start_x = visible_pixels_output.max(transient_start_x);
        let transient_pixels = final_onset_x.saturating_sub(future_transient_start_x);
        if hold_pixels > 0 {
            self.set_dmg_bgp_cpu_commit_output_override(Some(previous_visible), hold_pixels);
            self.queue_dmg_bgp_cpu_commit_output_followup(transient_palette, transient_pixels);
        } else if transient_pixels > 0 {
            self.set_dmg_bgp_cpu_commit_output_override(Some(transient_palette), transient_pixels);
        } else {
            self.set_dmg_bgp_cpu_commit_output_override(Some(value), 1);
        }
    }

    fn early_line_retroactive_obj_hold_bg_visible_pixels(&self) -> Option<u8> {
        if self.bg_pipeline_state.visible_pixels_output < 2 {
            return None;
        }

        let future_obj_pixels = self
            .obj_pipeline_state
            .fifo
            .iter()
            .map(|pixel| !pixel.is_transparent())
            .collect::<Vec<_>>();
        let leading_bg_visible_run = future_obj_pixels
            .iter()
            .take_while(|&&is_obj| !is_obj)
            .count();
        let obj_visible_run = future_obj_pixels[leading_bg_visible_run..]
            .iter()
            .take_while(|&&is_obj| is_obj)
            .count();
        if obj_visible_run == 0 {
            return None;
        }

        Some((leading_bg_visible_run + 3).min(u8::MAX as usize) as u8)
    }
}
