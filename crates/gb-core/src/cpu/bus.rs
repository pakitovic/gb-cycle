use super::decode::{MemoryAddressSource, Register16};
use super::trace::{CpuTraceBusAccessKind, CpuTraceBusActivity};
use super::*;

impl CpuCore {
    pub(super) fn last_bus_activity_trace_value(&self) -> String {
        match self.last_bus_activity {
            Some(CpuTraceBusActivity {
                kind,
                address,
                value,
            }) => format!("{}@{address:#06X}={value:#04X}", kind.trace_label()),
            None => "none".to_string(),
        }
    }

    pub(super) fn last_address_event_trace_value(&self) -> String {
        match self.last_address_event {
            Some(event) => event.trace_value(),
            None => "none".to_string(),
        }
    }

    fn record_bus_activity(&mut self, kind: CpuTraceBusAccessKind, address: u16, value: u8) {
        self.last_bus_activity = Some(CpuTraceBusActivity {
            kind,
            address,
            value,
        });
    }

    pub(super) fn record_address_event(&mut self, event: CpuAddressEvent) {
        self.last_address_event = Some(event);
    }

    pub(super) fn read_opcode_u8<F>(&mut self, bus_operation: &mut F) -> u8
    where
        F: FnMut(CpuBusOperation) -> Option<u8>,
    {
        self.read_pc_u8_with_kind(bus_operation, CpuTraceBusAccessKind::OpcodeFetch)
    }

    pub(super) fn read_pc_u8<F>(&mut self, bus_operation: &mut F) -> u8
    where
        F: FnMut(CpuBusOperation) -> Option<u8>,
    {
        self.read_pc_u8_with_kind(bus_operation, CpuTraceBusAccessKind::OperandRead)
    }

    fn read_pc_u8_with_kind<F>(&mut self, bus_operation: &mut F, kind: CpuTraceBusAccessKind) -> u8
    where
        F: FnMut(CpuBusOperation) -> Option<u8>,
    {
        let address = self.registers.pc;
        let value = self.read_byte_with_kind(address, bus_operation, kind);

        if self.halt_bug_pending {
            self.halt_bug_pending = false;
            self.record_address_event(CpuAddressEvent::read(address));
        } else {
            self.registers.pc = self.registers.pc.wrapping_add(1);
            self.record_address_event(CpuAddressEvent::read_with_incdec(
                address,
                self.registers.pc,
                CpuAddressUpdateDirection::Increment,
            ));
        }
        value
    }

    pub(super) fn read_byte<F>(&mut self, address: u16, bus_operation: &mut F) -> u8
    where
        F: FnMut(CpuBusOperation) -> Option<u8>,
    {
        let value =
            self.read_byte_with_kind(address, bus_operation, CpuTraceBusAccessKind::DataRead);
        self.record_address_event(CpuAddressEvent::read(address));
        value
    }

    fn read_byte_with_kind<F>(
        &mut self,
        address: u16,
        bus_operation: &mut F,
        kind: CpuTraceBusAccessKind,
    ) -> u8
    where
        F: FnMut(CpuBusOperation) -> Option<u8>,
    {
        let value = bus_operation(CpuBusOperation::Read { address })
            .expect("CPU bus read must produce a byte result");
        self.record_bus_activity(kind, address, value);
        value
    }

    pub(super) fn write_byte<F>(&mut self, address: u16, value: u8, bus_operation: &mut F)
    where
        F: FnMut(CpuBusOperation) -> Option<u8>,
    {
        let _ = bus_operation(CpuBusOperation::Write { address, value });
        self.record_bus_activity(CpuTraceBusAccessKind::DataWrite, address, value);
        self.record_address_event(CpuAddressEvent::write(address));
    }

    pub(super) fn resolve_memory_address(&self, source: MemoryAddressSource) -> u16 {
        match source {
            MemoryAddressSource::BC => self.bc(),
            MemoryAddressSource::DE => self.de(),
            MemoryAddressSource::Immediate16 => self.operand16_latch,
            MemoryAddressSource::HighImmediate8 => 0xFF00 | u16::from(self.operand8_latch),
            MemoryAddressSource::HighC => 0xFF00 | u16::from(self.registers.c),
        }
    }

    pub(super) fn read_hl_with_update<F>(
        &mut self,
        direction: CpuAddressUpdateDirection,
        bus_operation: &mut F,
    ) -> u8
    where
        F: FnMut(CpuBusOperation) -> Option<u8>,
    {
        let address = self.hl();
        let value = self.read_byte(address, bus_operation);
        let updated = self.increment_or_decrement_register16(Register16::HL, direction);
        self.record_address_event(CpuAddressEvent::read_with_incdec(
            address, updated, direction,
        ));
        value
    }

    pub(super) fn write_hl_with_update<F>(
        &mut self,
        value: u8,
        direction: CpuAddressUpdateDirection,
        bus_operation: &mut F,
    ) where
        F: FnMut(CpuBusOperation) -> Option<u8>,
    {
        let address = self.hl();
        self.write_byte(address, value, bus_operation);
        let updated = self.increment_or_decrement_register16(Register16::HL, direction);
        self.record_address_event(CpuAddressEvent::write_with_incdec(
            address, updated, direction,
        ));
    }

    pub(super) fn read_byte_and_increment_sp<F>(&mut self, bus_operation: &mut F) -> u8
    where
        F: FnMut(CpuBusOperation) -> Option<u8>,
    {
        let address = self.registers.sp;
        let value = self.read_byte(address, bus_operation);
        self.registers.sp = self.registers.sp.wrapping_add(1);
        self.record_address_event(CpuAddressEvent::read_with_incdec(
            address,
            self.registers.sp,
            CpuAddressUpdateDirection::Increment,
        ));
        value
    }

    pub(super) fn decrement_sp_and_record_idu_event(&mut self) {
        self.registers.sp = self.registers.sp.wrapping_sub(1);
        self.record_address_event(CpuAddressEvent::incdec(
            self.registers.sp,
            CpuAddressUpdateDirection::Decrement,
        ));
    }

    pub(super) fn write_byte_at_sp<F>(&mut self, value: u8, bus_operation: &mut F)
    where
        F: FnMut(CpuBusOperation) -> Option<u8>,
    {
        self.write_byte(self.registers.sp, value, bus_operation);
    }

    pub(super) fn write_byte_with_decremented_sp<F>(&mut self, value: u8, bus_operation: &mut F)
    where
        F: FnMut(CpuBusOperation) -> Option<u8>,
    {
        self.registers.sp = self.registers.sp.wrapping_sub(1);
        let address = self.registers.sp;
        self.write_byte(address, value, bus_operation);
        self.record_address_event(CpuAddressEvent::write_with_incdec(
            address,
            address,
            CpuAddressUpdateDirection::Decrement,
        ));
    }
}
