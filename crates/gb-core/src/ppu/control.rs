use super::*;

impl Ppu {
    pub(super) const fn current_mmio_visible_registers(&self) -> PpuVisibleRegisters {
        PpuVisibleRegisters {
            lcdc: self.lcdc,
            scy: self.scy,
            scx: self.scx,
            bgp: self.bgp,
            obp0: self.obp0,
            obp1: self.obp1,
            wy: self.wy,
            wx: self.wx,
        }
    }

    pub(super) const fn mode3_register_latches(&self) -> PpuMode3RegisterLatches {
        PpuMode3RegisterLatches::new(self.visible_registers, self.pipeline_registers)
    }

    pub(super) const fn current_mode3_live_register_write_context(
        &self,
        previous_mmio_registers: PpuVisibleRegisters,
    ) -> PpuMode3LiveRegisterWriteContext {
        PpuMode3LiveRegisterWriteContext::new(
            previous_mmio_registers,
            self.current_mmio_visible_registers(),
        )
    }

    pub(super) const fn current_mode3_live_background_refetch_context(
        &self,
    ) -> PpuMode3LiveBackgroundRefetchContext {
        PpuMode3LiveBackgroundRefetchContext::new(
            self.current_mmio_visible_registers(),
            self.ly,
            self.last_unsigned_tile_data_low_fetch,
            self.last_unsigned_tile_data_high_fetch,
        )
    }

    pub(super) fn set_mode3_register_latches(&mut self, latches: PpuMode3RegisterLatches) {
        self.visible_registers = latches.visible();
        self.pipeline_registers = latches.pipeline();
    }

    pub(super) fn reload_mode3_register_latches_from_mmio(&mut self) {
        self.set_mode3_register_latches(PpuMode3RegisterLatches::from_mmio(
            self.current_mmio_visible_registers(),
        ));
    }

    pub(super) fn advance_mode3_register_latches_from_mmio(&mut self) {
        self.set_mode3_register_latches(
            self.mode3_register_latches()
                .advance(self.current_mmio_visible_registers()),
        );
    }

    pub(super) fn is_lcd_enabled(&self) -> bool {
        self.lcd_state.is_enabled()
    }

    #[cfg(test)]
    pub(super) fn sync_visible_registers(&mut self) {
        self.visible_registers = self.current_mmio_visible_registers();
    }

    #[cfg(test)]
    pub(super) fn sync_pipeline_registers(&mut self) {
        self.pipeline_registers = self.visible_registers;
    }

    pub(super) fn read_lcdc(&self) -> u8 {
        self.lcdc
    }

    pub(super) fn write_lcdc(&mut self, value: u8, source: PpuRegisterWriteSource) {
        let was_lcd_enabled = self.is_lcd_enabled() || self.lcd_enable_pending_delay_tcycles != 0;
        self.lcdc = value;
        self.startup_mode_latch = None;

        match (was_lcd_enabled, value & LCDC_ENABLE_BIT != 0) {
            (true, false) => self.enter_lcd_disabled_state(),
            (false, true) => {
                if source == PpuRegisterWriteSource::CpuMmioCommit {
                    self.enter_lcd_enable_pending_state(CPU_LCDC_ENABLE_EFFECT_DELAY_T_CYCLES);
                } else {
                    self.enter_lcd_enabled_restart_state();
                }
            }
            _ => {
                self.lcd_state = lcd_state_from_lcdc(value);
                self.refresh_visible_output();
            }
        }

        self.refresh_stat_irq_line(false);
    }

    pub(super) fn read_stat(&self, source: PpuRegisterReadSource) -> u8 {
        STAT_FORCED_HIGH_BIT
            | self.stat_interrupt_enable
            | if self.read_stat_lyc_coincidence(source) {
                0x04
            } else {
                0x00
            }
            | if self.is_lcd_enabled() {
                match source {
                    PpuRegisterReadSource::Immediate => self.current_cpu_visible_access_mode(),
                    PpuRegisterReadSource::CpuBusOperation => {
                        self.current_published_stat_access_mode()
                    }
                }
                .stat_bits()
            } else {
                PpuAccessMode::HBlank.stat_bits()
            }
    }

    pub(super) fn read_stat_lyc_coincidence(&self, source: PpuRegisterReadSource) -> bool {
        if source == PpuRegisterReadSource::CpuBusOperation
            && self.is_lcd_enabled()
            && self.line_dot == 0
        {
            false
        } else {
            self.effective_lyc_coincidence()
        }
    }

    pub(super) fn write_stat(&mut self, value: u8) {
        self.stat_interrupt_enable = value & STAT_WRITABLE_ENABLE_MASK;
        self.refresh_stat_irq_line(self.stat_write_quirk_active());
    }

    pub(super) fn read_ly(&self) -> u8 {
        if self.is_lcd_enabled()
            && !self.blank_frame_active
            && self.line_dot >= self.current_ly_read_advance_start_dot()
            && self.ly + 1 < TOTAL_SCANLINES
        {
            self.ly + 1
        } else {
            self.ly
        }
    }

    pub(super) fn current_access_mode(&self) -> PpuAccessMode {
        self.current_raster_state().access_mode()
    }

    pub(super) fn access_mode_for_line_dot(&self, line_dot: u16) -> PpuAccessMode {
        if !self.is_lcd_enabled() {
            return PpuAccessMode::HBlank;
        }

        if let Some(raster_state) = self.lcd_restart_phase.raster_state(self.ly, line_dot) {
            return raster_state.access_mode();
        }

        access_mode_from_raster(self.ly, line_dot, self.current_mode0_start_dot())
    }

    pub(super) fn bus_access_mode_for_line_dot(&self, line_dot: u16) -> PpuAccessMode {
        let current_mode = self.access_mode_for_line_dot(line_dot);

        if !self.is_lcd_enabled() || self.ly >= VISIBLE_SCANLINES {
            return current_mode;
        }

        if current_mode == PpuAccessMode::HBlank
            && self.ly + 1 < VISIBLE_SCANLINES
            && line_dot + 4 >= self.current_scanline_length()
        {
            return PpuAccessMode::OamScan;
        }

        if current_mode == PpuAccessMode::OamScan && line_dot + 4 >= MODE2_DOTS {
            return PpuAccessMode::Drawing;
        }

        current_mode
    }

    pub(super) fn current_published_bus_access_mode(&self) -> PpuAccessMode {
        let published_line_dot = self.line_dot.saturating_sub(1);
        self.bus_access_mode_for_line_dot(published_line_dot)
    }

    pub(super) fn current_published_video_write_access_mode(&self) -> PpuAccessMode {
        if self.line_dot != 0 {
            self.access_mode_for_line_dot(self.line_dot - 1)
        } else if self.ly == 0 {
            self.current_access_mode()
        } else if self.ly > VISIBLE_SCANLINES {
            PpuAccessMode::VBlank
        } else {
            PpuAccessMode::HBlank
        }
    }

    pub(super) fn current_published_stat_access_mode(&self) -> PpuAccessMode {
        if self.line_dot != 0 {
            let published_mode = self.access_mode_for_line_dot(self.line_dot - 1);
            let sprite_extended_mode3 =
                self.current_mode0_start_dot() > self.baseline_mode0_start_dot();

            if published_mode == PpuAccessMode::OamScan
                && self.access_mode_for_line_dot(self.line_dot) == PpuAccessMode::Drawing
                && !self.blank_frame_active
                && self.ly < VISIBLE_SCANLINES
                && self.line_dot == MODE2_DOTS
            {
                return PpuAccessMode::Drawing;
            }

            if published_mode == PpuAccessMode::Drawing
                && self.saturated_placeholder_backed_terminal_bg_tail_should_publish_hblank_two_dots_early()
            {
                return PpuAccessMode::HBlank;
            }

            if published_mode == PpuAccessMode::Drawing
                && self.terminal_x167_visible_same_x_cluster_should_publish_hblank_two_dots_early()
            {
                return PpuAccessMode::HBlank;
            }

            if published_mode == PpuAccessMode::Drawing
                && self.saturated_placeholder_backed_terminal_bg_tail_should_publish_hblank_one_dot_early()
            {
                return PpuAccessMode::HBlank;
            }

            if published_mode == PpuAccessMode::Drawing
                && self.terminal_x167_visible_same_x_cluster_should_publish_hblank_one_dot_early()
            {
                return PpuAccessMode::HBlank;
            }

            if published_mode == PpuAccessMode::Drawing
                && self.single_left_sprite_placeholder_backed_tail_should_publish_hblank_early()
            {
                return PpuAccessMode::HBlank;
            }

            if published_mode == PpuAccessMode::Drawing
                && self.single_left_sprite_x4_placeholder_backed_preterminal_tail_should_publish_hblank_five_dots_early()
            {
                return PpuAccessMode::HBlank;
            }

            if published_mode == PpuAccessMode::Drawing
                && self.single_left_sprite_x5_placeholder_backed_preterminal_tail_should_publish_hblank_four_dots_early()
            {
                return PpuAccessMode::HBlank;
            }

            if published_mode == PpuAccessMode::Drawing
                && self.single_left_sprite_x6_to_x7_placeholder_backed_preterminal_tail_should_publish_hblank_from_fifo_tail()
            {
                return PpuAccessMode::HBlank;
            }

            if published_mode == PpuAccessMode::Drawing
                && self.single_left_sprite_x12_to_x16_terminal_tail_with_entry_delay_should_publish_hblank_two_dots_early()
            {
                return PpuAccessMode::HBlank;
            }

            if published_mode == PpuAccessMode::Drawing
                && self.single_offscreen_right_sprite_xa0_terminal_tail_without_entry_delay_should_publish_hblank_two_dots_early()
            {
                return PpuAccessMode::HBlank;
            }

            if published_mode == PpuAccessMode::Drawing
                && self.single_offscreen_right_sprite_xa7_terminal_tail_should_publish_hblank_two_dots_early()
            {
                return PpuAccessMode::HBlank;
            }

            if published_mode == PpuAccessMode::Drawing
                && self.single_offscreen_right_sprite_xa2_mode0_boundary_should_publish_hblank()
            {
                return PpuAccessMode::HBlank;
            }

            if published_mode == PpuAccessMode::Drawing
                && self.two_sprite_staggered_fifo_tail_should_publish_hblank_from_fifo_tail()
            {
                return PpuAccessMode::HBlank;
            }

            if published_mode == PpuAccessMode::Drawing
                && self.ten_sprite_step8_preterminal_tail_should_publish_hblank_early()
            {
                return PpuAccessMode::HBlank;
            }

            if published_mode == PpuAccessMode::Drawing
                && self.terminal_visible_tail_should_publish_hblank_early()
                && !self
                    .two_sprite_staggered_x8_to_x9_preterminal_tail_should_keep_published_drawing()
                && !self.saturated_placeholder_backed_terminal_bg_tail_still_owned_by_mode3()
            {
                return PpuAccessMode::HBlank;
            }

            if published_mode == PpuAccessMode::HBlank
                && self.two_sprite_staggered_x0_to_x1_terminal_tail_should_keep_published_drawing()
            {
                return PpuAccessMode::Drawing;
            }

            if published_mode == PpuAccessMode::HBlank
                && self.two_sprite_staggered_x9_terminal_boundary_should_keep_published_drawing()
            {
                return PpuAccessMode::Drawing;
            }

            if published_mode == PpuAccessMode::HBlank
                && self.ten_sprite_step8_terminal_tail_should_keep_published_drawing()
            {
                return PpuAccessMode::Drawing;
            }

            if published_mode == PpuAccessMode::Drawing
                && self.access_mode_for_line_dot(self.line_dot) == PpuAccessMode::HBlank
                && !self.blank_frame_active
                && self.ly < VISIBLE_SCANLINES
                && self.line_dot == self.current_mode0_start_dot()
                && !sprite_extended_mode3
            {
                return PpuAccessMode::HBlank;
            }

            if published_mode == PpuAccessMode::HBlank
                && !self.blank_frame_active
                && self.ly < VISIBLE_SCANLINES
                && sprite_extended_mode3
                && self.line_dot == self.current_mode0_start_dot().saturating_add(2)
            {
                return PpuAccessMode::Drawing;
            }

            return published_mode;
        }

        if self.ly == 0 {
            return self.current_access_mode();
        }

        if self.ly > VISIBLE_SCANLINES {
            PpuAccessMode::VBlank
        } else {
            PpuAccessMode::HBlank
        }
    }

    pub(super) fn terminal_visible_tail_should_publish_hblank_early(&self) -> bool {
        let mode0_interrupt_enabled =
            self.stat_interrupt_enable & STAT_MODE0_INTERRUPT_ENABLE_BIT != 0;
        let saturated_sprite_line = usize::from(self.mode2_scan_state.selected_sprite_count())
            == MAX_SELECTED_SPRITES_PER_LINE;
        let saturated_sprite_line_uses_earlier_terminal_hblank =
            saturated_sprite_line && self.bg_pipeline_state.current_transfer_x == 163;
        let saturated_sprite_line_placeholder_backed_visible_tail_can_publish_hblank =
            saturated_sprite_line
                && self.bg_pipeline_state.startup_fifo_placeholders > 0
                && if self.blank_frame_active {
                    self.bg_pipeline_state.current_transfer_x >= 162
                } else {
                    matches!(self.bg_pipeline_state.current_transfer_x, 162 | 163)
                };
        let saturated_sprite_line_exact_x151_ready_tail_can_publish_hblank = saturated_sprite_line
            && self.bg_pipeline_state.current_transfer_x == 151
            && self.bg_pipeline_state.fifo.len() == 1
            && self.bg_pipeline_state.startup_fifo_placeholders == 0
            && (0..self.mode2_scan_state.selected_sprite_count())
                .filter(|&slot| {
                    self.mode2_scan_state
                        .selected_sprite(slot)
                        .is_some_and(|sprite| sprite.x >= 15)
                })
                .count()
                >= 5;
        let saturated_sprite_line_exact_x159_ready_tail_can_publish_hblank = saturated_sprite_line
            && self.bg_pipeline_state.current_transfer_x == 159
            && self.bg_pipeline_state.fifo.len() == 1
            && self.bg_pipeline_state.startup_fifo_placeholders == 0
            && (0..self.mode2_scan_state.selected_sprite_count())
                .filter(|&slot| {
                    self.mode2_scan_state
                        .selected_sprite(slot)
                        .is_some_and(|sprite| sprite.x >= 16)
                })
                .count()
                >= 5
            && self.current_mode0_start_dot() >= MODE0_START_DOT + 65;
        let saturated_sprite_line_placeholder_tail_can_publish_hblank = mode0_interrupt_enabled
            && saturated_sprite_line
            && self.bg_pipeline_state.current_transfer_x >= 164;
        let saturated_sprite_line_waiting_for_fifo_tail_can_publish_hblank = saturated_sprite_line
            && self.bg_pipeline_state.current_transfer_x >= 152
            && self.bg_pipeline_state.fifo.is_empty()
            && self.bg_pipeline_state.startup_fifo_placeholders == 0
            && (0..self.mode2_scan_state.selected_sprite_count())
                .filter(|&slot| {
                    self.mode2_scan_state
                        .selected_sprite(slot)
                        .is_some_and(|sprite| sprite.x >= 10)
                })
                .count()
                >= 5;

        self.ly < VISIBLE_SCANLINES
            && self.line_dot + 1 == self.current_mode0_start_dot()
            && self.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.obj_pipeline_state.pending_match_x.is_none()
            && self.obj_pipeline_state.pending_sprite_slots.is_empty()
            && (((self.blank_frame_active && self.bg_pipeline_state.current_transfer_x >= 165)
                || self.bg_pipeline_state.current_transfer_x >= 167)
                || saturated_sprite_line_uses_earlier_terminal_hblank
                || saturated_sprite_line_placeholder_backed_visible_tail_can_publish_hblank
                || saturated_sprite_line_exact_x151_ready_tail_can_publish_hblank
                || saturated_sprite_line_exact_x159_ready_tail_can_publish_hblank
                || saturated_sprite_line_waiting_for_fifo_tail_can_publish_hblank)
            && (self.bg_pipeline_state.fifo_contains_real_pixels()
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

    pub(super) fn saturated_placeholder_backed_terminal_bg_tail_should_publish_hblank_two_dots_early(
        &self,
    ) -> bool {
        self.ly < VISIBLE_SCANLINES
            && self.line_dot + 2 == self.current_mode0_start_dot()
            && [164_u8, 167].into_iter().any(|sprite_x| {
                usize::from(self.bg_pipeline_state.startup_fifo_placeholders)
                    == 168_usize.saturating_sub(sprite_x as usize)
                    && (0..self.mode2_scan_state.selected_sprite_count())
                        .filter(|&slot| {
                            self.mode2_scan_state
                                .selected_sprite(slot)
                                .is_some_and(|sprite| sprite.x == sprite_x)
                        })
                        .count()
                        >= 5
            })
            && usize::from(self.mode2_scan_state.selected_sprite_count())
                == MAX_SELECTED_SPRITES_PER_LINE
            && self.bg_pipeline_state.current_transfer_x >= 168
            && self.bg_pipeline_state.visible_pixels_output as usize >= SCREEN_WIDTH
            && self.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.obj_pipeline_state.pending_match_x.is_none()
            && self.obj_pipeline_state.pending_sprite_slots.is_empty()
            && self.bg_pipeline_state.push.pending
            && self.bg_pipeline_state.push.entry_delay_remaining == 0
    }

    pub(super) fn saturated_placeholder_backed_terminal_bg_tail_should_publish_hblank_one_dot_early(
        &self,
    ) -> bool {
        self.ly < VISIBLE_SCANLINES
            && self.line_dot == self.current_mode0_start_dot()
            && [165_u8, 166].into_iter().any(|sprite_x| {
                usize::from(self.bg_pipeline_state.startup_fifo_placeholders)
                    == 168_usize.saturating_sub(sprite_x as usize)
                    && (0..self.mode2_scan_state.selected_sprite_count())
                        .filter(|&slot| {
                            self.mode2_scan_state
                                .selected_sprite(slot)
                                .is_some_and(|sprite| sprite.x == sprite_x)
                        })
                        .count()
                        >= 5
            })
            && usize::from(self.mode2_scan_state.selected_sprite_count())
                == MAX_SELECTED_SPRITES_PER_LINE
            && self.bg_pipeline_state.current_transfer_x >= 168
            && self.bg_pipeline_state.visible_pixels_output as usize >= SCREEN_WIDTH
            && self.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.obj_pipeline_state.pending_match_x.is_none()
            && self.obj_pipeline_state.pending_sprite_slots.is_empty()
            && self.bg_pipeline_state.push.pending
            && self.bg_pipeline_state.push.entry_delay_remaining == 0
    }

    pub(super) fn terminal_x167_visible_same_x_cluster_should_publish_hblank_two_dots_early(
        &self,
    ) -> bool {
        self.ly < VISIBLE_SCANLINES
            && self.line_dot + 2 == self.current_mode0_start_dot()
            && usize::from(self.mode2_scan_state.selected_sprite_count())
                == MAX_SELECTED_SPRITES_PER_LINE
            && self.bg_pipeline_state.startup_fifo_placeholders == 1
            && self.bg_pipeline_state.current_transfer_x == 167
            && self.bg_pipeline_state.visible_pixels_output == 159
            && self.obj_pipeline_state.pending_match_x.is_none()
            && self.obj_pipeline_state.pending_sprite_slots.is_empty()
            && (0..self.mode2_scan_state.selected_sprite_count())
                .filter(|&slot| {
                    self.mode2_scan_state
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

    pub(super) fn terminal_x167_visible_same_x_cluster_should_publish_hblank_one_dot_early(
        &self,
    ) -> bool {
        self.ly < VISIBLE_SCANLINES
            && self.line_dot + 1 == self.current_mode0_start_dot()
            && usize::from(self.mode2_scan_state.selected_sprite_count())
                == MAX_SELECTED_SPRITES_PER_LINE
            && self.bg_pipeline_state.startup_fifo_placeholders == 0
            && self.bg_pipeline_state.current_transfer_x == 167
            && self.bg_pipeline_state.visible_pixels_output == 159
            && self.bg_pipeline_state.fifo.len() == 1
            && self.bg_pipeline_state.push.pending
            && self.bg_pipeline_state.push.entry_delay_remaining == 0
            && self.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.obj_pipeline_state.pending_match_x == Some(167)
            && self.obj_pipeline_state.pending_sprite_slots.len() == 1
            && self.fetched_same_x_obj_sprite_count_for_pending_match_x() >= 4
            && (0..self.mode2_scan_state.selected_sprite_count())
                .filter(|&slot| {
                    self.mode2_scan_state
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

    pub(super) fn single_left_sprite_placeholder_backed_tail_should_publish_hblank_early(
        &self,
    ) -> bool {
        let Some(selected_sprite) = self.mode2_scan_state.selected_sprite(0) else {
            return false;
        };
        let sprite_x = selected_sprite.x;
        if self.ly >= VISIBLE_SCANLINES
            || usize::from(self.mode2_scan_state.selected_sprite_count()) != 1
            || !(2..=4).contains(&sprite_x)
            || !(163..=165).contains(&self.bg_pipeline_state.current_transfer_x)
            || self.bg_pipeline_state.current_transfer_x != 161 + sprite_x
        {
            return false;
        }

        let publication_advance_dots =
            u16::from(167_u8.saturating_sub(self.bg_pipeline_state.current_transfer_x));

        publication_advance_dots > 0
            && self.line_dot + publication_advance_dots == self.current_mode0_start_dot()
            && self.bg_pipeline_state.visible_pixels_output
                == self.bg_pipeline_state.current_transfer_x - 8
            && self.bg_pipeline_state.startup_fifo_placeholders == 4
            && self.bg_pipeline_state.fifo.len()
                == usize::from(168_u8.saturating_sub(self.bg_pipeline_state.current_transfer_x))
            && self.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.obj_pipeline_state.pending_match_x.is_none()
            && self.obj_pipeline_state.pending_sprite_slots.is_empty()
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

    pub(super) fn single_left_sprite_x4_placeholder_backed_preterminal_tail_should_publish_hblank_five_dots_early(
        &self,
    ) -> bool {
        self.ly < VISIBLE_SCANLINES
            && self.line_dot + 5 == self.current_mode0_start_dot()
            && usize::from(self.mode2_scan_state.selected_sprite_count()) == 1
            && self
                .mode2_scan_state
                .selected_sprite(0)
                .is_some_and(|sprite| sprite.x == 4)
            && self.bg_pipeline_state.current_transfer_x == 162
            && self.bg_pipeline_state.visible_pixels_output == 154
            && self.bg_pipeline_state.startup_fifo_placeholders == 4
            && self.bg_pipeline_state.fifo.len() == 6
            && self.bg_pipeline_state.push.pending
            && self.bg_pipeline_state.push.entry_delay_remaining == 1
            && self.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.obj_pipeline_state.pending_match_x.is_none()
            && self.obj_pipeline_state.pending_sprite_slots.is_empty()
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

    pub(super) fn single_left_sprite_x5_placeholder_backed_preterminal_tail_should_publish_hblank_four_dots_early(
        &self,
    ) -> bool {
        self.ly < VISIBLE_SCANLINES
            && self.line_dot + 4 == self.current_mode0_start_dot()
            && usize::from(self.mode2_scan_state.selected_sprite_count()) == 1
            && self
                .mode2_scan_state
                .selected_sprite(0)
                .is_some_and(|sprite| sprite.x == 5)
            && self.bg_pipeline_state.current_transfer_x == 163
            && self.bg_pipeline_state.visible_pixels_output == 155
            && self.bg_pipeline_state.startup_fifo_placeholders == 3
            && self.bg_pipeline_state.fifo.len() == 5
            && self.bg_pipeline_state.push.pending
            && self.bg_pipeline_state.push.entry_delay_remaining == 1
            && self.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.obj_pipeline_state.pending_match_x.is_none()
            && self.obj_pipeline_state.pending_sprite_slots.is_empty()
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

    pub(super) fn single_left_sprite_x6_to_x7_placeholder_backed_preterminal_tail_should_publish_hblank_from_fifo_tail(
        &self,
    ) -> bool {
        let Some(selected_sprite) = self.mode2_scan_state.selected_sprite(0) else {
            return false;
        };
        let current_transfer_x = self.bg_pipeline_state.current_transfer_x;
        let fifo_len = self.bg_pipeline_state.fifo.len();

        self.ly < VISIBLE_SCANLINES
            && usize::from(self.mode2_scan_state.selected_sprite_count()) == 1
            && (6..=7).contains(&selected_sprite.x)
            && (164..=165).contains(&current_transfer_x)
            && current_transfer_x == selected_sprite.x.saturating_add(158)
            && self.line_dot + (fifo_len as u16).saturating_sub(1) == self.current_mode0_start_dot()
            && self.bg_pipeline_state.visible_pixels_output == current_transfer_x - 8
            && self.bg_pipeline_state.startup_fifo_placeholders == 166 - current_transfer_x
            && fifo_len == usize::from(168_u8.saturating_sub(current_transfer_x))
            && self.bg_pipeline_state.push.pending
            && self.bg_pipeline_state.push.entry_delay_remaining == 1
            && self.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.obj_pipeline_state.pending_match_x.is_none()
            && self.obj_pipeline_state.pending_sprite_slots.is_empty()
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

    pub(super) fn single_left_sprite_x12_to_x16_terminal_tail_with_entry_delay_should_publish_hblank_two_dots_early(
        &self,
    ) -> bool {
        self.ly < VISIBLE_SCANLINES
            && self.line_dot + 2 == self.current_mode0_start_dot()
            && usize::from(self.mode2_scan_state.selected_sprite_count()) == 1
            && self
                .mode2_scan_state
                .selected_sprite(0)
                .is_some_and(|sprite| {
                    (12..=16).contains(&sprite.x) || (0xA4..=0xA6).contains(&sprite.x)
                })
            && self.bg_pipeline_state.current_transfer_x == 166
            && self.bg_pipeline_state.visible_pixels_output == 158
            && self.bg_pipeline_state.startup_fifo_placeholders == 0
            && self.bg_pipeline_state.fifo.len() == 2
            && self.bg_pipeline_state.push.pending
            && self.bg_pipeline_state.push.entry_delay_remaining == 1
            && self.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.obj_pipeline_state.pending_match_x.is_none()
            && self.obj_pipeline_state.pending_sprite_slots.is_empty()
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

    pub(super) fn single_offscreen_right_sprite_xa0_terminal_tail_without_entry_delay_should_publish_hblank_two_dots_early(
        &self,
    ) -> bool {
        self.ly < VISIBLE_SCANLINES
            && self.line_dot + 2 == self.current_mode0_start_dot()
            && usize::from(self.mode2_scan_state.selected_sprite_count()) == 1
            && self
                .mode2_scan_state
                .selected_sprite(0)
                .is_some_and(|sprite| sprite.x == 0xA0)
            && self.bg_pipeline_state.current_transfer_x == 166
            && self.bg_pipeline_state.visible_pixels_output == 158
            && self.bg_pipeline_state.startup_fifo_placeholders == 0
            && self.bg_pipeline_state.fifo.len() == 2
            && self.bg_pipeline_state.push.pending
            && self.bg_pipeline_state.push.entry_delay_remaining == 0
            && self.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.obj_pipeline_state.pending_match_x.is_none()
            && self.obj_pipeline_state.pending_sprite_slots.is_empty()
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

    pub(super) fn single_offscreen_right_sprite_xa7_terminal_tail_should_publish_hblank_two_dots_early(
        &self,
    ) -> bool {
        self.ly < VISIBLE_SCANLINES
            && self.line_dot + 2 == self.current_mode0_start_dot()
            && usize::from(self.mode2_scan_state.selected_sprite_count()) == 1
            && self
                .mode2_scan_state
                .selected_sprite(0)
                .is_some_and(|sprite| sprite.x == 0xA7)
            && self.bg_pipeline_state.current_transfer_x == 167
            && self.bg_pipeline_state.visible_pixels_output == 159
            && self.bg_pipeline_state.startup_fifo_placeholders == 0
            && self.bg_pipeline_state.fifo.len() == 1
            && self.bg_pipeline_state.push.pending
            && self.bg_pipeline_state.push.entry_delay_remaining == 0
            && self.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Push
            && self.obj_pipeline_state.pending_match_x.is_none()
            && self.obj_pipeline_state.pending_sprite_slots.is_empty()
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

    pub(super) fn single_offscreen_right_sprite_xa2_mode0_boundary_should_publish_hblank(
        &self,
    ) -> bool {
        self.ly < VISIBLE_SCANLINES
            && self.line_dot == self.current_mode0_start_dot()
            && usize::from(self.mode2_scan_state.selected_sprite_count()) == 1
            && self
                .mode2_scan_state
                .selected_sprite(0)
                .is_some_and(|sprite| sprite.x == 0xA2)
            && self.bg_pipeline_state.startup_fifo_placeholders == 0
            && self.bg_pipeline_state.current_transfer_x >= 168
            && self.bg_pipeline_state.fifo.is_empty()
            && !self.bg_pipeline_state.push.pending
            && self.obj_pipeline_state.pending_match_x.is_none()
            && self.obj_pipeline_state.pending_sprite_slots.is_empty()
            && self.current_transfer().is_none()
            && self.access_mode_for_line_dot(self.line_dot) == PpuAccessMode::HBlank
    }

    pub(super) fn two_sprite_staggered_fifo_tail_should_publish_hblank_from_fifo_tail(
        &self,
    ) -> bool {
        if self.ly >= VISIBLE_SCANLINES
            || usize::from(self.mode2_scan_state.selected_sprite_count()) != 2
            || self.bg_pipeline_state.fifo.is_empty()
            || self.bg_pipeline_state.push.pending
        {
            return false;
        }

        let Some(sprite_a) = self.mode2_scan_state.selected_sprite(0) else {
            return false;
        };
        let Some(sprite_b) = self.mode2_scan_state.selected_sprite(1) else {
            return false;
        };
        let (left_x, right_x) = if sprite_a.x <= sprite_b.x {
            (sprite_a.x, sprite_b.x)
        } else {
            (sprite_b.x, sprite_a.x)
        };
        let fifo_len = self.bg_pipeline_state.fifo.len();
        let current_transfer_x = self.bg_pipeline_state.current_transfer_x;
        let x2_x0a_tail = left_x == 0x02
            && right_x == 0x0A
            && self.bg_pipeline_state.startup_fifo_placeholders == 4
            && current_transfer_x == 164
            && fifo_len == 4;
        let x4_to_x7_visible_fifo_tail = (4..=7).contains(&left_x)
            && right_x == left_x.saturating_add(8)
            && right_x <= 0x0F
            && current_transfer_x < 168
            && usize::from(current_transfer_x) + fifo_len == 168
            && usize::from(current_transfer_x)
                + usize::from(self.bg_pipeline_state.startup_fifo_placeholders)
                == 163;

        (x2_x0a_tail || x4_to_x7_visible_fifo_tail)
            && self.line_dot + fifo_len as u16 - 2 == self.current_mode0_start_dot()
            && self.bg_pipeline_state.visible_pixels_output == current_transfer_x.saturating_sub(8)
            && self.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.obj_pipeline_state.pending_match_x.is_none()
            && self.obj_pipeline_state.pending_sprite_slots.is_empty()
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

    pub(super) fn two_sprite_staggered_x8_to_x9_preterminal_tail_should_keep_published_drawing(
        &self,
    ) -> bool {
        if self.ly >= VISIBLE_SCANLINES
            || usize::from(self.mode2_scan_state.selected_sprite_count()) != 2
        {
            return false;
        }

        let Some(sprite_a) = self.mode2_scan_state.selected_sprite(0) else {
            return false;
        };
        let Some(sprite_b) = self.mode2_scan_state.selected_sprite(1) else {
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
            && self.bg_pipeline_state.current_transfer_x == 167
            && self.bg_pipeline_state.visible_pixels_output == 159
            && self.bg_pipeline_state.startup_fifo_placeholders == 0
            && self.bg_pipeline_state.fifo.len() == 1
            && self.bg_pipeline_state.push.pending
            && self.bg_pipeline_state.push.entry_delay_remaining == 0
            && self.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.obj_pipeline_state.pending_match_x.is_none()
            && self.obj_pipeline_state.pending_sprite_slots.is_empty()
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

    pub(super) fn two_sprite_staggered_x0_to_x1_terminal_tail_should_keep_published_drawing(
        &self,
    ) -> bool {
        if self.ly >= VISIBLE_SCANLINES
            || usize::from(self.mode2_scan_state.selected_sprite_count()) != 2
        {
            return false;
        }

        let Some(sprite_a) = self.mode2_scan_state.selected_sprite(0) else {
            return false;
        };
        let Some(sprite_b) = self.mode2_scan_state.selected_sprite(1) else {
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
            && self.bg_pipeline_state.current_transfer_x >= 168
            && self.bg_pipeline_state.visible_pixels_output as usize >= SCREEN_WIDTH
            && self.bg_pipeline_state.startup_fifo_placeholders == expected_placeholders
            && self.bg_pipeline_state.fifo.len() == 8
            && !self.bg_pipeline_state.push.pending
            && self.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.obj_pipeline_state.pending_match_x.is_none()
            && self.obj_pipeline_state.pending_sprite_slots.is_empty()
            && self.current_transfer().is_none()
    }

    pub(super) fn two_sprite_staggered_x9_terminal_boundary_should_keep_published_drawing(
        &self,
    ) -> bool {
        if self.ly >= VISIBLE_SCANLINES
            || usize::from(self.mode2_scan_state.selected_sprite_count()) != 2
        {
            return false;
        }

        let Some(sprite_a) = self.mode2_scan_state.selected_sprite(0) else {
            return false;
        };
        let Some(sprite_b) = self.mode2_scan_state.selected_sprite(1) else {
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
            && self.bg_pipeline_state.current_transfer_x >= 168
            && self.bg_pipeline_state.visible_pixels_output as usize >= SCREEN_WIDTH
            && self.bg_pipeline_state.startup_fifo_placeholders == 0
            && self.bg_pipeline_state.fifo.is_empty()
            && !self.bg_pipeline_state.push.pending
            && self.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.obj_pipeline_state.pending_match_x.is_none()
            && self.obj_pipeline_state.pending_sprite_slots.is_empty()
            && self.current_transfer().is_none()
    }

    pub(super) fn ten_sprite_step8_terminal_tail_should_keep_published_drawing(&self) -> bool {
        let Some(min_x) = self.selected_sprite_step8_ramp_min_x() else {
            return false;
        };

        let terminal_offset = self.line_dot.saturating_sub(self.current_mode0_start_dot());
        let matches_family = match min_x {
            0 => {
                self.bg_pipeline_state.startup_fifo_placeholders == 2
                    && self.bg_pipeline_state.push.pending
                    && terminal_offset <= 27
            }
            1 => {
                self.bg_pipeline_state.startup_fifo_placeholders == 1
                    && !self.bg_pipeline_state.push.pending
                    && terminal_offset <= 20
            }
            2 => {
                self.bg_pipeline_state.startup_fifo_placeholders == 4
                    && self.bg_pipeline_state.push.pending
                    && terminal_offset <= 18
            }
            _ => false,
        };

        matches_family
            && self.bg_pipeline_state.mode3_started
            && self.bg_pipeline_state.visible_pixels_output as usize >= SCREEN_WIDTH
            && self.bg_pipeline_state.current_transfer_x >= 168
            && self.bg_pipeline_state.fifo.len() == 8
            && self.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.obj_pipeline_state.pending_match_x.is_none()
            && self.obj_pipeline_state.pending_sprite_slots.is_empty()
            && self.current_transfer().is_none()
    }

    pub(super) fn ten_sprite_step8_preterminal_tail_should_publish_hblank_early(&self) -> bool {
        let Some(min_x) = self.selected_sprite_step8_ramp_min_x() else {
            return false;
        };
        if !(4..=7).contains(&min_x) {
            return false;
        }

        let expected_placeholders = 8_u8.saturating_sub(min_x);
        let transfer_plus_fifo = usize::from(self.bg_pipeline_state.current_transfer_x)
            + self.bg_pipeline_state.fifo.len();
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
        ) && self.bg_pipeline_state.mode3_started
            && !self.bg_pipeline_state.push.pending
            && self.bg_pipeline_state.startup_fifo_placeholders == expected_placeholders
            && matches_transfer_sum
            && self.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.obj_pipeline_state.pending_match_x.is_none()
            && self.obj_pipeline_state.pending_sprite_slots.is_empty()
    }

    pub(super) fn selected_sprite_step8_ramp_min_x(&self) -> Option<u8> {
        let sprite_count = usize::from(self.mode2_scan_state.selected_sprite_count());
        if sprite_count != MAX_SELECTED_SPRITES_PER_LINE {
            return None;
        }

        let mut xs = [0_u8; MAX_SELECTED_SPRITES_PER_LINE];
        for (slot, x) in xs.iter_mut().enumerate().take(sprite_count) {
            let sprite = self.mode2_scan_state.selected_sprite(slot as u8)?;
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

    pub(super) fn current_published_oam_write_access_mode(&self) -> PpuAccessMode {
        let published_mode = self.current_published_video_write_access_mode();

        if published_mode == PpuAccessMode::OamScan
            && self.ly < VISIBLE_SCANLINES
            && self.line_dot == MODE2_DOTS
        {
            PpuAccessMode::HBlank
        } else {
            published_mode
        }
    }

    pub(super) fn current_published_oam_read_access_mode(&self) -> PpuAccessMode {
        let published_mode = self.current_published_bus_access_mode();

        if published_mode == PpuAccessMode::Drawing
            && !self.blank_frame_active
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

    pub(super) fn current_bus_access_mode(&self) -> PpuAccessMode {
        let current_mode = self.current_access_mode();

        if !self.is_lcd_enabled() || self.ly >= VISIBLE_SCANLINES {
            return current_mode;
        }

        if current_mode == PpuAccessMode::HBlank
            && self.ly + 1 < VISIBLE_SCANLINES
            && self.line_dot + 4 >= self.current_scanline_length()
        {
            return PpuAccessMode::OamScan;
        }

        if current_mode == PpuAccessMode::OamScan && self.line_dot + 4 >= MODE2_DOTS {
            return PpuAccessMode::Drawing;
        }

        current_mode
    }

    pub(super) fn current_cpu_visible_access_mode(&self) -> PpuAccessMode {
        if self.blank_frame_active && self.is_lcd_enabled() && self.line_dot != 0 {
            return self
                .lcd_restart_phase
                .raster_state(self.ly, self.line_dot - 1)
                .map(PpuRasterState::access_mode)
                .unwrap_or_else(|| self.current_access_mode());
        }

        self.current_access_mode()
    }

    pub(super) fn current_raster_state(&self) -> PpuRasterState {
        if !self.is_lcd_enabled() {
            return PpuRasterState::Disabled;
        }

        if let Some(raster_state) = self.lcd_restart_phase.raster_state(self.ly, self.line_dot) {
            return raster_state;
        }

        let mode0_start_dot = self.current_mode0_start_dot();
        let mode = self
            .startup_mode_latch
            .unwrap_or_else(|| access_mode_from_raster(self.ly, self.line_dot, mode0_start_dot));

        PpuRasterState::Active {
            mode,
            mode_dot: mode_dot_from_raster_mode(mode, self.line_dot, mode0_start_dot),
            mode2_scan_active: self.ly < VISIBLE_SCANLINES
                && self.line_dot != 0
                && self.line_dot <= MODE2_DOTS,
        }
    }

    pub(super) fn current_mode0_start_dot(&self) -> u16 {
        if self.ly >= VISIBLE_SCANLINES {
            return MODE0_START_DOT;
        }

        let selected_sprite_count = self.mode2_scan_state.selected_sprite_count();
        let all_selected_sprites_offscreen_right = selected_sprite_count > 0
            && (0..selected_sprite_count).all(|slot| {
                self.mode2_scan_state
                    .selected_sprite(slot)
                    .is_some_and(|sprite| sprite.x >= 168)
            });
        let pending_obj_hit_owns_current_transfer_x = self.obj_pipeline_state.pending_match_x
            == Some(self.bg_pipeline_state.current_transfer_x)
            && !self.obj_pipeline_state.pending_sprite_slots.is_empty();
        let live_transfer_still_owned_by_mode3 = self.current_transfer().is_some();
        self.mode3_line_timing_policy()
            .current_mode0_start_dot(PpuMode3LineTimingContext {
                line_dot: self.line_dot,
                selected_sprite_count,
                all_selected_sprites_offscreen_right,
                obj_fetch_active: self.obj_pipeline_state.fetch.stage != PpuObjFetcherStage::Idle,
                pending_obj_hit_owns_current_transfer_x,
                live_transfer_still_owned_by_mode3,
                saturated_placeholder_tail_still_owned_by_mode3: self
                    .saturated_placeholder_backed_terminal_bg_tail_still_owned_by_mode3(),
            })
    }

    pub(super) fn baseline_mode0_start_dot(&self) -> u16 {
        self.mode3_line_timing_policy().baseline_mode0_start_dot()
    }

    pub(super) fn saturated_placeholder_backed_terminal_bg_tail_still_owned_by_mode3(
        &self,
    ) -> bool {
        let terminal_bg_tail_has_unfinished_fetch_work = matches!(
            self.bg_pipeline_state.fetcher.stage,
            PpuBgFetcherStage::TileDataLow | PpuBgFetcherStage::TileDataHigh
        ) || (self.bg_pipeline_state.push.pending
            && (self.bg_pipeline_state.push.entry_delay_remaining > 0
                || self
                    .bg_pipeline_state
                    .push
                    .terminal_placeholder_tail_extra_hold_remaining
                    > 0));
        self.bg_pipeline_state.mode3_started
            && self.bg_pipeline_state.visible_pixels_output as usize >= SCREEN_WIDTH
            && self.bg_pipeline_state.current_transfer_x >= 168
            && usize::from(self.mode2_scan_state.selected_sprite_count())
                == MAX_SELECTED_SPRITES_PER_LINE
            && self.bg_pipeline_state.startup_fifo_placeholders > 0
            && self.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle
            && self.obj_pipeline_state.pending_match_x.is_none()
            && self.obj_pipeline_state.pending_sprite_slots.is_empty()
            && terminal_bg_tail_has_unfinished_fetch_work
    }

    pub(crate) fn current_mode2_oam_row(&self) -> Option<u8> {
        if !self.is_lcd_enabled()
            || self.ly >= VISIBLE_SCANLINES
            || self.current_access_mode() != PpuAccessMode::OamScan
        {
            return None;
        }

        Some((self.line_dot / OAM_CORRUPTION_DOTS_PER_ROW) as u8)
    }

    pub(crate) fn apply_oam_corruption_event(
        &mut self,
        event: OamCorruptionEventKind,
        oam_bytes: &mut [u8],
    ) -> bool {
        let Some(row) = self.current_mode2_oam_row() else {
            return false;
        };

        self.oam_corruption_controller
            .apply(self.console_model, row, event, oam_bytes)
    }

    pub(super) fn advance_mode2_scan(&mut self, oam: &OamBusView<'_>, dma_oam_active: bool) {
        let raster_state = self.current_raster_state();

        if self.ly >= VISIBLE_SCANLINES
            || !raster_state.is_mode2_scan()
            || self.line_dot == 0
            || !self.line_dot.is_multiple_of(MODE2_T_CYCLES_PER_OAM_ENTRY)
            || self.mode2_scan_state.scanned_entries() >= OAM_SPRITE_COUNT
        {
            return;
        }

        let oam_index = self.mode2_scan_state.scanned_entries();
        self.mode2_scan_state.increment_scanned_entries();

        if self.mode2_scan_state.is_full() {
            return;
        }

        let nominal_sprite = read_oam_sprite(oam, oam_index);
        let sprite = if dma_oam_active && self.console_model.is_dmg_family() {
            let Some((y, x)) = self.mode2_scan_state.latched_mode2_yx_word() else {
                return;
            };
            let (tile_index, attributes) = nominal_sprite
                .map(|sprite| (sprite.tile_index, sprite.attributes))
                .unwrap_or((0xFF, 0xFF));
            PpuSelectedSprite {
                oam_index,
                y,
                x,
                tile_index,
                attributes,
            }
        } else {
            let sprite = match nominal_sprite {
                Some(sprite) => sprite,
                None => return,
            };
            self.mode2_scan_state
                .latch_mode2_yx_word(sprite.y, sprite.x);
            sprite
        };

        if sprite_matches_line(sprite, self.ly, self.current_obj_height()) {
            self.mode2_scan_state.push(sprite);
        }
    }

    pub(super) fn current_obj_height(&self) -> u8 {
        self.mode3_register_latches().current_obj_height()
    }

    pub(super) fn visible_palette_register_value(&self, register: PpuPaletteRegister) -> u8 {
        self.mode3_register_latches()
            .visible()
            .palette_register(register, self.obj_palette_read_policy)
    }

    pub(super) fn write_palette_register_storage(
        &mut self,
        register: PpuPaletteRegister,
        value: u8,
    ) {
        match register {
            PpuPaletteRegister::Bgp => self.bgp = value,
            PpuPaletteRegister::Obp0 => self.obp0 = Some(value),
            PpuPaletteRegister::Obp1 => self.obp1 = Some(value),
        }
    }

    pub(super) fn window_activation_registers(&self) -> PpuVisibleRegisters {
        self.mode3_register_latches()
            .window_activation_registers(self.console_model)
    }

    pub(super) fn window_activation_state(&self) -> PpuMode3WindowActivationState {
        PpuMode3WindowActivationState::new(
            self.window_activation_registers(),
            self.bg_pipeline_state.window_force_x0_this_line,
        )
    }

    pub(super) fn mode3_window_policy(&self) -> PpuMode3WindowPolicy {
        PpuMode3WindowPolicy::new(
            self.mode3_register_latches().visible(),
            self.window_activation_state(),
            self.bg_pipeline_state.window_wy_latch,
            self.bg_pipeline_state.window_started_this_line,
        )
    }

    pub(super) fn mode3_transfer_policy(&self) -> PpuMode3TransferPolicy {
        PpuMode3TransferPolicy::new(
            self.bg_pipeline_state.mode3_started,
            self.bg_pipeline_state.startup_source_state,
            self.bg_pipeline_state
                .startup_pre_visible_transfer_dots_remaining,
            self.bg_pipeline_state.current_transfer_x,
            self.bg_pipeline_state.visible_pixels_output,
            self.bg_pipeline_state.scx_discard_remaining,
            self.line_dot,
        )
    }

    pub(super) fn mode3_line_timing_policy(&self) -> PpuMode3LineTimingPolicy {
        PpuMode3LineTimingPolicy::new(
            self.mode3_register_latches().visible(),
            self.bg_pipeline_state.mode3_started,
            self.bg_pipeline_state.mode0_start_dot,
        )
    }

    pub(super) fn mode3_bgwin_fetch_policy(&self) -> PpuMode3BgWinFetchPolicy {
        PpuMode3BgWinFetchPolicy::new(
            self.mode3_register_latches(),
            self.console_model,
            self.bg_pipeline_state
                .startup_background_tilemap_uses_pipeline_snapshot(),
            self.bg_pipeline_state
                .startup_background_tiledata_uses_pipeline_snapshot(),
            self.bg_pipeline_state
                .startup_background_tileindex_reads_on_stage_one(),
        )
    }

    pub(super) fn background_fetch_context(
        &self,
        next_fetch_pixel: u16,
    ) -> PpuMode3BackgroundFetchContext {
        self.mode3_bgwin_fetch_policy()
            .background_fetch_context(next_fetch_pixel, self.ly)
    }

    pub(super) fn window_fetch_context(&self) -> PpuMode3WindowFetchContext {
        self.mode3_bgwin_fetch_policy().window_fetch_context(
            self.window_state.window_line_counter,
            self.bg_pipeline_state.fetcher.window_tilemap_x,
        )
    }

    pub(super) fn pixel_pipeline_bgp(&self) -> u8 {
        self.mode3_register_latches().pixel_pipeline_bgp(
            self.console_model,
            self.dmg_bgp_cpu_commit_output_palette_override,
            self.dmg_bgp_cpu_commit_bg_visible_hold_palette_override,
        )
    }

    pub(super) fn pixel_transfer_bg_enabled(&self) -> bool {
        self.mode3_register_latches().pixel_transfer_bg_enabled(
            self.console_model,
            self.bg_pipeline_state.current_transfer_x,
        )
    }

    pub(super) fn pixel_transfer_obj_enabled(&self) -> bool {
        self.mode3_register_latches().pixel_transfer_obj_enabled(
            self.console_model,
            self.bg_pipeline_state.current_transfer_x,
        )
    }

    pub(super) fn prepare_visible_scanline_state(&mut self) {
        if self.line_dot != 1 || self.ly >= VISIBLE_SCANLINES {
            return;
        }

        let prepared_line = self.mode3_window_policy().prepare_line(
            self.ly,
            self.window_state.wy_triggered,
            self.window_state.pending_wx166_next_line,
        );
        self.window_state.wy_triggered = prepared_line.wy_triggered();
        self.window_state.pending_wx166_next_line = false;
        self.bg_pipeline_state
            .prepare_window_line(prepared_line.wy_latch(), prepared_line.force_x0_this_line());
    }

    pub(super) fn live_lyc_coincidence(&self) -> bool {
        self.ly == self.lyc
    }

    pub(super) fn effective_lyc_coincidence(&self) -> bool {
        if self.is_lcd_enabled() {
            self.live_lyc_coincidence()
        } else {
            self.stat_state.lcd_disabled_lyc_coincidence
        }
    }

    pub(super) fn lcd_enable_pending_lyc_rise_source(&self) -> bool {
        self.lcd_enable_pending_delay_tcycles == 2
            && self.stat_interrupt_enable & STAT_LYC_INTERRUPT_ENABLE_BIT != 0
            && !self.stat_state.lcd_disabled_lyc_coincidence
            && self.live_lyc_coincidence()
    }

    pub(super) fn ordinary_stat_irq_line(&self) -> bool {
        let coincidence_source = self.stat_interrupt_enable & STAT_LYC_INTERRUPT_ENABLE_BIT != 0
            && self.effective_lyc_coincidence();

        if !self.is_lcd_enabled() {
            return coincidence_source || self.lcd_enable_pending_lyc_rise_source();
        }

        let mode0_start_dot = self.current_mode0_start_dot();
        let mode0_pretrigger_source = self.stat_interrupt_enable & STAT_MODE0_INTERRUPT_ENABLE_BIT
            != 0
            && self.ly < VISIBLE_SCANLINES
            && self.line_dot < mode0_start_dot
            && self.line_dot + 4 >= mode0_start_dot;
        let mode2_pretrigger_source = self.stat_interrupt_enable & STAT_MODE2_INTERRUPT_ENABLE_BIT
            != 0
            && self.ly + 1 < VISIBLE_SCANLINES
            && self.line_dot + 4 >= self.current_scanline_length();
        let dmg_mode2_vblank_entry_source = self.console_model.is_dmg_family()
            && self.stat_interrupt_enable & STAT_MODE2_INTERRUPT_ENABLE_BIT != 0
            && self.current_access_mode() == PpuAccessMode::VBlank
            && self.ly == VISIBLE_SCANLINES
            && self.line_dot == 0;
        let mode_source = match self.current_access_mode() {
            PpuAccessMode::HBlank => {
                self.stat_interrupt_enable & STAT_MODE0_INTERRUPT_ENABLE_BIT != 0
            }
            PpuAccessMode::VBlank => {
                self.stat_interrupt_enable & STAT_MODE1_INTERRUPT_ENABLE_BIT != 0
            }
            PpuAccessMode::OamScan => {
                self.stat_interrupt_enable & STAT_MODE2_INTERRUPT_ENABLE_BIT != 0
            }
            PpuAccessMode::Drawing => false,
        };

        coincidence_source
            || mode_source
            || mode0_pretrigger_source
            || mode2_pretrigger_source
            || dmg_mode2_vblank_entry_source
    }

    pub(super) fn compute_stat_irq_line(&self, quirk_active: bool) -> bool {
        self.ordinary_stat_irq_line() || quirk_active
    }

    pub(super) fn refresh_stat_irq_line(&mut self, quirk_active: bool) {
        let new_line = self.compute_stat_irq_line(quirk_active);
        if !self.stat_state.irq_line && new_line {
            self.queue_interrupt_request(InterruptSource::LcdStat);
        }
        self.stat_state.irq_line = new_line;
    }

    pub(super) fn queue_interrupt_request(&mut self, source: InterruptSource) {
        let bit = match source {
            InterruptSource::VBlank => PPU_PENDING_VBLANK_INTERRUPT_BIT,
            InterruptSource::LcdStat => PPU_PENDING_LCD_STAT_INTERRUPT_BIT,
            InterruptSource::Timer | InterruptSource::Serial | InterruptSource::Joypad => {
                return;
            }
        };
        self.pending_interrupts |= bit;
    }

    pub(super) fn stat_write_quirk_active(&self) -> bool {
        self.console_model.is_dmg_family()
            && self.is_lcd_enabled()
            && (matches!(
                self.current_access_mode(),
                PpuAccessMode::HBlank | PpuAccessMode::VBlank | PpuAccessMode::OamScan
            ) || self.live_lyc_coincidence())
    }

    pub(super) fn refresh_visible_output(&mut self) {
        self.visible_output =
            if self.is_lcd_enabled() && !self.blank_frame_active && !self.system_stop_active {
                PpuVisibleOutputState::Driving
            } else {
                PpuVisibleOutputState::ForcedBlank
            };
    }

    pub(super) fn advance_lcd_restart_phase(&mut self) {
        self.lcd_restart_phase = self.lcd_restart_phase.advance(self.ly, self.line_dot);
    }

    pub(super) fn reset_runtime_pipeline_state(&mut self) {
        self.startup_mode_latch = None;
        self.mode2_scan_state.reset();
        self.window_state.reset();
        self.bg_pipeline_state.reset();
        self.obj_pipeline_state.reset();
        self.current_scanline_pixels.fill(0);
        self.current_scanline_mixed_pixels
            .fill(MixedPixel::background(0));
        self.current_scanline_dmg_bg_forced_white.fill(false);
    }

    pub(super) fn clear_visible_buffers(&mut self) {
        self.current_scanline_pixels.fill(0);
        self.framebuffer.fill(0);
    }

    pub(super) fn enter_lcd_disabled_state(&mut self) {
        self.lcd_state = PpuLcdState::Disabled;
        self.lcd_enable_pending_delay_tcycles = 0;
        self.blank_frame_active = false;
        self.stat_state.lcd_disabled_lyc_coincidence = self.live_lyc_coincidence();
        self.ly = 0;
        self.line_dot = 0;
        self.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
        self.reset_runtime_pipeline_state();
        self.reload_mode3_register_latches_from_mmio();
        self.clear_visible_buffers();
        self.refresh_visible_output();
    }

    pub(super) fn enter_lcd_enabled_restart_state(&mut self) {
        self.lcd_state = PpuLcdState::Enabled;
        self.lcd_enable_pending_delay_tcycles = 0;
        self.blank_frame_active = true;
        self.ly = 0;
        self.line_dot = LCD_REENABLE_INITIAL_LINE_DOT;
        self.lcd_restart_phase = PpuLcdRestartPhase::first_line_after_enable();
        self.stat_state.lcd_disabled_lyc_coincidence = false;
        self.reset_runtime_pipeline_state();
        self.reload_mode3_register_latches_from_mmio();
        self.clear_visible_buffers();
        self.refresh_visible_output();
    }

    pub(super) fn enter_lcd_enable_pending_state(&mut self, delay_tcycles: u8) {
        self.lcd_state = PpuLcdState::Disabled;
        self.lcd_enable_pending_delay_tcycles = delay_tcycles;
        self.startup_mode_latch = None;
        self.refresh_visible_output();
    }
}
