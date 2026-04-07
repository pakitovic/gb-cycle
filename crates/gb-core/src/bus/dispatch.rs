use crate::cartridge::CartridgeSlot;
use crate::scheduler::TCycle;

use super::{
    Bus, BusAccessDisposition, BusAccessKind, BusAccessResolution, BusAddressInfo,
    BusArbitrationState, BusIoReadView, BusIoWriteView, BusRegion, BusRequester,
    DmaCpuAccessPolicy,
};

impl Bus {
    pub fn resolve_access(
        &self,
        requester: BusRequester,
        kind: BusAccessKind,
        address: u16,
        state: &BusArbitrationState,
    ) -> BusAccessResolution {
        let target = self.resolve_nominal_target(kind, address, state);
        let disposition = self.evaluate_access_policy(requester, kind, target, state);

        BusAccessResolution::new(requester, kind, target, disposition)
    }

    pub fn read(&mut self, address: u16) -> u8 {
        self.read_with(address, BusRequester::Cpu, &BusArbitrationState::default())
    }

    pub fn read_with(
        &mut self,
        address: u16,
        requester: BusRequester,
        state: &BusArbitrationState,
    ) -> u8 {
        self.read_with_context(address, requester, state, None, BusIoReadView::default())
    }

    pub fn read_with_cartridge(
        &mut self,
        address: u16,
        requester: BusRequester,
        state: &BusArbitrationState,
        cartridge: Option<&CartridgeSlot>,
    ) -> u8 {
        self.read_with_context(
            address,
            requester,
            state,
            cartridge,
            BusIoReadView::default(),
        )
    }

    pub(crate) fn read_with_context(
        &mut self,
        address: u16,
        requester: BusRequester,
        state: &BusArbitrationState,
        cartridge: Option<&CartridgeSlot>,
        io: BusIoReadView<'_>,
    ) -> u8 {
        if let Some(conflict_source_address) =
            self.cpu_dma_conflict_source_address(requester, address, state)
        {
            let target =
                self.resolve_nominal_target(BusAccessKind::Read, conflict_source_address, state);
            return self.perform_allowed_read(target, cartridge, io);
        }

        let resolution = self.resolve_access(requester, BusAccessKind::Read, address, state);

        match resolution.disposition() {
            BusAccessDisposition::Allowed => {
                self.perform_allowed_read(resolution.target(), cartridge, io)
            }
            BusAccessDisposition::BlockedRead { value, .. } => value,
            BusAccessDisposition::IgnoredWrite { .. } => {
                panic!("read path received write-only access disposition")
            }
        }
    }

    pub(crate) fn read_with_t_cycle_context(
        &mut self,
        address: u16,
        requester: BusRequester,
        state: &BusArbitrationState,
        t_cycle: TCycle,
        cartridge: Option<&mut CartridgeSlot>,
        io: BusIoReadView<'_>,
    ) -> u8 {
        if let Some(conflict_source_address) =
            self.cpu_dma_conflict_source_address(requester, address, state)
        {
            let target =
                self.resolve_nominal_target(BusAccessKind::Read, conflict_source_address, state);
            return self.perform_allowed_read_timed(target, t_cycle, cartridge, io);
        }

        let resolution = self.resolve_access(requester, BusAccessKind::Read, address, state);

        match resolution.disposition() {
            BusAccessDisposition::Allowed => {
                self.perform_allowed_read_timed(resolution.target(), t_cycle, cartridge, io)
            }
            BusAccessDisposition::BlockedRead { value, .. } => value,
            BusAccessDisposition::IgnoredWrite { .. } => {
                panic!("read path received write-only access disposition")
            }
        }
    }

    pub fn write(&mut self, address: u16, value: u8) {
        self.write_with(
            address,
            value,
            BusRequester::Cpu,
            &BusArbitrationState::default(),
        );
    }

    pub fn write_with(
        &mut self,
        address: u16,
        value: u8,
        requester: BusRequester,
        state: &BusArbitrationState,
    ) {
        self.write_with_context(
            address,
            value,
            requester,
            state,
            None,
            BusIoWriteView::default(),
        );
    }

    pub fn write_with_cartridge(
        &mut self,
        address: u16,
        value: u8,
        requester: BusRequester,
        state: &BusArbitrationState,
        cartridge: Option<&mut CartridgeSlot>,
    ) {
        self.write_with_context(
            address,
            value,
            requester,
            state,
            cartridge,
            BusIoWriteView::default(),
        );
    }

    pub(crate) fn write_with_context(
        &mut self,
        address: u16,
        value: u8,
        requester: BusRequester,
        state: &BusArbitrationState,
        cartridge: Option<&mut CartridgeSlot>,
        io: BusIoWriteView<'_>,
    ) {
        if let Some(conflict_source_address) =
            self.cpu_dma_conflict_source_address(requester, address, state)
        {
            let target =
                self.resolve_nominal_target(BusAccessKind::Write, conflict_source_address, state);
            self.perform_allowed_write(target, value, cartridge, io);
            return;
        }

        let resolution = self.resolve_access(requester, BusAccessKind::Write, address, state);

        match resolution.disposition() {
            BusAccessDisposition::Allowed => {
                self.perform_allowed_write(resolution.target(), value, cartridge, io)
            }
            BusAccessDisposition::IgnoredWrite { .. } => {}
            BusAccessDisposition::BlockedRead { .. } => {
                panic!("write path received read-only access disposition")
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn write_with_t_cycle_context(
        &mut self,
        address: u16,
        value: u8,
        requester: BusRequester,
        state: &BusArbitrationState,
        t_cycle: TCycle,
        cartridge: Option<&mut CartridgeSlot>,
        io: BusIoWriteView<'_>,
    ) {
        if let Some(conflict_source_address) =
            self.cpu_dma_conflict_source_address(requester, address, state)
        {
            let target =
                self.resolve_nominal_target(BusAccessKind::Write, conflict_source_address, state);
            self.perform_allowed_write_timed(target, value, t_cycle, cartridge, io);
            return;
        }

        let resolution = self.resolve_access(requester, BusAccessKind::Write, address, state);

        match resolution.disposition() {
            BusAccessDisposition::Allowed => {
                self.perform_allowed_write_timed(resolution.target(), value, t_cycle, cartridge, io)
            }
            BusAccessDisposition::IgnoredWrite { .. } => {}
            BusAccessDisposition::BlockedRead { .. } => {
                panic!("write path received read-only access disposition")
            }
        }
    }

    fn cpu_dma_conflict_source_address(
        &self,
        requester: BusRequester,
        address: u16,
        state: &BusArbitrationState,
    ) -> Option<u16> {
        if requester != BusRequester::Cpu
            || state.dma.cpu_access_policy() != DmaCpuAccessPolicy::ExternalBusBlocked
            || address >= 0xFE00
        {
            return None;
        }

        let target = self.decode_address(address);
        if !matches!(
            target.region(),
            BusRegion::CartridgeRomBank0
                | BusRegion::CartridgeRomBankN
                | BusRegion::CartridgeExternal
                | BusRegion::WramBank0
                | BusRegion::WramBankN
                | BusRegion::EchoRam
        ) {
            return None;
        }

        state.dma.cpu_conflict_source_address()
    }

    fn resolve_nominal_target(
        &self,
        kind: BusAccessKind,
        address: u16,
        state: &BusArbitrationState,
    ) -> BusAddressInfo {
        self.router.resolve_nominal_target(kind, address, state)
    }
}
