use super::*;

impl PocketCameraFrame {
    fn normalize(self) -> Result<Vec<u8>, PocketCameraFrameError> {
        let width = self.width as usize;
        let height = self.height as usize;
        if width == 0 || height == 0 || self.grayscale_pixels.len() != width.saturating_mul(height)
        {
            return Err(PocketCameraFrameError::InvalidDimensions {
                width: self.width,
                height: self.height,
                pixel_len: self.grayscale_pixels.len(),
            });
        }

        if width == POCKET_CAMERA_CAPTURE_WIDTH && height == POCKET_CAMERA_CAPTURE_HEIGHT {
            return Ok(self.grayscale_pixels);
        }

        let mut normalized = vec![0; POCKET_CAMERA_CAPTURE_PIXEL_COUNT];
        for y in 0..POCKET_CAMERA_CAPTURE_HEIGHT {
            let source_y = y * height / POCKET_CAMERA_CAPTURE_HEIGHT;
            for x in 0..POCKET_CAMERA_CAPTURE_WIDTH {
                let source_x = x * width / POCKET_CAMERA_CAPTURE_WIDTH;
                normalized[y * POCKET_CAMERA_CAPTURE_WIDTH + x] =
                    self.grayscale_pixels[source_y * width + source_x];
            }
        }
        Ok(normalized)
    }
}

impl PocketCameraCartridge {
    pub(in crate::cartridge) fn placeholder_frame() -> Vec<u8> {
        let mut frame = vec![0; POCKET_CAMERA_CAPTURE_PIXEL_COUNT];
        let shades = [0_u8, 85, 170, 255];

        for y in 0..POCKET_CAMERA_CAPTURE_HEIGHT {
            for x in 0..POCKET_CAMERA_CAPTURE_WIDTH {
                let shade = if x == 0
                    || y == 0
                    || x + 1 == POCKET_CAMERA_CAPTURE_WIDTH
                    || y + 1 == POCKET_CAMERA_CAPTURE_HEIGHT
                {
                    0
                } else {
                    shades[(x * shades.len()) / POCKET_CAMERA_CAPTURE_WIDTH]
                };
                frame[y * POCKET_CAMERA_CAPTURE_WIDTH + x] = shade;
            }
        }

        frame
    }

    pub(in crate::cartridge) fn set_host_frame(
        &mut self,
        frame: PocketCameraFrame,
    ) -> Result<(), PocketCameraFrameError> {
        self.host_frame = frame.normalize()?;
        Ok(())
    }

    pub(in crate::cartridge) fn clear_host_frame(&mut self) {
        self.host_frame = Self::placeholder_frame();
    }

    pub(in crate::cartridge) fn capture_ready_at(&self) -> Option<TCycle> {
        match self.capture_state {
            PocketCameraCaptureState::Working { ready_at, .. } => Some(ready_at),
            PocketCameraCaptureState::Idle | PocketCameraCaptureState::Paused { .. } => None,
        }
    }

    pub(in crate::cartridge) fn registers_selected(&self) -> bool {
        self.ram_bank_or_register_select & 0x10 != 0
    }

    pub(in crate::cartridge) fn describe_external_access(
        &self,
        address: u16,
    ) -> CartridgeExternalAccessInfo {
        if self.registers_selected() {
            let offset = self.register_offset(address) as u8;
            let defined = usize::from(offset) < POCKET_CAMERA_REGISTER_COUNT;
            let read_behavior = if offset == 0 {
                CartridgeExternalReadBehavior::Storage
            } else {
                CartridgeExternalReadBehavior::FallbackValue(0)
            };
            let write_behavior = if defined {
                CartridgeExternalWriteBehavior::Storage
            } else {
                CartridgeExternalWriteBehavior::Ignored
            };
            return CartridgeExternalAccessInfo::new(
                address,
                CartridgeExternalTarget::PocketCameraRegister { offset },
                if defined {
                    CartridgeExternalAvailability::Accessible
                } else {
                    CartridgeExternalAvailability::Reserved
                },
                read_behavior,
                write_behavior,
            );
        }

        CartridgeExternalAccessInfo::new(
            address,
            CartridgeExternalTarget::BankedRam {
                bank: self.selected_ram_bank(),
            },
            if self.capture_is_working() {
                CartridgeExternalAvailability::Disabled
            } else {
                CartridgeExternalAvailability::Accessible
            },
            if self.capture_is_working() {
                CartridgeExternalReadBehavior::FallbackValue(POCKET_CAMERA_WORKING_RAM_READ_VALUE)
            } else {
                CartridgeExternalReadBehavior::Storage
            },
            if self.capture_is_working() || !self.ram_enabled {
                CartridgeExternalWriteBehavior::Ignored
            } else {
                CartridgeExternalWriteBehavior::Storage
            },
        )
    }

    pub(in crate::cartridge) fn read_rom(&self, address: u16) -> u8 {
        let address = address as usize;
        let bank_count = self.header.rom_size.bank_count.unwrap_or(1).max(1);

        let rom_index = if address < 0x4000 {
            address
        } else {
            self.effective_rom_bank(bank_count) * 0x4000 + (address - 0x4000)
        };

        self.rom
            .get(rom_index)
            .copied()
            .unwrap_or(RAM_ABSENT_READ_VALUE)
    }

    pub(in crate::cartridge) fn mapped_rom_window(
        &self,
        address: u16,
    ) -> Option<CartridgeMappedRomWindow> {
        if address >= 0x8000 {
            return None;
        }

        let bank = if address < 0x4000 {
            0
        } else {
            self.effective_rom_bank(self.header.rom_size.bank_count.unwrap_or(1).max(1))
        };
        let bank_offset = if address < 0x4000 {
            address as usize
        } else {
            address as usize - 0x4000
        };
        Some(CartridgeMappedRomWindow::rom(bank, 0x4000, bank_offset))
    }

    pub(in crate::cartridge) fn write_rom(&mut self, address: u16, value: u8) {
        match address {
            0x0000..=0x1FFF => {
                self.ram_enabled = value & 0x0F == 0x0A;
            }
            0x2000..=0x3FFF => {
                self.rom_bank = value & 0x3F;
            }
            0x4000..=0x5FFF => {
                self.ram_bank_or_register_select = value & 0x1F;
            }
            _ => {}
        }
    }

    pub(in crate::cartridge) fn read_ram(&self, address: u16) -> u8 {
        self.read_ram_after_ready_resolution(address)
    }

    pub(in crate::cartridge) fn read_ram_timed(&mut self, address: u16, t_cycle: TCycle) -> u8 {
        self.maybe_complete_capture(Some(t_cycle));
        self.read_ram_after_ready_resolution(address)
    }

    pub(in crate::cartridge) fn write_ram(&mut self, address: u16, value: u8) {
        self.write_ram_after_ready_resolution(address, value, None);
    }

    pub(in crate::cartridge) fn write_ram_timed(
        &mut self,
        address: u16,
        value: u8,
        t_cycle: TCycle,
    ) {
        self.maybe_complete_capture(Some(t_cycle));
        self.write_ram_after_ready_resolution(address, value, Some(t_cycle));
    }

    pub(in crate::cartridge) fn persistence_metadata(&self) -> CartridgePersistenceMetadata {
        CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: false,
            profile: CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear {
                    byte_len: self.ram.len(),
                },
            },
        }
    }

    pub(in crate::cartridge) fn persistent_state(&self) -> PersistentCartState {
        PersistentCartState::PocketCameraRam {
            ram: self.ram.clone(),
        }
    }

    pub(in crate::cartridge) fn restore_persistent_state(
        &mut self,
        state: &PersistentCartState,
    ) -> Result<(), CartridgePersistentStateError> {
        match state {
            PersistentCartState::PocketCameraRam { ram } => {
                if self.ram.len() != ram.len() {
                    return Err(CartridgePersistentStateError::RamLengthMismatch {
                        expected: self.ram.len(),
                        actual: ram.len(),
                    });
                }
                self.ram.copy_from_slice(ram);
                Ok(())
            }
            other => Err(CartridgePersistentStateError::KindMismatch {
                expected: "PocketCameraRam",
                actual: other.kind_name(),
            }),
        }
    }

    fn read_ram_after_ready_resolution(&self, address: u16) -> u8 {
        if self.registers_selected() {
            return self.read_register(address);
        }

        if self.capture_is_working() {
            return POCKET_CAMERA_WORKING_RAM_READ_VALUE;
        }

        self.ram
            .get(self.effective_ram_offset(address))
            .copied()
            .unwrap_or(RAM_ABSENT_READ_VALUE)
    }

    fn write_ram_after_ready_resolution(
        &mut self,
        address: u16,
        value: u8,
        t_cycle: Option<TCycle>,
    ) {
        if self.registers_selected() {
            self.write_register(address, value, t_cycle);
            return;
        }

        if self.capture_is_working() || !self.ram_enabled {
            return;
        }

        let offset = self.effective_ram_offset(address);
        if let Some(byte) = self.ram.get_mut(offset) {
            *byte = value;
        }
    }

    fn read_register(&self, address: u16) -> u8 {
        match self.register_offset(address) {
            0x00 => (self.registers[0] & 0x06) | u8::from(self.capture_is_working()),
            0x01..=0x35 => 0,
            _ => 0,
        }
    }

    fn write_register(&mut self, address: u16, value: u8, t_cycle: Option<TCycle>) {
        match self.register_offset(address) {
            0x00 => {
                self.registers[0] = value & 0x06;
                if value & 0x01 != 0 {
                    if matches!(self.capture_state, PocketCameraCaptureState::Idle) {
                        self.start_new_capture(t_cycle);
                    } else if matches!(self.capture_state, PocketCameraCaptureState::Paused { .. })
                    {
                        self.resume_capture(t_cycle);
                    }
                } else if self.capture_is_working() {
                    self.pause_capture(t_cycle);
                }
            }
            0x01..=0x35 => {
                self.registers[self.register_offset(address)] = value;
            }
            _ => {}
        }
    }

    fn effective_rom_bank(&self, bank_count: usize) -> usize {
        self.rom_bank as usize % bank_count.max(1)
    }

    fn effective_ram_offset(&self, address: u16) -> usize {
        self.selected_ram_bank() as usize * POCKET_CAMERA_RAM_BANK_BYTES
            + (address - 0xA000) as usize
    }

    fn selected_ram_bank(&self) -> u8 {
        self.ram_bank_or_register_select & 0x0F
    }

    fn register_offset(&self, address: u16) -> usize {
        (address as usize - 0xA000) & POCKET_CAMERA_REGISTER_MIRROR_MASK
    }

    fn capture_is_working(&self) -> bool {
        matches!(self.capture_state, PocketCameraCaptureState::Working { .. })
    }

    fn maybe_complete_capture(&mut self, t_cycle: Option<TCycle>) {
        let Some(t_cycle) = t_cycle else {
            return;
        };
        let ready = matches!(
            self.capture_state,
            PocketCameraCaptureState::Working { ready_at, .. } if ready_at.get() <= t_cycle.get()
        );
        if !ready {
            return;
        }

        let finished = std::mem::replace(&mut self.capture_state, PocketCameraCaptureState::Idle);
        let PocketCameraCaptureState::Working { staged_tiles, .. } = finished else {
            unreachable!("ready capture completion must come from the working state");
        };
        self.commit_staged_tiles(&staged_tiles);
    }

    fn start_new_capture(&mut self, t_cycle: Option<TCycle>) {
        let staged_tiles = self.build_capture_tiles();
        let duration = self.capture_duration_t_cycles();
        let ready_at =
            TCycle::new(t_cycle.map_or(duration, |cycle| cycle.get().saturating_add(duration)));
        self.capture_state = PocketCameraCaptureState::Working {
            ready_at,
            staged_tiles,
        };
    }

    fn pause_capture(&mut self, t_cycle: Option<TCycle>) {
        let working = std::mem::replace(&mut self.capture_state, PocketCameraCaptureState::Idle);
        let PocketCameraCaptureState::Working {
            ready_at,
            staged_tiles,
        } = working
        else {
            self.capture_state = working;
            return;
        };

        let remaining_t_cycles = t_cycle.map_or(ready_at.get(), |cycle| {
            ready_at.get().saturating_sub(cycle.get())
        });
        self.capture_state = PocketCameraCaptureState::Paused {
            remaining_t_cycles,
            staged_tiles,
        };
    }

    fn resume_capture(&mut self, t_cycle: Option<TCycle>) {
        let paused = std::mem::replace(&mut self.capture_state, PocketCameraCaptureState::Idle);
        let PocketCameraCaptureState::Paused {
            remaining_t_cycles,
            staged_tiles,
        } = paused
        else {
            self.capture_state = paused;
            return;
        };

        let ready_at = TCycle::new(t_cycle.map_or(remaining_t_cycles, |cycle| {
            cycle.get().saturating_add(remaining_t_cycles)
        }));
        self.capture_state = PocketCameraCaptureState::Working {
            ready_at,
            staged_tiles,
        };
    }

    fn commit_staged_tiles(&mut self, staged_tiles: &[u8]) {
        let start = POCKET_CAMERA_CAPTURE_TILE_BASE_OFFSET;
        let end = start + staged_tiles.len();
        if let Some(window) = self.ram.get_mut(start..end) {
            window.copy_from_slice(staged_tiles);
        }
    }

    fn capture_duration_t_cycles(&self) -> u64 {
        let n_bit_extra = if self.registers[1] & 0x80 != 0 {
            0
        } else {
            512
        };
        let exposure = u16::from(self.registers[2]) << 8 | u16::from(self.registers[3]);
        4 * (32_446_u64 + n_bit_extra + 16 * u64::from(exposure))
    }

    fn build_capture_tiles(&self) -> Vec<u8> {
        let sensor = self.build_sensor_image();
        let mut processed = sensor.iter().map(|value| value - 128).collect::<Vec<_>>();
        self.apply_filtering(&mut processed);

        for pixel in &mut processed {
            *pixel += 128;
        }

        let mut four_color = vec![0_u8; POCKET_CAMERA_CAPTURE_PIXEL_COUNT];
        for y in 0..POCKET_CAMERA_CAPTURE_HEIGHT {
            for x in 0..POCKET_CAMERA_CAPTURE_WIDTH {
                let source_index =
                    (y + POCKET_CAMERA_SENSOR_EXTRA_LINES / 2) * POCKET_CAMERA_CAPTURE_WIDTH + x;
                let value = processed[source_index].clamp(0, 255) as u8;
                four_color[y * POCKET_CAMERA_CAPTURE_WIDTH + x] = self.matrix_map(value, x, y);
            }
        }

        let mut tiles = vec![0_u8; POCKET_CAMERA_CAPTURE_TILE_BYTES];
        for y in 0..POCKET_CAMERA_CAPTURE_HEIGHT {
            for x in 0..POCKET_CAMERA_CAPTURE_WIDTH {
                let outcolor = 3 - (four_color[y * POCKET_CAMERA_CAPTURE_WIDTH + x] >> 6);
                let tile_row = y / 8;
                let tile_col = x / 8;
                let tile_index = tile_row * POCKET_CAMERA_CAPTURE_TILE_WIDTH + tile_col;
                let tile_offset = tile_index * 16 + (y & 7) * 2;
                let bit = 1 << (7 - (x & 7));
                if outcolor & 0x01 != 0 {
                    tiles[tile_offset] |= bit;
                }
                if outcolor & 0x02 != 0 {
                    tiles[tile_offset + 1] |= bit;
                }
            }
        }
        tiles
    }

    fn build_sensor_image(&self) -> Vec<i32> {
        let exposure = (u16::from(self.registers[2]) << 8) | u16::from(self.registers[3]);
        let invert = self.registers[4] & 0x08 != 0;
        let mut sensor = vec![0_i32; POCKET_CAMERA_SENSOR_PIXEL_COUNT];

        for y in 0..POCKET_CAMERA_SENSOR_HEIGHT {
            let source_y = if y < POCKET_CAMERA_SENSOR_EXTRA_LINES / 2 {
                0
            } else if y >= POCKET_CAMERA_CAPTURE_HEIGHT + POCKET_CAMERA_SENSOR_EXTRA_LINES / 2 {
                POCKET_CAMERA_CAPTURE_HEIGHT - 1
            } else {
                y - POCKET_CAMERA_SENSOR_EXTRA_LINES / 2
            };

            for x in 0..POCKET_CAMERA_CAPTURE_WIDTH {
                let mut value =
                    i32::from(self.host_frame[source_y * POCKET_CAMERA_CAPTURE_WIDTH + x]);
                value = value * i32::from(exposure) / 0x0300;
                value = 128 + ((value - 128) / 8);
                value = value.clamp(0, 255);
                if invert {
                    value = 255 - value;
                }
                sensor[y * POCKET_CAMERA_CAPTURE_WIDTH + x] = value;
            }
        }

        sensor
    }

    fn apply_filtering(&self, pixels: &mut [i32]) {
        let p_bits = match (self.registers[0] >> 1) & 0x03 {
            0 => 0x00,
            1 => 0x01,
            2 | 3 => 0x01,
            _ => unreachable!(),
        };
        let m_bits = match (self.registers[0] >> 1) & 0x03 {
            0 => 0x01,
            1 => 0x00,
            2 | 3 => 0x02,
            _ => unreachable!(),
        };
        let n_bit = u32::from((self.registers[1] & 0x80) != 0);
        let vh_bits = (self.registers[1] >> 5) & 0x03;
        let edge_alpha = [0.50_f32, 0.75, 1.00, 1.25, 2.00, 3.00, 4.00, 5.00]
            [((self.registers[4] & 0x70) >> 4) as usize];
        let e3_bit = u32::from((self.registers[4] & 0x80) != 0);
        let filtering_mode = (n_bit << 3) | (u32::from(vh_bits) << 1) | e3_bit;
        let original = pixels.to_vec();
        let mut temp = original.clone();

        match filtering_mode {
            0x0 => {
                for y in 0..POCKET_CAMERA_SENSOR_HEIGHT {
                    for x in 0..POCKET_CAMERA_CAPTURE_WIDTH {
                        let south =
                            temp[self.pixel_index(x, (y + 1).min(POCKET_CAMERA_SENSOR_HEIGHT - 1))];
                        let current = temp[self.pixel_index(x, y)];
                        let mut value = 0;
                        if p_bits & 0x01 != 0 {
                            value += current;
                        }
                        if p_bits & 0x02 != 0 {
                            value += south;
                        }
                        if m_bits & 0x01 != 0 {
                            value -= current;
                        }
                        if m_bits & 0x02 != 0 {
                            value -= south;
                        }
                        pixels[self.pixel_index(x, y)] = value.clamp(-128, 127);
                    }
                }
            }
            0x2 => {
                for y in 0..POCKET_CAMERA_SENSOR_HEIGHT {
                    for x in 0..POCKET_CAMERA_CAPTURE_WIDTH {
                        let west = original[self.pixel_index(x.saturating_sub(1), y)];
                        let east = original
                            [self.pixel_index((x + 1).min(POCKET_CAMERA_CAPTURE_WIDTH - 1), y)];
                        let current = original[self.pixel_index(x, y)];
                        temp[self.pixel_index(x, y)] = (current
                            + (((2 * current - west - east) as f32) * edge_alpha) as i32)
                            .clamp(0, 255);
                    }
                }
                for y in 0..POCKET_CAMERA_SENSOR_HEIGHT {
                    for x in 0..POCKET_CAMERA_CAPTURE_WIDTH {
                        let south =
                            temp[self.pixel_index(x, (y + 1).min(POCKET_CAMERA_SENSOR_HEIGHT - 1))];
                        let current = temp[self.pixel_index(x, y)];
                        let mut value = 0;
                        if p_bits & 0x01 != 0 {
                            value += current;
                        }
                        if p_bits & 0x02 != 0 {
                            value += south;
                        }
                        if m_bits & 0x01 != 0 {
                            value -= current;
                        }
                        if m_bits & 0x02 != 0 {
                            value -= south;
                        }
                        pixels[self.pixel_index(x, y)] = value.clamp(-128, 127);
                    }
                }
            }
            0xE => {
                for y in 0..POCKET_CAMERA_SENSOR_HEIGHT {
                    for x in 0..POCKET_CAMERA_CAPTURE_WIDTH {
                        let south = original
                            [self.pixel_index(x, (y + 1).min(POCKET_CAMERA_SENSOR_HEIGHT - 1))];
                        let north = original[self.pixel_index(x, y.saturating_sub(1))];
                        let west = original[self.pixel_index(x.saturating_sub(1), y)];
                        let east = original
                            [self.pixel_index((x + 1).min(POCKET_CAMERA_CAPTURE_WIDTH - 1), y)];
                        let current = original[self.pixel_index(x, y)];
                        pixels[self.pixel_index(x, y)] = (current
                            + (((4 * current - west - east - north - south) as f32) * edge_alpha)
                                as i32)
                            .clamp(-128, 127);
                    }
                }
            }
            0x1 => pixels.fill(0),
            _ => {}
        }
    }

    fn matrix_map(&self, value: u8, x: usize, y: usize) -> u8 {
        let base = 6 + (((y & 3) * 4 + (x & 3)) * 3);
        let r0 = self.registers[base];
        let r1 = self.registers[base + 1];
        let r2 = self.registers[base + 2];
        if value < r0 {
            0x00
        } else if value < r1 {
            0x40
        } else if value < r2 {
            0x80
        } else {
            0xC0
        }
    }

    fn pixel_index(&self, x: usize, y: usize) -> usize {
        y * POCKET_CAMERA_CAPTURE_WIDTH + x
    }
}
