use super::{
    PRINTER_MAGIC_0, PRINTER_MAGIC_1, PRINTER_MAX_DATA_PACKET_BYTES, PrinterCommand, PrinterDevice,
    PrinterMargins, PrinterParserState, PrinterStatusBits,
};

impl PrinterCommand {
    pub(super) fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0x01 => Some(Self::Initialize),
            0x02 => Some(Self::Print),
            0x04 => Some(Self::Data),
            0x0F => Some(Self::Status),
            _ => None,
        }
    }
}

impl PrinterMargins {
    pub(super) fn from_byte(byte: u8) -> Self {
        Self {
            before: byte >> 4,
            after: byte & 0x0F,
        }
    }
}

impl PrinterStatusBits {
    pub(super) fn to_byte(self) -> u8 {
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

impl PrinterDevice {
    pub(super) fn consume_byte(&mut self, byte: u8) {
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
        debug_assert_eq!(command_data.len(), expected_data_len as usize);

        if checksum != self.packet_checksum_sum {
            self.status.checksum_error = true;
            self.queue_response(0x81, self.status.to_byte());
            self.clear_current_packet();
            return;
        }

        self.status.checksum_error = false;
        let Some(command) = PrinterCommand::from_byte(command) else {
            self.reject_current_packet();
            return;
        };

        let decoded_command_data;
        let command_data = match (command, compression_flag) {
            (PrinterCommand::Data, 0x00) => command_data.as_slice(),
            (PrinterCommand::Data, 0x01) => {
                let Some(decoded_data) = decode_compressed_data_packet(&command_data) else {
                    self.reject_current_packet();
                    return;
                };
                decoded_command_data = decoded_data;
                decoded_command_data.as_slice()
            }
            (_, 0x00) => command_data.as_slice(),
            _ => {
                self.reject_current_packet();
                return;
            }
        };

        if !command_payload_len_is_valid(command, command_data.len()) {
            self.reject_current_packet();
            return;
        }

        self.status.packet_error = false;
        self.execute_command(command, command_data);
        self.queue_response(0x81, self.status.to_byte());
        self.clear_current_packet();
    }

    fn reject_current_packet(&mut self) {
        self.status.packet_error = true;
        self.queue_response(0x81, self.status.to_byte());
        self.clear_current_packet();
    }

    fn clear_current_packet(&mut self) {
        self.packet_data.clear();
        self.packet_checksum_sum = 0;
    }
}

fn command_payload_len_is_valid(command: PrinterCommand, len: usize) -> bool {
    match command {
        PrinterCommand::Initialize | PrinterCommand::Status => len == 0,
        PrinterCommand::Print => len == 4,
        PrinterCommand::Data => len <= PRINTER_MAX_DATA_PACKET_BYTES,
    }
}

fn decode_compressed_data_packet(data: &[u8]) -> Option<Vec<u8>> {
    let mut decoded = Vec::new();
    let mut index = 0;

    while index < data.len() {
        let control = data[index];
        index += 1;

        if control & 0x80 == 0 {
            let len = usize::from(control & 0x7F) + 1;
            let end = index.checked_add(len)?;
            if end > data.len() || decoded.len().checked_add(len)? > PRINTER_MAX_DATA_PACKET_BYTES {
                return None;
            }
            decoded.extend_from_slice(&data[index..end]);
            index = end;
        } else {
            let len = usize::from(control & 0x7F) + 2;
            let &byte = data.get(index)?;
            if decoded.len().checked_add(len)? > PRINTER_MAX_DATA_PACKET_BYTES {
                return None;
            }
            decoded.extend(std::iter::repeat_n(byte, len));
            index += 1;
        }
    }

    Some(decoded)
}
