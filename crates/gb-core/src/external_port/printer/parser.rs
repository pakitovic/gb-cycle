use super::{
    PRINTER_MAGIC_0, PRINTER_MAGIC_1, PrinterCommand, PrinterDevice, PrinterMargins,
    PrinterParserState, PrinterStatusBits,
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
}
