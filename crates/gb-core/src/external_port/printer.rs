use std::collections::VecDeque;

const PRINTER_MAGIC_0: u8 = 0x88;
const PRINTER_MAGIC_1: u8 = 0x33;
const PRINTER_PACKET_TIMEOUT_T_CYCLES: u32 = 419_430;
const PRINTER_IMAGE_BUFFER_CAPACITY_BYTES: usize = 8_000;
const PRINTER_TILE_WIDTH: usize = 20;
const PRINTER_PAGE_WIDTH_PIXELS: u16 = 160;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrinterCommand {
    Initialize = 0x01,
    Print = 0x02,
    Data = 0x04,
    Status = 0x0F,
}

impl PrinterCommand {
    fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0x01 => Some(Self::Initialize),
            0x02 => Some(Self::Print),
            0x04 => Some(Self::Data),
            0x0F => Some(Self::Status),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PrinterMargins {
    pub before: u8,
    pub after: u8,
}

impl PrinterMargins {
    fn from_byte(byte: u8) -> Self {
        Self {
            before: byte >> 4,
            after: byte & 0x0F,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PrinterPrintArgs {
    pub sheets: u8,
    pub margins: PrinterMargins,
    pub palette: u8,
    pub exposure: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrintedPage {
    pub width: u16,
    pub height: u16,
    pub pixels: Vec<u8>,
    pub print_args: PrinterPrintArgs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PrinterStatusBits {
    pub low_battery: bool,
    pub other_error: bool,
    pub paper_jam: bool,
    pub packet_error: bool,
    pub unprocessed_data: bool,
    pub image_data_full: bool,
    pub currently_printing: bool,
    pub checksum_error: bool,
}

impl PrinterStatusBits {
    fn to_byte(self) -> u8 {
        (u8::from(self.low_battery) << 7)
            | (u8::from(self.other_error) << 6)
            | (u8::from(self.paper_jam) << 5)
            | (u8::from(self.packet_error) << 4)
            | (u8::from(self.unprocessed_data) << 3)
            | (u8::from(self.image_data_full) << 2)
            | (u8::from(self.currently_printing) << 1)
            | u8::from(self.checksum_error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrinterSnapshot {
    pub parser_state: PrinterParserState,
    pub status: PrinterStatusBits,
    pub staged_response_byte: u8,
    pub response_queue_len: usize,
    pub image_buffer_len: usize,
    pub print_armed: bool,
    pub packet_timeout_t_cycles: u32,
    pub completed_page_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PrinterParserState {
    #[default]
    AwaitMagic0,
    AwaitMagic1,
    AwaitCommand,
    AwaitCompressionFlag {
        command: u8,
    },
    AwaitLengthLo {
        command: u8,
        compression_flag: u8,
    },
    AwaitLengthHi {
        command: u8,
        compression_flag: u8,
        data_len_lo: u8,
    },
    ReceiveData {
        command: u8,
        compression_flag: u8,
        expected_data_len: u16,
        received_len: u16,
    },
    AwaitChecksumLo {
        command: u8,
        compression_flag: u8,
        expected_data_len: u16,
    },
    AwaitChecksumHi {
        command: u8,
        compression_flag: u8,
        expected_data_len: u16,
        checksum_lo: u8,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum PrinterProcessingState {
    Idle,
    DataBuffered,
    PrintingPendingStatus,
    Printing,
    PrintComplete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PrinterDevice {
    parser_state: PrinterParserState,
    processing_state: PrinterProcessingState,
    staged_response_byte: u8,
    response_queue: VecDeque<u8>,
    image_buffer: Vec<u8>,
    packet_data: Vec<u8>,
    packet_checksum_sum: u16,
    last_print_args: Option<PrinterPrintArgs>,
    printed_pages: Vec<PrintedPage>,
    status: PrinterStatusBits,
    packet_timeout_t_cycles: u32,
    print_armed: bool,
}

impl PrinterDevice {
    pub(super) fn new() -> Self {
        Self {
            parser_state: PrinterParserState::AwaitMagic0,
            processing_state: PrinterProcessingState::Idle,
            staged_response_byte: 0x00,
            response_queue: VecDeque::new(),
            image_buffer: Vec::new(),
            packet_data: Vec::new(),
            packet_checksum_sum: 0,
            last_print_args: None,
            printed_pages: Vec::new(),
            status: PrinterStatusBits::default(),
            packet_timeout_t_cycles: 0,
            print_armed: false,
        }
    }

    pub(super) fn staged_response_byte(&self) -> u8 {
        self.staged_response_byte
    }

    pub(super) fn snapshot(&self) -> PrinterSnapshot {
        PrinterSnapshot {
            parser_state: self.parser_state,
            status: self.status,
            staged_response_byte: self.staged_response_byte,
            response_queue_len: self.response_queue.len(),
            image_buffer_len: self.image_buffer.len(),
            print_armed: self.print_armed,
            packet_timeout_t_cycles: self.packet_timeout_t_cycles,
            completed_page_count: self.printed_pages.len(),
        }
    }

    pub(super) fn tick_t_cycle(&mut self) {
        if matches!(self.parser_state, PrinterParserState::AwaitMagic0) {
            return;
        }

        self.packet_timeout_t_cycles = self.packet_timeout_t_cycles.saturating_add(1);
        if self.packet_timeout_t_cycles >= PRINTER_PACKET_TIMEOUT_T_CYCLES {
            self.reset_to_initialized_state();
        }
    }

    pub(super) fn receive_serial_byte(&mut self, byte: u8) {
        self.packet_timeout_t_cycles = 0;
        self.consume_byte(byte);

        if let Some(next_response_byte) = self.response_queue.pop_front() {
            self.staged_response_byte = next_response_byte;
        } else {
            self.staged_response_byte = 0x00;
        }
    }

    pub(super) fn take_printed_pages(&mut self) -> Vec<PrintedPage> {
        std::mem::take(&mut self.printed_pages)
    }

    fn consume_byte(&mut self, byte: u8) {
        match self.parser_state {
            PrinterParserState::AwaitMagic0 => {
                if byte == PRINTER_MAGIC_0 {
                    self.parser_state = PrinterParserState::AwaitMagic1;
                }
            }
            PrinterParserState::AwaitMagic1 => {
                if byte == PRINTER_MAGIC_1 {
                    self.packet_data.clear();
                    self.packet_checksum_sum = 0;
                    self.parser_state = PrinterParserState::AwaitCommand;
                } else if byte == PRINTER_MAGIC_0 {
                    self.parser_state = PrinterParserState::AwaitMagic1;
                } else {
                    self.parser_state = PrinterParserState::AwaitMagic0;
                }
            }
            PrinterParserState::AwaitCommand => {
                self.packet_data.push(byte);
                self.packet_checksum_sum = self.packet_checksum_sum.wrapping_add(byte as u16);
                self.parser_state = PrinterParserState::AwaitCompressionFlag { command: byte };
            }
            PrinterParserState::AwaitCompressionFlag { command } => {
                self.packet_data.push(byte);
                self.packet_checksum_sum = self.packet_checksum_sum.wrapping_add(byte as u16);
                self.parser_state = PrinterParserState::AwaitLengthLo {
                    command,
                    compression_flag: byte,
                };
            }
            PrinterParserState::AwaitLengthLo {
                command,
                compression_flag,
            } => {
                self.packet_data.push(byte);
                self.packet_checksum_sum = self.packet_checksum_sum.wrapping_add(byte as u16);
                self.parser_state = PrinterParserState::AwaitLengthHi {
                    command,
                    compression_flag,
                    data_len_lo: byte,
                };
            }
            PrinterParserState::AwaitLengthHi {
                command,
                compression_flag,
                data_len_lo,
            } => {
                self.packet_data.push(byte);
                self.packet_checksum_sum = self.packet_checksum_sum.wrapping_add(byte as u16);
                let expected_data_len = u16::from_le_bytes([data_len_lo, byte]);
                if expected_data_len == 0 {
                    self.parser_state = PrinterParserState::AwaitChecksumLo {
                        command,
                        compression_flag,
                        expected_data_len,
                    };
                } else {
                    self.parser_state = PrinterParserState::ReceiveData {
                        command,
                        compression_flag,
                        expected_data_len,
                        received_len: 0,
                    };
                }
            }
            PrinterParserState::ReceiveData {
                command,
                compression_flag,
                expected_data_len,
                received_len,
            } => {
                self.packet_data.push(byte);
                self.packet_checksum_sum = self.packet_checksum_sum.wrapping_add(byte as u16);
                let received_len = received_len + 1;
                if received_len == expected_data_len {
                    self.parser_state = PrinterParserState::AwaitChecksumLo {
                        command,
                        compression_flag,
                        expected_data_len,
                    };
                } else {
                    self.parser_state = PrinterParserState::ReceiveData {
                        command,
                        compression_flag,
                        expected_data_len,
                        received_len,
                    };
                }
            }
            PrinterParserState::AwaitChecksumLo {
                command,
                compression_flag,
                expected_data_len,
            } => {
                self.parser_state = PrinterParserState::AwaitChecksumHi {
                    command,
                    compression_flag,
                    expected_data_len,
                    checksum_lo: byte,
                };
            }
            PrinterParserState::AwaitChecksumHi {
                command,
                compression_flag,
                expected_data_len,
                checksum_lo,
            } => {
                self.finish_packet(
                    command,
                    compression_flag,
                    expected_data_len,
                    u16::from_le_bytes([checksum_lo, byte]),
                );
            }
        }
    }

    fn finish_packet(
        &mut self,
        command: u8,
        compression_flag: u8,
        expected_data_len: u16,
        checksum: u16,
    ) {
        self.parser_state = PrinterParserState::AwaitMagic0;
        let command_data = self.packet_data.get(4..).unwrap_or_default().to_vec();

        if checksum != self.packet_checksum_sum {
            self.status.checksum_error = true;
            self.queue_response(0x81, self.status.to_byte());
            self.packet_data.clear();
            self.packet_checksum_sum = 0;
            return;
        }

        self.status.checksum_error = false;
        if compression_flag != 0 {
            self.status.packet_error = true;
            self.queue_response(0x81, self.status.to_byte());
            self.packet_data.clear();
            self.packet_checksum_sum = 0;
            return;
        }

        let Some(command) = PrinterCommand::from_byte(command) else {
            self.status.packet_error = true;
            self.queue_response(0x81, self.status.to_byte());
            self.packet_data.clear();
            self.packet_checksum_sum = 0;
            return;
        };

        self.status.packet_error = false;
        self.execute_command(command, expected_data_len, &command_data);
        self.queue_response(0x81, self.status.to_byte());
        self.packet_data.clear();
        self.packet_checksum_sum = 0;
    }

    fn execute_command(
        &mut self,
        command: PrinterCommand,
        expected_data_len: u16,
        command_data: &[u8],
    ) {
        match command {
            PrinterCommand::Initialize => self.handle_initialize(),
            PrinterCommand::Print => self.handle_print(command_data),
            PrinterCommand::Data => self.handle_data(expected_data_len, command_data),
            PrinterCommand::Status => self.handle_status_command(),
        }
    }

    fn handle_initialize(&mut self) {
        self.reset_to_initialized_state();
    }

    fn handle_print(&mut self, command_data: &[u8]) {
        let [sheets, margins, palette, exposure] = *command_data else {
            self.status.packet_error = true;
            return;
        };

        let print_args = PrinterPrintArgs {
            sheets,
            margins: PrinterMargins::from_byte(margins),
            palette,
            exposure,
        };
        self.last_print_args = Some(print_args);

        if self.image_buffer.is_empty() || !self.print_armed {
            self.recompute_status();
            return;
        }

        self.processing_state = PrinterProcessingState::PrintingPendingStatus;
        self.print_armed = false;
        self.printed_pages
            .push(render_printed_page(&self.image_buffer, print_args));
        self.recompute_status();
    }

    fn handle_data(&mut self, expected_data_len: u16, command_data: &[u8]) {
        if command_data.len() != expected_data_len as usize {
            self.status.packet_error = true;
            return;
        }

        if command_data.is_empty() {
            if !self.image_buffer.is_empty() {
                self.print_armed = true;
            }
            self.recompute_status();
            return;
        }

        let available_space =
            PRINTER_IMAGE_BUFFER_CAPACITY_BYTES.saturating_sub(self.image_buffer.len());
        let accepted_len = available_space.min(command_data.len());
        self.image_buffer
            .extend_from_slice(&command_data[..accepted_len]);
        self.processing_state = PrinterProcessingState::DataBuffered;
        self.print_armed = false;
        self.recompute_status();
    }

    fn handle_status_command(&mut self) {
        if matches!(
            self.processing_state,
            PrinterProcessingState::PrintingPendingStatus
        ) {
            self.processing_state = PrinterProcessingState::Printing;
            self.recompute_status();
        } else if matches!(self.processing_state, PrinterProcessingState::Printing) {
            self.processing_state = PrinterProcessingState::PrintComplete;
            self.recompute_status();
        }
    }

    fn queue_response(&mut self, alive_indicator: u8, status: u8) {
        self.response_queue.push_back(alive_indicator);
        self.response_queue.push_back(status);
    }

    fn recompute_status(&mut self) {
        let mut status = PrinterStatusBits {
            packet_error: self.status.packet_error,
            checksum_error: self.status.checksum_error,
            ..PrinterStatusBits::default()
        };

        match self.processing_state {
            PrinterProcessingState::Idle => {}
            PrinterProcessingState::DataBuffered => {
                status.unprocessed_data = !self.image_buffer.is_empty();
            }
            PrinterProcessingState::PrintingPendingStatus => {
                status.unprocessed_data = !self.image_buffer.is_empty();
            }
            PrinterProcessingState::Printing => {
                status.image_data_full = !self.image_buffer.is_empty();
                status.currently_printing = true;
            }
            PrinterProcessingState::PrintComplete => {
                status.image_data_full = !self.image_buffer.is_empty();
            }
        }

        if self.image_buffer.len() >= PRINTER_IMAGE_BUFFER_CAPACITY_BYTES {
            status.image_data_full = true;
        }

        self.status = status;
    }

    fn reset_to_initialized_state(&mut self) {
        self.parser_state = PrinterParserState::AwaitMagic0;
        self.processing_state = PrinterProcessingState::Idle;
        self.staged_response_byte = 0x00;
        self.response_queue.clear();
        self.image_buffer.clear();
        self.packet_data.clear();
        self.packet_checksum_sum = 0;
        self.last_print_args = None;
        self.status = PrinterStatusBits::default();
        self.packet_timeout_t_cycles = 0;
        self.print_armed = false;
    }
}

fn render_printed_page(image_buffer: &[u8], print_args: PrinterPrintArgs) -> PrintedPage {
    let total_tiles = image_buffer.len() / 16;
    let tile_rows = total_tiles.div_ceil(PRINTER_TILE_WIDTH);
    let height = (tile_rows * 8) as u16;
    let mut pixels = vec![0; usize::from(PRINTER_PAGE_WIDTH_PIXELS) * usize::from(height)];

    for tile_index in 0..total_tiles {
        let tile_base = tile_index * 16;
        let tile_x = tile_index % PRINTER_TILE_WIDTH;
        let tile_y = tile_index / PRINTER_TILE_WIDTH;

        for row in 0..8 {
            let plane_lo = image_buffer[tile_base + row * 2];
            let plane_hi = image_buffer[tile_base + row * 2 + 1];

            for bit in 0..8 {
                let shift = 7 - bit;
                let color = ((plane_lo >> shift) & 1) | (((plane_hi >> shift) & 1) << 1);
                let x = tile_x * 8 + bit;
                let y = tile_y * 8 + row;
                let pixel_index = y * usize::from(PRINTER_PAGE_WIDTH_PIXELS) + x;
                pixels[pixel_index] = color;
            }
        }
    }

    PrintedPage {
        width: PRINTER_PAGE_WIDTH_PIXELS,
        height,
        pixels,
        print_args,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn send_packet(printer: &mut PrinterDevice, bytes: &[u8]) -> Vec<u8> {
        let mut responses = Vec::new();
        for &byte in bytes {
            responses.push(printer.staged_response_byte());
            printer.receive_serial_byte(byte);
        }
        responses
    }

    fn printer_packet(command: PrinterCommand, data: &[u8]) -> Vec<u8> {
        printer_packet_with_flag(command, 0x00, data)
    }

    fn printer_packet_with_flag(
        command: PrinterCommand,
        compression_flag: u8,
        data: &[u8],
    ) -> Vec<u8> {
        let mut packet = vec![
            PRINTER_MAGIC_0,
            PRINTER_MAGIC_1,
            command as u8,
            compression_flag,
            (data.len() & 0xFF) as u8,
            ((data.len() >> 8) & 0xFF) as u8,
        ];
        packet.extend_from_slice(data);
        let checksum = packet[2..]
            .iter()
            .fold(0u16, |sum, &byte| sum.wrapping_add(byte as u16));
        packet.push((checksum & 0xFF) as u8);
        packet.push((checksum >> 8) as u8);
        packet
    }

    fn consume_reply(printer: &mut PrinterDevice) -> (u8, u8) {
        let alive = printer.staged_response_byte();
        printer.receive_serial_byte(0x00);
        let status = printer.staged_response_byte();
        printer.receive_serial_byte(0x00);
        (alive, status)
    }

    #[test]
    fn status_command_queues_alive_indicator_and_zero_status_for_detection() {
        let mut printer = PrinterDevice::new();
        let packet = printer_packet(PrinterCommand::Status, &[]);

        let responses = send_packet(&mut printer, &packet);

        assert!(responses.iter().all(|&byte| byte == 0x00));
        assert_eq!(printer.staged_response_byte(), 0x81);
        printer.receive_serial_byte(0x00);
        assert_eq!(printer.staged_response_byte(), 0x00);
    }

    #[test]
    fn non_empty_data_then_empty_data_then_print_produces_a_page() {
        let mut printer = PrinterDevice::new();
        let tile_row = vec![0xFF; 320];

        send_packet(
            &mut printer,
            &printer_packet(PrinterCommand::Data, &tile_row),
        );
        assert_eq!(printer.snapshot().status.to_byte(), 0x08);

        send_packet(&mut printer, &printer_packet(PrinterCommand::Data, &[]));
        assert_eq!(printer.snapshot().status.to_byte(), 0x08);

        send_packet(
            &mut printer,
            &printer_packet(PrinterCommand::Print, &[0x01, 0x13, 0xE4, 0x40]),
        );
        assert_eq!(printer.snapshot().status.to_byte(), 0x08);

        send_packet(&mut printer, &printer_packet(PrinterCommand::Status, &[]));
        assert_eq!(printer.snapshot().status.to_byte(), 0x06);

        send_packet(&mut printer, &printer_packet(PrinterCommand::Status, &[]));
        assert_eq!(printer.snapshot().status.to_byte(), 0x04);

        let pages = printer.take_printed_pages();
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].width, 160);
        assert_eq!(pages[0].height, 8);
        assert_eq!(pages[0].print_args.palette, 0xE4);
        assert_eq!(pages[0].print_args.exposure, 0x40);
    }

    #[test]
    fn print_command_is_ignored_until_an_empty_data_packet_arms_printing() {
        let mut printer = PrinterDevice::new();
        let tile_row = vec![0xAA; 320];

        send_packet(
            &mut printer,
            &printer_packet(PrinterCommand::Data, &tile_row),
        );
        send_packet(
            &mut printer,
            &printer_packet(PrinterCommand::Print, &[0x01, 0x13, 0xE4, 0x40]),
        );

        assert!(printer.take_printed_pages().is_empty());
        assert_eq!(printer.snapshot().status.to_byte(), 0x08);
    }

    #[test]
    fn packet_timeout_resets_parser_and_clears_buffered_image_data() {
        let mut printer = PrinterDevice::new();

        send_packet(
            &mut printer,
            &[PRINTER_MAGIC_0, PRINTER_MAGIC_1, PrinterCommand::Data as u8],
        );

        for _ in 0..PRINTER_PACKET_TIMEOUT_T_CYCLES {
            printer.tick_t_cycle();
        }

        let snapshot = printer.snapshot();
        assert_eq!(snapshot.parser_state, PrinterParserState::AwaitMagic0);
        assert_eq!(snapshot.status.to_byte(), 0x00);
        assert_eq!(snapshot.image_buffer_len, 0);
    }

    #[test]
    fn checksum_mismatch_sets_the_checksum_error_bit_and_drops_the_packet() {
        let mut printer = PrinterDevice::new();
        let mut packet = printer_packet(PrinterCommand::Data, &[0xAA, 0x55]);
        let checksum_hi_index = packet.len() - 1;
        packet[checksum_hi_index] ^= 0x01;

        send_packet(&mut printer, &packet);
        let (alive, status) = consume_reply(&mut printer);

        assert_eq!(alive, 0x81);
        assert_eq!(status, 0x01);
        assert_eq!(printer.snapshot().image_buffer_len, 0);
        assert!(printer.take_printed_pages().is_empty());
    }

    #[test]
    fn compressed_packets_are_rejected_with_packet_error_status() {
        let mut printer = PrinterDevice::new();
        let packet = printer_packet_with_flag(PrinterCommand::Data, 0x01, &[0xAA, 0x55]);

        send_packet(&mut printer, &packet);
        let (alive, status) = consume_reply(&mut printer);

        assert_eq!(alive, 0x81);
        assert_eq!(status, 0x10);
        assert_eq!(printer.snapshot().image_buffer_len, 0);
        assert!(printer.take_printed_pages().is_empty());
    }

    #[test]
    fn render_printed_page_decodes_gb_tile_bytes_into_shade_indices() {
        let mut tile_bytes = vec![0; 16];
        tile_bytes[0] = 0b1000_0000;
        tile_bytes[1] = 0b0100_0000;

        let page = render_printed_page(
            &tile_bytes,
            PrinterPrintArgs {
                sheets: 1,
                margins: PrinterMargins {
                    before: 1,
                    after: 3,
                },
                palette: 0xE4,
                exposure: 0x40,
            },
        );

        assert_eq!(page.width, 160);
        assert_eq!(page.height, 8);
        assert_eq!(page.pixels[0], 1);
        assert_eq!(page.pixels[1], 2);
        assert_eq!(page.pixels[2], 0);
    }
}
