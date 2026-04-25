use std::fmt;

use crate::apu::Apu;
use crate::boot::{BootController, BootRomKind};
use crate::bus::Bus;
use crate::cartridge::{CartridgeSlot, CartridgeSlotState};
use crate::cpu::CpuCore;
use crate::dma::DmaController;
use crate::external_port::ExternalPort;
use crate::interrupts::InterruptController;
use crate::joypad::Joypad;
use crate::machine::PendingExternalEvents;
use crate::model::{CompatibilityPolicy, ConsoleModel, HostPlatform, OperatingMode, StartupMode};
use crate::ppu::Ppu;
use crate::scheduler::TCycle;
use crate::serial::Serial;
use crate::timer::Timer;

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
    pub(crate) pending_external_events: PendingExternalEvents,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CpuSaveState {
    pub(crate) core: CpuCore,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BusSaveState {
    pub(crate) bus: Bus,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ApuSaveState {
    pub(crate) apu: Apu,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PpuSaveState {
    pub(crate) ppu: Ppu,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DmaSaveState {
    pub(crate) dma: DmaController,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TimerSaveState {
    pub(crate) timer: Timer,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SerialSaveState {
    pub(crate) serial: Serial,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ExternalPortSaveState {
    pub(crate) external_port: ExternalPort,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BootSaveState {
    pub(crate) boot: BootController,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InterruptSaveState {
    pub(crate) interrupts: InterruptController,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JoypadSaveState {
    pub(crate) joypad: Joypad,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CartridgeRuntimeSaveState {
    pub(crate) cartridge: CartridgeSlot,
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
        let metadata = MachineSaveStateMetadata {
            console_model: ConsoleModel::Dmg,
            operating_mode: OperatingMode::Dmg,
            host_platform: HostPlatform::Handheld,
            startup_mode: StartupMode::SkipBoot,
            compatibility: CompatibilityPolicy::strict(),
            next_t_cycle: TCycle::new(42),
            cartridge: cartridge_metadata(),
            boot: boot_metadata(),
        };
        let state = MachineSaveState::new(
            metadata.clone(),
            MachineCoreSaveState {
                scheduler: SchedulerSaveState {
                    next_t_cycle: TCycle::new(42),
                },
                machine: MachineRuntimeSaveState {
                    pending_external_events: PendingExternalEvents::default(),
                },
                cpu: CpuSaveState {
                    core: CpuCore::new(ConsoleModel::Dmg),
                },
                bus: BusSaveState {
                    bus: Bus::new(ConsoleModel::Dmg),
                },
                apu: ApuSaveState {
                    apu: Apu::new(ConsoleModel::Dmg),
                },
                ppu: PpuSaveState {
                    ppu: Ppu::new(ConsoleModel::Dmg),
                },
                dma: DmaSaveState {
                    dma: DmaController::new(ConsoleModel::Dmg),
                },
                timer: TimerSaveState {
                    timer: Timer::new(ConsoleModel::Dmg),
                },
                serial: SerialSaveState {
                    serial: Serial::new(ConsoleModel::Dmg),
                },
                external_port: ExternalPortSaveState {
                    external_port: ExternalPort::new(),
                },
                boot: BootSaveState {
                    boot: BootController::new(
                        ConsoleModel::Dmg,
                        StartupMode::SkipBoot,
                        crate::boot::BootRomAssets::none(),
                    ),
                },
                interrupts: InterruptSaveState {
                    interrupts: InterruptController::new(ConsoleModel::Dmg),
                },
                joypad: JoypadSaveState {
                    joypad: Joypad::new(ConsoleModel::Dmg),
                },
                cartridge: CartridgeRuntimeSaveState {
                    cartridge: CartridgeSlot::empty(),
                },
            },
        );

        assert_eq!(state.metadata(), &metadata);
        assert_eq!(state.core().scheduler.next_t_cycle, TCycle::new(42));
    }
}
