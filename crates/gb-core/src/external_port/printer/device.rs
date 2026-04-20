use super::render::render_printed_page;
use super::{
    PRINTER_IMAGE_BUFFER_CAPACITY_BYTES, PRINTER_PACKET_TIMEOUT_T_CYCLES, PrintedPage,
    PrinterCommand, PrinterDevice, PrinterMargins, PrinterParserState, PrinterPrintArgs,
    PrinterProcessingState, PrinterSnapshot, PrinterStatusBits,
};

impl PrinterDevice {
    pub(in crate::external_port) fn new() -> Self {
        Self {
            parser_state: PrinterParserState::AwaitMagic0,
            processing_state: PrinterProcessingState::Idle,
            staged_response_byte: 0x00,
            response_queue: std::collections::VecDeque::new(),
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

    pub(in crate::external_port) fn staged_response_byte(&self) -> u8 {
        self.staged_response_byte
    }

    pub(in crate::external_port) fn snapshot(&self) -> PrinterSnapshot {
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

    pub(in crate::external_port) fn tick_t_cycle(&mut self) {
        if matches!(self.parser_state, PrinterParserState::AwaitMagic0) {
            return;
        }

        self.packet_timeout_t_cycles = self.packet_timeout_t_cycles.saturating_add(1);
        if self.packet_timeout_t_cycles >= PRINTER_PACKET_TIMEOUT_T_CYCLES {
            self.reset_to_initialized_state();
        }
    }

    pub(in crate::external_port) fn receive_serial_byte(&mut self, byte: u8) {
        self.packet_timeout_t_cycles = 0;
        self.consume_byte(byte);

        if let Some(next_response_byte) = self.response_queue.pop_front() {
            self.staged_response_byte = next_response_byte;
        } else {
            self.staged_response_byte = 0x00;
        }
    }

    pub(in crate::external_port) fn take_printed_pages(&mut self) -> Vec<PrintedPage> {
        std::mem::take(&mut self.printed_pages)
    }

    pub(super) fn execute_command(
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

    pub(super) fn queue_response(&mut self, alive_indicator: u8, status: u8) {
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
