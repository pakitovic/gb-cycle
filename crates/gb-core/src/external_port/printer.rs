mod device;
mod parser;
mod render;
#[cfg(test)]
mod tests;

use std::{collections::VecDeque, mem};

const PRINTER_MAGIC_0: u8 = 0x88;
const PRINTER_MAGIC_1: u8 = 0x33;
const PRINTER_PACKET_TIMEOUT_T_CYCLES: u32 = 419_430;
const PRINTER_IMAGE_BUFFER_CAPACITY_BYTES: usize = 8_000;
const PRINTER_TILE_WIDTH: usize = 20;
const PRINTER_PAGE_WIDTH_PIXELS: u16 = 160;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum PrinterCommand {
    Initialize = 0x01,
    Print = 0x02,
    Data = 0x04,
    Status = 0x0F,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct PrinterMargins {
    pub before: u8,
    pub after: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PrinterPrintArgs {
    pub sheets: u8,
    pub margins: PrinterMargins,
    pub palette: u8,
    pub exposure: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PrintedPage {
    pub width: u16,
    pub height: u16,
    pub pixels: Vec<u8>,
    pub print_args: PrinterPrintArgs,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
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

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
enum PrinterProcessingState {
    Idle,
    DataBuffered,
    PrintingPendingStatus,
    Printing,
    PrintComplete,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    pub(super) fn dynamic_payload_bytes(&self) -> usize {
        self.response_queue
            .len()
            .saturating_mul(mem::size_of::<u8>())
            .saturating_add(self.image_buffer.len())
            .saturating_add(self.packet_data.len())
            .saturating_add(
                self.printed_pages
                    .len()
                    .saturating_mul(mem::size_of::<PrintedPage>()),
            )
            .saturating_add(
                self.printed_pages
                    .iter()
                    .map(|page| page.pixels.len())
                    .sum::<usize>(),
            )
    }
}
