use crate::cpu::{CpuAddressEvent, CpuAddressEventKind, CpuAddressUpdateDirection};
use crate::ppu::{OamCorruptionEventKind, Ppu};

use super::{Bus, BusArbitrationState};

impl Bus {
    pub(crate) fn route_cpu_address_event(
        &mut self,
        event: CpuAddressEvent,
        state: &BusArbitrationState,
        ppu: &mut Ppu,
    ) {
        let Some(kind) = self.classify_oam_corruption_event(event, state) else {
            return;
        };

        let _ = ppu.apply_oam_corruption_event(kind, self.oam.bytes_mut());
    }

    fn classify_oam_corruption_event(
        &self,
        event: CpuAddressEvent,
        state: &BusArbitrationState,
    ) -> Option<OamCorruptionEventKind> {
        match event.kind {
            CpuAddressEventKind::IncDec => {
                let glitched_address = idu_glitched_address(event)?;
                if self.idu_event_reaches_oam(glitched_address, state) {
                    Some(OamCorruptionEventKind::Write)
                } else {
                    None
                }
            }
            CpuAddressEventKind::Read
            | CpuAddressEventKind::Write
            | CpuAddressEventKind::ReadWithIncDec
            | CpuAddressEventKind::WriteWithIncDec => {
                let access_hits_corruption = event
                    .access_address
                    .is_some_and(cpu_address_event_access_reaches_oam_corruption);
                let idu_hits_corruption = matches!(
                    event.kind,
                    CpuAddressEventKind::ReadWithIncDec | CpuAddressEventKind::WriteWithIncDec
                ) && idu_glitched_address(event)
                    .is_some_and(|address| self.idu_event_reaches_oam(address, state));

                if access_hits_corruption || idu_hits_corruption {
                    Some(oam_corruption_event_kind(event.kind))
                } else {
                    None
                }
            }
        }
    }

    fn idu_event_reaches_oam(&self, address: u16, state: &BusArbitrationState) -> bool {
        let _ = state;
        self.console_model.is_dmg_family()
            && cpu_address_event_access_reaches_oam_corruption(address)
    }
}

fn oam_corruption_event_kind(kind: CpuAddressEventKind) -> OamCorruptionEventKind {
    match kind {
        CpuAddressEventKind::Read => OamCorruptionEventKind::Read,
        CpuAddressEventKind::Write => OamCorruptionEventKind::Write,
        CpuAddressEventKind::IncDec => OamCorruptionEventKind::Write,
        CpuAddressEventKind::ReadWithIncDec => OamCorruptionEventKind::ReadWithIncDec,
        CpuAddressEventKind::WriteWithIncDec => OamCorruptionEventKind::WriteWithIncDec,
    }
}

fn idu_glitched_address(event: CpuAddressEvent) -> Option<u16> {
    let driven_address = event.idu_address?;
    match event.update_direction? {
        CpuAddressUpdateDirection::Increment => Some(driven_address.wrapping_sub(1)),
        CpuAddressUpdateDirection::Decrement => Some(driven_address.wrapping_add(1)),
    }
}

fn cpu_address_event_access_reaches_oam_corruption(address: u16) -> bool {
    (0xFE00..=0xFEFF).contains(&address)
}
