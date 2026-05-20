mod access;
mod corruption;
mod dispatch;
mod infrared;
mod iohram;
mod map;
mod meta;
mod policy;
mod router;
mod state;
mod video;
mod view;
mod wram;

use crate::cartridge::{CartridgeHeader, CgbFlag};
use crate::model::{ConsoleModel, HeuristicPolicy, OperatingMode, StartupMode};
pub use infrared::CgbInfraredStatus;
pub(crate) use iohram::{BusIoReadView, BusIoWriteView, IoHramDomain};
pub use map::{
    BusAddressInfo, BusDomain, BusRegion, BusRegionOwner, IoRegisterAccess, IoRegisterAvailability,
    IoRegisterImplementation, IoRegisterInfo, IoRegisterKind, IoRegisterOwner, UnusableAreaInfo,
    UnusableAreaReadProfile, UnusableAreaWriteProfile,
};
pub use meta::BusSnapshot;
pub use router::AddressRouter;
pub use state::{
    BootRomBusState, BusAccessDisposition, BusAccessKind, BusAccessResolution, BusArbitrationState,
    BusBlockReason, BusMaster, BusRequester, BusStatus, DmaBusState, DmaCpuAccessPolicy,
    DmaMemoryRegionImpact,
};
pub(crate) use video::{OamDomain, VramDomain, VramSaveState};
pub(crate) use view::{OamBusView, VramBusView};
pub(crate) use wram::{WramDomain, WramSaveState};

const DMG_VRAM_LEN: usize = 0x2000;
const CGB_VRAM_LEN: usize = 0x4000;
const DMG_WRAM_LEN: usize = 0x2000;
const CGB_WRAM_LEN: usize = 0x8000;
#[cfg(test)]
const VRAM_LEN: usize = DMG_VRAM_LEN;
const OAM_LEN: usize = 0x00A0;
const HRAM_LEN: usize = 0x007F;

const BLOCKED_READ_VALUE: u8 = 0xFF;
const DMG_UNUSABLE_READ_VALUE: u8 = 0x00;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DebugWramAddressSample {
    pub address: u16,
    pub bank: u8,
    pub bank_offset: u16,
    pub value: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Bus {
    console_model: ConsoleModel,
    operating_mode: OperatingMode,
    status: BusStatus,
    router: AddressRouter,
    vram: VramDomain,
    wram: WramDomain,
    oam: OamDomain,
    iohram: IoHramDomain,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BusSaveState {
    console_model: ConsoleModel,
    operating_mode: OperatingMode,
    status: BusStatus,
    router: AddressRouter,
    vram: VramSaveState,
    wram: WramSaveState,
    oam: OamDomain,
    iohram: IoHramDomain,
}

impl BusSaveState {
    pub(crate) fn dynamic_payload_bytes(&self) -> usize {
        self.vram
            .dynamic_payload_bytes()
            .saturating_add(self.wram.dynamic_payload_bytes())
    }
}

impl Bus {
    pub fn new(console_model: ConsoleModel) -> Self {
        Self::new_with_operating_mode(console_model, console_model.default_operating_mode())
    }

    pub fn new_with_operating_mode(
        console_model: ConsoleModel,
        operating_mode: OperatingMode,
    ) -> Self {
        Self {
            console_model,
            operating_mode,
            status: BusStatus::Ready,
            router: AddressRouter::new(),
            vram: VramDomain::new_for_model(console_model),
            wram: WramDomain::new_for_model(console_model),
            oam: OamDomain::new(),
            iohram: IoHramDomain::new(),
        }
    }

    pub fn console_model(&self) -> ConsoleModel {
        self.console_model
    }

    pub fn operating_mode(&self) -> OperatingMode {
        self.operating_mode
    }

    pub(crate) fn apply_operating_mode_state(&mut self, operating_mode: OperatingMode) {
        self.operating_mode = operating_mode;
    }

    pub fn cgb_extensions_enabled(&self) -> bool {
        self.console_model.is_cgb_family() && self.operating_mode.enables_cgb_extensions()
    }

    pub fn cgb_infrared_register_enabled(&self) -> bool {
        self.console_model.is_cgb_family() && self.operating_mode.enables_cgb_infrared_register()
    }

    pub(crate) fn io_register_info_is_live(&self, info: IoRegisterInfo) -> bool {
        iohram::io_register_kind_is_available(
            info.kind(),
            info.availability(),
            self.console_model,
            self.operating_mode,
        )
    }

    pub fn status(&self) -> BusStatus {
        self.status
    }

    pub(crate) fn capture_save_state(&self) -> BusSaveState {
        BusSaveState {
            console_model: self.console_model,
            operating_mode: self.operating_mode,
            status: self.status,
            router: self.router,
            vram: self.vram.capture_save_state(),
            wram: self.wram.capture_save_state(),
            oam: self.oam.clone(),
            iohram: self.iohram.clone(),
        }
    }

    pub(crate) fn restore_save_state(&mut self, state: &BusSaveState) {
        self.console_model = state.console_model;
        self.operating_mode = state.operating_mode;
        self.status = state.status;
        self.router = state.router;
        self.vram.restore_save_state(&state.vram);
        self.wram.restore_save_state(&state.wram);
        self.oam = state.oam.clone();
        self.iohram = state.iohram.clone();
    }

    /// Returns the static DMG memory-map classification for `address`.
    ///
    /// This is an address-only decode surface. It does not apply boot ROM
    /// overlay windows or any other live arbitration state.
    pub fn decode_address(&self, address: u16) -> BusAddressInfo {
        self.router.decode_address(address)
    }

    pub fn describe_io_register(&self, address: u16) -> Option<IoRegisterInfo> {
        self.router.describe_io_register(address)
    }

    pub fn describe_unusable_area(&self, address: u16) -> Option<UnusableAreaInfo> {
        self.router
            .describe_unusable_area(self.console_model, address)
    }

    /// Returns the raw VRAM backing bytes for deterministic debug probes.
    ///
    /// This is intentionally not a CPU bus read: it bypasses live PPU/DMA arbitration so external tooling can compare emulator state without perturbing the machine or conflating blocked CPU visibility with actual storage.
    pub fn debug_vram_bytes(&self) -> &[u8] {
        self.vram.debug_bytes()
    }

    /// Returns the raw OAM backing bytes for deterministic debug probes.
    ///
    /// This is intentionally not a CPU bus read: it bypasses live PPU/DMA arbitration so external tooling can compare emulator state without perturbing the machine or conflating blocked CPU visibility with actual storage.
    pub fn debug_oam_bytes(&self) -> &[u8] {
        self.oam.bytes()
    }

    /// Returns the raw WRAM backing bytes for deterministic debug probes.
    ///
    /// This is intentionally not a CPU bus read: it bypasses echo routing and arbitration side effects so external tooling can compare storage state directly.
    pub fn debug_wram_bytes(&self) -> &[u8] {
        self.wram.debug_bytes()
    }

    /// Returns the current CPU-visible WRAM value for a WRAM or echo-RAM address without performing a bus read.
    ///
    /// This is intended for deterministic debug probes that need to annotate traces with live WRAM state while preserving the current bus side effects, arbitration state, and trace timing.
    pub fn debug_wram_address_sample(&self, address: u16) -> Option<DebugWramAddressSample> {
        self.wram.debug_address_sample(address)
    }

    pub(crate) fn apply_cgb_startup_state(
        &mut self,
        startup_mode: StartupMode,
        header: Option<&CartridgeHeader>,
    ) {
        self.vram.reset_bank_select();
        self.wram.reset_bank_select();
        self.iohram.reset_cgb_misc_io();

        if startup_mode.uses_direct_boot_state() {
            let cgb_flag = header.map_or(CgbFlag::Supported, |header| header.cgb_flag);
            self.iohram
                .apply_direct_boot_key0(self.console_model, self.operating_mode, cgb_flag);
        } else {
            self.iohram
                .reset_real_boot_key0(self.console_model, self.operating_mode);
        }
    }

    pub(crate) fn lock_cgb_real_boot_key0_at_handoff(
        &mut self,
        heuristic_policy: HeuristicPolicy,
    ) -> Option<OperatingMode> {
        if !self.console_model.is_cgb_family() {
            return None;
        }

        let operating_mode = self
            .iohram
            .lock_cgb_real_boot_key0_at_handoff(self.console_model, heuristic_policy);
        self.operating_mode = operating_mode;
        Some(operating_mode)
    }

    /// Returns the raw HRAM backing bytes for deterministic debug probes.
    ///
    /// This excludes MMIO registers and the interrupt-enable register; those are captured through subsystem snapshots or non-perturbing cloned reads by tooling that needs CPU-visible values.
    pub fn debug_hram_bytes(&self) -> &[u8] {
        self.iohram.hram_bytes()
    }

    pub(crate) fn tick_cgb_infrared_t_cycle(&mut self) {
        if self.cgb_infrared_register_enabled() {
            self.iohram.tick_cgb_infrared_t_cycle();
        }
    }

    pub(crate) fn set_cgb_infrared_external_input(&mut self, active: bool) {
        let active = self.cgb_infrared_register_enabled() && active;
        self.iohram.set_cgb_infrared_external_input(active);
    }

    pub(crate) fn cgb_infrared_emitter_on(&self) -> bool {
        self.cgb_infrared_register_enabled() && self.iohram.cgb_infrared_emitter_on()
    }

    pub fn cgb_infrared_status(&self) -> Option<CgbInfraredStatus> {
        self.cgb_infrared_register_enabled()
            .then(|| self.iohram.cgb_infrared_status())
    }

    #[cfg(test)]
    pub(crate) fn cgb_infrared_effective_signal_detected(&self) -> bool {
        self.cgb_infrared_register_enabled() && self.iohram.cgb_infrared_effective_signal_detected()
    }
}

#[cfg(test)]
mod tests;
