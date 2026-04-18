use super::*;

impl CpuAddressEvent {
    pub(super) const fn read(address: u16) -> Self {
        Self {
            kind: CpuAddressEventKind::Read,
            access_address: Some(address),
            idu_address: None,
            update_direction: None,
        }
    }

    pub(super) const fn write(address: u16) -> Self {
        Self {
            kind: CpuAddressEventKind::Write,
            access_address: Some(address),
            idu_address: None,
            update_direction: None,
        }
    }

    pub(super) const fn incdec(address: u16, direction: CpuAddressUpdateDirection) -> Self {
        Self {
            kind: CpuAddressEventKind::IncDec,
            access_address: None,
            idu_address: Some(address),
            update_direction: Some(direction),
        }
    }

    pub(super) const fn read_with_incdec(
        access_address: u16,
        idu_address: u16,
        direction: CpuAddressUpdateDirection,
    ) -> Self {
        Self {
            kind: CpuAddressEventKind::ReadWithIncDec,
            access_address: Some(access_address),
            idu_address: Some(idu_address),
            update_direction: Some(direction),
        }
    }

    pub(super) const fn write_with_incdec(
        access_address: u16,
        idu_address: u16,
        direction: CpuAddressUpdateDirection,
    ) -> Self {
        Self {
            kind: CpuAddressEventKind::WriteWithIncDec,
            access_address: Some(access_address),
            idu_address: Some(idu_address),
            update_direction: Some(direction),
        }
    }

    pub(super) fn trace_value(self) -> String {
        match self.kind {
            CpuAddressEventKind::Read => {
                let address = self
                    .access_address
                    .expect("read event must carry an access address");
                format!("read@{address:#06X}")
            }
            CpuAddressEventKind::Write => {
                let address = self
                    .access_address
                    .expect("write event must carry an access address");
                format!("write@{address:#06X}")
            }
            CpuAddressEventKind::IncDec => {
                let address = self
                    .idu_address
                    .expect("inc/dec event must carry an IDU address");
                format!(
                    "{}@{address:#06X}",
                    self.update_direction
                        .expect("inc/dec event must carry a direction")
                        .trace_label()
                )
            }
            CpuAddressEventKind::ReadWithIncDec | CpuAddressEventKind::WriteWithIncDec => {
                let access_address = self
                    .access_address
                    .expect("combined event must carry an access address");
                let idu_address = self
                    .idu_address
                    .expect("combined event must carry an IDU address");
                let access_label = match self.kind {
                    CpuAddressEventKind::ReadWithIncDec => "read",
                    CpuAddressEventKind::WriteWithIncDec => "write",
                    _ => unreachable!("combined event match already constrained"),
                };
                format!(
                    "{access_label}+{}@{access_address:#06X}->{idu_address:#06X}",
                    self.update_direction
                        .expect("combined event must carry a direction")
                        .trace_label()
                )
            }
        }
    }
}

impl CpuAddressUpdateDirection {
    pub(super) const fn trace_label(self) -> &'static str {
        match self {
            Self::Increment => "inc",
            Self::Decrement => "dec",
        }
    }
}

impl CpuStartupState {
    pub const fn power_on_reset() -> Self {
        Self {
            a: 0,
            f: 0,
            b: 0,
            c: 0,
            d: 0,
            e: 0,
            h: 0,
            l: 0,
            sp: 0,
            pc: 0x0000,
        }
    }
}

impl CpuRegisters {
    pub const fn from_startup_state(startup_state: CpuStartupState) -> Self {
        Self {
            a: startup_state.a,
            f: startup_state.f & 0xF0,
            b: startup_state.b,
            c: startup_state.c,
            d: startup_state.d,
            e: startup_state.e,
            h: startup_state.h,
            l: startup_state.l,
            sp: startup_state.sp,
            pc: startup_state.pc,
        }
    }
}

impl CpuExecutionState {
    pub const fn fetch_opcode() -> Self {
        Self::FetchOpcode { t_cycle: 0 }
    }
}

impl InFlightInstruction {
    const fn clear_decoded_state(&mut self) {
        self.kind = None;
        self.execution_group = None;
        self.cb_instruction_kind = None;
        self.operand8_latch = 0;
        self.operand16_latch = 0;
    }

    const fn clear(&mut self) {
        self.opcode = None;
        self.clear_decoded_state();
    }
}

impl ImeState {
    pub(super) const fn ime_enabled(self) -> bool {
        matches!(self, Self::Enabled | Self::EnabledPendingEnable { .. })
    }

    pub(super) const fn delayed_enable_pending(self) -> bool {
        matches!(
            self,
            Self::DisabledPendingEnable { .. } | Self::EnabledPendingEnable { .. }
        )
    }
}

impl CpuCore {
    pub(super) fn ime_enabled(&self) -> bool {
        self.ime_state.ime_enabled()
    }

    pub(super) fn set_ime_enabled(&mut self) {
        self.ime_state = ImeState::Enabled;
    }

    pub(super) fn set_ime_disabled(&mut self) {
        self.ime_state = ImeState::Disabled;
    }

    #[cfg(test)]
    pub(super) fn force_delayed_ime_enable_for_test(&mut self, instructions_remaining: u8) {
        self.ime_state = ImeState::DisabledPendingEnable {
            instructions_remaining,
        };
    }

    pub(super) fn request_halt_after_current_instruction(
        &mut self,
        ime_enabled: bool,
        had_pending_ei: bool,
    ) {
        self.halt_control = HaltControlState::PendingRequest(HaltRequestContext {
            ime_enabled,
            had_pending_ei,
        });
    }

    pub(super) fn take_halt_request(&mut self) -> Option<HaltRequestContext> {
        match self.halt_control {
            HaltControlState::PendingRequest(context) => {
                self.halt_control = HaltControlState::Idle;
                Some(context)
            }
            _ => None,
        }
    }

    pub(super) fn arm_halt_bug(&mut self) {
        self.halt_control = HaltControlState::HaltBugPending;
    }

    pub(super) fn consume_halt_bug_pending(&mut self) -> bool {
        if matches!(self.halt_control, HaltControlState::HaltBugPending) {
            self.halt_control = HaltControlState::Idle;
            true
        } else {
            false
        }
    }

    #[cfg(test)]
    pub(super) fn halt_request_pending_for_test(&self) -> bool {
        matches!(self.halt_control, HaltControlState::PendingRequest(_))
    }

    pub(super) fn clear_decoded_instruction_state(&mut self) {
        self.in_flight.clear_decoded_state();
    }

    pub(super) fn clear_in_flight_instruction_state(&mut self) {
        self.in_flight.clear();
    }
}
