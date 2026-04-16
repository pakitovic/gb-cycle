mod common;

use gb_core::{
    ConsoleModel, ExternalPortAttachmentKind, Machine, MachineConfig, PrinterCommand, StartupMode,
};

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
    let mut packet = vec![
        0x88,
        0x33,
        command as u8,
        0x00,
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

#[test]
fn printer_attachment_supports_the_documented_detection_sequence() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.set_external_port_attachment(ExternalPortAttachmentKind::Printer);

    let packet_responses = send_printer_packet(&mut machine, PrinterCommand::Status, &[]);

    assert!(packet_responses.iter().all(|&byte| byte == 0x00));
    assert_eq!(serial_transfer_byte(&mut machine, 0x00), 0x81);
    assert_eq!(serial_transfer_byte(&mut machine, 0x00), 0x00);
}

#[test]
fn printer_attachment_produces_a_typed_printed_page() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.set_external_port_attachment(ExternalPortAttachmentKind::Printer);

    let tile_row = vec![0xFF; 320];

    send_printer_packet(&mut machine, PrinterCommand::Data, &tile_row);
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
    assert_eq!(serial_transfer_byte(&mut machine, 0x00), 0x81);
    assert_eq!(serial_transfer_byte(&mut machine, 0x00), 0x08);

    send_printer_packet(&mut machine, PrinterCommand::Status, &[]);
    serial_transfer_byte(&mut machine, 0x00);
    assert_eq!(serial_transfer_byte(&mut machine, 0x00), 0x06);

    send_printer_packet(&mut machine, PrinterCommand::Status, &[]);
    serial_transfer_byte(&mut machine, 0x00);
    assert_eq!(serial_transfer_byte(&mut machine, 0x00), 0x04);

    let pages = machine.take_printed_pages();
    assert_eq!(pages.len(), 1);
    assert_eq!(pages[0].width, 160);
    assert_eq!(pages[0].height, 8);
    assert_eq!(pages[0].print_args.palette, 0xE4);
    assert_eq!(pages[0].print_args.exposure, 0x40);
}
