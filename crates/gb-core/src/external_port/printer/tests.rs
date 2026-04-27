use super::render::render_printed_page;
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

fn printer_packet_with_flag(command: PrinterCommand, compression_flag: u8, data: &[u8]) -> Vec<u8> {
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
fn dynamic_payload_bytes_counts_printer_owned_buffers() {
    let mut printer = PrinterDevice::new();
    assert_eq!(printer.dynamic_payload_bytes(), 0);

    printer.response_queue.extend([0x81, 0x00]);
    printer.image_buffer.extend([0x11, 0x22, 0x33, 0x44]);
    printer.packet_data.extend([0x55, 0x66, 0x77]);
    printer.printed_pages.push(PrintedPage {
        width: 2,
        height: 2,
        pixels: vec![0, 1, 2, 3],
        print_args: PrinterPrintArgs {
            sheets: 1,
            margins: PrinterMargins::default(),
            palette: 0xE4,
            exposure: 0x40,
        },
    });

    assert_eq!(
        printer.dynamic_payload_bytes(),
        2 * std::mem::size_of::<u8>() + 4 + 3 + std::mem::size_of::<PrintedPage>() + 4
    );
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
