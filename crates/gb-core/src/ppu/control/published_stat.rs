use super::*;

impl Ppu {
    pub(in crate::ppu) fn current_published_stat_access_mode(&self) -> PpuAccessMode {
        let Some(context) = self.current_published_stat_mode_context() else {
            return self.published_stat_mode_at_line_start();
        };

        self.resolve_published_stat_access_mode(context)
    }

    fn current_published_stat_mode_context(&self) -> Option<PpuPublishedStatModeContext> {
        if self.line_dot == 0 {
            return None;
        }

        Some(PpuPublishedStatModeContext {
            published_mode: self.access_mode_for_line_dot(self.line_dot - 1),
            current_mode: self.access_mode_for_line_dot(self.line_dot),
            sprite_extended_mode3: self.current_mode0_start_dot() > self.baseline_mode0_start_dot(),
        })
    }

    fn published_stat_mode_at_line_start(&self) -> PpuAccessMode {
        if self.ly > VISIBLE_SCANLINES {
            PpuAccessMode::VBlank
        } else {
            PpuAccessMode::HBlank
        }
    }

    fn resolve_published_stat_access_mode(
        &self,
        context: PpuPublishedStatModeContext,
    ) -> PpuAccessMode {
        if self.published_stat_mode2_to_mode3_override_applies(context) {
            return PpuAccessMode::Drawing;
        }

        if self.published_stat_early_hblank_override_applies(context) {
            return PpuAccessMode::HBlank;
        }

        if self.published_stat_keep_drawing_override_applies(context) {
            return PpuAccessMode::Drawing;
        }

        if let Some(mode) = self.published_stat_terminal_boundary_override(context) {
            return mode;
        }

        context.published_mode
    }

    fn published_stat_mode2_to_mode3_override_applies(
        &self,
        context: PpuPublishedStatModeContext,
    ) -> bool {
        context.published_mode == PpuAccessMode::OamScan
            && context.current_mode == PpuAccessMode::Drawing
            && !self.runtime.blank_frame_active
            && self.ly < VISIBLE_SCANLINES
            && self.line_dot == MODE2_DOTS
    }

    fn published_stat_early_hblank_override_applies(
        &self,
        context: PpuPublishedStatModeContext,
    ) -> bool {
        if context.published_mode != PpuAccessMode::Drawing {
            return false;
        }

        let ordered_early_hblank_rules: [PpuPublishedStatPredicate; 14] = [
            Self::saturated_placeholder_backed_terminal_bg_tail_should_publish_hblank_two_dots_early,
            Self::terminal_x167_visible_same_x_cluster_should_publish_hblank_two_dots_early,
            Self::saturated_placeholder_backed_terminal_bg_tail_should_publish_hblank_one_dot_early,
            Self::terminal_x167_visible_same_x_cluster_should_publish_hblank_one_dot_early,
            Self::single_left_sprite_placeholder_backed_tail_should_publish_hblank_early,
            Self::single_left_sprite_x4_placeholder_backed_preterminal_tail_should_publish_hblank_five_dots_early,
            Self::single_left_sprite_x5_placeholder_backed_preterminal_tail_should_publish_hblank_four_dots_early,
            Self::single_left_sprite_x6_to_x7_placeholder_backed_preterminal_tail_should_publish_hblank_from_fifo_tail,
            Self::single_left_sprite_x12_to_x16_terminal_tail_with_entry_delay_should_publish_hblank_two_dots_early,
            Self::single_offscreen_right_sprite_xa0_terminal_tail_without_entry_delay_should_publish_hblank_two_dots_early,
            Self::single_offscreen_right_sprite_xa7_terminal_tail_should_publish_hblank_two_dots_early,
            Self::single_offscreen_right_sprite_xa2_mode0_boundary_should_publish_hblank,
            Self::two_sprite_staggered_fifo_tail_should_publish_hblank_from_fifo_tail,
            Self::ten_sprite_step8_preterminal_tail_should_publish_hblank_early,
        ];

        ordered_early_hblank_rules
            .into_iter()
            .any(|rule| rule(self))
            || (self.terminal_visible_tail_should_publish_hblank_early()
                && !self
                    .two_sprite_staggered_x8_to_x9_preterminal_tail_should_keep_published_drawing()
                && !self.saturated_placeholder_backed_terminal_bg_tail_still_owned_by_mode3())
    }

    fn published_stat_keep_drawing_override_applies(
        &self,
        context: PpuPublishedStatModeContext,
    ) -> bool {
        if context.published_mode != PpuAccessMode::HBlank {
            return false;
        }

        let ordered_keep_drawing_rules: [PpuPublishedStatPredicate; 3] = [
            Self::two_sprite_staggered_x0_to_x1_terminal_tail_should_keep_published_drawing,
            Self::two_sprite_staggered_x9_terminal_boundary_should_keep_published_drawing,
            Self::ten_sprite_step8_terminal_tail_should_keep_published_drawing,
        ];

        ordered_keep_drawing_rules
            .into_iter()
            .any(|rule| rule(self))
    }

    fn published_stat_terminal_boundary_override(
        &self,
        context: PpuPublishedStatModeContext,
    ) -> Option<PpuAccessMode> {
        if context.published_mode == PpuAccessMode::Drawing
            && context.current_mode == PpuAccessMode::HBlank
            && !self.runtime.blank_frame_active
            && self.ly < VISIBLE_SCANLINES
            && self.line_dot == self.current_mode0_start_dot()
            && !context.sprite_extended_mode3
        {
            return Some(PpuAccessMode::HBlank);
        }

        if context.published_mode == PpuAccessMode::HBlank
            && !self.runtime.blank_frame_active
            && self.ly < VISIBLE_SCANLINES
            && context.sprite_extended_mode3
            && self.line_dot == self.current_mode0_start_dot().saturating_add(2)
        {
            return Some(PpuAccessMode::Drawing);
        }

        None
    }

    pub(in crate::ppu) fn terminal_visible_tail_should_publish_hblank_early(&self) -> bool {
        let mode0_interrupt_enabled =
            self.stat_interrupt_enable & STAT_MODE0_INTERRUPT_ENABLE_BIT != 0;
        let saturated_sprite_line =
            usize::from(self.runtime.mode2_scan_state.selected_sprite_count())
                == MAX_SELECTED_SPRITES_PER_LINE;
        let saturated_sprite_line_uses_earlier_terminal_hblank =
            saturated_sprite_line && self.runtime.bg_pipeline_state.current_transfer_x == 163;
        let saturated_sprite_line_placeholder_backed_visible_tail_can_publish_hblank =
            saturated_sprite_line
                && self.runtime.bg_pipeline_state.startup_fifo_placeholders > 0
                && if self.runtime.blank_frame_active {
                    self.runtime.bg_pipeline_state.current_transfer_x >= 162
                } else {
                    matches!(self.runtime.bg_pipeline_state.current_transfer_x, 162 | 163)
                };
        let saturated_sprite_line_exact_x151_ready_tail_can_publish_hblank = saturated_sprite_line
            && self.runtime.bg_pipeline_state.current_transfer_x == 151
            && self.runtime.bg_pipeline_state.fifo.len() == 1
            && self.runtime.bg_pipeline_state.startup_fifo_placeholders == 0
            && (0..self.runtime.mode2_scan_state.selected_sprite_count())
                .filter(|&slot| {
                    self.runtime
                        .mode2_scan_state
                        .selected_sprite(slot)
                        .is_some_and(|sprite| sprite.x >= 15)
                })
                .count()
                >= 5;
        let saturated_sprite_line_exact_x159_ready_tail_can_publish_hblank = saturated_sprite_line
            && self.runtime.bg_pipeline_state.current_transfer_x == 159
            && self.runtime.bg_pipeline_state.fifo.len() == 1
            && self.runtime.bg_pipeline_state.startup_fifo_placeholders == 0
            && (0..self.runtime.mode2_scan_state.selected_sprite_count())
                .filter(|&slot| {
                    self.runtime
                        .mode2_scan_state
                        .selected_sprite(slot)
                        .is_some_and(|sprite| sprite.x >= 16)
                })
                .count()
                >= 5
            && self.current_mode0_start_dot() >= MODE0_START_DOT + 65;
        let saturated_sprite_line_placeholder_tail_can_publish_hblank = mode0_interrupt_enabled
            && saturated_sprite_line
            && self.runtime.bg_pipeline_state.current_transfer_x >= 164;
        let saturated_sprite_line_waiting_for_fifo_tail_can_publish_hblank = saturated_sprite_line
            && self.runtime.bg_pipeline_state.current_transfer_x >= 152
            && self.runtime.bg_pipeline_state.fifo.is_empty()
            && self.runtime.bg_pipeline_state.startup_fifo_placeholders == 0
            && (0..self.runtime.mode2_scan_state.selected_sprite_count())
                .filter(|&slot| {
                    self.runtime
                        .mode2_scan_state
                        .selected_sprite(slot)
                        .is_some_and(|sprite| sprite.x >= 10)
                })
                .count()
                >= 5;

        if self.dmg_wx0_scx3_window_tail_should_keep_published_drawing() {
            return false;
        }

        self.ly < VISIBLE_SCANLINES
            && self.line_dot + 1 == self.current_mode0_start_dot()
            && self.runtime.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.runtime.obj_pipeline_state.pending_match_x.is_none()
            && self
                .runtime
                .obj_pipeline_state
                .pending_sprite_slots
                .is_empty()
            && (((self.runtime.blank_frame_active
                && self.runtime.bg_pipeline_state.current_transfer_x >= 165)
                || self.runtime.bg_pipeline_state.current_transfer_x >= 167)
                || saturated_sprite_line_uses_earlier_terminal_hblank
                || saturated_sprite_line_placeholder_backed_visible_tail_can_publish_hblank
                || saturated_sprite_line_exact_x151_ready_tail_can_publish_hblank
                || saturated_sprite_line_exact_x159_ready_tail_can_publish_hblank
                || saturated_sprite_line_waiting_for_fifo_tail_can_publish_hblank)
            && (self.runtime.bg_pipeline_state.fifo_contains_real_pixels()
                || saturated_sprite_line_placeholder_backed_visible_tail_can_publish_hblank
                || saturated_sprite_line_placeholder_tail_can_publish_hblank
                || saturated_sprite_line_waiting_for_fifo_tail_can_publish_hblank)
            && self.current_transfer().is_some_and(|transfer| {
                matches!(transfer.context.lane, Mode3TransferLane::Visible)
                    && transfer.context.source_window == Mode3TransferSourceWindow::FifoBacked
                    && (matches!(transfer.readiness, Mode3TransferReadiness::Ready(_))
                        || (saturated_sprite_line_waiting_for_fifo_tail_can_publish_hblank
                            && matches!(
                                transfer.readiness,
                                Mode3TransferReadiness::WaitingForFifo(_)
                            )))
            })
    }

    fn dmg_wx0_scx3_window_tail_should_keep_published_drawing(&self) -> bool {
        let visible_registers = self.mode3_register_latches().visible();

        self.console_model.is_dmg_family()
            && self.ly < VISIBLE_SCANLINES
            && self.line_dot + 1 == self.current_mode0_start_dot()
            && self.runtime.mode2_scan_state.selected_sprite_count() == 0
            && self.runtime.bg_pipeline_state.window_started_this_line
            && self.runtime.bg_pipeline_state.fetcher.source == PpuBgFetcherSource::Window
            && visible_registers.window_enabled()
            && visible_registers.wx == 0
            && visible_registers.scx & 0x07 == 3
    }

    pub(in crate::ppu) fn saturated_placeholder_backed_terminal_bg_tail_should_publish_hblank_two_dots_early(
        &self,
    ) -> bool {
        self.ly < VISIBLE_SCANLINES
            && self.line_dot + 2 == self.current_mode0_start_dot()
            && [164_u8, 167].into_iter().any(|sprite_x| {
                usize::from(self.runtime.bg_pipeline_state.startup_fifo_placeholders)
                    == 168_usize.saturating_sub(sprite_x as usize)
                    && (0..self.runtime.mode2_scan_state.selected_sprite_count())
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
            && self.runtime.bg_pipeline_state.current_transfer_x >= 168
            && self.runtime.bg_pipeline_state.visible_pixels_output as usize >= SCREEN_WIDTH
            && self.runtime.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.runtime.obj_pipeline_state.pending_match_x.is_none()
            && self
                .runtime
                .obj_pipeline_state
                .pending_sprite_slots
                .is_empty()
            && self.runtime.bg_pipeline_state.push.pending
            && self.runtime.bg_pipeline_state.push.entry_delay_remaining == 0
    }

    pub(in crate::ppu) fn saturated_placeholder_backed_terminal_bg_tail_should_publish_hblank_one_dot_early(
        &self,
    ) -> bool {
        self.ly < VISIBLE_SCANLINES
            && self.line_dot == self.current_mode0_start_dot()
            && [165_u8, 166].into_iter().any(|sprite_x| {
                usize::from(self.runtime.bg_pipeline_state.startup_fifo_placeholders)
                    == 168_usize.saturating_sub(sprite_x as usize)
                    && (0..self.runtime.mode2_scan_state.selected_sprite_count())
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
            && self.runtime.bg_pipeline_state.current_transfer_x >= 168
            && self.runtime.bg_pipeline_state.visible_pixels_output as usize >= SCREEN_WIDTH
            && self.runtime.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.runtime.obj_pipeline_state.pending_match_x.is_none()
            && self
                .runtime
                .obj_pipeline_state
                .pending_sprite_slots
                .is_empty()
            && self.runtime.bg_pipeline_state.push.pending
            && self.runtime.bg_pipeline_state.push.entry_delay_remaining == 0
    }

    pub(in crate::ppu) fn terminal_x167_visible_same_x_cluster_should_publish_hblank_two_dots_early(
        &self,
    ) -> bool {
        self.ly < VISIBLE_SCANLINES
            && self.line_dot + 2 == self.current_mode0_start_dot()
            && usize::from(self.runtime.mode2_scan_state.selected_sprite_count())
                == MAX_SELECTED_SPRITES_PER_LINE
            && self.runtime.bg_pipeline_state.startup_fifo_placeholders == 1
            && self.runtime.bg_pipeline_state.current_transfer_x == 167
            && self.runtime.bg_pipeline_state.visible_pixels_output == 159
            && self.runtime.obj_pipeline_state.pending_match_x.is_none()
            && self
                .runtime
                .obj_pipeline_state
                .pending_sprite_slots
                .is_empty()
            && (0..self.runtime.mode2_scan_state.selected_sprite_count())
                .filter(|&slot| {
                    self.runtime
                        .mode2_scan_state
                        .selected_sprite(slot)
                        .is_some_and(|sprite| sprite.x == 167)
                })
                .count()
                >= 5
            && self.current_transfer().is_some_and(|transfer| {
                matches!(
                    transfer,
                    Mode3CurrentTransfer {
                        context: Mode3TransferContext {
                            lane: Mode3TransferLane::Visible,
                            source_window: Mode3TransferSourceWindow::FifoBacked,
                        },
                        readiness: Mode3TransferReadiness::Ready(_),
                    }
                )
            })
    }

    pub(in crate::ppu) fn terminal_x167_visible_same_x_cluster_should_publish_hblank_one_dot_early(
        &self,
    ) -> bool {
        self.ly < VISIBLE_SCANLINES
            && self.line_dot + 1 == self.current_mode0_start_dot()
            && usize::from(self.runtime.mode2_scan_state.selected_sprite_count())
                == MAX_SELECTED_SPRITES_PER_LINE
            && self.runtime.bg_pipeline_state.startup_fifo_placeholders == 0
            && self.runtime.bg_pipeline_state.current_transfer_x == 167
            && self.runtime.bg_pipeline_state.visible_pixels_output == 159
            && self.runtime.bg_pipeline_state.fifo.len() == 1
            && self.runtime.bg_pipeline_state.push.pending
            && self.runtime.bg_pipeline_state.push.entry_delay_remaining == 0
            && self.runtime.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.runtime.obj_pipeline_state.pending_match_x == Some(167)
            && self.runtime.obj_pipeline_state.pending_sprite_slots.len() == 1
            && self.fetched_same_x_obj_sprite_count_for_pending_match_x() >= 4
            && (0..self.runtime.mode2_scan_state.selected_sprite_count())
                .filter(|&slot| {
                    self.runtime
                        .mode2_scan_state
                        .selected_sprite(slot)
                        .is_some_and(|sprite| sprite.x == 167)
                })
                .count()
                >= 5
            && self.current_transfer().is_some_and(|transfer| {
                matches!(
                    transfer,
                    Mode3CurrentTransfer {
                        context: Mode3TransferContext {
                            lane: Mode3TransferLane::Visible,
                            source_window: Mode3TransferSourceWindow::FifoBacked,
                        },
                        readiness: Mode3TransferReadiness::Ready(_),
                    }
                )
            })
    }

    pub(in crate::ppu) fn single_left_sprite_placeholder_backed_tail_should_publish_hblank_early(
        &self,
    ) -> bool {
        let Some(selected_sprite) = self.runtime.mode2_scan_state.selected_sprite(0) else {
            return false;
        };
        let sprite_x = selected_sprite.x;
        if self.ly >= VISIBLE_SCANLINES
            || usize::from(self.runtime.mode2_scan_state.selected_sprite_count()) != 1
            || !(2..=4).contains(&sprite_x)
            || !(163..=165).contains(&self.runtime.bg_pipeline_state.current_transfer_x)
            || self.runtime.bg_pipeline_state.current_transfer_x != 161 + sprite_x
        {
            return false;
        }

        let publication_advance_dots =
            u16::from(167_u8.saturating_sub(self.runtime.bg_pipeline_state.current_transfer_x));

        publication_advance_dots > 0
            && self.line_dot + publication_advance_dots == self.current_mode0_start_dot()
            && self.runtime.bg_pipeline_state.visible_pixels_output
                == self.runtime.bg_pipeline_state.current_transfer_x - 8
            && self.runtime.bg_pipeline_state.startup_fifo_placeholders == 4
            && self.runtime.bg_pipeline_state.fifo.len()
                == usize::from(
                    168_u8.saturating_sub(self.runtime.bg_pipeline_state.current_transfer_x),
                )
            && self.runtime.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.runtime.obj_pipeline_state.pending_match_x.is_none()
            && self
                .runtime
                .obj_pipeline_state
                .pending_sprite_slots
                .is_empty()
            && self.current_transfer().is_some_and(|transfer| {
                matches!(
                    transfer,
                    Mode3CurrentTransfer {
                        context: Mode3TransferContext {
                            lane: Mode3TransferLane::Visible,
                            source_window: Mode3TransferSourceWindow::FifoBacked,
                        },
                        readiness: Mode3TransferReadiness::Ready(_),
                    }
                )
            })
    }

    pub(in crate::ppu) fn single_left_sprite_x4_placeholder_backed_preterminal_tail_should_publish_hblank_five_dots_early(
        &self,
    ) -> bool {
        self.ly < VISIBLE_SCANLINES
            && self.line_dot + 5 == self.current_mode0_start_dot()
            && usize::from(self.runtime.mode2_scan_state.selected_sprite_count()) == 1
            && self
                .runtime
                .mode2_scan_state
                .selected_sprite(0)
                .is_some_and(|sprite| sprite.x == 4)
            && self.runtime.bg_pipeline_state.current_transfer_x == 162
            && self.runtime.bg_pipeline_state.visible_pixels_output == 154
            && self.runtime.bg_pipeline_state.startup_fifo_placeholders == 4
            && self.runtime.bg_pipeline_state.fifo.len() == 6
            && self.runtime.bg_pipeline_state.push.pending
            && self.runtime.bg_pipeline_state.push.entry_delay_remaining == 1
            && self.runtime.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.runtime.obj_pipeline_state.pending_match_x.is_none()
            && self
                .runtime
                .obj_pipeline_state
                .pending_sprite_slots
                .is_empty()
            && self.current_transfer().is_some_and(|transfer| {
                matches!(
                    transfer,
                    Mode3CurrentTransfer {
                        context: Mode3TransferContext {
                            lane: Mode3TransferLane::Visible,
                            source_window: Mode3TransferSourceWindow::FifoBacked,
                        },
                        readiness: Mode3TransferReadiness::Ready(_),
                    }
                )
            })
    }

    pub(in crate::ppu) fn single_left_sprite_x5_placeholder_backed_preterminal_tail_should_publish_hblank_four_dots_early(
        &self,
    ) -> bool {
        self.ly < VISIBLE_SCANLINES
            && self.line_dot + 4 == self.current_mode0_start_dot()
            && usize::from(self.runtime.mode2_scan_state.selected_sprite_count()) == 1
            && self
                .runtime
                .mode2_scan_state
                .selected_sprite(0)
                .is_some_and(|sprite| sprite.x == 5)
            && self.runtime.bg_pipeline_state.current_transfer_x == 163
            && self.runtime.bg_pipeline_state.visible_pixels_output == 155
            && self.runtime.bg_pipeline_state.startup_fifo_placeholders == 3
            && self.runtime.bg_pipeline_state.fifo.len() == 5
            && self.runtime.bg_pipeline_state.push.pending
            && self.runtime.bg_pipeline_state.push.entry_delay_remaining == 1
            && self.runtime.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.runtime.obj_pipeline_state.pending_match_x.is_none()
            && self
                .runtime
                .obj_pipeline_state
                .pending_sprite_slots
                .is_empty()
            && self.current_transfer().is_some_and(|transfer| {
                matches!(
                    transfer,
                    Mode3CurrentTransfer {
                        context: Mode3TransferContext {
                            lane: Mode3TransferLane::Visible,
                            source_window: Mode3TransferSourceWindow::FifoBacked,
                        },
                        readiness: Mode3TransferReadiness::Ready(_),
                    }
                )
            })
    }

    pub(in crate::ppu) fn single_left_sprite_x6_to_x7_placeholder_backed_preterminal_tail_should_publish_hblank_from_fifo_tail(
        &self,
    ) -> bool {
        let Some(selected_sprite) = self.runtime.mode2_scan_state.selected_sprite(0) else {
            return false;
        };
        let current_transfer_x = self.runtime.bg_pipeline_state.current_transfer_x;
        let fifo_len = self.runtime.bg_pipeline_state.fifo.len();

        self.ly < VISIBLE_SCANLINES
            && usize::from(self.runtime.mode2_scan_state.selected_sprite_count()) == 1
            && (6..=7).contains(&selected_sprite.x)
            && (164..=165).contains(&current_transfer_x)
            && current_transfer_x == selected_sprite.x.saturating_add(158)
            && self.line_dot + (fifo_len as u16).saturating_sub(1) == self.current_mode0_start_dot()
            && self.runtime.bg_pipeline_state.visible_pixels_output == current_transfer_x - 8
            && self.runtime.bg_pipeline_state.startup_fifo_placeholders == 166 - current_transfer_x
            && fifo_len == usize::from(168_u8.saturating_sub(current_transfer_x))
            && self.runtime.bg_pipeline_state.push.pending
            && self.runtime.bg_pipeline_state.push.entry_delay_remaining == 1
            && self.runtime.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.runtime.obj_pipeline_state.pending_match_x.is_none()
            && self
                .runtime
                .obj_pipeline_state
                .pending_sprite_slots
                .is_empty()
            && self.current_transfer().is_some_and(|transfer| {
                matches!(
                    transfer,
                    Mode3CurrentTransfer {
                        context: Mode3TransferContext {
                            lane: Mode3TransferLane::Visible,
                            source_window: Mode3TransferSourceWindow::FifoBacked,
                        },
                        readiness: Mode3TransferReadiness::Ready(_),
                    }
                )
            })
    }

    pub(in crate::ppu) fn single_left_sprite_x12_to_x16_terminal_tail_with_entry_delay_should_publish_hblank_two_dots_early(
        &self,
    ) -> bool {
        self.ly < VISIBLE_SCANLINES
            && self.line_dot + 2 == self.current_mode0_start_dot()
            && usize::from(self.runtime.mode2_scan_state.selected_sprite_count()) == 1
            && self
                .runtime
                .mode2_scan_state
                .selected_sprite(0)
                .is_some_and(|sprite| {
                    (12..=16).contains(&sprite.x) || (0xA4..=0xA6).contains(&sprite.x)
                })
            && self.runtime.bg_pipeline_state.current_transfer_x == 166
            && self.runtime.bg_pipeline_state.visible_pixels_output == 158
            && self.runtime.bg_pipeline_state.startup_fifo_placeholders == 0
            && self.runtime.bg_pipeline_state.fifo.len() == 2
            && self.runtime.bg_pipeline_state.push.pending
            && self.runtime.bg_pipeline_state.push.entry_delay_remaining == 1
            && self.runtime.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.runtime.obj_pipeline_state.pending_match_x.is_none()
            && self
                .runtime
                .obj_pipeline_state
                .pending_sprite_slots
                .is_empty()
            && self.current_transfer().is_some_and(|transfer| {
                matches!(
                    transfer,
                    Mode3CurrentTransfer {
                        context: Mode3TransferContext {
                            lane: Mode3TransferLane::Visible,
                            source_window: Mode3TransferSourceWindow::FifoBacked,
                        },
                        readiness: Mode3TransferReadiness::Ready(_),
                    }
                )
            })
    }

    pub(in crate::ppu) fn single_offscreen_right_sprite_xa0_terminal_tail_without_entry_delay_should_publish_hblank_two_dots_early(
        &self,
    ) -> bool {
        self.ly < VISIBLE_SCANLINES
            && self.line_dot + 2 == self.current_mode0_start_dot()
            && usize::from(self.runtime.mode2_scan_state.selected_sprite_count()) == 1
            && self
                .runtime
                .mode2_scan_state
                .selected_sprite(0)
                .is_some_and(|sprite| sprite.x == 0xA0)
            && self.runtime.bg_pipeline_state.current_transfer_x == 166
            && self.runtime.bg_pipeline_state.visible_pixels_output == 158
            && self.runtime.bg_pipeline_state.startup_fifo_placeholders == 0
            && self.runtime.bg_pipeline_state.fifo.len() == 2
            && self.runtime.bg_pipeline_state.push.pending
            && self.runtime.bg_pipeline_state.push.entry_delay_remaining == 0
            && self.runtime.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.runtime.obj_pipeline_state.pending_match_x.is_none()
            && self
                .runtime
                .obj_pipeline_state
                .pending_sprite_slots
                .is_empty()
            && self.current_transfer().is_some_and(|transfer| {
                matches!(
                    transfer,
                    Mode3CurrentTransfer {
                        context: Mode3TransferContext {
                            lane: Mode3TransferLane::Visible,
                            source_window: Mode3TransferSourceWindow::FifoBacked,
                        },
                        readiness: Mode3TransferReadiness::Ready(_),
                    }
                )
            })
    }

    pub(in crate::ppu) fn single_offscreen_right_sprite_xa7_terminal_tail_should_publish_hblank_two_dots_early(
        &self,
    ) -> bool {
        self.ly < VISIBLE_SCANLINES
            && self.line_dot + 2 == self.current_mode0_start_dot()
            && usize::from(self.runtime.mode2_scan_state.selected_sprite_count()) == 1
            && self
                .runtime
                .mode2_scan_state
                .selected_sprite(0)
                .is_some_and(|sprite| sprite.x == 0xA7)
            && self.runtime.bg_pipeline_state.current_transfer_x == 167
            && self.runtime.bg_pipeline_state.visible_pixels_output == 159
            && self.runtime.bg_pipeline_state.startup_fifo_placeholders == 0
            && self.runtime.bg_pipeline_state.fifo.len() == 1
            && self.runtime.bg_pipeline_state.push.pending
            && self.runtime.bg_pipeline_state.push.entry_delay_remaining == 0
            && self.runtime.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Push
            && self.runtime.obj_pipeline_state.pending_match_x.is_none()
            && self
                .runtime
                .obj_pipeline_state
                .pending_sprite_slots
                .is_empty()
            && self.current_transfer().is_some_and(|transfer| {
                matches!(
                    transfer,
                    Mode3CurrentTransfer {
                        context: Mode3TransferContext {
                            lane: Mode3TransferLane::Visible,
                            source_window: Mode3TransferSourceWindow::FifoBacked,
                        },
                        readiness: Mode3TransferReadiness::Ready(_),
                    }
                )
            })
    }

    pub(in crate::ppu) fn single_offscreen_right_sprite_xa2_mode0_boundary_should_publish_hblank(
        &self,
    ) -> bool {
        self.ly < VISIBLE_SCANLINES
            && self.line_dot == self.current_mode0_start_dot()
            && usize::from(self.runtime.mode2_scan_state.selected_sprite_count()) == 1
            && self
                .runtime
                .mode2_scan_state
                .selected_sprite(0)
                .is_some_and(|sprite| sprite.x == 0xA2)
            && self.runtime.bg_pipeline_state.startup_fifo_placeholders == 0
            && self.runtime.bg_pipeline_state.current_transfer_x >= 168
            && self.runtime.bg_pipeline_state.fifo.is_empty()
            && !self.runtime.bg_pipeline_state.push.pending
            && self.runtime.obj_pipeline_state.pending_match_x.is_none()
            && self
                .runtime
                .obj_pipeline_state
                .pending_sprite_slots
                .is_empty()
            && self.current_transfer().is_none()
            && self.access_mode_for_line_dot(self.line_dot) == PpuAccessMode::HBlank
    }

    pub(in crate::ppu) fn two_sprite_staggered_fifo_tail_should_publish_hblank_from_fifo_tail(
        &self,
    ) -> bool {
        if self.ly >= VISIBLE_SCANLINES
            || usize::from(self.runtime.mode2_scan_state.selected_sprite_count()) != 2
            || self.runtime.bg_pipeline_state.fifo.is_empty()
            || self.runtime.bg_pipeline_state.push.pending
        {
            return false;
        }

        let Some(sprite_a) = self.runtime.mode2_scan_state.selected_sprite(0) else {
            return false;
        };
        let Some(sprite_b) = self.runtime.mode2_scan_state.selected_sprite(1) else {
            return false;
        };
        let (left_x, right_x) = if sprite_a.x <= sprite_b.x {
            (sprite_a.x, sprite_b.x)
        } else {
            (sprite_b.x, sprite_a.x)
        };
        let fifo_len = self.runtime.bg_pipeline_state.fifo.len();
        let current_transfer_x = self.runtime.bg_pipeline_state.current_transfer_x;
        let x2_x0a_tail = left_x == 0x02
            && right_x == 0x0A
            && self.runtime.bg_pipeline_state.startup_fifo_placeholders == 4
            && current_transfer_x == 164
            && fifo_len == 4;
        let x4_to_x7_visible_fifo_tail = (4..=7).contains(&left_x)
            && right_x == left_x.saturating_add(8)
            && right_x <= 0x0F
            && current_transfer_x < 168
            && usize::from(current_transfer_x) + fifo_len == 168
            && usize::from(current_transfer_x)
                + usize::from(self.runtime.bg_pipeline_state.startup_fifo_placeholders)
                == 163;

        (x2_x0a_tail || x4_to_x7_visible_fifo_tail)
            && self.line_dot + fifo_len as u16 - 2 == self.current_mode0_start_dot()
            && self.runtime.bg_pipeline_state.visible_pixels_output
                == current_transfer_x.saturating_sub(8)
            && self.runtime.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.runtime.obj_pipeline_state.pending_match_x.is_none()
            && self
                .runtime
                .obj_pipeline_state
                .pending_sprite_slots
                .is_empty()
            && self.current_transfer().is_some_and(|transfer| {
                matches!(
                    transfer,
                    Mode3CurrentTransfer {
                        context: Mode3TransferContext {
                            lane: Mode3TransferLane::Visible,
                            source_window: Mode3TransferSourceWindow::FifoBacked,
                        },
                        readiness: Mode3TransferReadiness::Ready(_),
                    }
                )
            })
    }

    pub(in crate::ppu) fn two_sprite_staggered_x8_to_x9_preterminal_tail_should_keep_published_drawing(
        &self,
    ) -> bool {
        if self.ly >= VISIBLE_SCANLINES
            || usize::from(self.runtime.mode2_scan_state.selected_sprite_count()) != 2
        {
            return false;
        }

        let Some(sprite_a) = self.runtime.mode2_scan_state.selected_sprite(0) else {
            return false;
        };
        let Some(sprite_b) = self.runtime.mode2_scan_state.selected_sprite(1) else {
            return false;
        };
        let (left_x, right_x) = if sprite_a.x <= sprite_b.x {
            (sprite_a.x, sprite_b.x)
        } else {
            (sprite_b.x, sprite_a.x)
        };

        (8..=9).contains(&left_x)
            && right_x == left_x.saturating_add(8)
            && self.line_dot + 1 == self.current_mode0_start_dot()
            && self.runtime.bg_pipeline_state.current_transfer_x == 167
            && self.runtime.bg_pipeline_state.visible_pixels_output == 159
            && self.runtime.bg_pipeline_state.startup_fifo_placeholders == 0
            && self.runtime.bg_pipeline_state.fifo.len() == 1
            && self.runtime.bg_pipeline_state.push.pending
            && self.runtime.bg_pipeline_state.push.entry_delay_remaining == 0
            && self.runtime.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.runtime.obj_pipeline_state.pending_match_x.is_none()
            && self
                .runtime
                .obj_pipeline_state
                .pending_sprite_slots
                .is_empty()
            && self.current_transfer().is_some_and(|transfer| {
                matches!(
                    transfer,
                    Mode3CurrentTransfer {
                        context: Mode3TransferContext {
                            lane: Mode3TransferLane::Visible,
                            source_window: Mode3TransferSourceWindow::FifoBacked,
                        },
                        readiness: Mode3TransferReadiness::Ready(_),
                    }
                )
            })
    }

    pub(in crate::ppu) fn two_sprite_staggered_x0_to_x1_terminal_tail_should_keep_published_drawing(
        &self,
    ) -> bool {
        if self.ly >= VISIBLE_SCANLINES
            || usize::from(self.runtime.mode2_scan_state.selected_sprite_count()) != 2
        {
            return false;
        }

        let Some(sprite_a) = self.runtime.mode2_scan_state.selected_sprite(0) else {
            return false;
        };
        let Some(sprite_b) = self.runtime.mode2_scan_state.selected_sprite(1) else {
            return false;
        };
        let (left_x, right_x) = if sprite_a.x <= sprite_b.x {
            (sprite_a.x, sprite_b.x)
        } else {
            (sprite_b.x, sprite_a.x)
        };
        let terminal_offset = self.line_dot.saturating_sub(self.current_mode0_start_dot());
        let expected_placeholders = match left_x {
            0 => 2,
            1 => 1,
            _ => return false,
        };

        right_x == left_x.saturating_add(8)
            && matches!(terminal_offset, 3 | 4)
            && self.runtime.bg_pipeline_state.current_transfer_x >= 168
            && self.runtime.bg_pipeline_state.visible_pixels_output as usize >= SCREEN_WIDTH
            && self.runtime.bg_pipeline_state.startup_fifo_placeholders == expected_placeholders
            && self.runtime.bg_pipeline_state.fifo.len() == 8
            && !self.runtime.bg_pipeline_state.push.pending
            && self.runtime.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.runtime.obj_pipeline_state.pending_match_x.is_none()
            && self
                .runtime
                .obj_pipeline_state
                .pending_sprite_slots
                .is_empty()
            && self.current_transfer().is_none()
    }

    pub(in crate::ppu) fn two_sprite_staggered_x9_terminal_boundary_should_keep_published_drawing(
        &self,
    ) -> bool {
        if self.ly >= VISIBLE_SCANLINES
            || usize::from(self.runtime.mode2_scan_state.selected_sprite_count()) != 2
        {
            return false;
        }

        let Some(sprite_a) = self.runtime.mode2_scan_state.selected_sprite(0) else {
            return false;
        };
        let Some(sprite_b) = self.runtime.mode2_scan_state.selected_sprite(1) else {
            return false;
        };
        let (left_x, right_x) = if sprite_a.x <= sprite_b.x {
            (sprite_a.x, sprite_b.x)
        } else {
            (sprite_b.x, sprite_a.x)
        };

        left_x == 9
            && right_x == 17
            && self.line_dot == self.current_mode0_start_dot().saturating_add(1)
            && self.runtime.bg_pipeline_state.current_transfer_x >= 168
            && self.runtime.bg_pipeline_state.visible_pixels_output as usize >= SCREEN_WIDTH
            && self.runtime.bg_pipeline_state.startup_fifo_placeholders == 0
            && self.runtime.bg_pipeline_state.fifo.is_empty()
            && !self.runtime.bg_pipeline_state.push.pending
            && self.runtime.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.runtime.obj_pipeline_state.pending_match_x.is_none()
            && self
                .runtime
                .obj_pipeline_state
                .pending_sprite_slots
                .is_empty()
            && self.current_transfer().is_none()
    }

    pub(in crate::ppu) fn ten_sprite_step8_terminal_tail_should_keep_published_drawing(
        &self,
    ) -> bool {
        let Some(min_x) = self.selected_sprite_step8_ramp_min_x() else {
            return false;
        };

        let terminal_offset = self.line_dot.saturating_sub(self.current_mode0_start_dot());
        let matches_family = match min_x {
            0 => {
                self.runtime.bg_pipeline_state.startup_fifo_placeholders == 2
                    && self.runtime.bg_pipeline_state.push.pending
                    && terminal_offset <= 27
            }
            1 => {
                self.runtime.bg_pipeline_state.startup_fifo_placeholders == 1
                    && !self.runtime.bg_pipeline_state.push.pending
                    && terminal_offset <= 20
            }
            2 => {
                self.runtime.bg_pipeline_state.startup_fifo_placeholders == 4
                    && self.runtime.bg_pipeline_state.push.pending
                    && terminal_offset <= 18
            }
            _ => false,
        };

        matches_family
            && self.runtime.bg_pipeline_state.mode3_started
            && self.runtime.bg_pipeline_state.visible_pixels_output as usize >= SCREEN_WIDTH
            && self.runtime.bg_pipeline_state.current_transfer_x >= 168
            && self.runtime.bg_pipeline_state.fifo.len() == 8
            && self.runtime.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.runtime.obj_pipeline_state.pending_match_x.is_none()
            && self
                .runtime
                .obj_pipeline_state
                .pending_sprite_slots
                .is_empty()
            && self.current_transfer().is_none()
    }

    pub(in crate::ppu) fn ten_sprite_step8_preterminal_tail_should_publish_hblank_early(
        &self,
    ) -> bool {
        let Some(min_x) = self.selected_sprite_step8_ramp_min_x() else {
            return false;
        };
        if !(4..=7).contains(&min_x) {
            return false;
        }

        let expected_placeholders = 8_u8.saturating_sub(min_x);
        let transfer_plus_fifo = usize::from(self.runtime.bg_pipeline_state.current_transfer_x)
            + self.runtime.bg_pipeline_state.fifo.len();
        let matches_transfer_sum = if min_x == 4 {
            matches!(transfer_plus_fifo, 136 | 168)
        } else {
            matches!(transfer_plus_fifo, 128 | 160)
        };
        let Some(transfer) = self.current_transfer() else {
            return false;
        };

        matches!(
            transfer,
            Mode3CurrentTransfer {
                context: Mode3TransferContext {
                    lane: Mode3TransferLane::Visible,
                    source_window: Mode3TransferSourceWindow::FifoBacked,
                    ..
                },
                readiness: Mode3TransferReadiness::Ready(_),
                ..
            }
        ) && self.runtime.bg_pipeline_state.mode3_started
            && !self.runtime.bg_pipeline_state.push.pending
            && self.runtime.bg_pipeline_state.startup_fifo_placeholders == expected_placeholders
            && matches_transfer_sum
            && self.runtime.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.runtime.obj_pipeline_state.pending_match_x.is_none()
            && self
                .runtime
                .obj_pipeline_state
                .pending_sprite_slots
                .is_empty()
    }

    pub(in crate::ppu) fn selected_sprite_step8_ramp_min_x(&self) -> Option<u8> {
        let sprite_count = usize::from(self.runtime.mode2_scan_state.selected_sprite_count());
        if sprite_count != MAX_SELECTED_SPRITES_PER_LINE {
            return None;
        }

        let mut xs = [0_u8; MAX_SELECTED_SPRITES_PER_LINE];
        for (slot, x) in xs.iter_mut().enumerate().take(sprite_count) {
            let sprite = self.runtime.mode2_scan_state.selected_sprite(slot as u8)?;
            *x = sprite.x;
        }
        xs.sort_unstable();
        if xs
            .windows(2)
            .all(|pair| pair[1] == pair[0].saturating_add(8))
        {
            Some(xs[0])
        } else {
            None
        }
    }

    pub(in crate::ppu) fn current_published_oam_write_access_mode(&self) -> PpuAccessMode {
        let published_mode = self.current_published_video_write_access_mode();
        self.current_published_oam_write_access_mode_from(published_mode)
    }

    pub(in crate::ppu) fn current_published_oam_write_access_mode_from(
        &self,
        published_mode: PpuAccessMode,
    ) -> PpuAccessMode {
        if published_mode == PpuAccessMode::OamScan
            && self.ly < VISIBLE_SCANLINES
            && self.line_dot == MODE2_DOTS
        {
            PpuAccessMode::HBlank
        } else {
            published_mode
        }
    }

    pub(in crate::ppu) fn current_published_oam_read_access_mode(&self) -> PpuAccessMode {
        let published_mode = self.current_published_bus_access_mode();
        self.current_published_oam_read_access_mode_from(published_mode)
    }

    pub(in crate::ppu) fn current_published_oam_read_access_mode_from(
        &self,
        published_mode: PpuAccessMode,
    ) -> PpuAccessMode {
        if published_mode == PpuAccessMode::Drawing
            && !self.runtime.blank_frame_active
            && self.ly < VISIBLE_SCANLINES
            && ((self.access_mode_for_line_dot(self.line_dot) == PpuAccessMode::HBlank
                && self.line_dot == self.current_mode0_start_dot())
                || (self.line_dot != 0
                    && self.access_mode_for_line_dot(self.line_dot - 1) == PpuAccessMode::OamScan
                    && self.access_mode_for_line_dot(self.line_dot) == PpuAccessMode::Drawing
                    && self.line_dot == MODE2_DOTS))
        {
            PpuAccessMode::HBlank
        } else {
            published_mode
        }
    }
}
