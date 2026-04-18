use super::decode::{DirectAddressSource, Register16};
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

    pub(super) fn read_opcode_u8(&mut self, bus_operation: &mut CpuExternalCallback<'_>) -> u8 {
        self.read_pc_u8_with_kind(bus_operation, CpuTraceBusAccessKind::OpcodeFetch)
    }

    pub(super) fn read_pc_u8(&mut self, bus_operation: &mut CpuExternalCallback<'_>) -> u8 {
        self.read_pc_u8_with_kind(bus_operation, CpuTraceBusAccessKind::OperandRead)
    }

    fn read_pc_u8_with_kind(
        &mut self,
        bus_operation: &mut CpuExternalCallback<'_>,
        kind: CpuTraceBusAccessKind,
    ) -> u8 {
        let address = self.registers.pc;
        let value = self.read_byte_with_kind(address, bus_operation, kind);

        if self.consume_halt_bug_pending() {
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

    pub(super) fn read_byte(
        &mut self,
        address: u16,
        bus_operation: &mut CpuExternalCallback<'_>,
    ) -> u8 {
        let value =
            self.read_byte_with_kind(address, bus_operation, CpuTraceBusAccessKind::DataRead);
        self.record_address_event(CpuAddressEvent::read(address));
        value
    }

    fn read_byte_with_kind(
        &mut self,
        address: u16,
        bus_operation: &mut CpuExternalCallback<'_>,
        kind: CpuTraceBusAccessKind,
    ) -> u8 {
        let value = bus_operation(CpuExternalOperation::Bus(CpuBusOperation::Read { address }))
            .expect("CPU bus read must produce a byte result");
        self.record_bus_activity(kind, address, value);
        value
    }

    pub(super) fn stop_wake_line_asserted(
        &mut self,
        bus_operation: &mut CpuExternalCallback<'_>,
    ) -> bool {
        bus_operation(CpuExternalOperation::StopWakeLineAsserted).unwrap_or(0) != 0
    }

    pub(super) fn write_byte(
        &mut self,
        address: u16,
        value: u8,
        bus_operation: &mut CpuExternalCallback<'_>,
    ) {
        self.write_byte_without_address_event(address, value, bus_operation);
        self.record_address_event(CpuAddressEvent::write(address));
    }

    fn write_byte_without_address_event(
        &mut self,
        address: u16,
        value: u8,
        bus_operation: &mut CpuExternalCallback<'_>,
    ) {
        let _ = bus_operation(CpuExternalOperation::Bus(CpuBusOperation::Write {
            address,
            value,
        }));
        self.record_bus_activity(CpuTraceBusAccessKind::DataWrite, address, value);
    }

    pub(super) fn resolve_direct_address(&self, source: DirectAddressSource) -> u16 {
        match source {
            DirectAddressSource::BC => self.bc(),
            DirectAddressSource::DE => self.de(),
            DirectAddressSource::HighC => 0xFF00 | u16::from(self.registers.c),
        }
    }

    pub(super) fn resolve_immediate16_address(&self) -> u16 {
        self.in_flight.operand16_latch
    }

    pub(super) fn resolve_high_immediate_address(&self) -> u16 {
        0xFF00 | u16::from(self.in_flight.operand8_latch)
    }

    pub(super) fn prepare_pc_stack_push(&mut self) {
        let [low, _high] = self.registers.pc.to_le_bytes();
        self.in_flight.operand8_latch = low;
        self.decrement_sp_and_record_idu_event();
    }

    pub(super) fn push_pc_high_at_sp(&mut self, bus_operation: &mut CpuExternalCallback<'_>) {
        let [_low, high] = self.registers.pc.to_le_bytes();
        self.write_byte_at_sp(high, bus_operation);
    }

    pub(super) fn push_latched_u16_high_at_sp(
        &mut self,
        bus_operation: &mut CpuExternalCallback<'_>,
    ) {
        let [_low, high] = self.in_flight.operand16_latch.to_le_bytes();
        self.write_byte_at_sp(high, bus_operation);
    }

    pub(super) fn push_latched_low_with_decremented_sp(
        &mut self,
        bus_operation: &mut CpuExternalCallback<'_>,
    ) {
        self.write_byte_with_decremented_sp(self.in_flight.operand8_latch, bus_operation);
    }

    pub(super) fn read_latched_stack_low_byte(
        &mut self,
        bus_operation: &mut CpuExternalCallback<'_>,
    ) {
        let low = self.read_byte_and_increment_sp(bus_operation);
        self.in_flight.operand16_latch = u16::from(low);
    }

    pub(super) fn read_latched_stack_high_byte(
        &mut self,
        bus_operation: &mut CpuExternalCallback<'_>,
    ) {
        let high = self.read_byte_and_increment_sp(bus_operation);
        self.in_flight.operand16_latch |= u16::from(high) << 8;
    }

    pub(super) fn read_hl_with_update(
        &mut self,
        direction: CpuAddressUpdateDirection,
        bus_operation: &mut CpuExternalCallback<'_>,
    ) -> u8 {
        let address = self.hl();
        let value =
            self.read_byte_with_kind(address, bus_operation, CpuTraceBusAccessKind::DataRead);
        let updated = self.increment_or_decrement_register16(Register16::HL, direction);
        self.record_address_event(CpuAddressEvent::read_with_incdec(
            address, updated, direction,
        ));
        value
    }

    pub(super) fn write_hl_with_update(
        &mut self,
        value: u8,
        direction: CpuAddressUpdateDirection,
        bus_operation: &mut CpuExternalCallback<'_>,
    ) {
        let address = self.hl();
        self.write_byte_without_address_event(address, value, bus_operation);
        let updated = self.increment_or_decrement_register16(Register16::HL, direction);
        self.record_address_event(CpuAddressEvent::write_with_incdec(
            address, updated, direction,
        ));
    }

    pub(super) fn read_byte_and_increment_sp(
        &mut self,
        bus_operation: &mut CpuExternalCallback<'_>,
    ) -> u8 {
        let address = self.registers.sp;
        let value =
            self.read_byte_with_kind(address, bus_operation, CpuTraceBusAccessKind::DataRead);
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

    pub(super) fn write_byte_at_sp(
        &mut self,
        value: u8,
        bus_operation: &mut CpuExternalCallback<'_>,
    ) {
        self.write_byte(self.registers.sp, value, bus_operation);
    }

    pub(super) fn write_byte_with_decremented_sp(
        &mut self,
        value: u8,
        bus_operation: &mut CpuExternalCallback<'_>,
    ) {
        self.registers.sp = self.registers.sp.wrapping_sub(1);
        let address = self.registers.sp;
        self.write_byte_without_address_event(address, value, bus_operation);
        self.record_address_event(CpuAddressEvent::write_with_incdec(
            address,
            address,
            CpuAddressUpdateDirection::Decrement,
        ));
    }
}
