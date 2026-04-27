use std::{fmt, mem};

pub use crate::apu::ApuSaveState;
use crate::boot::BootRomKind;
pub use crate::boot::BootSaveState;
pub use crate::bus::BusSaveState;
use crate::cartridge::CartridgeSlotState;
pub use crate::cartridge::{CartridgeRuntimeSaveState, CartridgeRuntimeSaveStateError};
pub use crate::cpu::CpuSaveState;
pub use crate::dma::DmaSaveState;
pub use crate::external_port::ExternalPortSaveState;
pub use crate::interrupts::InterruptSaveState;
pub use crate::joypad::JoypadSaveState;
use crate::model::{CompatibilityPolicy, ConsoleModel, HostPlatform, OperatingMode, StartupMode};
pub use crate::ppu::PpuSaveState;
use crate::scheduler::TCycle;
pub use crate::serial::SerialSaveState;
pub use crate::timer::TimerSaveState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SaveStateByteFingerprint {
    pub len: u64,
    pub fnv1a64: u64,
}

impl SaveStateByteFingerprint {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
        const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

        let mut hash = FNV_OFFSET_BASIS;
        for &byte in bytes {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }

        Self {
            len: bytes.len() as u64,
            fnv1a64: hash,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MachineCartridgeSaveStateMetadata {
    pub state: CartridgeSlotState,
    pub rom_fingerprint: Option<SaveStateByteFingerprint>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MachineBootSaveStateMetadata {
    pub startup_mode: StartupMode,
    pub boot_rom_kind: BootRomKind,
    pub boot_rom_mapped: bool,
    pub boot_rom_fingerprint: Option<SaveStateByteFingerprint>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MachineSaveStateMetadata {
    pub console_model: ConsoleModel,
    pub operating_mode: OperatingMode,
    pub host_platform: HostPlatform,
    pub startup_mode: StartupMode,
    pub compatibility: CompatibilityPolicy,
    pub next_t_cycle: TCycle,
    pub cartridge: MachineCartridgeSaveStateMetadata,
    pub boot: MachineBootSaveStateMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SchedulerSaveState {
    pub next_t_cycle: TCycle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct MachineRuntimeSaveState {
    pub(crate) joypad_pressed_mask: u8,
    pub(crate) joypad_state_dirty: bool,
    pub(crate) external_serial_clock_pulses_pending: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct MachineCoreSaveState {
    pub(crate) scheduler: SchedulerSaveState,
    pub(crate) machine: MachineRuntimeSaveState,
    pub(crate) cpu: CpuSaveState,
    pub(crate) bus: BusSaveState,
    pub(crate) apu: ApuSaveState,
    pub(crate) ppu: PpuSaveState,
    pub(crate) dma: DmaSaveState,
    pub(crate) timer: TimerSaveState,
    pub(crate) serial: SerialSaveState,
    pub(crate) external_port: ExternalPortSaveState,
    pub(crate) boot: BootSaveState,
    pub(crate) interrupts: InterruptSaveState,
    pub(crate) joypad: JoypadSaveState,
    pub(crate) cartridge: CartridgeRuntimeSaveState,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MachineSaveState {
    metadata: MachineSaveStateMetadata,
    core: MachineCoreSaveState,
}

impl MachineSaveState {
    pub(crate) const fn new(
        metadata: MachineSaveStateMetadata,
        core: MachineCoreSaveState,
    ) -> Self {
        Self { metadata, core }
    }

    pub const fn metadata(&self) -> &MachineSaveStateMetadata {
        &self.metadata
    }

    /// Returns the rewind memory-accounting size for this snapshot.
    ///
    /// This is a deterministic deep-size of the save-state DTO payload: inline
    /// struct storage plus the bytes owned by dynamic containers inside the
    /// snapshot. It intentionally excludes allocator/bookkeeping overhead and
    /// host process RSS so frontends can use it as a stable budget signal.
    pub fn deep_size_bytes(&self) -> usize {
        mem::size_of_val(self).saturating_add(self.core.dynamic_payload_bytes())
    }

    pub(crate) const fn core(&self) -> &MachineCoreSaveState {
        &self.core
    }
}

impl MachineCoreSaveState {
    fn dynamic_payload_bytes(&self) -> usize {
        self.bus
            .dynamic_payload_bytes()
            .saturating_add(self.cpu.dynamic_payload_bytes())
            .saturating_add(self.apu.dynamic_payload_bytes())
            .saturating_add(self.ppu.dynamic_payload_bytes())
            .saturating_add(self.dma.dynamic_payload_bytes())
            .saturating_add(self.timer.dynamic_payload_bytes())
            .saturating_add(self.serial.dynamic_payload_bytes())
            .saturating_add(self.external_port.dynamic_payload_bytes())
            .saturating_add(self.boot.dynamic_payload_bytes())
            .saturating_add(self.interrupts.dynamic_payload_bytes())
            .saturating_add(self.joypad.dynamic_payload_bytes())
            .saturating_add(self.cartridge.dynamic_payload_bytes())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MachineSaveStateRestoreError {
    ConsoleModelMismatch {
        expected: ConsoleModel,
        actual: ConsoleModel,
    },
    OperatingModeMismatch {
        expected: OperatingMode,
        actual: OperatingMode,
    },
    HostPlatformMismatch {
        expected: HostPlatform,
        actual: HostPlatform,
    },
    StartupModeMismatch {
        expected: StartupMode,
        actual: StartupMode,
    },
    CompatibilityMismatch,
    CartridgeMismatch {
        expected: MachineCartridgeSaveStateMetadata,
        actual: MachineCartridgeSaveStateMetadata,
    },
    BootRomMismatch {
        expected: MachineBootSaveStateMetadata,
        actual: MachineBootSaveStateMetadata,
    },
    CartridgeRuntime(CartridgeRuntimeSaveStateError),
}

impl fmt::Display for MachineSaveStateRestoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConsoleModelMismatch { expected, actual } => write!(
                f,
                "save-state console model mismatch: expected {:?}, got {:?}",
                expected, actual
            ),
            Self::OperatingModeMismatch { expected, actual } => write!(
                f,
                "save-state operating mode mismatch: expected {:?}, got {:?}",
                expected, actual
            ),
            Self::HostPlatformMismatch { expected, actual } => write!(
                f,
                "save-state host platform mismatch: expected {:?}, got {:?}",
                expected, actual
            ),
            Self::StartupModeMismatch { expected, actual } => write!(
                f,
                "save-state startup mode mismatch: expected {:?}, got {:?}",
                expected, actual
            ),
            Self::CompatibilityMismatch => f.write_str("save-state compatibility policy mismatch"),
            Self::CartridgeMismatch { expected, actual } => write!(
                f,
                "save-state cartridge mismatch: expected {:?}, got {:?}",
                expected, actual
            ),
            Self::BootRomMismatch { expected, actual } => write!(
                f,
                "save-state boot ROM mismatch: expected {:?}, got {:?}",
                expected, actual
            ),
            Self::CartridgeRuntime(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for MachineSaveStateRestoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CartridgeRuntime(error) => Some(error),
            _ => None,
        }
    }
}

impl From<CartridgeRuntimeSaveStateError> for MachineSaveStateRestoreError {
    fn from(error: CartridgeRuntimeSaveStateError) -> Self {
        Self::CartridgeRuntime(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;

    fn build_banked_test_rom(cartridge_type: u8, rom_size: u8, ram_size: u8) -> Vec<u8> {
        let rom_len = match rom_size {
            0x00 => 32 * 1024,
            0x01 => 64 * 1024,
            0x02 => 128 * 1024,
            0x03 => 256 * 1024,
            0x04 => 512 * 1024,
            0x05 => 1024 * 1024,
            _ => 32 * 1024,
        };
        let mut rom = vec![0xFF; HEADER_MINIMUM_ROM_LEN.max(rom_len)];
        rom[0x0100] = 0x00;
        rom[0x0147] = cartridge_type;
        rom[0x0148] = rom_size;
        rom[0x0149] = ram_size;
        rom
    }

    fn machine_with_cartridge(cartridge_type: u8, rom_size: u8, ram_size: u8) -> crate::Machine {
        let mut machine = crate::Machine::new(
            crate::MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
        );
        machine
            .load_cartridge(build_banked_test_rom(cartridge_type, rom_size, ram_size))
            .expect("test ROM should load");
        machine.step_t_cycle();
        machine
    }

    fn cartridge_metadata() -> MachineCartridgeSaveStateMetadata {
        MachineCartridgeSaveStateMetadata {
            state: CartridgeSlotState::Empty,
            rom_fingerprint: None,
        }
    }

    fn boot_metadata() -> MachineBootSaveStateMetadata {
        MachineBootSaveStateMetadata {
            startup_mode: StartupMode::SkipBoot,
            boot_rom_kind: BootRomKind::Dmg,
            boot_rom_mapped: false,
            boot_rom_fingerprint: None,
        }
    }

    #[test]
    fn byte_fingerprint_tracks_length_and_stable_fnv1a64_hash() {
        let fingerprint = SaveStateByteFingerprint::from_bytes(b"gb-cycle");

        assert_eq!(fingerprint.len, 8);
        assert_eq!(fingerprint.fnv1a64, 0x9a41_fbc6_7267_2fdb);
    }

    #[test]
    fn restore_error_messages_cover_each_validation_axis() {
        let expected_cartridge = MachineCartridgeSaveStateMetadata {
            state: CartridgeSlotState::Mbc1,
            rom_fingerprint: Some(SaveStateByteFingerprint {
                len: 2,
                fnv1a64: 0x1234,
            }),
        };
        let actual_cartridge = cartridge_metadata();
        let expected_boot = MachineBootSaveStateMetadata {
            startup_mode: StartupMode::RealBoot,
            boot_rom_kind: BootRomKind::Mgb,
            boot_rom_mapped: true,
            boot_rom_fingerprint: Some(SaveStateByteFingerprint {
                len: 0x100,
                fnv1a64: 0x5678,
            }),
        };
        let actual_boot = boot_metadata();

        let errors = [
            (
                MachineSaveStateRestoreError::ConsoleModelMismatch {
                    expected: ConsoleModel::Dmg,
                    actual: ConsoleModel::Mgb,
                },
                "console model mismatch",
            ),
            (
                MachineSaveStateRestoreError::OperatingModeMismatch {
                    expected: OperatingMode::Dmg,
                    actual: OperatingMode::CgbCompatibility,
                },
                "operating mode mismatch",
            ),
            (
                MachineSaveStateRestoreError::HostPlatformMismatch {
                    expected: HostPlatform::Handheld,
                    actual: HostPlatform::Sgb1,
                },
                "host platform mismatch",
            ),
            (
                MachineSaveStateRestoreError::StartupModeMismatch {
                    expected: StartupMode::SkipBoot,
                    actual: StartupMode::RealBoot,
                },
                "startup mode mismatch",
            ),
            (
                MachineSaveStateRestoreError::CompatibilityMismatch,
                "compatibility policy mismatch",
            ),
            (
                MachineSaveStateRestoreError::CartridgeMismatch {
                    expected: expected_cartridge,
                    actual: actual_cartridge,
                },
                "cartridge mismatch",
            ),
            (
                MachineSaveStateRestoreError::BootRomMismatch {
                    expected: expected_boot,
                    actual: actual_boot,
                },
                "boot ROM mismatch",
            ),
        ];

        for (error, expected_message) in errors {
            assert!(error.to_string().contains(expected_message));
            assert!(std::error::Error::source(&error).is_none());
        }

        let cartridge_runtime_error = MachineSaveStateRestoreError::CartridgeRuntime(
            CartridgeRuntimeSaveStateError::SlotStateMismatch {
                expected: CartridgeSlotState::Mbc1,
                actual: CartridgeSlotState::Mbc5,
            },
        );
        assert!(
            cartridge_runtime_error
                .to_string()
                .contains("cartridge runtime state mismatch")
        );
        assert!(std::error::Error::source(&cartridge_runtime_error).is_some());
    }

    #[test]
    fn machine_save_state_metadata_accessor_returns_the_core_contract() {
        let state = crate::Machine::new_summary(
            crate::MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
        )
        .capture_save_state();
        let metadata = state.metadata().clone();

        assert_eq!(state.metadata(), &metadata);
        assert_eq!(state.core().scheduler.next_t_cycle, metadata.next_t_cycle);
    }

    #[test]
    fn restore_prevalidates_cartridge_mapper_payload_before_mutating() {
        let mut target = machine_with_cartridge(0x1B, 0x04, 0x04);
        let before = target.capture_save_state();
        let source = machine_with_cartridge(0x03, 0x03, 0x03);
        let mut corrupt = target.capture_save_state();
        corrupt.core.cartridge = source.capture_save_state().core.cartridge;

        let error = target
            .restore_save_state(&corrupt)
            .expect_err("corrupt cartridge DTO must be rejected before restore");

        assert!(matches!(
            error,
            MachineSaveStateRestoreError::CartridgeRuntime(
                CartridgeRuntimeSaveStateError::SlotStateMismatch {
                    expected: CartridgeSlotState::Mbc5,
                    actual: CartridgeSlotState::Mbc1,
                }
            )
        ));
        assert_eq!(target.capture_save_state(), before);
    }

    #[test]
    fn restore_prevalidates_cartridge_ram_shape_before_mutating() {
        let mut target = machine_with_cartridge(0x1B, 0x04, 0x04);
        let before = target.capture_save_state();
        let source = machine_with_cartridge(0x1B, 0x04, 0x02);
        let mut corrupt = target.capture_save_state();
        corrupt.core.cartridge = source.capture_save_state().core.cartridge;

        let error = target
            .restore_save_state(&corrupt)
            .expect_err("corrupt cartridge RAM payload must be rejected before restore");

        assert!(matches!(
            error,
            MachineSaveStateRestoreError::CartridgeRuntime(
                CartridgeRuntimeSaveStateError::RamShapeMismatch {
                    field: "MBC5 RAM",
                    expected,
                    actual,
                }
            ) if expected == Some(128 * 1024) && actual == Some(8 * 1024)
        ));
        assert_eq!(target.capture_save_state(), before);
    }

    #[test]
    fn subsystem_save_state_contracts_do_not_wrap_runtime_roots() {
        let sources = [
            include_str!("save_state.rs"),
            include_str!("cpu.rs"),
            include_str!("bus.rs"),
            include_str!("apu.rs"),
            include_str!("ppu.rs"),
            include_str!("dma.rs"),
            include_str!("timer.rs"),
            include_str!("serial.rs"),
            include_str!("external_port.rs"),
            include_str!("boot.rs"),
            include_str!("interrupts.rs"),
            include_str!("joypad.rs"),
            include_str!("cartridge.rs"),
        ]
        .join("\n");
        let forbidden_runtime_root_fields = [
            concat!("pub struct CpuSaveState {\n    core", ": ", "CpuCore"),
            concat!("pub struct BusSaveState {\n    bus", ": ", "Bus"),
            concat!("pub struct ApuSaveState {\n    apu", ": ", "Apu"),
            concat!("pub struct PpuSaveState {\n    ppu", ": ", "Ppu"),
            concat!("pub struct DmaSaveState {\n    dma", ": ", "DmaController"),
            concat!("pub struct TimerSaveState {\n    timer", ": ", "Timer"),
            concat!("pub struct SerialSaveState {\n    serial", ": ", "Serial"),
            concat!(
                "pub struct ExternalPortSaveState {\n    external_port",
                ": ",
                "ExternalPort"
            ),
            concat!(
                "pub struct BootSaveState {\n    boot",
                ": ",
                "BootController"
            ),
            concat!(
                "pub struct InterruptSaveState {\n    interrupts",
                ": ",
                "InterruptController"
            ),
            concat!("pub struct JoypadSaveState {\n    joypad", ": ", "Joypad"),
            concat!(
                "pub struct CartridgeRuntimeSaveState {\n    cartridge",
                ": ",
                "CartridgeSlot"
            ),
            concat!("struct NoMbcCartridgeSaveState {\n    rom", ": ", "Vec<u8>"),
            concat!("struct Mmm01CartridgeSaveState {\n    rom", ": ", "Vec<u8>"),
            concat!("struct M161CartridgeSaveState {\n    rom", ": ", "Vec<u8>"),
            concat!("struct Huc1CartridgeSaveState {\n    rom", ": ", "Vec<u8>"),
            concat!("struct Huc3CartridgeSaveState {\n    rom", ": ", "Vec<u8>"),
            concat!("struct Mbc1CartridgeSaveState {\n    rom", ": ", "Vec<u8>"),
            concat!("struct Mbc2CartridgeSaveState {\n    rom", ": ", "Vec<u8>"),
            concat!("struct Mbc3CartridgeSaveState {\n    rom", ": ", "Vec<u8>"),
            concat!("struct Mbc5CartridgeSaveState {\n    rom", ": ", "Vec<u8>"),
            concat!(
                "struct PocketCameraCartridgeSaveState {\n    rom",
                ": ",
                "Vec<u8>"
            ),
        ];

        for forbidden in forbidden_runtime_root_fields {
            assert!(
                !sources.contains(forbidden),
                "save-state DTOs must not wrap runtime root field `{forbidden}`"
            );
        }
    }
}
