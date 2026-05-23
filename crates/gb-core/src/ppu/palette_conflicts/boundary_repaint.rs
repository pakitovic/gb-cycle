use super::*;

impl Ppu {
    pub(in crate::ppu) fn record_dmg_bgp_cpu_commit_visible_write(
        &mut self,
        effect_kind: PpuDmgBgpCpuCommitEffectKind,
        transient_visible_x: u8,
        transient_palette: u8,
        repaint_visible_x: u8,
        value: u8,
    ) {
        if !self.uses_dmg_palette_live_write_model()
            || self.ly >= VISIBLE_SCANLINES
            || self.visible_output != PpuVisibleOutputState::Driving
        {
            return;
        }

        let transfer_lead_pixels = self.bg_pipeline_state.current_transfer_x.saturating_sub(
            self.bg_pipeline_state
                .visible_pixels_output
                .saturating_add(8),
        );
        self.dmg_panel_live_write_state
            .bgp_cpu_commit
            .current_line_writes
            .push(PpuDmgBgpCpuCommitWrite {
                effect_kind,
                transient_visible_x,
                transient_palette,
                repaint_visible_x,
                transfer_lead_pixels,
                value,
            });
    }

    pub(in crate::ppu) fn finalize_dmg_bgp_cpu_commit_scanline(&mut self) {
        if self.console_model.is_dmg_family()
            && self.ly < VISIBLE_SCANLINES
            && self.visible_output == PpuVisibleOutputState::Driving
        {
            if let Some(previous_ly) = self.previous_scanline_ly
                && previous_ly + 1 == self.ly
                && previous_ly % 8 == 7
                && self.ly.is_multiple_of(8)
                && (self
                    .dmg_panel_live_write_state
                    .bgp_cpu_commit
                    .current_line_writes
                    .iter()
                    .any(|write| {
                        write.effect_kind == PpuDmgBgpCpuCommitEffectKind::PipelineDelayed
                    })
                    || self.current_mode0_start_dot() > self.baseline_mode0_start_dot())
                && self.mode2_scan_state.selected_sprite_count() == 0
                && !self.bg_pipeline_state.window_started_this_line
                && !self
                    .dmg_panel_live_write_state
                    .bgp_cpu_commit
                    .current_line_writes
                    .is_empty()
                && self
                    .dmg_panel_live_write_state
                    .bgp_cpu_commit
                    .current_line_writes
                    != self
                        .dmg_panel_live_write_state
                        .bgp_cpu_commit
                        .previous_line_writes
            {
                self.recolor_previous_scanline_from_current_bgp_cpu_commit_writes(
                    previous_ly,
                    self.current_mode0_start_dot() > self.baseline_mode0_start_dot(),
                );
            }

            self.previous_scanline_mixed_pixels = self.current_scanline_mixed_pixels;
            self.previous_scanline_dmg_bg_forced_white = self.current_scanline_dmg_bg_forced_white;
            self.previous_scanline_ly = Some(self.ly);
            self.dmg_panel_live_write_state
                .bgp_cpu_commit
                .previous_line_start_palette = self
                .dmg_panel_live_write_state
                .bgp_cpu_commit
                .current_line_start_palette;
            self.dmg_panel_live_write_state
                .bgp_cpu_commit
                .previous_line_writes = self
                .dmg_panel_live_write_state
                .bgp_cpu_commit
                .current_line_writes
                .clone();
        } else {
            self.previous_scanline_ly = None;
            self.previous_scanline_dmg_bg_forced_white.fill(false);
            self.dmg_panel_live_write_state
                .bgp_cpu_commit
                .previous_line_start_palette = self
                .dmg_panel_live_write_state
                .bgp_cpu_commit
                .current_line_start_palette;
            self.dmg_panel_live_write_state
                .bgp_cpu_commit
                .previous_line_writes
                .clear();
        }
    }

    pub(in crate::ppu) fn recolor_previous_scanline_from_current_bgp_cpu_commit_writes(
        &mut self,
        previous_ly: u8,
        include_retroactive_panel_writes: bool,
    ) {
        let boundary_writes = self.dmg_bgp_cpu_commit_boundary_repaint_writes();
        let allow_zero_start_retroactive_panel_writes = include_retroactive_panel_writes
            && !self.previous_scanline_mixed_pixels[..DMG_PALETTE_RETROACTIVE_PIXELS]
                .iter()
                .any(|pixel| matches!(pixel.source, MixedPixelSource::Object { .. }));
        let earliest_pipeline_delayed_repaint_x = boundary_writes
            .iter()
            .find(|boundary| {
                boundary.write.effect_kind == PpuDmgBgpCpuCommitEffectKind::PipelineDelayed
            })
            .map(|boundary| boundary.write.repaint_visible_x);
        let row_start = previous_ly as usize * SCREEN_WIDTH;
        for x in 0..SCREEN_WIDTH {
            let palette = self.dmg_bgp_cpu_commit_palette_for_visible_x(
                self.dmg_panel_live_write_state
                    .bgp_cpu_commit
                    .previous_line_start_palette,
                &boundary_writes,
                x,
                include_retroactive_panel_writes,
                allow_zero_start_retroactive_panel_writes,
                earliest_pipeline_delayed_repaint_x,
            );
            self.recolor_bgwin_framebuffer_pixel_with_palette(row_start + x, palette);
            if self.previous_scanline_dmg_bg_forced_white[x] {
                continue;
            }
            let mixed_pixel = self.previous_scanline_mixed_pixels[x];
            let panel_pixel = self.map_mixed_pixel_to_panel_shade_with_palette_override(
                mixed_pixel,
                PpuPaletteRegister::Bgp,
                palette,
            );
            self.write_framebuffer_palette_override_pixel(
                row_start + x,
                x,
                mixed_pixel,
                panel_pixel,
                PpuPaletteRegister::Bgp,
                palette,
            );
        }
    }

    pub(in crate::ppu) fn dmg_bgp_cpu_commit_palette_for_visible_x(
        &self,
        start_palette: u8,
        writes: &[PpuDmgBgpBoundaryRepaintWrite],
        x: usize,
        include_retroactive_panel_writes: bool,
        allow_zero_start_retroactive_panel_writes: bool,
        earliest_pipeline_delayed_repaint_x: Option<u8>,
    ) -> u8 {
        let mut palette = start_palette;
        let has_pipeline_delayed = writes.iter().any(|boundary| {
            boundary.write.effect_kind == PpuDmgBgpCpuCommitEffectKind::PipelineDelayed
        });
        let row_uses_delayed_selected_current_retroactive_commit = has_pipeline_delayed
            && writes
                .iter()
                .find(|boundary| {
                    boundary.selected_current
                        && boundary.write.effect_kind
                            == PpuDmgBgpCpuCommitEffectKind::RetroactivePanel
                })
                .is_some_and(|boundary| boundary.write.transient_visible_x <= 8);
        for boundary in writes {
            let write = boundary.write;
            match write.effect_kind {
                PpuDmgBgpCpuCommitEffectKind::PipelineDelayed => {
                    let repaint_threshold_x = write
                        .repaint_visible_x
                        .saturating_add(write.transfer_lead_pixels);
                    if x < usize::from(repaint_threshold_x) {
                        break;
                    }

                    if x == usize::from(repaint_threshold_x) {
                        palette = write.transient_palette;
                    } else {
                        palette = write.value;
                    }
                }
                PpuDmgBgpCpuCommitEffectKind::RetroactivePanel => {
                    let include_write = include_retroactive_panel_writes
                        && (write.transient_visible_x > 0
                            || allow_zero_start_retroactive_panel_writes)
                        && earliest_pipeline_delayed_repaint_x
                            .is_none_or(|earliest| write.transient_visible_x >= earliest);
                    if !include_write {
                        continue;
                    }

                    let transient_x = usize::from(write.transient_visible_x);
                    let final_x = if boundary.selected_current
                        && row_uses_delayed_selected_current_retroactive_commit
                    {
                        usize::from(write.repaint_visible_x.saturating_sub(3))
                    } else {
                        transient_x
                    };
                    if x < transient_x {
                        break;
                    }

                    if x == transient_x {
                        palette = write.transient_palette;
                        continue;
                    }

                    if x >= final_x {
                        palette = write.value;
                    }
                }
                PpuDmgBgpCpuCommitEffectKind::CurrentDotTransient => {
                    let transient_x = usize::from(write.transient_visible_x);
                    let final_x = usize::from(write.repaint_visible_x);
                    if x < transient_x {
                        break;
                    }

                    if x == transient_x {
                        palette = write.transient_palette;
                        continue;
                    }

                    if x >= final_x {
                        palette = write.value;
                    }
                }
            }
        }
        palette
    }

    fn dmg_bgp_cpu_commit_boundary_repaint_writes(&self) -> Vec<PpuDmgBgpBoundaryRepaintWrite> {
        fn repaint_onset_x(write: PpuDmgBgpCpuCommitWrite) -> u8 {
            match write.effect_kind {
                PpuDmgBgpCpuCommitEffectKind::PipelineDelayed => write.repaint_visible_x,
                PpuDmgBgpCpuCommitEffectKind::RetroactivePanel
                | PpuDmgBgpCpuCommitEffectKind::CurrentDotTransient => write.transient_visible_x,
            }
        }

        if self
            .dmg_panel_live_write_state
            .bgp_cpu_commit
            .current_line_writes
            .len()
            != self
                .dmg_panel_live_write_state
                .bgp_cpu_commit
                .previous_line_writes
                .len()
        {
            return self
                .dmg_panel_live_write_state
                .bgp_cpu_commit
                .current_line_writes
                .iter()
                .copied()
                .map(|write| PpuDmgBgpBoundaryRepaintWrite {
                    write,
                    selected_current: true,
                })
                .collect();
        }

        self.dmg_panel_live_write_state
            .bgp_cpu_commit
            .current_line_writes
            .iter()
            .copied()
            .zip(
                self.dmg_panel_live_write_state
                    .bgp_cpu_commit
                    .previous_line_writes
                    .iter()
                    .copied(),
            )
            .map(|(current, previous)| {
                if repaint_onset_x(current) >= repaint_onset_x(previous) {
                    PpuDmgBgpBoundaryRepaintWrite {
                        write: current,
                        selected_current: true,
                    }
                } else {
                    PpuDmgBgpBoundaryRepaintWrite {
                        write: previous,
                        selected_current: false,
                    }
                }
            })
            .collect()
    }
}
