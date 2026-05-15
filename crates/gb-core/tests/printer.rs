mod common;

use gb_core::{
    ConsoleModel, ExternalPortAttachmentKind, Machine, MachineConfig, PrintedPage, PrinterCommand,
    StartupMode,
};

const FIXTURE_ACCEPT_ENV: &str = common::fixture_env::PRINTER;
const PRINTED_PAGE_FIXTURE_NAME: &str = "printer_typed_page_fixture.txt";

fn serial_transfer_byte(machine: &mut Machine, outgoing_byte: u8) -> u8 {
    machine.write_bus(0xFF01, outgoing_byte);
    machine.write_bus(0xFF02, 0x81);

    while !matches!(
        machine.serial().transfer_state(),
        gb_core::SerialTransferState::Idle
    ) {
        machine.step_t_cycle();
    }

    machine.read_bus(0xFF01)
}

fn printer_packet(command: PrinterCommand, data: &[u8]) -> Vec<u8> {
    printer_packet_with_flag(command, 0x00, data)
}

fn printer_packet_with_flag(command: PrinterCommand, compression_flag: u8, data: &[u8]) -> Vec<u8> {
    let mut packet = vec![
        0x88,
        0x33,
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

fn send_printer_packet(machine: &mut Machine, command: PrinterCommand, data: &[u8]) -> Vec<u8> {
    printer_packet(command, data)
        .into_iter()
        .map(|byte| serial_transfer_byte(machine, byte))
        .collect()
}

fn send_printer_packet_with_flag(
    machine: &mut Machine,
    command: PrinterCommand,
    compression_flag: u8,
    data: &[u8],
) -> Vec<u8> {
    printer_packet_with_flag(command, compression_flag, data)
        .into_iter()
        .map(|byte| serial_transfer_byte(machine, byte))
        .collect()
}

fn run_print_sequence(
    machine: &mut Machine,
    image_data: &[u8],
    print_args: [u8; 4],
) -> PrintedPage {
    send_printer_packet(machine, PrinterCommand::Data, image_data);
    assert_eq!(serial_transfer_byte(machine, 0x00), 0x81);
    assert_eq!(serial_transfer_byte(machine, 0x00), 0x08);

    send_printer_packet(machine, PrinterCommand::Data, &[]);
    serial_transfer_byte(machine, 0x00);
    serial_transfer_byte(machine, 0x00);

    send_printer_packet(machine, PrinterCommand::Print, &print_args);
    assert_eq!(serial_transfer_byte(machine, 0x00), 0x81);
    assert_eq!(serial_transfer_byte(machine, 0x00), 0x08);

    send_printer_packet(machine, PrinterCommand::Status, &[]);
    serial_transfer_byte(machine, 0x00);
    assert_eq!(serial_transfer_byte(machine, 0x00), 0x06);

    send_printer_packet(machine, PrinterCommand::Status, &[]);
    serial_transfer_byte(machine, 0x00);
    assert_eq!(serial_transfer_byte(machine, 0x00), 0x04);

    let mut pages = machine.take_printed_pages();
    assert_eq!(pages.len(), 1);
    pages.remove(0)
}

fn render_printed_page_fixture(page: &PrintedPage) -> String {
    let mut rendered = String::new();
    rendered.push_str(&format!("width={}\n", page.width));
    rendered.push_str(&format!("height={}\n", page.height));
    rendered.push_str(&format!("sheets={}\n", page.print_args.sheets));
    rendered.push_str(&format!(
        "margins.before={}\n",
        page.print_args.margins.before
    ));
    rendered.push_str(&format!(
        "margins.after={}\n",
        page.print_args.margins.after
    ));
    rendered.push_str(&format!("palette={:#04X}\n", page.print_args.palette));
    rendered.push_str(&format!("exposure={:#04X}\n", page.print_args.exposure));

    let width = usize::from(page.width);
    for row in 0..usize::from(page.height) {
        let row_start = row * width;
        let row_pixels = &page.pixels[row_start..row_start + 16];
        rendered.push_str(&format!("row{row}[0..16]="));
        for &pixel in row_pixels {
            rendered.push(char::from(b'0' + pixel));
        }
        rendered.push('\n');
    }

    rendered
}

#[test]
fn printer_serial_path_accepts_compressed_data_packet() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.set_external_port_attachment(ExternalPortAttachmentKind::Printer);
    let compressed_tile = vec![0x01, 0x80, 0x40, 0x82, 0x00, 0x88, 0xFF];

    send_printer_packet_with_flag(&mut machine, PrinterCommand::Data, 0x01, &compressed_tile);
    assert_eq!(serial_transfer_byte(&mut machine, 0x00), 0x81);
    assert_eq!(serial_transfer_byte(&mut machine, 0x00), 0x08);

    send_printer_packet(&mut machine, PrinterCommand::Data, &[]);
    serial_transfer_byte(&mut machine, 0x00);
    serial_transfer_byte(&mut machine, 0x00);

    send_printer_packet(
        &mut machine,
        PrinterCommand::Print,
        &[0x01, 0x13, 0xE4, 0x40],
    );
    serial_transfer_byte(&mut machine, 0x00);
    serial_transfer_byte(&mut machine, 0x00);

    send_printer_packet(&mut machine, PrinterCommand::Status, &[]);
    serial_transfer_byte(&mut machine, 0x00);
    assert_eq!(serial_transfer_byte(&mut machine, 0x00), 0x06);
    send_printer_packet(&mut machine, PrinterCommand::Status, &[]);
    serial_transfer_byte(&mut machine, 0x00);
    assert_eq!(serial_transfer_byte(&mut machine, 0x00), 0x04);

    let mut pages = machine.take_printed_pages();
    assert_eq!(pages.len(), 1);
    let page = pages.remove(0);
    assert_eq!(page.width, 160);
    assert_eq!(page.height, 8);
    assert_eq!(page.pixels[0], 1);
    assert_eq!(page.pixels[1], 2);
    assert_eq!(page.pixels[2], 0);
}

#[test]
fn printer_attachment_supports_the_documented_detection_sequence() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.set_external_port_attachment(ExternalPortAttachmentKind::Printer);

    let packet_responses = send_printer_packet(&mut machine, PrinterCommand::Status, &[]);

    assert!(packet_responses.iter().all(|&byte| byte == 0x00));
    assert_eq!(serial_transfer_byte(&mut machine, 0x00), 0x81);
    assert_eq!(serial_transfer_byte(&mut machine, 0x00), 0x00);
}

#[test]
fn printer_typed_page_fixture_matches_the_golden_output() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.set_external_port_attachment(ExternalPortAttachmentKind::Printer);

    let tile_bytes = vec![
        0x80, 0x40, 0x55, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ];

    let page = run_print_sequence(&mut machine, &tile_bytes, [0x01, 0x13, 0xE4, 0x40]);
    assert_eq!(page.width, 160);
    assert_eq!(page.height, 8);
    assert_eq!(page.print_args.palette, 0xE4);
    assert_eq!(page.print_args.exposure, 0x40);

    let rendered = render_printed_page_fixture(&page);
    let fixture_path = common::paths::trace_fixture_path(PRINTED_PAGE_FIXTURE_NAME);
    common::fixtures::ensure_text_fixture(&fixture_path, &rendered, FIXTURE_ACCEPT_ENV);
}
