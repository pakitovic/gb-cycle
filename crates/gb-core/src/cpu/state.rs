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

pub(super) const fn interrupt_vector(source: InterruptSource) -> u16 {
    match source {
        InterruptSource::VBlank => 0x0040,
        InterruptSource::LcdStat => 0x0048,
        InterruptSource::Timer => 0x0050,
        InterruptSource::Serial => 0x0058,
        InterruptSource::Joypad => 0x0060,
    }
}

pub(super) const fn highest_pending_interrupt_from_mask(mask: u8) -> Option<InterruptSource> {
    if mask & 0x01 != 0 {
        Some(InterruptSource::VBlank)
    } else if mask & 0x02 != 0 {
        Some(InterruptSource::LcdStat)
    } else if mask & 0x04 != 0 {
        Some(InterruptSource::Timer)
    } else if mask & 0x08 != 0 {
        Some(InterruptSource::Serial)
    } else if mask & 0x10 != 0 {
        Some(InterruptSource::Joypad)
    } else {
        None
    }
}
