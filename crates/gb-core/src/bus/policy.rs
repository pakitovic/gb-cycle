use crate::ppu::PpuAccessMode;

use super::{
    BLOCKED_READ_VALUE, Bus, BusAccessDisposition, BusAccessKind, BusAddressInfo,
    BusArbitrationState, BusBlockReason, BusRegion, BusRequester, DmaCpuAccessPolicy, OamDomain,
    VramDomain,
};

impl Bus {
    pub(super) fn evaluate_access_policy(
        &self,
        requester: BusRequester,
        kind: BusAccessKind,
        target: BusAddressInfo,
        state: &BusArbitrationState,
    ) -> BusAccessDisposition {
        if let Some(disposition) = self.evaluate_global_dma_policy(requester, kind, target, state) {
            return disposition;
        }

        if let Some(disposition) = self.evaluate_domain_policy(requester, kind, target, state) {
            return disposition;
        }

        if let Some(disposition) = self.evaluate_unusable_policy(requester, kind, target, state) {
            return disposition;
        }

        BusAccessDisposition::Allowed
    }

    fn evaluate_global_dma_policy(
        &self,
        requester: BusRequester,
        kind: BusAccessKind,
        target: BusAddressInfo,
        state: &BusArbitrationState,
    ) -> Option<BusAccessDisposition> {
        if requester != BusRequester::Cpu {
            return None;
        }

        match state.dma.cpu_access_policy() {
            DmaCpuAccessPolicy::Unrestricted => None,
            DmaCpuAccessPolicy::ExternalBusBlocked => {
                if target.region() == BusRegion::Hram
                    || target.region() == BusRegion::Vram
                    || target.address() == 0xFF46
                {
                    return None;
                }

                Some(self.block_access(kind, BusBlockReason::DmaExternalBusConflict))
            }
            DmaCpuAccessPolicy::VideoBusBlocked => {
                if matches!(target.region(), BusRegion::Vram | BusRegion::Oam) {
                    Some(self.block_access(kind, BusBlockReason::DmaVideoBusConflict))
                } else {
                    None
                }
            }
        }
    }

    fn evaluate_domain_policy(
        &self,
        requester: BusRequester,
        kind: BusAccessKind,
        target: BusAddressInfo,
        state: &BusArbitrationState,
    ) -> Option<BusAccessDisposition> {
        match target.region() {
            BusRegion::Vram => VramDomain::evaluate_access(requester, kind, state.ppu, state.dma),
            BusRegion::Oam => OamDomain::evaluate_access(requester, kind, state.ppu, state.dma),
            _ => None,
        }
    }

    fn evaluate_unusable_policy(
        &self,
        requester: BusRequester,
        kind: BusAccessKind,
        target: BusAddressInfo,
        state: &BusArbitrationState,
    ) -> Option<BusAccessDisposition> {
        if target.region() != BusRegion::Unusable {
            return None;
        }

        if kind == BusAccessKind::Write {
            return Some(BusAccessDisposition::IgnoredWrite {
                reason: BusBlockReason::UnusableRegion,
            });
        }

        if requester == BusRequester::Cpu
            && state.ppu.is_lcd_enabled()
            && matches!(
                state.ppu.mode(),
                PpuAccessMode::OamScan | PpuAccessMode::Drawing
            )
        {
            return Some(BusAccessDisposition::BlockedRead {
                value: BLOCKED_READ_VALUE,
                reason: BusBlockReason::UnusableRegionDuringOamBlock,
            });
        }

        None
    }

    fn block_access(&self, kind: BusAccessKind, reason: BusBlockReason) -> BusAccessDisposition {
        match kind {
            BusAccessKind::Read => BusAccessDisposition::BlockedRead {
                value: BLOCKED_READ_VALUE,
                reason,
            },
            BusAccessKind::Write => BusAccessDisposition::IgnoredWrite { reason },
        }
    }
}
