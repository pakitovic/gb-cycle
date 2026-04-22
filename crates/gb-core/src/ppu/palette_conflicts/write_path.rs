use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DmgPaletteWritePlan {
    BgpCpuCommit(DmgBgpCpuCommitWritePlan),
    RetroactiveRecolor(DmgRetroactivePaletteWritePlan),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DmgRetroactivePaletteWritePlan {
    register: PpuPaletteRegister,
    transient_palette: u8,
    final_palette: u8,
    retroactive_pixels: usize,
    delay_final_palette: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DmgBgpCpuCommitWriteContext {
    register: PpuPaletteRegister,
    previous_visible: u8,
    value: u8,
    visible_pixels_output: u8,
    retroactive_pixels: usize,
    write_index: usize,
}

impl DmgBgpCpuCommitWriteContext {
    const fn transient_palette(self) -> u8 {
        self.previous_visible | self.value
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DmgBgpCpuCommitWriteCase {
    SingleLeftSpriteSecondWriteTransient {
        transient_start_x: u8,
        final_onset_x: u8,
    },
    OnsetAtVisibleX {
        desired_onset_x: u8,
    },
    WindowRestartBackdate {
        retroactive_pixels: usize,
    },
    Generic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DmgBgpCpuCommitWritePlan {
    SingleLeftSpriteSecondWriteTransient {
        context: DmgBgpCpuCommitWriteContext,
        transient_start_x: u8,
        final_onset_x: u8,
    },
    OnsetAtVisibleX {
        context: DmgBgpCpuCommitWriteContext,
        desired_onset_x: u8,
    },
    WindowRestartBackdate {
        context: DmgBgpCpuCommitWriteContext,
        retroactive_pixels: usize,
    },
    Generic(DmgBgpCpuCommitGenericPlan),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DmgBgpCpuCommitGenericPlan {
    recorded_write: PpuDmgBgpCpuCommitWrite,
    recolor: Option<DmgRetroactivePaletteWritePlan>,
    output_override: DmgBgpCpuCommitOutputOverridePlan,
    bg_visible_hold: Option<DmgBgpCpuCommitBgVisibleHoldPlan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DmgBgpCpuCommitOutputOverridePlan {
    palette_override: Option<u8>,
    pixels_remaining: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DmgBgpCpuCommitBgVisibleHoldPlan {
    palette: u8,
    bg_visible_pixels: u8,
    fallback_palette: u8,
}

impl Ppu {
    pub(in crate::ppu) fn write_dmg_palette_register(
        &mut self,
        register: PpuPaletteRegister,
        value: u8,
        source: PpuRegisterWriteSource,
    ) {
        let previous_visible = self.visible_palette_register_value(register);
        self.write_palette_register_storage(register, value);

        let Some(plan) = self.plan_dmg_palette_write(register, previous_visible, value, source)
        else {
            return;
        };
        self.apply_dmg_palette_write_plan(plan);
    }

    fn plan_dmg_palette_write(
        &self,
        register: PpuPaletteRegister,
        previous_visible: u8,
        value: u8,
        source: PpuRegisterWriteSource,
    ) -> Option<DmgPaletteWritePlan> {
        if let Some(plan) =
            self.plan_dmg_bgp_cpu_commit_write(register, previous_visible, value, source)
        {
            return Some(DmgPaletteWritePlan::BgpCpuCommit(plan));
        }

        let retroactive_pixels = self.dmg_palette_conflict_retroactive_pixels(register)?;
        Some(DmgPaletteWritePlan::RetroactiveRecolor(
            DmgRetroactivePaletteWritePlan {
                register,
                transient_palette: previous_visible | value,
                final_palette: value,
                retroactive_pixels,
                delay_final_palette: false,
            },
        ))
    }

    fn plan_dmg_bgp_cpu_commit_write(
        &self,
        register: PpuPaletteRegister,
        previous_visible: u8,
        value: u8,
        source: PpuRegisterWriteSource,
    ) -> Option<DmgBgpCpuCommitWritePlan> {
        if register != PpuPaletteRegister::Bgp
            || source != PpuRegisterWriteSource::CpuMmioCommit
            || !matches!(
                self.current_raster_state(),
                PpuRasterState::Active {
                    mode: PpuAccessMode::Drawing,
                    ..
                }
            )
        {
            return None;
        }

        let retroactive_pixels = self.dmg_palette_conflict_retroactive_pixels(register)?;
        let context = DmgBgpCpuCommitWriteContext {
            register,
            previous_visible,
            value,
            visible_pixels_output: self.bg_pipeline_state.visible_pixels_output,
            retroactive_pixels,
            write_index: self
                .dmg_panel_live_write_state
                .bgp_cpu_commit
                .current_line_writes
                .len(),
        };
        let write_case = self.classify_dmg_bgp_cpu_commit_write_case(context);
        Some(self.build_dmg_bgp_cpu_commit_write_plan(context, write_case))
    }

    fn classify_dmg_bgp_cpu_commit_write_case(
        &self,
        context: DmgBgpCpuCommitWriteContext,
    ) -> DmgBgpCpuCommitWriteCase {
        if let Some((transient_start_x, final_onset_x)) =
            self.dmg_single_left_sprite_bgp_second_write_transient_range()
        {
            return DmgBgpCpuCommitWriteCase::SingleLeftSpriteSecondWriteTransient {
                transient_start_x,
                final_onset_x,
            };
        }

        if let Some(desired_onset_x) = self
            .dmg_single_left_sprite_bgp_late_black_pulse_onset_visible_x(
                context.previous_visible,
                context.value,
                context.visible_pixels_output,
                context.write_index,
            )
        {
            return DmgBgpCpuCommitWriteCase::OnsetAtVisibleX { desired_onset_x };
        }

        if let Some(desired_onset_x) = self.dmg_single_left_sprite_bgp_live_write_onset_visible_x(
            context.write_index,
            context.visible_pixels_output,
        ) {
            return DmgBgpCpuCommitWriteCase::OnsetAtVisibleX { desired_onset_x };
        }

        if let Some(retroactive_pixels) =
            self.dmg_window_restart_bgp_backdate_pixels(context.visible_pixels_output)
        {
            return DmgBgpCpuCommitWriteCase::WindowRestartBackdate { retroactive_pixels };
        }

        if let Some(desired_onset_x) = self.dmg_window_restart_bgp_second_write_onset_visible_x(
            context.previous_visible,
            context.visible_pixels_output,
        ) {
            return DmgBgpCpuCommitWriteCase::OnsetAtVisibleX { desired_onset_x };
        }

        DmgBgpCpuCommitWriteCase::Generic
    }

    fn build_dmg_bgp_cpu_commit_write_plan(
        &self,
        context: DmgBgpCpuCommitWriteContext,
        write_case: DmgBgpCpuCommitWriteCase,
    ) -> DmgBgpCpuCommitWritePlan {
        match write_case {
            DmgBgpCpuCommitWriteCase::SingleLeftSpriteSecondWriteTransient {
                transient_start_x,
                final_onset_x,
            } => DmgBgpCpuCommitWritePlan::SingleLeftSpriteSecondWriteTransient {
                context,
                transient_start_x,
                final_onset_x,
            },
            DmgBgpCpuCommitWriteCase::OnsetAtVisibleX { desired_onset_x } => {
                DmgBgpCpuCommitWritePlan::OnsetAtVisibleX {
                    context,
                    desired_onset_x,
                }
            }
            DmgBgpCpuCommitWriteCase::WindowRestartBackdate { retroactive_pixels } => {
                DmgBgpCpuCommitWritePlan::WindowRestartBackdate {
                    context,
                    retroactive_pixels,
                }
            }
            DmgBgpCpuCommitWriteCase::Generic => DmgBgpCpuCommitWritePlan::Generic(
                self.build_dmg_bgp_cpu_commit_generic_plan(context),
            ),
        }
    }

    fn build_dmg_bgp_cpu_commit_generic_plan(
        &self,
        context: DmgBgpCpuCommitWriteContext,
    ) -> DmgBgpCpuCommitGenericPlan {
        let base_effect_kind = self.dmg_bgp_cpu_commit_effect_kind(context.retroactive_pixels);
        let line_has_pipeline_delayed = self
            .dmg_panel_live_write_state
            .bgp_cpu_commit
            .current_line_writes
            .iter()
            .any(|write| write.effect_kind == PpuDmgBgpCpuCommitEffectKind::PipelineDelayed);
        let retroactive_write_count = self
            .dmg_panel_live_write_state
            .bgp_cpu_commit
            .current_line_writes
            .iter()
            .filter(|write| write.effect_kind == PpuDmgBgpCpuCommitEffectKind::RetroactivePanel)
            .count();
        let first_retroactive_write = self
            .dmg_panel_live_write_state
            .bgp_cpu_commit
            .current_line_writes
            .iter()
            .find(|write| write.effect_kind == PpuDmgBgpCpuCommitEffectKind::RetroactivePanel);
        let transient_palette = context.transient_palette();
        let transient_visible_x = context
            .visible_pixels_output
            .saturating_sub(context.retroactive_pixels as u8);
        let repaint_visible_x = context.visible_pixels_output.saturating_add(4);
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
        let recorded_write = PpuDmgBgpCpuCommitWrite {
            effect_kind,
            transient_visible_x: if effect_kind == PpuDmgBgpCpuCommitEffectKind::CurrentDotTransient
            {
                context.visible_pixels_output
            } else {
                transient_visible_x
            },
            transient_palette,
            repaint_visible_x: if effect_kind == PpuDmgBgpCpuCommitEffectKind::CurrentDotTransient {
                context.visible_pixels_output.saturating_add(1)
            } else {
                repaint_visible_x
            },
            transfer_lead_pixels: self.bg_pipeline_state.current_transfer_x.saturating_sub(
                self.bg_pipeline_state
                    .visible_pixels_output
                    .saturating_add(8),
            ),
            value: context.value,
        };
        let recolor = (effect_kind == PpuDmgBgpCpuCommitEffectKind::RetroactivePanel).then_some(
            DmgRetroactivePaletteWritePlan {
                register: context.register,
                transient_palette,
                final_palette: context.value,
                retroactive_pixels: context.retroactive_pixels,
                delay_final_palette: delay_final_visible_commit,
            },
        );
        let bg_visible_hold = if effect_kind == PpuDmgBgpCpuCommitEffectKind::RetroactivePanel
            && !delay_final_visible_commit
            && context.write_index == 0
        {
            self.early_line_retroactive_obj_hold_bg_visible_pixels()
                .map(|bg_visible_pixels| DmgBgpCpuCommitBgVisibleHoldPlan {
                    palette: context.value,
                    bg_visible_pixels,
                    fallback_palette: context.value,
                })
        } else {
            None
        };
        let output_override = match effect_kind {
            PpuDmgBgpCpuCommitEffectKind::PipelineDelayed => {
                let output_delay_pixels =
                    self.dmg_bgp_cpu_commit_output_delay_pixels(context.visible_pixels_output);
                DmgBgpCpuCommitOutputOverridePlan {
                    palette_override: (output_delay_pixels > 0).then(|| self.pixel_pipeline_bgp()),
                    pixels_remaining: output_delay_pixels,
                }
            }
            PpuDmgBgpCpuCommitEffectKind::CurrentDotTransient => {
                DmgBgpCpuCommitOutputOverridePlan {
                    palette_override: Some(transient_palette),
                    pixels_remaining: 1,
                }
            }
            PpuDmgBgpCpuCommitEffectKind::RetroactivePanel => {
                if bg_visible_hold.is_some() {
                    DmgBgpCpuCommitOutputOverridePlan {
                        palette_override: None,
                        pixels_remaining: 0,
                    }
                } else if delay_final_visible_commit {
                    let output_delay_pixels = self
                        .dmg_bgp_cpu_commit_retroactive_final_delay_pixels(
                            context.visible_pixels_output,
                        );
                    if output_delay_pixels == 0 {
                        DmgBgpCpuCommitOutputOverridePlan {
                            palette_override: Some(context.value),
                            pixels_remaining: 1,
                        }
                    } else {
                        DmgBgpCpuCommitOutputOverridePlan {
                            palette_override: Some(self.pixel_pipeline_bgp()),
                            pixels_remaining: output_delay_pixels,
                        }
                    }
                } else {
                    DmgBgpCpuCommitOutputOverridePlan {
                        palette_override: Some(context.value),
                        pixels_remaining: 1,
                    }
                }
            }
        };

        DmgBgpCpuCommitGenericPlan {
            recorded_write,
            recolor,
            output_override,
            bg_visible_hold,
        }
    }

    fn apply_dmg_palette_write_plan(&mut self, plan: DmgPaletteWritePlan) {
        match plan {
            DmgPaletteWritePlan::BgpCpuCommit(plan) => {
                self.clear_dmg_bgp_cpu_commit_bg_visible_hold();
                self.apply_dmg_bgp_cpu_commit_write_plan(plan);
            }
            DmgPaletteWritePlan::RetroactiveRecolor(plan) => {
                self.apply_dmg_retroactive_palette_write_plan(plan);
            }
        }
    }

    fn apply_dmg_bgp_cpu_commit_write_plan(&mut self, plan: DmgBgpCpuCommitWritePlan) {
        match plan {
            DmgBgpCpuCommitWritePlan::SingleLeftSpriteSecondWriteTransient {
                context,
                transient_start_x,
                final_onset_x,
            } => self.apply_single_left_sprite_bgp_second_write_transient_range(
                context.register,
                context.previous_visible,
                context.value,
                context.visible_pixels_output,
                transient_start_x,
                final_onset_x,
            ),
            DmgBgpCpuCommitWritePlan::OnsetAtVisibleX {
                context,
                desired_onset_x,
            } => self.apply_dmg_bgp_cpu_commit_onset_at_visible_x(
                context.register,
                context.previous_visible,
                context.value,
                context.visible_pixels_output,
                desired_onset_x,
            ),
            DmgBgpCpuCommitWritePlan::WindowRestartBackdate {
                context,
                retroactive_pixels,
            } => self.apply_window_restart_bgp_backdate(
                context.register,
                context.value,
                context.visible_pixels_output,
                retroactive_pixels,
            ),
            DmgBgpCpuCommitWritePlan::Generic(plan) => {
                self.apply_dmg_bgp_cpu_commit_generic_plan(plan);
            }
        }
    }

    fn apply_dmg_bgp_cpu_commit_generic_plan(&mut self, plan: DmgBgpCpuCommitGenericPlan) {
        self.record_dmg_bgp_cpu_commit_visible_write(
            plan.recorded_write.effect_kind,
            plan.recorded_write.transient_visible_x,
            plan.recorded_write.transient_palette,
            plan.recorded_write.repaint_visible_x,
            plan.recorded_write.value,
        );
        if let Some(recolor) = plan.recolor {
            self.apply_dmg_retroactive_palette_write_plan(recolor);
        }
        if let Some(bg_visible_hold) = plan.bg_visible_hold {
            self.start_dmg_bgp_cpu_commit_bg_visible_hold(
                bg_visible_hold.palette,
                bg_visible_hold.bg_visible_pixels,
                bg_visible_hold.fallback_palette,
            );
        }
        self.set_dmg_bgp_cpu_commit_output_override(
            plan.output_override.palette_override,
            plan.output_override.pixels_remaining,
        );
    }

    fn apply_dmg_retroactive_palette_write_plan(&mut self, plan: DmgRetroactivePaletteWritePlan) {
        self.retroactively_recolor_recent_pixels(
            plan.register,
            plan.transient_palette,
            plan.final_palette,
            plan.retroactive_pixels,
            plan.delay_final_palette,
        );
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

    fn dmg_window_restart_bgp_backdate_pixels(&self, visible_pixels_output: u8) -> Option<usize> {
        if self.mode2_scan_state.selected_sprite_count() != 0
            || self.visible_registers.lcdc & LCDC_WINDOW_ENABLE_BIT == 0
            || !self.bg_pipeline_state.window_wy_latch
            || visible_pixels_output == 0
        {
            return None;
        }

        let write_count = self
            .dmg_panel_live_write_state
            .bgp_cpu_commit
            .current_line_writes
            .len();
        if write_count != 0 {
            return None;
        }

        let retroactive_pixels =
            usize::from(visible_pixels_output.min(DMG_PALETTE_RETROACTIVE_DOT_HISTORY as u8));
        let recent_bg_tail = self
            .dmg_panel_live_write_state
            .recent_panel_dots
            .iter()
            .rev()
            .take(retroactive_pixels)
            .copied()
            .collect::<Vec<_>>();
        if recent_bg_tail.len() != retroactive_pixels {
            return None;
        }

        for (offset, dot) in recent_bg_tail.iter().rev().enumerate() {
            let expected_visible_x =
                visible_pixels_output - retroactive_pixels as u8 + offset as u8;
            if dot.visible_x != expected_visible_x
                || dot.dmg_bg_forced_white
                || !matches!(dot.pixel.source, MixedPixelSource::Background)
            {
                return None;
            }
        }

        Some(retroactive_pixels)
    }

    fn apply_window_restart_bgp_backdate(
        &mut self,
        register: PpuPaletteRegister,
        value: u8,
        visible_pixels_output: u8,
        retroactive_pixels: usize,
    ) {
        let desired_onset_x = self.visible_registers.wx.saturating_sub(7).clamp(3, 9);
        let transient_visible_x = visible_pixels_output.saturating_sub(retroactive_pixels as u8);
        self.record_dmg_bgp_cpu_commit_visible_write(
            PpuDmgBgpCpuCommitEffectKind::RetroactivePanel,
            transient_visible_x,
            value,
            desired_onset_x.saturating_add(1),
            value,
        );
        self.retroactively_recolor_recent_pixels(register, value, value, retroactive_pixels, false);
        let hold_pixels = desired_onset_x.saturating_sub(visible_pixels_output);
        if hold_pixels == 0 {
            self.set_dmg_bgp_cpu_commit_output_override(Some(value), 1);
        } else {
            self.set_dmg_bgp_cpu_commit_output_override(Some(value), hold_pixels);
        }
    }

    fn dmg_window_restart_bgp_second_write_onset_visible_x(
        &self,
        previous_visible: u8,
        visible_pixels_output: u8,
    ) -> Option<u8> {
        if self.mode2_scan_state.selected_sprite_count() != 0
            || self.visible_registers.lcdc & LCDC_WINDOW_ENABLE_BIT == 0
            || !self.bg_pipeline_state.window_wy_latch
            || self
                .dmg_panel_live_write_state
                .bgp_cpu_commit
                .current_line_writes
                .len()
                != 1
        {
            return None;
        }

        let first_write = self
            .dmg_panel_live_write_state
            .bgp_cpu_commit
            .current_line_writes
            .first()
            .expect("len checked above");
        if !matches!(
            first_write.effect_kind,
            PpuDmgBgpCpuCommitEffectKind::PipelineDelayed
                | PpuDmgBgpCpuCommitEffectKind::RetroactivePanel
        ) || first_write.transient_visible_x != 0
            || first_write.transfer_lead_pixels != 0
            || first_write.value != previous_visible
        {
            return None;
        }

        let desired_onset_x = if self.visible_registers.wx == 0 {
            const WX0_SECOND_WRITE_ONSETS: [u8; 8] = [11, 9, 8, 7, 6, 5, 4, 3];
            let row_capped_onset =
                WX0_SECOND_WRITE_ONSETS[self.current_window_line_counter() as usize % 8];
            row_capped_onset.min(visible_pixels_output.saturating_sub(4).max(3))
        } else {
            self.visible_registers.wx.saturating_sub(7).clamp(3, 9)
        };
        if desired_onset_x < visible_pixels_output {
            for x in usize::from(desired_onset_x)..usize::from(visible_pixels_output) {
                if self.current_scanline_dmg_bg_forced_white[x]
                    || !matches!(
                        self.current_scanline_mixed_pixels[x].source,
                        MixedPixelSource::Background
                    )
                {
                    return None;
                }
            }
        }

        Some(desired_onset_x)
    }

    fn apply_dmg_bgp_cpu_commit_onset_at_visible_x(
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
            let row_start = self.ly as usize * SCREEN_WIDTH;
            for x in usize::from(desired_onset_x)..usize::from(visible_pixels_output) {
                self.recolor_bgwin_framebuffer_pixel_with_palette(row_start + x, value);
                let mixed_pixel = self.current_scanline_mixed_pixels[x];
                if !register_affects_pixel(register, mixed_pixel) {
                    continue;
                }

                let panel_pixel = self.map_mixed_pixel_to_panel_shade_with_palette_override(
                    mixed_pixel,
                    register,
                    value,
                );
                self.framebuffer[row_start + x] = panel_pixel;
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
        visible_pixels_output: u8,
    ) -> Option<u8> {
        if self.mode2_scan_state.selected_sprite_count() != 1 {
            return None;
        }

        let sprite_x = self.mode2_scan_state.selected_sprite(0)?.x as usize;

        if matches!(write_index, 0 | 1) && visible_pixels_output > 16 {
            // The dedicated left-edge seam only applies while the write is still
            // within the first visible OBJ boundary window. Late same-line BGP
            // writes, like the Mealybug LCDC OBJ variant's end-of-line black
            // visualization pulse, should fall back to the generic CPU-commit
            // path instead of repainting the whole left prefix.
            return None;
        }

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

    fn dmg_single_left_sprite_bgp_late_black_pulse_onset_visible_x(
        &self,
        previous_visible: u8,
        value: u8,
        visible_pixels_output: u8,
        write_index: usize,
    ) -> Option<u8> {
        if write_index != 0
            || previous_visible != 0x00
            || value != 0xFF
            || visible_pixels_output < 140
        {
            return None;
        }

        let sprite_x = self.mode2_scan_state.selected_sprite(0)?.x as usize;
        if sprite_x == 0 {
            return Some(visible_pixels_output.saturating_sub(6));
        }
        const LATE_BLACK_PULSE_ONSETS: [u8; 18] = [
            150, 147, 148, 149, 150, 151, 151, 151, 150, 151, 152, 153, 154, 155, 155, 155, 156,
            157,
        ];

        LATE_BLACK_PULSE_ONSETS.get(sprite_x).copied()
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
            let palette = if x < usize::from(final_onset_x) {
                transient_palette
            } else {
                value
            };
            self.recolor_bgwin_framebuffer_pixel_with_palette(row_start + x, palette);
            let mixed_pixel = self.current_scanline_mixed_pixels[x];
            if !register_affects_pixel(register, mixed_pixel) {
                continue;
            }

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
