use crate::apu::{ApuStartupState, WaveRamStartupPolicy, div_apu_phase_from_system_counter};
use crate::bus::BootRomBusState;
use crate::cartridge::{CartridgeHeader, CartridgeSlot};
use crate::cpu::CpuStartupState;
use crate::dma::DmaStartupState;
use crate::interrupts::InterruptStartupState;
use crate::joypad::JoypadStartupState;
use crate::model::{ConsoleModel, StartupMode};
use crate::ppu::PpuStartupState;
use crate::save_state::SaveStateByteFingerprint;
use crate::scheduler::CycleContext;
use crate::serial::SerialStartupState;
use crate::timer::TimerStartupState;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const DMG_FAMILY_BOOT_ROM_LEN: usize = 0x0100;
const CGB_BOOT_ROM_RAW_LEN: usize = 0x0800;
const CGB_BOOT_ROM_MAPPED_LEN: usize = 0x0900;
const CGB_BOOT_ROM_UPPER_WINDOW_START: usize = 0x0200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum BootRomKind {
    Dmg0,
    Dmg,
    Mgb,
    Cgb0,
    Cgb,
    CgbE,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct BootRomAssets {
    dmg0: Option<Vec<u8>>,
    dmg: Option<Vec<u8>>,
    mgb: Option<Vec<u8>>,
    cgb0: Option<Vec<u8>>,
    cgb: Option<Vec<u8>>,
    cgb_e: Option<Vec<u8>>,
}

#[derive(Debug)]
pub enum BootRomAssetError {
    DirectoryNotFound {
        path: PathBuf,
    },
    NotADirectory {
        path: PathBuf,
    },
    ReadFailed {
        path: PathBuf,
        source: io::Error,
    },
    ImageTooShort {
        kind: BootRomKind,
        path: PathBuf,
        expected_at_least: usize,
        actual: usize,
    },
}

impl fmt::Display for BootRomAssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DirectoryNotFound { path } => {
                write!(
                    f,
                    "boot ROM asset directory does not exist: {}",
                    path.display()
                )
            }
            Self::NotADirectory { path } => {
                write!(
                    f,
                    "boot ROM asset path is not a directory: {}",
                    path.display()
                )
            }
            Self::ReadFailed { path, .. } => {
                write!(f, "failed to read boot ROM asset: {}", path.display())
            }
            Self::ImageTooShort {
                kind,
                path,
                expected_at_least,
                actual,
            } => write!(
                f,
                "boot ROM asset {:?} at {} is too short: expected at least {} bytes, got {}",
                kind,
                path.display(),
                expected_at_least,
                actual
            ),
        }
    }
}

impl std::error::Error for BootRomAssetError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ReadFailed { source, .. } => Some(source),
            Self::DirectoryNotFound { .. }
            | Self::NotADirectory { .. }
            | Self::ImageTooShort { .. } => None,
        }
    }
}

impl BootRomAssets {
    pub const fn none() -> Self {
        Self {
            dmg0: None,
            dmg: None,
            mgb: None,
            cgb0: None,
            cgb: None,
            cgb_e: None,
        }
    }

    pub fn from_directory(path: impl AsRef<Path>) -> Result<Self, BootRomAssetError> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(BootRomAssetError::DirectoryNotFound {
                path: path.to_path_buf(),
            });
        }
        if !path.is_dir() {
            return Err(BootRomAssetError::NotADirectory {
                path: path.to_path_buf(),
            });
        }

        Ok(Self {
            dmg0: read_boot_rom_file(path, BootRomKind::Dmg0)?,
            dmg: read_boot_rom_file(path, BootRomKind::Dmg)?,
            mgb: read_boot_rom_file(path, BootRomKind::Mgb)?,
            cgb0: read_boot_rom_file(path, BootRomKind::Cgb0)?,
            cgb: read_boot_rom_file(path, BootRomKind::Cgb)?,
            cgb_e: read_boot_rom_file(path, BootRomKind::CgbE)?,
        })
    }

    pub fn with_bytes(
        mut self,
        kind: BootRomKind,
        bytes: Vec<u8>,
    ) -> Result<Self, BootRomAssetError> {
        self.insert_bytes(kind, bytes)?;
        Ok(self)
    }

    pub fn insert_bytes(
        &mut self,
        kind: BootRomKind,
        bytes: Vec<u8>,
    ) -> Result<(), BootRomAssetError> {
        validate_boot_rom_len(kind, &bytes, Path::new(Self::filename(kind)))?;
        *self.bytes_slot_mut(kind) = Some(bytes);
        Ok(())
    }

    pub const fn filename(kind: BootRomKind) -> &'static str {
        match kind {
            BootRomKind::Dmg0 => "dmg0_boot.bin",
            BootRomKind::Dmg => "dmg_boot.bin",
            BootRomKind::Mgb => "mgb_boot.bin",
            BootRomKind::Cgb0 => "cgb0_boot.bin",
            BootRomKind::Cgb => "cgb_boot.bin",
            BootRomKind::CgbE => "cgbE_boot.bin",
        }
    }

    pub fn has_image(&self, kind: BootRomKind) -> bool {
        self.bytes_for(kind).is_some()
    }

    pub fn is_empty(&self) -> bool {
        !self.has_image(BootRomKind::Dmg0)
            && !self.has_image(BootRomKind::Dmg)
            && !self.has_image(BootRomKind::Mgb)
            && !self.has_image(BootRomKind::Cgb0)
            && !self.has_image(BootRomKind::Cgb)
            && !self.has_image(BootRomKind::CgbE)
    }

    pub fn read_byte(&self, kind: BootRomKind, address: u16) -> Option<u8> {
        let bytes = self.bytes_for(kind)?;

        match kind {
            BootRomKind::Dmg0 | BootRomKind::Dmg | BootRomKind::Mgb => {
                bytes.get(address as usize).copied()
            }
            BootRomKind::Cgb0 | BootRomKind::Cgb | BootRomKind::CgbE => {
                read_cgb_boot_rom_byte(bytes, address)
            }
        }
    }

    pub fn fingerprint(&self, kind: BootRomKind) -> Option<SaveStateByteFingerprint> {
        self.bytes_for(kind)
            .map(SaveStateByteFingerprint::from_bytes)
    }

    pub(crate) fn dynamic_payload_bytes(&self) -> usize {
        self.dmg0.as_ref().map(Vec::len).unwrap_or(0)
            + self.dmg.as_ref().map(Vec::len).unwrap_or(0)
            + self.mgb.as_ref().map(Vec::len).unwrap_or(0)
            + self.cgb0.as_ref().map(Vec::len).unwrap_or(0)
            + self.cgb.as_ref().map(Vec::len).unwrap_or(0)
            + self.cgb_e.as_ref().map(Vec::len).unwrap_or(0)
    }

    fn bytes_for(&self, kind: BootRomKind) -> Option<&[u8]> {
        match kind {
            BootRomKind::Dmg0 => self.dmg0.as_deref(),
            BootRomKind::Dmg => self.dmg.as_deref(),
            BootRomKind::Mgb => self.mgb.as_deref(),
            BootRomKind::Cgb0 => self.cgb0.as_deref(),
            BootRomKind::Cgb => self.cgb.as_deref(),
            BootRomKind::CgbE => self.cgb_e.as_deref(),
        }
    }

    fn bytes_slot_mut(&mut self, kind: BootRomKind) -> &mut Option<Vec<u8>> {
        match kind {
            BootRomKind::Dmg0 => &mut self.dmg0,
            BootRomKind::Dmg => &mut self.dmg,
            BootRomKind::Mgb => &mut self.mgb,
            BootRomKind::Cgb0 => &mut self.cgb0,
            BootRomKind::Cgb => &mut self.cgb,
            BootRomKind::CgbE => &mut self.cgb_e,
        }
    }
}

fn read_cgb_boot_rom_byte(bytes: &[u8], address: u16) -> Option<u8> {
    let address = address as usize;

    match address {
        0x0000..=0x00FF => bytes.get(address).copied(),
        0x0200..=0x08FF => {
            let source_address = if bytes.len() >= CGB_BOOT_ROM_MAPPED_LEN {
                address
            } else {
                address - (CGB_BOOT_ROM_UPPER_WINDOW_START - DMG_FAMILY_BOOT_ROM_LEN)
            };
            bytes.get(source_address).copied()
        }
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum StartupMemoryPolicy {
    DeterministicPatterned,
    DmgBootLogoVram,
    CgbRealBootEntry,
    CgbRealBootEntryWithDmgBootLogoVram,
    CgbRealBootEntryWithDmgBootLogoTiles,
}

impl StartupMemoryPolicy {
    pub(crate) fn initialize_vram(self, bytes: &mut [u8]) {
        match self {
            Self::DeterministicPatterned => {}
            Self::DmgBootLogoVram => apply_dmg_boot_logo_vram(bytes),
            Self::CgbRealBootEntry => initialize_cgb_real_boot_entry_vram(bytes),
            Self::CgbRealBootEntryWithDmgBootLogoTiles => {
                initialize_cgb_real_boot_entry_vram(bytes);
                apply_dmg_boot_logo_tile_vram(bytes);
            }
            Self::CgbRealBootEntryWithDmgBootLogoVram => {
                initialize_cgb_real_boot_entry_vram(bytes);
                apply_dmg_boot_logo_vram(bytes);
            }
        }
    }

    pub(crate) fn initialize_wram(self, bytes: &mut [u8]) {
        match self {
            Self::DeterministicPatterned | Self::DmgBootLogoVram => self.fill_bytes(bytes, 0xC000),
            Self::CgbRealBootEntry
            | Self::CgbRealBootEntryWithDmgBootLogoVram
            | Self::CgbRealBootEntryWithDmgBootLogoTiles => bytes.fill(0),
        }
    }

    pub(crate) fn initialize_hram(self, bytes: &mut [u8]) {
        match self {
            Self::DeterministicPatterned | Self::DmgBootLogoVram => self.fill_bytes(bytes, 0xFF80),
            Self::CgbRealBootEntry
            | Self::CgbRealBootEntryWithDmgBootLogoVram
            | Self::CgbRealBootEntryWithDmgBootLogoTiles => {
                initialize_cgb_real_boot_entry_hram(bytes);
            }
        }
    }

    fn fill_bytes(self, bytes: &mut [u8], base_address: u16) {
        match self {
            Self::DeterministicPatterned | Self::DmgBootLogoVram => {
                fill_deterministic_startup_pattern(bytes, base_address)
            }
            Self::CgbRealBootEntry
            | Self::CgbRealBootEntryWithDmgBootLogoVram
            | Self::CgbRealBootEntryWithDmgBootLogoTiles => {}
        }
    }
}

pub const DMG_BOOT_LOGO_TILE_VRAM_START: u16 = 0x8010;
pub const DMG_BOOT_LOGO_MAP_VRAM_START: u16 = 0x9904;
pub const DMG_BOOT_LOGO_TILE_BYTES: [u8; 200] = [
    0xF0, 0xF0, 0xFC, 0xFC, 0xFC, 0xFC, 0xF3, 0xF3, 0x3C, 0x3C, 0x3C, 0x3C, 0x3C, 0x3C, 0x3C, 0x3C,
    0xF0, 0xF0, 0xF0, 0xF0, 0x00, 0x00, 0xF3, 0xF3, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xCF, 0xCF,
    0x00, 0x00, 0x0F, 0x0F, 0x3F, 0x3F, 0x0F, 0x0F, 0x00, 0x00, 0x00, 0x00, 0xC0, 0xC0, 0x0F, 0x0F,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xF0, 0xF0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xF3, 0xF3,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0xC0, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0xFF, 0xFF,
    0xC0, 0xC0, 0xC0, 0xC0, 0xC0, 0xC0, 0xC3, 0xC3, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFC, 0xFC,
    0xF3, 0xF3, 0xF0, 0xF0, 0xF0, 0xF0, 0xF0, 0xF0, 0x3C, 0x3C, 0xFC, 0xFC, 0xFC, 0xFC, 0x3C, 0x3C,
    0xF3, 0xF3, 0xF3, 0xF3, 0xF3, 0xF3, 0xF3, 0xF3, 0xF3, 0xF3, 0xC3, 0xC3, 0xC3, 0xC3, 0xC3, 0xC3,
    0xCF, 0xCF, 0xCF, 0xCF, 0xCF, 0xCF, 0xCF, 0xCF, 0x3C, 0x3C, 0x3F, 0x3F, 0x3C, 0x3C, 0x0F, 0x0F,
    0x3C, 0x3C, 0xFC, 0xFC, 0x00, 0x00, 0xFC, 0xFC, 0xFC, 0xFC, 0xF0, 0xF0, 0xF0, 0xF0, 0xF0, 0xF0,
    0xF3, 0xF3, 0xF3, 0xF3, 0xF3, 0xF3, 0xF0, 0xF0, 0xC3, 0xC3, 0xC3, 0xC3, 0xC3, 0xC3, 0xFF, 0xFF,
    0xCF, 0xCF, 0xCF, 0xCF, 0xCF, 0xCF, 0xC3, 0xC3, 0x0F, 0x0F, 0x0F, 0x0F, 0x0F, 0x0F, 0xFC, 0xFC,
    0x3C, 0x42, 0xB9, 0xA5, 0xB9, 0xA5, 0x42, 0x3C,
];
pub const DMG_BOOT_LOGO_MAP_BYTES: [u8; 44] = [
    0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x19, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18,
];

fn initialize_cgb_real_boot_entry_vram(bytes: &mut [u8]) {
    bytes.fill(0);
    let copy_len = CGB_REAL_BOOT_VRAM_PREFIX.len().min(bytes.len());
    bytes[..copy_len].copy_from_slice(&CGB_REAL_BOOT_VRAM_PREFIX[..copy_len]);
}

fn initialize_cgb_real_boot_entry_hram(bytes: &mut [u8]) {
    bytes.fill(0);
    let copy_len = CGB_BOOT_LOGO_HRAM_PREFIX.len().min(bytes.len());
    bytes[..copy_len].copy_from_slice(&CGB_BOOT_LOGO_HRAM_PREFIX[..copy_len]);
}

fn apply_dmg_boot_logo_vram(bytes: &mut [u8]) {
    apply_dmg_boot_logo_tile_vram(bytes);
    apply_dmg_boot_logo_tilemap_vram(bytes);
}

fn apply_dmg_boot_logo_tile_vram(bytes: &mut [u8]) {
    for (index, byte) in DMG_BOOT_LOGO_TILE_BYTES.iter().copied().enumerate() {
        write_vram_backing_byte(
            bytes,
            DMG_BOOT_LOGO_TILE_VRAM_START + (index as u16 * 2),
            byte,
        );
    }
}

fn apply_dmg_boot_logo_tilemap_vram(bytes: &mut [u8]) {
    for (index, byte) in DMG_BOOT_LOGO_MAP_BYTES.iter().copied().enumerate() {
        write_vram_backing_byte(bytes, DMG_BOOT_LOGO_MAP_VRAM_START + index as u16, byte);
    }
}

fn write_vram_backing_byte(bytes: &mut [u8], address: u16, value: u8) {
    if let Some(offset) = address.checked_sub(0x8000).map(usize::from)
        && let Some(byte) = bytes.get_mut(offset)
    {
        *byte = value;
    }
}

const CGB_BOOT_LOGO_HRAM_PREFIX: [u8; 48] = [
    0xCE, 0xED, 0x66, 0x66, 0xCC, 0x0D, 0x00, 0x0B, 0x03, 0x73, 0x00, 0x83, 0x00, 0x0C, 0x00, 0x0D,
    0x00, 0x08, 0x11, 0x1F, 0x88, 0x89, 0x00, 0x0E, 0xDC, 0xCC, 0x6E, 0xE6, 0xDD, 0xDD, 0xD9, 0x99,
    0xBB, 0xBB, 0x67, 0x63, 0x6E, 0x0E, 0xEC, 0xCC, 0xDD, 0xDC, 0x99, 0x9F, 0xBB, 0xB9, 0x33, 0x3E,
];

const CGB_REAL_BOOT_VRAM_PREFIX: [u8; 32] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0xF0, 0x00, 0xF0, 0x00, 0xFC, 0x00, 0xFC, 0x00, 0xFC, 0x00, 0xFC, 0x00, 0xF3, 0x00, 0xF3, 0x00,
];

fn fill_deterministic_startup_pattern(bytes: &mut [u8], base_address: u16) {
    for (offset, byte) in bytes.iter_mut().enumerate() {
        let address = base_address.wrapping_add(offset as u16);
        *byte = deterministic_startup_byte(address);
    }
}

const fn deterministic_startup_byte(address: u16) -> u8 {
    let low = address as u8;
    let high = (address >> 8) as u8;
    let mixed = low.wrapping_mul(0x3D) ^ high.wrapping_mul(0xA7) ^ 0x5A;
    mixed.rotate_left(((address >> 1) & 0x07) as u32) ^ 0xA5
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct BootAudioSnapshot {
    pub nr10: u8,
    pub nr11: u8,
    pub nr12: u8,
    pub nr13: u8,
    pub nr14: u8,
    pub nr21: u8,
    pub nr22: u8,
    pub nr23: u8,
    pub nr24: u8,
    pub nr30: u8,
    pub nr31: u8,
    pub nr32: u8,
    pub nr33: u8,
    pub nr34: u8,
    pub nr41: u8,
    pub nr42: u8,
    pub nr43: u8,
    pub nr44: u8,
    pub nr50: u8,
    pub nr51: u8,
    pub nr52: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct BootIoSnapshot {
    pub p1: u8,
    pub sb: u8,
    pub sc: u8,
    pub div: u8,
    pub tima: u8,
    pub tma: u8,
    pub tac: u8,
    pub interrupt_flag: u8,
    pub lcdc: u8,
    pub stat: u8,
    pub scy: u8,
    pub scx: u8,
    pub ly: u8,
    pub lyc: u8,
    pub dma: u8,
    pub bgp: u8,
    pub wy: u8,
    pub wx: u8,
    pub interrupt_enable: u8,
    pub audio: BootAudioSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct BootDirectBootState {
    pub cpu: CpuStartupState,
    pub io: BootIoSnapshot,
    pub apu: ApuStartupState,
    pub ppu: PpuStartupState,
    pub serial: SerialStartupState,
    pub timer: TimerStartupState,
    pub dma: DmaStartupState,
    pub interrupts: InterruptStartupState,
    pub joypad: JoypadStartupState,
    pub startup_memory_policy: StartupMemoryPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BootRealBootPowerOnState {
    pub timer: TimerStartupState,
    pub serial: SerialStartupState,
    pub dma: DmaStartupState,
    pub joypad: JoypadStartupState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BootStatus {
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BootController {
    console_model: ConsoleModel,
    startup_mode: StartupMode,
    status: BootStatus,
    boot_rom_kind: BootRomKind,
    boot_rom_mapped: bool,
    boot_rom_assets: BootRomAssets,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BootSaveState {
    console_model: ConsoleModel,
    startup_mode: StartupMode,
    status: BootStatus,
    boot_rom_kind: BootRomKind,
    boot_rom_mapped: bool,
    boot_rom_assets: BootRomAssets,
}

impl BootSaveState {
    pub(crate) fn dynamic_payload_bytes(&self) -> usize {
        self.boot_rom_assets.dynamic_payload_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BootSnapshot {
    pub console_model: ConsoleModel,
    pub startup_mode: StartupMode,
    pub status: BootStatus,
    pub boot_rom_kind: BootRomKind,
    pub boot_rom_mapped: bool,
    pub boot_rom_asset_configured: bool,
    pub startup_memory_policy: StartupMemoryPolicy,
}

impl BootController {
    pub fn new(
        console_model: ConsoleModel,
        startup_mode: StartupMode,
        boot_rom_kind: BootRomKind,
        boot_rom_assets: BootRomAssets,
    ) -> Self {
        Self {
            console_model,
            startup_mode,
            status: BootStatus::Ready,
            boot_rom_kind,
            boot_rom_mapped: startup_mode.requires_boot_rom(),
            boot_rom_assets,
        }
    }

    pub fn console_model(&self) -> ConsoleModel {
        self.console_model
    }

    pub(crate) fn capture_save_state(&self) -> BootSaveState {
        BootSaveState {
            console_model: self.console_model,
            startup_mode: self.startup_mode,
            status: self.status,
            boot_rom_kind: self.boot_rom_kind,
            boot_rom_mapped: self.boot_rom_mapped,
            boot_rom_assets: self.boot_rom_assets.clone(),
        }
    }

    pub(crate) fn restore_save_state(&mut self, state: &BootSaveState) {
        self.console_model = state.console_model;
        self.startup_mode = state.startup_mode;
        self.status = state.status;
        self.boot_rom_kind = state.boot_rom_kind;
        self.boot_rom_mapped = state.boot_rom_mapped;
        self.boot_rom_assets = state.boot_rom_assets.clone();
    }

    pub fn startup_mode(&self) -> StartupMode {
        self.startup_mode
    }

    pub fn status(&self) -> BootStatus {
        self.status
    }

    pub fn boot_rom_kind(&self) -> BootRomKind {
        self.boot_rom_kind
    }

    pub fn is_boot_rom_mapped(&self) -> bool {
        self.boot_rom_mapped
    }

    pub fn boot_rom_fingerprint(&self) -> Option<SaveStateByteFingerprint> {
        self.boot_rom_assets.fingerprint(self.boot_rom_kind)
    }

    pub fn has_boot_rom_asset(&self) -> bool {
        self.boot_rom_assets.has_image(self.boot_rom_kind)
    }

    pub fn bus_state(&self) -> BootRomBusState {
        if !self.boot_rom_mapped {
            return BootRomBusState::unmapped();
        }

        match self.console_model {
            ConsoleModel::GameBoyColor => BootRomBusState::map_cgb_windows(),
            ConsoleModel::GameBoy | ConsoleModel::GameBoyPocket | ConsoleModel::GameBoyLight => {
                BootRomBusState::map_dmg_low_bytes()
            }
        }
    }

    pub fn read_ff50(&self) -> u8 {
        0xFF
    }

    pub fn startup_memory_policy(&self) -> StartupMemoryPolicy {
        match (self.startup_mode, self.console_model.is_cgb_family()) {
            (StartupMode::CustomBoot, true) => {
                StartupMemoryPolicy::CgbRealBootEntryWithDmgBootLogoTiles
            }
            (StartupMode::CustomBoot, false) => StartupMemoryPolicy::DmgBootLogoVram,
            (StartupMode::SkipBoot, true) => StartupMemoryPolicy::CgbRealBootEntry,
            _ => StartupMemoryPolicy::DeterministicPatterned,
        }
    }

    pub fn read_boot_rom(&self, address: u16) -> u8 {
        self.boot_rom_assets
            .read_byte(self.boot_rom_kind, address)
            .unwrap_or(0xFF)
    }

    pub fn write_ff50(&mut self, value: u8) -> bool {
        if value != 0 && self.boot_rom_mapped {
            self.boot_rom_mapped = false;
            return true;
        }

        false
    }

    pub fn direct_boot_state(
        &self,
        cartridge: Option<&CartridgeSlot>,
    ) -> Option<BootDirectBootState> {
        if self.startup_mode != StartupMode::SkipBoot {
            return None;
        }

        let header = cartridge.and_then(CartridgeSlot::header);
        let system_counter = direct_start_system_counter(self.console_model, header);
        let mut io = verified_boot_entry_io_snapshot(self.console_model);
        io.div = div_from_system_counter(system_counter);
        let apu = build_verified_boot_entry_apu_state(self.console_model, system_counter, io);

        Some(self.build_skip_boot_state(cartridge, io, apu, system_counter))
    }

    pub(crate) fn real_boot_power_on_state(&self) -> Option<BootRealBootPowerOnState> {
        if self.startup_mode != StartupMode::RealBoot {
            return None;
        }

        Some(BootRealBootPowerOnState {
            timer: TimerStartupState {
                system_counter: real_boot_power_on_system_counter(self.console_model),
                tima: 0x00,
                tma: 0x00,
                tac: 0x00,
            },
            serial: SerialStartupState::from_registers(0x00, 0x00)
                .with_clock_counter(real_boot_power_on_serial_clock_counter(self.console_model)),
            dma: DmaStartupState {
                source_page_latch: 0xFF,
            },
            joypad: real_boot_power_on_joypad_state(self.console_model),
        })
    }

    pub(crate) fn machine_skip_boot_state(
        &self,
        cartridge: Option<&CartridgeSlot>,
    ) -> Option<BootDirectBootState> {
        if !self.startup_mode.uses_direct_boot_state() {
            return None;
        }

        let header = cartridge.and_then(CartridgeSlot::header);
        let system_counter = direct_start_system_counter(self.console_model, header);
        let mut io = synthetic_skip_boot_io_snapshot(self.console_model);
        io.div = div_from_system_counter(system_counter);
        let apu = build_skip_boot_apu_state(self.console_model, system_counter, io);

        Some(self.build_skip_boot_state(cartridge, io, apu, system_counter))
    }

    pub fn snapshot(&self) -> BootSnapshot {
        BootSnapshot {
            console_model: self.console_model,
            startup_mode: self.startup_mode,
            status: self.status,
            boot_rom_kind: self.boot_rom_kind,
            boot_rom_mapped: self.boot_rom_mapped,
            boot_rom_asset_configured: self.has_boot_rom_asset(),
            startup_memory_policy: self.startup_memory_policy(),
        }
    }

    pub fn scheduler_trace_message(&self, context: &CycleContext) -> String {
        format!(
            "t_cycle={} phase={} console_model={:?} startup_mode={:?} status={:?} boot_rom_kind={:?} boot_rom_mapped={}",
            context.t_cycle().get(),
            context.phase(),
            self.console_model,
            self.startup_mode,
            self.status,
            self.boot_rom_kind,
            self.boot_rom_mapped,
        )
    }

    fn build_skip_boot_state(
        &self,
        cartridge: Option<&CartridgeSlot>,
        io: BootIoSnapshot,
        apu: ApuStartupState,
        system_counter: u16,
    ) -> BootDirectBootState {
        BootDirectBootState {
            cpu: build_skip_boot_cpu_state(
                self.console_model,
                cartridge.and_then(CartridgeSlot::header),
            ),
            io,
            apu,
            ppu: build_skip_boot_ppu_state(io),
            serial: SerialStartupState::from_registers(io.sb, io.sc)
                .with_clock_counter(DMG_FAMILY_SKIP_BOOT_SERIAL_CLOCK_COUNTER),
            timer: TimerStartupState {
                system_counter,
                tima: io.tima,
                tma: io.tma,
                tac: io.tac & 0x07,
            },
            dma: DmaStartupState {
                source_page_latch: io.dma,
            },
            interrupts: InterruptStartupState {
                interrupt_flags: io.interrupt_flag & 0x1F,
                interrupt_enable: io.interrupt_enable,
            },
            joypad: JoypadStartupState {
                selection_bits: io.p1 & 0x30,
                pressed_mask: 0,
            },
            startup_memory_policy: self.startup_memory_policy(),
        }
    }
}

const fn build_skip_boot_ppu_state(io: BootIoSnapshot) -> PpuStartupState {
    PpuStartupState {
        lcdc: io.lcdc,
        stat: io.stat,
        scy: io.scy,
        scx: io.scx,
        ly: io.ly,
        lyc: io.lyc,
        bgp: io.bgp,
        wy: io.wy,
        wx: io.wx,
        obj_palette_read_policy: crate::ppu::DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    }
}

fn build_skip_boot_cpu_state(
    console_model: ConsoleModel,
    header: Option<&CartridgeHeader>,
) -> CpuStartupState {
    match console_model {
        ConsoleModel::GameBoy => CpuStartupState {
            a: 0x01,
            f: dmg_family_skip_boot_flags(header.map(|header| header.header_checksum)),
            b: 0x00,
            c: 0x13,
            d: 0x00,
            e: 0xD8,
            h: 0x01,
            l: 0x4D,
            sp: 0xFFFE,
            pc: 0x0100,
        },
        ConsoleModel::GameBoyPocket | ConsoleModel::GameBoyLight => CpuStartupState {
            a: 0xFF,
            f: dmg_family_skip_boot_flags(header.map(|header| header.header_checksum)),
            b: 0x00,
            c: 0x13,
            d: 0x00,
            e: 0xD8,
            h: 0x01,
            l: 0x4D,
            sp: 0xFFFE,
            pc: 0x0100,
        },
        ConsoleModel::GameBoyColor => {
            let cgb_native = header
                .map(|header| header.cgb_flag.enables_cgb_native_mode())
                .unwrap_or(false);
            CpuStartupState {
                a: 0x11,
                f: 0x80,
                b: 0x00,
                c: 0x00,
                d: if cgb_native { 0xFF } else { 0x00 },
                e: if cgb_native { 0x56 } else { 0x08 },
                h: 0x00,
                l: if cgb_native { 0x0D } else { 0x7C },
                sp: 0xFFFE,
                pc: 0x0100,
            }
        }
    }
}

const fn dmg_family_skip_boot_flags(header_checksum: Option<u8>) -> u8 {
    if matches!(header_checksum, Some(0x00)) {
        0x80
    } else {
        0xB0
    }
}

const fn dmg_family_synthetic_skip_boot_io_snapshot() -> BootIoSnapshot {
    BootIoSnapshot {
        p1: 0xCF,
        sb: 0x00,
        sc: 0x7E,
        div: 0xAB,
        tima: 0x00,
        tma: 0x00,
        tac: 0xF8,
        interrupt_flag: 0xE1,
        lcdc: 0x91,
        stat: 0x85,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        dma: 0xFF,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        interrupt_enable: 0x00,
        audio: dmg_family_skip_boot_audio_snapshot(),
    }
}

const fn synthetic_skip_boot_io_snapshot(console_model: ConsoleModel) -> BootIoSnapshot {
    match console_model {
        ConsoleModel::GameBoy | ConsoleModel::GameBoyPocket | ConsoleModel::GameBoyLight => {
            dmg_family_synthetic_skip_boot_io_snapshot()
        }
        ConsoleModel::GameBoyColor => {
            let mut io = dmg_family_synthetic_skip_boot_io_snapshot();
            io.p1 = 0xFF;
            io.div = CGB_SKIP_BOOT_DIV;
            io
        }
    }
}

const fn verified_boot_entry_io_snapshot(console_model: ConsoleModel) -> BootIoSnapshot {
    match console_model {
        ConsoleModel::GameBoy | ConsoleModel::GameBoyPocket | ConsoleModel::GameBoyLight => {
            BootIoSnapshot {
                p1: 0xCF,
                sb: 0x00,
                sc: 0x7E,
                div: 0xAB,
                tima: 0x00,
                tma: 0x00,
                tac: 0xF8,
                interrupt_flag: 0xE1,
                lcdc: 0x91,
                stat: 0x81,
                scy: 0x00,
                scx: 0x00,
                ly: 153,
                lyc: 0x00,
                dma: 0xFF,
                bgp: 0xFC,
                wy: 0x00,
                wx: 0x00,
                interrupt_enable: 0x00,
                audio: dmg_family_skip_boot_audio_snapshot(),
            }
        }
        ConsoleModel::GameBoyColor => synthetic_skip_boot_io_snapshot(console_model),
    }
}

const fn dmg_family_skip_boot_audio_snapshot() -> BootAudioSnapshot {
    BootAudioSnapshot {
        nr10: 0x80,
        nr11: 0xBF,
        nr12: 0xF3,
        nr13: 0xFF,
        nr14: 0xBF,
        nr21: 0x3F,
        nr22: 0x00,
        nr23: 0xFF,
        nr24: 0xBF,
        nr30: 0x7F,
        nr31: 0xFF,
        nr32: 0x9F,
        nr33: 0xFF,
        nr34: 0xBF,
        nr41: 0xFF,
        nr42: 0x00,
        nr43: 0x00,
        nr44: 0xBF,
        nr50: 0x77,
        nr51: 0xF3,
        nr52: 0xF1,
    }
}

fn build_skip_boot_apu_state(
    console_model: ConsoleModel,
    system_counter: u16,
    io: BootIoSnapshot,
) -> ApuStartupState {
    let audio = io.audio;

    ApuStartupState {
        powered: audio.nr52 & 0x80 != 0,
        nr10: audio.nr10 & 0x7F,
        nr11: audio.nr11 & 0xC0,
        nr12: audio.nr12,
        nr13: 0x00,
        nr14: audio.nr14 & 0x40,
        nr21: audio.nr21 & 0xC0,
        nr22: audio.nr22,
        nr23: 0x00,
        nr24: audio.nr24 & 0x40,
        nr30: audio.nr30 & 0x80,
        nr31: 0x00,
        nr32: audio.nr32 & 0x60,
        nr33: 0x00,
        nr34: audio.nr34 & 0x40,
        nr41: 0x00,
        nr42: audio.nr42,
        nr43: audio.nr43,
        nr44: audio.nr44 & 0x40,
        nr50: audio.nr50,
        nr51: audio.nr51,
        channel_active_mask: audio.nr52 & 0x0F,
        div_apu: div_apu_phase_from_system_counter(system_counter),
        wave_ram_startup_policy: if matches!(console_model, ConsoleModel::GameBoyColor) {
            WaveRamStartupPolicy::CgbRealBootAlternating
        } else {
            WaveRamStartupPolicy::DeterministicZeroed
        },
    }
}

fn build_verified_boot_entry_apu_state(
    console_model: ConsoleModel,
    system_counter: u16,
    io: BootIoSnapshot,
) -> ApuStartupState {
    let mut startup_state = build_skip_boot_apu_state(console_model, system_counter, io);
    startup_state.div_apu = verified_boot_entry_div_apu(console_model, system_counter);
    startup_state
}

fn read_boot_rom_file(
    directory: &Path,
    kind: BootRomKind,
) -> Result<Option<Vec<u8>>, BootRomAssetError> {
    let path = directory.join(BootRomAssets::filename(kind));
    match fs::read(&path) {
        Ok(bytes) => {
            validate_boot_rom_len(kind, &bytes, &path)?;
            Ok(Some(bytes))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(BootRomAssetError::ReadFailed { path, source }),
    }
}

fn validate_boot_rom_len(
    kind: BootRomKind,
    bytes: &[u8],
    path: &Path,
) -> Result<(), BootRomAssetError> {
    if bytes.len() < minimum_boot_rom_len(kind) {
        return Err(BootRomAssetError::ImageTooShort {
            kind,
            path: path.to_path_buf(),
            expected_at_least: minimum_boot_rom_len(kind),
            actual: bytes.len(),
        });
    }

    Ok(())
}

const fn minimum_boot_rom_len(kind: BootRomKind) -> usize {
    match kind {
        BootRomKind::Dmg0 | BootRomKind::Dmg | BootRomKind::Mgb => DMG_FAMILY_BOOT_ROM_LEN,
        BootRomKind::Cgb0 | BootRomKind::Cgb | BootRomKind::CgbE => CGB_BOOT_ROM_RAW_LEN,
    }
}

const DMG_FAMILY_SKIP_BOOT_SYSTEM_COUNTER_LOW: u8 = 0xC8;
const DMG_FAMILY_SKIP_BOOT_SERIAL_CLOCK_COUNTER: u16 = 0xABCC;
const CGB_SKIP_BOOT_DIV: u8 = 0x26;
// Mooneye's `boot_div-cgbABCDE.gb` is a DMG-compatible CGB header and owns the fallback direct-start phase.
const CGB_DEFAULT_DIRECT_BOOT_SYSTEM_COUNTER: u16 = 0x2674;
// Hacktix `bully.gb` is a native-CGB, non-Nintendo old-licensee header. This value matches gb-cycle's observed standard `cgb_boot.bin` handoff phase for that bucket; the complete CGB header table is tracked as follow-up documentation debt.
const CGB_NATIVE_NON_NINTENDO_DIRECT_BOOT_SYSTEM_COUNTER: u16 = 0x1E84;
// Nitro2k01 `whichboot.gb` is a native-CGB old-licensee `$33` header with binary-zero new-licensee bytes. Its hardware-facing timing reference identifies a distinct CGB boot-entry bucket one final boot-ROM IF-poll period after gb-cycle's raw `cgb_boot.bin` handoff for that header.
const CGB_NATIVE_BINARY_ZERO_NEW_LICENSEE_DIRECT_BOOT_SYSTEM_COUNTER: u16 = 0x1E98;
const CGB_NATIVE_BINARY_ZERO_NEW_LICENSEE_HANDOFF_CORRECTION_T_CYCLES: u16 = 24;
const OLD_LICENSEE_NINTENDO: u8 = 0x01;
const OLD_LICENSEE_USE_NEW_LICENSEE_CODE: u8 = 0x33;
const DMG_FAMILY_REAL_BOOT_POWER_ON_SYSTEM_COUNTER: u16 = 0x0064;
const DMG_FAMILY_REAL_BOOT_POWER_ON_SERIAL_CLOCK_COUNTER: u16 = 0x0068;
const CGB_REAL_BOOT_POWER_ON_SYSTEM_COUNTER: u16 = 0xFFFB;
const DMG_FAMILY_SYNTHETIC_SKIP_BOOT_SYSTEM_COUNTER: u16 =
    ((dmg_family_synthetic_skip_boot_io_snapshot().div as u16) << 8)
        | (DMG_FAMILY_SKIP_BOOT_SYSTEM_COUNTER_LOW as u16);

fn direct_start_system_counter(
    console_model: ConsoleModel,
    header: Option<&CartridgeHeader>,
) -> u16 {
    match console_model {
        ConsoleModel::GameBoy | ConsoleModel::GameBoyPocket | ConsoleModel::GameBoyLight => {
            DMG_FAMILY_SYNTHETIC_SKIP_BOOT_SYSTEM_COUNTER
        }
        ConsoleModel::GameBoyColor => cgb_direct_start_system_counter(header),
    }
}

fn cgb_direct_start_system_counter(header: Option<&CartridgeHeader>) -> u16 {
    let Some(header) = header else {
        return CGB_DEFAULT_DIRECT_BOOT_SYSTEM_COUNTER;
    };

    if !header.cgb_flag.enables_cgb_native_mode() {
        return CGB_DEFAULT_DIRECT_BOOT_SYSTEM_COUNTER;
    }

    if cgb_native_binary_zero_new_licensee_boot_bucket(Some(header)) {
        return CGB_NATIVE_BINARY_ZERO_NEW_LICENSEE_DIRECT_BOOT_SYSTEM_COUNTER;
    }

    if matches!(
        header.old_licensee_code,
        OLD_LICENSEE_NINTENDO | OLD_LICENSEE_USE_NEW_LICENSEE_CODE
    ) {
        return CGB_DEFAULT_DIRECT_BOOT_SYSTEM_COUNTER;
    }

    CGB_NATIVE_NON_NINTENDO_DIRECT_BOOT_SYSTEM_COUNTER
}

pub(crate) fn cgb_real_boot_handoff_correction_t_cycles(header: Option<&CartridgeHeader>) -> u16 {
    if cgb_native_binary_zero_new_licensee_boot_bucket(header) {
        CGB_NATIVE_BINARY_ZERO_NEW_LICENSEE_HANDOFF_CORRECTION_T_CYCLES
    } else {
        0
    }
}

fn cgb_native_binary_zero_new_licensee_boot_bucket(header: Option<&CartridgeHeader>) -> bool {
    let Some(header) = header else {
        return false;
    };

    header.cgb_flag.enables_cgb_native_mode()
        && header.old_licensee_code == OLD_LICENSEE_USE_NEW_LICENSEE_CODE
        && header.new_licensee_code == [0x00, 0x00]
}

const fn div_from_system_counter(system_counter: u16) -> u8 {
    (system_counter >> 8) as u8
}

const fn verified_boot_entry_div_apu(console_model: ConsoleModel, system_counter: u16) -> u8 {
    match console_model {
        ConsoleModel::GameBoy | ConsoleModel::GameBoyPocket | ConsoleModel::GameBoyLight => 0x01,
        ConsoleModel::GameBoyColor => div_apu_phase_from_system_counter(system_counter),
    }
}

const fn real_boot_power_on_system_counter(console_model: ConsoleModel) -> u16 {
    match console_model {
        ConsoleModel::GameBoy | ConsoleModel::GameBoyPocket | ConsoleModel::GameBoyLight => {
            DMG_FAMILY_REAL_BOOT_POWER_ON_SYSTEM_COUNTER
        }
        ConsoleModel::GameBoyColor => CGB_REAL_BOOT_POWER_ON_SYSTEM_COUNTER,
    }
}

const fn real_boot_power_on_serial_clock_counter(console_model: ConsoleModel) -> u16 {
    match console_model {
        ConsoleModel::GameBoy | ConsoleModel::GameBoyPocket | ConsoleModel::GameBoyLight => {
            DMG_FAMILY_REAL_BOOT_POWER_ON_SERIAL_CLOCK_COUNTER
        }
        ConsoleModel::GameBoyColor => 0,
    }
}

const fn real_boot_power_on_joypad_state(console_model: ConsoleModel) -> JoypadStartupState {
    match console_model {
        ConsoleModel::GameBoy | ConsoleModel::GameBoyPocket | ConsoleModel::GameBoyLight => {
            JoypadStartupState {
                selection_bits: 0x00,
                pressed_mask: 0x00,
            }
        }
        ConsoleModel::GameBoyColor => JoypadStartupState {
            selection_bits: 0x30,
            pressed_mask: 0x00,
        },
    }
}

#[cfg(test)]
mod tests;
