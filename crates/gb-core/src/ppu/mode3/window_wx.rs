impl Ppu {
    pub(super) fn maybe_apply_wx0_shortening_after_transfer_dot(
        &mut self,
        transfer_dot: Mode3TransferDot,
    ) {
        self.maybe_expire_dmg_previsible_wx_retarget();
        self.maybe_expire_pending_dmg_live_wx_trigger_glitch();
        if !self.mode3_window_policy().can_apply_wx0_shortening(
            transfer_dot,
            self.runtime.bg_pipeline_state.visible_pixels_output,
            self.runtime.bg_pipeline_state.current_transfer_x,
            self.runtime.bg_pipeline_state.initial_scx_discard,
            self.runtime.bg_pipeline_state.scx_discard_remaining,
        ) {
            return;
        }

        self.runtime.bg_pipeline_state.apply_wx0_scx_shortening();
    }

    pub(super) fn maybe_arm_dmg_previsible_wx_retarget(&mut self, previous_wx: u8, wx: u8) {
        let pending_previsible_trigger_x = self
            .runtime
            .bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_retarget
            .and_then(|retarget| retarget.trigger_x);
        let pending_one_hidden_prefix_resume = self
            .runtime
            .bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_retarget
            .is_some_and(|retarget| {
                matches!(
                    retarget.kind,
                    DmgPrevisibleWxRetargetKind::OneHiddenPrefixResume
                )
            });
        let visible_registers = self.mode3_register_latches().visible();
        let plan = Self::plan_dmg_previsible_wx_retarget(
            DmgPrevisibleWxRetargetPlanContext {
                is_dmg_family: self.console_model.is_dmg_family(),
                drawing_mode: self.current_access_mode() == PpuAccessMode::Drawing,
                window_started_this_line: self.runtime.bg_pipeline_state.window_started_this_line,
                window_wy_latch: self.runtime.bg_pipeline_state.window_wy_latch,
                window_enabled: visible_registers.window_enabled(),
                bg_enabled: visible_registers.bg_enabled(),
                visible_pixels_output: self.runtime.bg_pipeline_state.visible_pixels_output,
                window_active_line_counter: self
                    .runtime
                    .bg_pipeline_state
                    .window_active_line_counter,
                pending_previsible_trigger_x,
                pending_one_hidden_prefix_resume,
                live_wx_can_still_start_later_this_line: self
                    .dmg_live_wx_write_can_still_start_later_this_line(wx),
                fetcher_source: self.runtime.bg_pipeline_state.fetcher.source,
            },
            previous_wx,
            wx,
        );
        self.apply_dmg_previsible_wx_plan(plan);
    }

    pub(super) fn maybe_arm_dmg_live_wx_trigger_glitch(&mut self, wx: u8) {
        if !self.console_model.is_dmg_family()
            || self.current_access_mode() != PpuAccessMode::Drawing
            || !self.runtime.bg_pipeline_state.window_started_this_line
            || !self.runtime.bg_pipeline_state.window_wy_latch
            || self.runtime.bg_pipeline_state.visible_pixels_output == 0
            || !self.mode3_register_latches().visible().window_enabled()
            || !self.mode3_register_latches().visible().bg_enabled()
        {
            return;
        }

        let trigger_x = match wx {
            7..=166 => wx.saturating_sub(7),
            _ => {
                self.clear_dmg_previsible_wx_live_trigger_glitch();
                return;
            }
        };

        if trigger_x < self.runtime.bg_pipeline_state.visible_pixels_output {
            self.clear_dmg_previsible_wx_live_trigger_glitch();
            return;
        }

        if trigger_x == self.runtime.bg_pipeline_state.visible_pixels_output {
            self.push_dmg_live_wx_trigger_glitch_pixel();
            self.clear_dmg_previsible_wx_live_trigger_glitch();
            return;
        }

        self.arm_dmg_previsible_wx_live_trigger_glitch(trigger_x);
    }

    fn maybe_apply_dmg_previsible_wx_retarget(&mut self, vram: &VramBusView<'_>) {
        let Some(retarget) = self
            .runtime
            .bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_retarget
        else {
            return;
        };

        if !self.console_model.is_dmg_family()
            || self.runtime.bg_pipeline_state.visible_pixels_output != 0
            || self.runtime.bg_pipeline_state.fetcher.source != PpuBgFetcherSource::Window
        {
            return;
        }

        if matches!(
            (
                self.runtime.bg_pipeline_state.fetcher.stage,
                self.runtime.bg_pipeline_state.fetcher.stage_dot,
            ),
            (PpuBgFetcherStage::WindowActivating, _) | (PpuBgFetcherStage::TileIndex, 0)
        ) {
            self.abort_window_fetcher_to_background_now(vram);
            self.runtime.bg_pipeline_state.window_started_this_line = false;
            self.runtime.bg_pipeline_state.window_active_line_counter =
                retarget.active_line_counter;
            self.clear_dmg_previsible_wx_live_trigger_glitch();
            if matches!(
                retarget.kind,
                DmgPrevisibleWxRetargetKind::OneHiddenPrefixResume
            ) {
                self.restore_current_fetcher_cached_slice_to_fifo();
            }
            if matches!(retarget.kind, DmgPrevisibleWxRetargetKind::CancelOnly) {
                self.restore_current_fetcher_cached_slice_to_fifo();
                self.runtime.bg_pipeline_state.window_start_count_this_line = self
                    .runtime
                    .bg_pipeline_state
                    .window_start_count_this_line
                    .saturating_sub(1);
                self.clear_dmg_previsible_wx_retarget_state();
            }
        }
    }

    pub(super) fn maybe_apply_pending_dmg_live_wx_trigger_glitch(
        &mut self,
        transfer_dot: Mode3TransferDot,
    ) {
        let Some(glitch) = self
            .runtime
            .bg_pipeline_state
            .dmg_window_restart
            .pending_live_wx_trigger_glitch
        else {
            return;
        };

        if !self.console_model.is_dmg_family()
            || transfer_dot.kind != Mode3TransferDotKind::ServedVisiblePixel
        {
            return;
        }

        if self.runtime.bg_pipeline_state.visible_pixels_output == glitch.trigger_x {
            self.push_dmg_live_wx_trigger_glitch_pixel();
            self.clear_dmg_previsible_wx_live_trigger_glitch();
        } else if self.runtime.bg_pipeline_state.visible_pixels_output > glitch.trigger_x {
            self.clear_dmg_previsible_wx_live_trigger_glitch();
        }
    }

    fn maybe_apply_pending_dmg_previsible_wx_carry(
        &mut self,
        transfer_dot: Mode3TransferDot,
        vram: &VramBusView<'_>,
    ) {
        let Some(mut carry) = self
            .runtime
            .bg_pipeline_state
            .dmg_window_restart
            .pending_previsible_wx_carry
        else {
            return;
        };

        if !self.console_model.is_dmg_family()
            || transfer_dot.kind != Mode3TransferDotKind::ServedVisiblePixel
        {
            return;
        }

        if self.runtime.bg_pipeline_state.visible_pixels_output == carry.next_trigger_x {
            if let Some(pixel) = self.compute_window_pixel_for_logical_offset(
                carry.active_line_counter,
                carry.next_window_pixel_offset,
                vram,
            ) {
                self.runtime.bg_pipeline_state.fifo.push_front(pixel);
            }
            carry.next_trigger_x = carry.next_trigger_x.saturating_add(1);
            carry.next_window_pixel_offset = carry.next_window_pixel_offset.saturating_add(1);
            if carry.next_trigger_x >= carry.end_trigger_x {
                self.clear_dmg_previsible_wx_carry();
            } else {
                self.runtime
                    .bg_pipeline_state
                    .dmg_window_restart
                    .pending_previsible_wx_carry = Some(carry);
            }
        } else if self.runtime.bg_pipeline_state.visible_pixels_output > carry.next_trigger_x {
            self.clear_dmg_previsible_wx_carry();
        }
    }

    fn maybe_apply_pending_dmg_previsible_wx_onset_glitch_repaint(
        &mut self,
        vram: &VramBusView<'_>,
    ) {
        if let Some(trigger_x) = self
            .runtime
            .bg_pipeline_state
            .dmg_window_restart
            .pending_previsible_wx_onset_glitch
            && self.runtime.bg_pipeline_state.visible_pixels_output > trigger_x
        {
            self.repaint_current_scanline_dot_with_bg_override(usize::from(trigger_x), 0, vram);
            self.clear_dmg_previsible_wx_onset_glitch();
        }
    }

    fn maybe_expire_dmg_previsible_wx_retarget(&mut self) {
        let Some(retarget) = self
            .runtime
            .bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_retarget
        else {
            return;
        };

        if !self.console_model.is_dmg_family()
            || retarget.trigger_x.is_some_and(|trigger_x| {
                self.runtime.bg_pipeline_state.visible_pixels_output > trigger_x
            })
            || matches!(retarget.kind, DmgPrevisibleWxRetargetKind::CancelOnly)
                && self.runtime.bg_pipeline_state.visible_pixels_output != 0
        {
            self.clear_dmg_previsible_wx_expired_retarget_state();
        }
    }

    fn maybe_expire_pending_dmg_live_wx_trigger_glitch(&mut self) {
        let Some(glitch) = self
            .runtime
            .bg_pipeline_state
            .dmg_window_restart
            .pending_live_wx_trigger_glitch
        else {
            return;
        };

        if self.runtime.bg_pipeline_state.visible_pixels_output > glitch.trigger_x {
            self.clear_dmg_previsible_wx_live_trigger_glitch();
        }
    }

    fn push_dmg_live_wx_trigger_glitch_pixel(&mut self) {
        self.runtime.bg_pipeline_state.fifo.push_back(0);
    }

    fn dmg_live_wx_write_can_still_start_later_this_line(&self, wx: u8) -> bool {
        matches!(wx, 7..=165)
            && wx.saturating_sub(7) > self.runtime.bg_pipeline_state.visible_pixels_output
    }

    pub(super) fn maybe_start_window_after_transfer_dot(
        &mut self,
        transfer_dot: Mode3TransferDot,
    ) -> bool {
        if self.console_model.is_dmg_family()
            && transfer_dot.kind == Mode3TransferDotKind::ServedVisiblePixel
            && self.runtime.bg_pipeline_state.scx_discard_remaining == 0
            && self.runtime.bg_pipeline_state.window_wy_latch
            && self.mode3_register_latches().visible().window_enabled()
            && self.mode3_register_latches().visible().bg_enabled()
            && self
                .runtime
                .bg_pipeline_state
                .dmg_window_restart
                .previsible_wx_retarget
                .is_some_and(|retarget| {
                    retarget.trigger_x == Some(self.runtime.bg_pipeline_state.visible_pixels_output)
                })
        {
            self.runtime
                .bg_pipeline_state
                .dmg_window_restart
                .pending_window_reenable_resume = None;
            self.runtime
                .bg_pipeline_state
                .dmg_late_window_enable_override = None;
            self.clear_dmg_previsible_wx_live_trigger_glitch();
            let retarget = self
                .runtime
                .bg_pipeline_state
                .dmg_window_restart
                .previsible_wx_retarget
                .take()
                .expect("checked above");
            let retained_same_scanline_trigger = self
                .runtime
                .bg_pipeline_state
                .dmg_window_restart
                .previsible_wx_retained_trigger_glitch_x
                .is_some();
            let retained_window_pixel_offset = if retained_same_scanline_trigger
                && retarget.window_pixel_offset % u16::from(BG_TILE_WIDTH) == 7
            {
                retarget.window_pixel_offset.saturating_add(1)
            } else {
                retarget.window_pixel_offset
            };
            self.apply_dmg_previsible_wx_restart(retarget, retained_window_pixel_offset);
            return true;
        }

        let decision = self
            .mode3_window_policy()
            .start_decision_after_transfer_dot(
                transfer_dot,
                self.runtime.bg_pipeline_state.visible_pixels_output,
                self.runtime.bg_pipeline_state.current_transfer_x,
                self.runtime.bg_pipeline_state.initial_scx_discard,
                self.runtime.bg_pipeline_state.scx_discard_remaining,
                self.runtime.bg_pipeline_state.wx166_armed_this_line,
            );
        self.runtime
            .bg_pipeline_state
            .dmg_window_restart
            .previsible_wx_cancel_uses_visible_wx_once = false;

        match decision {
            PpuMode3WindowStartDecision::NotReady => {
                self.maybe_arm_dmg_late_window_enable_override_after_transfer_dot(transfer_dot);
                false
            }
            PpuMode3WindowStartDecision::ArmWx166NextLine => {
                self.runtime.window_state.pending_wx166_next_line = true;
                self.runtime.bg_pipeline_state.wx166_armed_this_line = true;
                false
            }
            PpuMode3WindowStartDecision::StartNow => {
                self.runtime
                    .bg_pipeline_state
                    .dmg_window_restart
                    .pending_window_reenable_resume = None;
                self.runtime
                    .bg_pipeline_state
                    .dmg_late_window_enable_override = None;
                self.clear_dmg_previsible_wx_live_trigger_glitch();
                if let Some(retarget) = self
                    .runtime
                    .bg_pipeline_state
                    .dmg_window_restart
                    .previsible_wx_retarget
                    .take()
                {
                    self.apply_dmg_previsible_wx_restart(retarget, retarget.window_pixel_offset);
                } else {
                    self.start_window_fetcher_restart();
                }
                true
            }
        }
    }
    pub(super) fn start_window_fetcher_restart(&mut self) {
        let window_line_counter = self
            .runtime
            .window_state
            .window_line_counter
            .wrapping_add(self.runtime.bg_pipeline_state.window_start_count_this_line);
        self.start_window_fetcher_restart_with_row_mode(window_line_counter, true, 0, false);
    }

    fn apply_dmg_previsible_wx_restart(
        &mut self,
        retarget: DmgPrevisibleWxRetarget,
        window_pixel_offset: u16,
    ) {
        let (preserve_fifo, advance_tilemap) = match retarget.kind {
            DmgPrevisibleWxRetargetKind::RetainedFifoPrefixResume { advance_tilemap } => {
                (true, advance_tilemap)
            }
            DmgPrevisibleWxRetargetKind::CancelOnly
            | DmgPrevisibleWxRetargetKind::OneHiddenPrefixResume
            | DmgPrevisibleWxRetargetKind::PlainRestart => (false, false),
        };
        self.start_window_fetcher_restart_with_row_mode(
            retarget.active_line_counter,
            false,
            window_pixel_offset,
            preserve_fifo,
        );
        if advance_tilemap {
            self.runtime.bg_pipeline_state.fetcher.window_tilemap_x = self
                .runtime
                .bg_pipeline_state
                .fetcher
                .window_tilemap_x
                .wrapping_add(1);
        }
    }

    fn plan_dmg_previsible_wx_retarget(
        ctx: DmgPrevisibleWxRetargetPlanContext,
        previous_wx: u8,
        wx: u8,
    ) -> DmgPrevisibleWxPlan {
        let late_visible_write =
            ctx.is_dmg_family && ctx.drawing_mode && ctx.visible_pixels_output != 0;
        if late_visible_write
            && ctx.pending_one_hidden_prefix_resume
            && !ctx.live_wx_can_still_start_later_this_line
        {
            let pending_distance = ctx
                .pending_previsible_trigger_x
                .unwrap_or(ctx.visible_pixels_output)
                .saturating_sub(ctx.visible_pixels_output);
            return DmgPrevisibleWxPlan {
                followup_markers: DmgPrevisibleWxFollowupMarkers::cleared(),
                action: if Self::dmg_late_write_keeps_one_hidden_prefix_resume(pending_distance) {
                    DmgPrevisibleWxPlanAction::KeepState
                } else {
                    DmgPrevisibleWxPlanAction::ClearRetargetAndGapArtifacts
                },
            };
        }

        let cancel_uses_visible_wx_once = late_visible_write
            && ctx.pending_previsible_trigger_x
                == Some(ctx.visible_pixels_output.saturating_add(1));
        let followup_markers = DmgPrevisibleWxFollowupMarkers {
            cancel_uses_visible_wx_once,
            cancel_background_override_onset_x: if cancel_uses_visible_wx_once
                && !ctx.live_wx_can_still_start_later_this_line
            {
                ctx.pending_previsible_trigger_x
            } else {
                None
            },
            retained_trigger_glitch_x: if late_visible_write
                && ctx.pending_previsible_trigger_x.is_some()
                && !cancel_uses_visible_wx_once
            {
                ctx.pending_previsible_trigger_x
            } else {
                None
            },
        };

        if late_visible_write {
            return DmgPrevisibleWxPlan {
                followup_markers,
                action: if cancel_uses_visible_wx_once {
                    DmgPrevisibleWxPlanAction::ClearRetargetAndGapArtifacts
                } else if ctx.pending_previsible_trigger_x.is_some() {
                    DmgPrevisibleWxPlanAction::ClearOnsetGlitch
                } else {
                    DmgPrevisibleWxPlanAction::KeepState
                },
            };
        }

        if !ctx.is_dmg_family
            || !ctx.drawing_mode
            || !ctx.window_started_this_line
            || !ctx.window_wy_latch
            || !ctx.window_enabled
            || !ctx.bg_enabled
        {
            return DmgPrevisibleWxPlan {
                followup_markers,
                action: DmgPrevisibleWxPlanAction::ClearRetargetAndGapArtifacts,
            };
        }

        if wx == previous_wx {
            return DmgPrevisibleWxPlan {
                followup_markers,
                action: DmgPrevisibleWxPlanAction::KeepState,
            };
        }

        let cancel_only_low_wx =
            Self::is_dmg_low_wx_cancel_only_retarget(previous_wx, wx, ctx.visible_pixels_output);
        let trigger_x = match wx {
            Self::DMG_VISIBLE_WINDOW_ORIGIN_WX..=165 => {
                Some(wx - Self::DMG_VISIBLE_WINDOW_ORIGIN_WX)
            }
            _ if cancel_only_low_wx => None,
            _ => {
                return DmgPrevisibleWxPlan {
                    followup_markers,
                    action: DmgPrevisibleWxPlanAction::ClearRetargetAndGapArtifacts,
                };
            }
        };
        let initial_hidden_skip = 7u8.saturating_sub(previous_wx);
        let visible_tail_len = BG_TILE_WIDTH.saturating_sub(initial_hidden_skip);
        let retained_fifo_prefix_resume = wx == Self::DMG_VISIBLE_WINDOW_ORIGIN_WX
            && trigger_x == Some(0)
            && Self::can_retain_dmg_fifo_prefix_resume(initial_hidden_skip)
            && ctx.visible_pixels_output == 0
            && ctx.fetcher_source == PpuBgFetcherSource::Window;
        let one_hidden_prefix_resume_offset =
            Self::uses_dmg_one_hidden_prefix_resume(initial_hidden_skip, cancel_only_low_wx)
                .then_some(
                    if trigger_x.is_some_and(|trigger_x| trigger_x <= visible_tail_len) {
                        0
                    } else {
                        u16::from(BG_TILE_WIDTH)
                    },
                );
        let raw_window_pixel_offset = if cancel_only_low_wx || trigger_x == Some(0) {
            0
        } else if let Some(resume_offset) = one_hidden_prefix_resume_offset {
            resume_offset
        } else {
            u16::from(initial_hidden_skip)
                + u16::from(trigger_x.expect("non-cancel retargets have a visible trigger"))
        };
        let visible_gap_len = trigger_x.unwrap_or(0).saturating_sub(visible_tail_len);
        let (window_pixel_offset, onset_glitch, carry) = if !cancel_only_low_wx
            && one_hidden_prefix_resume_offset.is_none()
            && initial_hidden_skip != 0
            && raw_window_pixel_offset != 0
        {
            let mut window_pixel_offset = raw_window_pixel_offset;
            let boundary_restart = window_pixel_offset % u16::from(BG_TILE_WIDTH) == 0;
            if boundary_restart {
                window_pixel_offset -= 1;
            }
            let onset_glitch = boundary_restart
                .then_some(trigger_x.expect("boundary restarts have a visible trigger"));
            let carry = (visible_gap_len != 0).then_some(DmgPendingPrevisibleWxCarry::new(
                visible_tail_len,
                trigger_x.expect("carry spans have a visible trigger"),
                ctx.window_active_line_counter,
                raw_window_pixel_offset - u16::from(visible_gap_len),
            ));
            (window_pixel_offset, onset_glitch, carry)
        } else {
            (raw_window_pixel_offset, None, None)
        };

        let retarget = if cancel_only_low_wx {
            DmgPrevisibleWxRetarget::new_cancel_only(
                ctx.window_active_line_counter,
                window_pixel_offset,
            )
        } else if retained_fifo_prefix_resume {
            DmgPrevisibleWxRetarget::new_retained_fifo_prefix_resume(
                trigger_x.expect("retained FIFO restarts have a visible trigger"),
                ctx.window_active_line_counter,
                window_pixel_offset,
                Self::dmg_retained_fifo_prefix_resume_advances_next_tilemap(initial_hidden_skip),
            )
        } else if one_hidden_prefix_resume_offset.is_some() {
            DmgPrevisibleWxRetarget::new_one_hidden_prefix_resume(
                trigger_x.expect("one-hidden-prefix resumes have a visible trigger"),
                ctx.window_active_line_counter,
                window_pixel_offset,
            )
        } else {
            DmgPrevisibleWxRetarget::new(
                trigger_x.expect("plain restarts have a visible trigger"),
                ctx.window_active_line_counter,
                window_pixel_offset,
            )
        };

        DmgPrevisibleWxPlan {
            followup_markers,
            action: DmgPrevisibleWxPlanAction::ArmRetarget {
                retarget,
                onset_glitch,
                carry,
            },
        }
    }

    fn dmg_late_write_keeps_one_hidden_prefix_resume(pending_distance: u8) -> bool {
        pending_distance <= Self::DMG_LATE_WRITE_ONE_HIDDEN_PREFIX_KEEP_DISTANCE
    }

    fn is_dmg_low_wx_cancel_only_retarget(
        previous_wx: u8,
        wx: u8,
        visible_pixels_output: u8,
    ) -> bool {
        previous_wx >= Self::DMG_LOW_WX_CANCEL_ONLY_PREVIOUS_WX_MIN
            && wx < Self::DMG_VISIBLE_WINDOW_ORIGIN_WX
            && visible_pixels_output == 0
    }

    fn can_retain_dmg_fifo_prefix_resume(initial_hidden_skip: u8) -> bool {
        initial_hidden_skip >= Self::DMG_RETAINED_FIFO_PREFIX_RESUME_MIN_HIDDEN_SKIP
    }

    fn uses_dmg_one_hidden_prefix_resume(
        initial_hidden_skip: u8,
        cancel_only_low_wx: bool,
    ) -> bool {
        initial_hidden_skip == Self::DMG_ONE_HIDDEN_PREFIX_SKIP && !cancel_only_low_wx
    }

    fn dmg_retained_fifo_prefix_resume_advances_next_tilemap(initial_hidden_skip: u8) -> bool {
        initial_hidden_skip >= Self::DMG_RETAINED_FIFO_PREFIX_NEXT_TILEMAP_MIN_HIDDEN_SKIP
    }

    fn apply_dmg_previsible_wx_plan(&mut self, plan: DmgPrevisibleWxPlan) {
        self.apply_dmg_previsible_wx_followup_markers(plan.followup_markers);

        match plan.action {
            DmgPrevisibleWxPlanAction::KeepState => {}
            DmgPrevisibleWxPlanAction::ClearOnsetGlitch => {
                self.clear_dmg_previsible_wx_onset_glitch();
            }
            DmgPrevisibleWxPlanAction::ClearRetargetAndGapArtifacts => {
                self.clear_dmg_previsible_wx_retarget_and_gap_artifacts();
            }
            DmgPrevisibleWxPlanAction::ArmRetarget {
                retarget,
                onset_glitch,
                carry,
            } => {
                self.arm_dmg_previsible_wx_retarget_state(retarget, onset_glitch, carry);
            }
        }
    }

    fn start_window_fetcher_restart_with_row_mode(
        &mut self,
        active_line_counter: u8,
        increment_start_count: bool,
        window_pixel_offset: u16,
        preserve_fifo: bool,
    ) {
        let bg_resume_fetch_pixel = self.runtime.bg_pipeline_state.fetcher.next_fetch_pixel;
        if !preserve_fifo {
            self.runtime.bg_pipeline_state.fifo.clear();
        }
        self.runtime.bg_pipeline_state.startup_fifo_placeholders = 0;
        self.runtime.bg_pipeline_state.push.reset();
        self.runtime.bg_pipeline_state.fill.reset();
        if window_pixel_offset == 0 {
            self.runtime
                .bg_pipeline_state
                .fetcher
                .start_window(bg_resume_fetch_pixel);
        } else {
            self.runtime
                .bg_pipeline_state
                .fetcher
                .start_window_with_pixel_offset(bg_resume_fetch_pixel, window_pixel_offset);
        }
        self.runtime.bg_pipeline_state.scx_discard_remaining = 0;
        self.runtime.bg_pipeline_state.window_started_this_line = true;
        self.runtime.bg_pipeline_state.window_active_line_counter = active_line_counter;
        if increment_start_count {
            self.runtime.bg_pipeline_state.window_start_count_this_line = self
                .runtime
                .bg_pipeline_state
                .window_start_count_this_line
                .wrapping_add(1);
        }
        self.runtime.bg_pipeline_state.window_force_x0_this_line = false;
        self.clear_dmg_previsible_wx_restart_transients();
    }

    fn clear_dmg_previsible_wx_restart_transients(&mut self) {
        self.runtime
            .bg_pipeline_state
            .dmg_window_restart
            .clear_restart_transients();
    }

    fn restore_current_fetcher_cached_slice_to_fifo(&mut self) {
        let cached = BgCachedSlice::from_fetcher(self.runtime.bg_pipeline_state.fetcher);
        self.runtime.bg_pipeline_state.fifo.clear();
        self.runtime
            .bg_pipeline_state
            .push_cached_slice_fifo_pixels_with_skip(cached, 0);
    }

    fn apply_dmg_previsible_wx_followup_markers(
        &mut self,
        markers: DmgPrevisibleWxFollowupMarkers,
    ) {
        self.runtime
            .bg_pipeline_state
            .dmg_window_restart
            .apply_followup_markers(
                markers.cancel_uses_visible_wx_once,
                markers.cancel_background_override_onset_x,
                markers.retained_trigger_glitch_x,
            );
    }

    fn arm_dmg_previsible_wx_retarget_state(
        &mut self,
        retarget: DmgPrevisibleWxRetarget,
        onset_glitch: Option<u8>,
        carry: Option<DmgPendingPrevisibleWxCarry>,
    ) {
        self.runtime
            .bg_pipeline_state
            .dmg_window_restart
            .arm_previsible_wx_retarget_state(retarget, onset_glitch, carry);
    }

    fn arm_dmg_previsible_wx_live_trigger_glitch(&mut self, trigger_x: u8) {
        self.runtime
            .bg_pipeline_state
            .dmg_window_restart
            .arm_live_trigger_glitch(trigger_x);
    }

    fn clear_dmg_previsible_wx_retarget_and_gap_artifacts(&mut self) {
        self.runtime
            .bg_pipeline_state
            .dmg_window_restart
            .clear_retarget_and_gap_artifacts();
    }

    fn clear_dmg_previsible_wx_expired_retarget_state(&mut self) {
        self.runtime
            .bg_pipeline_state
            .dmg_window_restart
            .clear_expired_retarget_state();
    }

    fn clear_dmg_previsible_wx_retarget_state(&mut self) {
        self.runtime
            .bg_pipeline_state
            .dmg_window_restart
            .clear_retarget_state();
    }

    fn clear_dmg_previsible_wx_carry(&mut self) {
        self.runtime
            .bg_pipeline_state
            .dmg_window_restart
            .clear_carry();
    }

    fn clear_dmg_previsible_wx_live_trigger_glitch(&mut self) {
        self.runtime
            .bg_pipeline_state
            .dmg_window_restart
            .clear_live_trigger_glitch();
    }

    fn clear_dmg_previsible_wx_onset_glitch(&mut self) {
        self.runtime
            .bg_pipeline_state
            .dmg_window_restart
            .clear_onset_glitch();
    }
}
