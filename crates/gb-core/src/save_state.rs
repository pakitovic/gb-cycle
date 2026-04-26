use std::fmt;

pub use crate::apu::ApuSaveState;
use crate::boot::BootRomKind;
pub use crate::boot::BootSaveState;
pub use crate::bus::BusSaveState;
pub use crate::cartridge::CartridgeRuntimeSaveState;
use crate::cartridge::CartridgeSlotState;
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

    pub(crate) const fn core(&self) -> &MachineCoreSaveState {
        &self.core
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
        }
    }
}

impl std::error::Error for MachineSaveStateRestoreError {}

#[cfg(test)]
mod tests {
    use super::*;

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
        ];

        for forbidden in forbidden_runtime_root_fields {
            assert!(
                !sources.contains(forbidden),
                "save-state DTOs must not wrap runtime root field `{forbidden}`"
            );
        }
    }
}
