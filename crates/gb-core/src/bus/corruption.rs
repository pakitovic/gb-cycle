use crate::cpu::{CpuAddressEvent, CpuAddressEventKind, CpuAddressUpdateDirection};
use crate::ppu::{OamCorruptionEventKind, Ppu, PpuAccessMode};

use super::{Bus, BusAccessKind, BusArbitrationState, BusBlockReason, BusRegion};

impl Bus {
    pub(crate) fn route_cpu_address_event(
        &mut self,
        event: CpuAddressEvent,
        state: &BusArbitrationState,
        ppu: &mut Ppu,
    ) {
        if !self.console_model.is_dmg_family() {
            return;
        }

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
                let access_address = event.access_address?;
                let idu_may_hit_oam_corruption = matches!(
                    event.kind,
                    CpuAddressEventKind::ReadWithIncDec | CpuAddressEventKind::WriteWithIncDec
                ) && idu_glitched_address(event)
                    .is_some_and(address_may_hit_oam_corruption);

                if !address_may_hit_oam_corruption(access_address) && !idu_may_hit_oam_corruption {
                    return None;
                }

                let access_hits_corruption = if address_may_hit_oam_corruption(access_address) {
                    let access_kind = access_kind_for_cpu_address_event(event.kind);
                    let resolution = self.resolve_access(access_kind, access_address, state, None);

                    match resolution.target().region() {
                        BusRegion::Oam
                            if resolution.disposition().blocked_reason()
                                == Some(BusBlockReason::PpuOamBlockedDuringMode2) =>
                        {
                            true
                        }
                        BusRegion::Unusable
                            if access_kind == BusAccessKind::Write
                                && state.ppu.is_lcd_enabled()
                                && state.ppu.mode() == PpuAccessMode::OamScan =>
                        {
                            true
                        }
                        BusRegion::Unusable
                            if access_kind == BusAccessKind::Read
                                && state.ppu.is_lcd_enabled()
                                && state.ppu.mode() == PpuAccessMode::OamScan
                                && resolution.disposition().blocked_reason()
                                    == Some(BusBlockReason::UnusableRegionDuringOamBlock) =>
                        {
                            true
                        }
                        _ => false,
                    }
                } else {
                    false
                };

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
        self.console_model.is_dmg_family() && address_may_hit_oam_corruption(address)
    }
}

fn access_kind_for_cpu_address_event(kind: CpuAddressEventKind) -> BusAccessKind {
    match kind {
        CpuAddressEventKind::Read | CpuAddressEventKind::ReadWithIncDec => BusAccessKind::Read,
        CpuAddressEventKind::Write | CpuAddressEventKind::WriteWithIncDec => BusAccessKind::Write,
        CpuAddressEventKind::IncDec => {
            unreachable!("pure IDU events do not have an ordinary bus access kind")
        }
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

#[inline(always)]
fn address_may_hit_oam_corruption(address: u16) -> bool {
    (address & 0xFF00) == 0xFE00
}

#[cfg(test)]
mod tests {
    use super::address_may_hit_oam_corruption;

    #[test]
    fn address_may_hit_oam_corruption_matches_the_fe_page_only() {
        assert!(address_may_hit_oam_corruption(0xFE00));
        assert!(address_may_hit_oam_corruption(0xFE9F));
        assert!(address_may_hit_oam_corruption(0xFEFF));
        assert!(!address_may_hit_oam_corruption(0xFDFF));
        assert!(!address_may_hit_oam_corruption(0xFF00));
    }
}
