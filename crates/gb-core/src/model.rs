use crate::boot::{BootRomAssetKind, BootRomAssets};
use crate::cartridge::{CartridgeHeader, CgbFlag};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ConsoleFamily {
    Dmg,
    Cgb,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum ConsoleModel {
    #[default]
    GameBoy,
    GameBoyPocket,
    GameBoyLight,
    GameBoyColor,
    GameBoyAdvance,
}

impl ConsoleModel {
    pub const fn family(self) -> ConsoleFamily {
        match self {
            Self::GameBoyColor | Self::GameBoyAdvance => ConsoleFamily::Cgb,
            Self::GameBoy | Self::GameBoyPocket | Self::GameBoyLight => ConsoleFamily::Dmg,
        }
    }

    pub const fn is_dmg_family(self) -> bool {
        matches!(self.family(), ConsoleFamily::Dmg)
    }

    pub const fn is_cgb_family(self) -> bool {
        matches!(self.family(), ConsoleFamily::Cgb)
    }

    pub const fn has_cgb_infrared_port(self) -> bool {
        matches!(self, Self::GameBoyColor)
    }

    pub const fn default_operating_mode(self) -> OperatingMode {
        match self.family() {
            ConsoleFamily::Dmg => OperatingMode::Dmg,
            ConsoleFamily::Cgb => OperatingMode::Cgb,
        }
    }

    pub const fn default_revision(self) -> HardwareRevision {
        match self {
            Self::GameBoy => HardwareRevision::DmgCpuC,
            Self::GameBoyPocket | Self::GameBoyLight => HardwareRevision::CpuMgb,
            Self::GameBoyColor => HardwareRevision::CpuCgbE,
            Self::GameBoyAdvance => HardwareRevision::CpuAgbA,
        }
    }

    pub const fn active_revisions(self) -> &'static [HardwareRevision] {
        match self {
            Self::GameBoy => &ACTIVE_DMG_REVISIONS,
            Self::GameBoyPocket | Self::GameBoyLight => &ACTIVE_MGB_REVISIONS,
            Self::GameBoyColor => &ACTIVE_CGB_REVISIONS,
            Self::GameBoyAdvance => &ACTIVE_AGB_REVISIONS,
        }
    }

    pub const fn supports_revision(self, revision: HardwareRevision) -> bool {
        match self {
            Self::GameBoy => matches!(revision, HardwareRevision::DmgCpuC),
            Self::GameBoyPocket | Self::GameBoyLight => {
                matches!(revision, HardwareRevision::CpuMgb)
            }
            Self::GameBoyColor => matches!(
                revision,
                HardwareRevision::CpuCgbC | HardwareRevision::CpuCgbD | HardwareRevision::CpuCgbE
            ),
            Self::GameBoyAdvance => matches!(revision, HardwareRevision::CpuAgbA),
        }
    }

    pub const fn supports_operating_mode(self, operating_mode: OperatingMode) -> bool {
        match self.family() {
            ConsoleFamily::Dmg => matches!(operating_mode, OperatingMode::Dmg),
            ConsoleFamily::Cgb => {
                matches!(
                    operating_mode,
                    OperatingMode::Cgb | OperatingMode::GbCompatible | OperatingMode::CgbDmgExt
                )
            }
        }
    }

    pub const fn direct_boot_operating_mode_for_cgb_flag(self, cgb_flag: CgbFlag) -> OperatingMode {
        match self.family() {
            ConsoleFamily::Dmg => OperatingMode::Dmg,
            ConsoleFamily::Cgb => {
                if cgb_flag.enables_cgb_native_mode() {
                    OperatingMode::Cgb
                } else {
                    OperatingMode::GbCompatible
                }
            }
        }
    }

    pub const fn direct_boot_operating_mode_for_cgb_flag_with_heuristic(
        self,
        cgb_flag: CgbFlag,
        heuristic_policy: HeuristicPolicy,
    ) -> OperatingMode {
        match self.family() {
            ConsoleFamily::Dmg => OperatingMode::Dmg,
            ConsoleFamily::Cgb => {
                if matches!(heuristic_policy, HeuristicPolicy::AllowExperimental)
                    && cgb_flag.requests_cgb_dmg_ext_mode()
                {
                    OperatingMode::CgbDmgExt
                } else if matches!(heuristic_policy, HeuristicPolicy::AllowExperimental)
                    && cgb_flag.requests_cgb_dmg_compatibility_mode()
                {
                    OperatingMode::GbCompatible
                } else {
                    self.direct_boot_operating_mode_for_cgb_flag(cgb_flag)
                }
            }
        }
    }
}

const ACTIVE_DMG_REVISIONS: [HardwareRevision; 1] = [HardwareRevision::DmgCpuC];
const ACTIVE_MGB_REVISIONS: [HardwareRevision; 1] = [HardwareRevision::CpuMgb];
const ACTIVE_CGB_REVISIONS: [HardwareRevision; 3] = [
    HardwareRevision::CpuCgbC,
    HardwareRevision::CpuCgbD,
    HardwareRevision::CpuCgbE,
];
const ACTIVE_AGB_REVISIONS: [HardwareRevision; 1] = [HardwareRevision::CpuAgbA];

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum HardwareRevision {
    DmgCpu,
    DmgCpuA,
    DmgCpuB,
    #[default]
    DmgCpuC,
    CpuMgb,
    CpuCgb,
    CpuCgbA,
    CpuCgbB,
    CpuCgbC,
    CpuCgbD,
    CpuCgbE,
    CpuAgbA,
}

impl HardwareRevision {
    pub const fn boot_rom_filename(self) -> &'static str {
        BootRomAssetKind::from_revision(self).filename()
    }

    pub const fn boot_rom_expected_sha256(self) -> &'static str {
        BootRomAssetKind::from_revision(self).expected_sha256()
    }

    pub const fn boot_rom_expected_size(self) -> usize {
        BootRomAssetKind::from_revision(self).expected_size()
    }

    pub const fn uses_cgb_boot_rom(self) -> bool {
        matches!(
            self,
            Self::CpuCgb
                | Self::CpuCgbA
                | Self::CpuCgbB
                | Self::CpuCgbC
                | Self::CpuCgbD
                | Self::CpuCgbE
                | Self::CpuAgbA
        )
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum OperatingMode {
    #[default]
    Dmg,
    Cgb,
    GbCompatible,
    CgbDmgExt,
}

impl OperatingMode {
    pub const fn uses_dmg_software_contract(self) -> bool {
        matches!(self, Self::Dmg | Self::GbCompatible | Self::CgbDmgExt)
    }

    pub const fn enables_cgb_extensions(self) -> bool {
        matches!(self, Self::Cgb)
    }

    pub const fn enables_cgb_speed_switch(self) -> bool {
        matches!(self, Self::Cgb | Self::CgbDmgExt)
    }

    pub const fn enables_cgb_high_speed_serial(self) -> bool {
        matches!(self, Self::Cgb | Self::CgbDmgExt)
    }

    pub const fn enables_cgb_infrared_register(self) -> bool {
        matches!(self, Self::Cgb | Self::CgbDmgExt)
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum HostPlatform {
    #[default]
    Handheld,
    Sgb,
    Sgb2,
}

impl HostPlatform {
    pub const fn is_sgb(self) -> bool {
        matches!(self, Self::Sgb | Self::Sgb2)
    }
}

pub const DMG_MASTER_CLOCK_HZ: u32 = 4_194_304;
pub const SGB_ICD2_CLOCK_DIVISOR: u32 = 5;
pub const SGB_NTSC_SOURCE_MASTER_CLOCK_HZ: u32 = 21_477_272;
pub const SGB_PAL_SOURCE_MASTER_CLOCK_HZ: u32 = 21_281_370;
pub const SGB2_SOURCE_MASTER_CLOCK_HZ: u32 = DMG_MASTER_CLOCK_HZ * SGB_ICD2_CLOCK_DIVISOR;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum SgbVideoStandard {
    #[default]
    Ntsc,
    Pal,
}

impl SgbVideoStandard {
    pub const fn argument_name(self) -> &'static str {
        match self {
            Self::Ntsc => "ntsc",
            Self::Pal => "pal",
        }
    }

    pub const fn menu_name(self) -> &'static str {
        match self {
            Self::Ntsc => "NTSC",
            Self::Pal => "PAL",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SgbClockRate {
    pub numerator_hz: u64,
    pub denominator: u32,
}

impl SgbClockRate {
    pub const fn from_hz(hz: u32) -> Self {
        Self {
            numerator_hz: hz as u64,
            denominator: 1,
        }
    }

    pub const fn divided_by(self, divisor: u32) -> Self {
        Self {
            numerator_hz: self.numerator_hz,
            denominator: self.denominator * divisor,
        }
    }

    pub const fn rounded_hz(self) -> u32 {
        ((self.numerator_hz + (self.denominator / 2) as u64) / self.denominator as u64) as u32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SgbProfileTiming {
    pub source_master_clock_hz: SgbClockRate,
    pub gb_master_clock_hz: SgbClockRate,
    pub gb_clock_divisor: u32,
    pub video_standard: SgbVideoStandard,
    pub corrected_clock: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SgbHostProfile {
    SgbNtsc,
    SgbPal,
    Sgb2Ntsc,
}

impl SgbHostProfile {
    pub const ALL: [Self; 3] = [Self::SgbNtsc, Self::SgbPal, Self::Sgb2Ntsc];

    pub const fn default_for_host_platform(host_platform: HostPlatform) -> Option<Self> {
        match host_platform {
            HostPlatform::Handheld => None,
            HostPlatform::Sgb => Some(Self::SgbNtsc),
            HostPlatform::Sgb2 => Some(Self::Sgb2Ntsc),
        }
    }

    pub const fn host_platform(self) -> HostPlatform {
        match self {
            Self::SgbNtsc | Self::SgbPal => HostPlatform::Sgb,
            Self::Sgb2Ntsc => HostPlatform::Sgb2,
        }
    }

    pub const fn video_standard(self) -> SgbVideoStandard {
        match self {
            Self::SgbNtsc | Self::Sgb2Ntsc => SgbVideoStandard::Ntsc,
            Self::SgbPal => SgbVideoStandard::Pal,
        }
    }

    pub const fn ui_label(self) -> &'static str {
        match self {
            Self::SgbNtsc | Self::SgbPal => "SUPER GB",
            Self::Sgb2Ntsc => "SUPER GB2",
        }
    }

    pub const fn machine_profile_name(self) -> &'static str {
        match self {
            Self::SgbNtsc | Self::SgbPal => "SGB",
            Self::Sgb2Ntsc => "SGB2",
        }
    }

    pub const fn revision_label(self) -> &'static str {
        match self {
            Self::SgbNtsc | Self::SgbPal => "SGB-CPU 01",
            Self::Sgb2Ntsc => "CPU SGB2",
        }
    }

    pub const fn real_boot_filename(self) -> &'static str {
        match self {
            Self::SgbNtsc | Self::SgbPal => "sgb_boot.bin",
            Self::Sgb2Ntsc => "sgb2_boot.bin",
        }
    }

    pub const fn game_link_supported(self) -> bool {
        matches!(self, Self::Sgb2Ntsc)
    }

    pub const fn corrected_clock(self) -> bool {
        matches!(self, Self::Sgb2Ntsc)
    }

    pub const fn timing(self) -> SgbProfileTiming {
        let source_master_clock_hz = match self {
            Self::SgbNtsc => SgbClockRate::from_hz(SGB_NTSC_SOURCE_MASTER_CLOCK_HZ),
            Self::SgbPal => SgbClockRate::from_hz(SGB_PAL_SOURCE_MASTER_CLOCK_HZ),
            Self::Sgb2Ntsc => SgbClockRate::from_hz(SGB2_SOURCE_MASTER_CLOCK_HZ),
        };
        SgbProfileTiming {
            source_master_clock_hz,
            gb_master_clock_hz: source_master_clock_hz.divided_by(SGB_ICD2_CLOCK_DIVISOR),
            gb_clock_divisor: SGB_ICD2_CLOCK_DIVISOR,
            video_standard: self.video_standard(),
            corrected_clock: self.corrected_clock(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct CapabilitySet {
    console_model: ConsoleModel,
    console_family: ConsoleFamily,
    operating_mode: OperatingMode,
    host_platform: HostPlatform,
    dmg_software_contract: bool,
    cgb_extensions_enabled: bool,
    dmg_family_quirks_enabled: bool,
    sgb_enhancements_enabled: bool,
}

impl CapabilitySet {
    pub const fn from_model_axes(
        console_model: ConsoleModel,
        operating_mode: OperatingMode,
        host_platform: HostPlatform,
    ) -> Self {
        Self {
            console_model,
            console_family: console_model.family(),
            operating_mode,
            host_platform,
            dmg_software_contract: operating_mode.uses_dmg_software_contract(),
            cgb_extensions_enabled: console_model.is_cgb_family()
                && operating_mode.enables_cgb_extensions(),
            // DMG-family-only quirks such as OAM corruption follow the silicon family,
            // not the software-facing compatibility mode.
            dmg_family_quirks_enabled: console_model.is_dmg_family(),
            sgb_enhancements_enabled: host_platform.is_sgb(),
        }
    }

    pub const fn console_model(self) -> ConsoleModel {
        self.console_model
    }

    pub const fn console_family(self) -> ConsoleFamily {
        self.console_family
    }

    pub const fn operating_mode(self) -> OperatingMode {
        self.operating_mode
    }

    pub const fn host_platform(self) -> HostPlatform {
        self.host_platform
    }

    pub const fn dmg_software_contract(self) -> bool {
        self.dmg_software_contract
    }

    pub const fn cgb_extensions_enabled(self) -> bool {
        self.cgb_extensions_enabled
    }

    pub const fn dmg_family_quirks_enabled(self) -> bool {
        self.dmg_family_quirks_enabled
    }

    pub const fn sgb_enhancements_enabled(self) -> bool {
        self.sgb_enhancements_enabled
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum StartupMode {
    #[default]
    SkipBoot,
    CustomBoot,
    RealBoot,
}

impl StartupMode {
    pub const fn requires_boot_rom(self) -> bool {
        matches!(self, Self::RealBoot)
    }

    pub const fn uses_direct_boot_state(self) -> bool {
        matches!(self, Self::SkipBoot | Self::CustomBoot)
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum ExecutionMode {
    #[default]
    Strict,
    Permissive,
    Experimental,
}

impl ExecutionMode {
    pub fn is_oracle(self) -> bool {
        matches!(self, Self::Strict)
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum ValidationPolicy {
    #[default]
    Strict,
    Warn,
    Ignore,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum HeuristicPolicy {
    #[default]
    Disabled,
    AllowExperimental,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum DiagnosticPolicy {
    Quiet,
    #[default]
    Standard,
    Verbose,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct OverridePolicy {
    pub forced_console_model: Option<ConsoleModel>,
    pub forced_operating_mode: Option<OperatingMode>,
    pub forced_host_platform: Option<HostPlatform>,
    pub forced_startup_mode: Option<StartupMode>,
}

impl OverridePolicy {
    pub fn has_overrides(&self) -> bool {
        self.forced_console_model.is_some()
            || self.forced_operating_mode.is_some()
            || self.forced_host_platform.is_some()
            || self.forced_startup_mode.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CompatibilityPolicy {
    pub execution_mode: ExecutionMode,
    pub validation_policy: ValidationPolicy,
    pub heuristic_policy: HeuristicPolicy,
    pub override_policy: OverridePolicy,
    pub diagnostic_policy: DiagnosticPolicy,
}

impl CompatibilityPolicy {
    pub fn strict() -> Self {
        Self {
            execution_mode: ExecutionMode::Strict,
            validation_policy: ValidationPolicy::Strict,
            heuristic_policy: HeuristicPolicy::Disabled,
            override_policy: OverridePolicy::default(),
            diagnostic_policy: DiagnosticPolicy::Standard,
        }
    }

    pub fn permissive() -> Self {
        Self {
            execution_mode: ExecutionMode::Permissive,
            validation_policy: ValidationPolicy::Warn,
            heuristic_policy: HeuristicPolicy::Disabled,
            override_policy: OverridePolicy::default(),
            diagnostic_policy: DiagnosticPolicy::Standard,
        }
    }

    pub fn experimental() -> Self {
        Self {
            execution_mode: ExecutionMode::Experimental,
            validation_policy: ValidationPolicy::Warn,
            heuristic_policy: HeuristicPolicy::AllowExperimental,
            override_policy: OverridePolicy::default(),
            diagnostic_policy: DiagnosticPolicy::Verbose,
        }
    }
}

impl Default for CompatibilityPolicy {
    fn default() -> Self {
        Self::strict()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MachineConfig {
    pub console_model: ConsoleModel,
    pub operating_mode: OperatingMode,
    pub revision: HardwareRevision,
    pub host_platform: HostPlatform,
    pub sgb_profile: Option<SgbHostProfile>,
    pub startup_mode: StartupMode,
    pub boot_rom_assets: BootRomAssets,
    pub compatibility: CompatibilityPolicy,
}

impl MachineConfig {
    pub fn new(console_model: ConsoleModel) -> Self {
        Self {
            console_model,
            operating_mode: console_model.default_operating_mode(),
            revision: console_model.default_revision(),
            ..Self::default()
        }
    }

    pub fn with_console_model(mut self, console_model: ConsoleModel) -> Self {
        self.console_model = console_model;
        if !console_model.supports_operating_mode(self.operating_mode) {
            self.operating_mode = console_model.default_operating_mode();
        }
        if !console_model.supports_revision(self.revision) {
            self.revision = console_model.default_revision();
        }
        self
    }

    pub fn with_operating_mode(mut self, operating_mode: OperatingMode) -> Self {
        self.operating_mode = operating_mode;
        self
    }

    pub fn with_revision(mut self, revision: HardwareRevision) -> Self {
        self.revision = revision;
        self
    }

    pub fn with_host_platform(mut self, host_platform: HostPlatform) -> Self {
        self.host_platform = host_platform;
        self.sgb_profile = SgbHostProfile::default_for_host_platform(host_platform);
        self
    }

    pub fn with_sgb_profile(mut self, sgb_profile: SgbHostProfile) -> Self {
        self.host_platform = sgb_profile.host_platform();
        self.sgb_profile = Some(sgb_profile);
        self
    }

    pub fn with_startup_mode(mut self, startup_mode: StartupMode) -> Self {
        self.startup_mode = startup_mode;
        self
    }

    pub fn with_boot_rom_assets(mut self, boot_rom_assets: BootRomAssets) -> Self {
        self.boot_rom_assets = boot_rom_assets;
        self
    }

    pub const fn boot_rom_asset_kind(&self) -> BootRomAssetKind {
        BootRomAssetKind::from_machine_profile(self.revision, self.sgb_profile)
    }

    pub fn with_compatibility(mut self, compatibility: CompatibilityPolicy) -> Self {
        self.compatibility = compatibility;
        self
    }

    pub fn with_execution_mode(mut self, execution_mode: ExecutionMode) -> Self {
        self.compatibility.execution_mode = execution_mode;
        self
    }

    pub const fn capability_set(&self) -> CapabilitySet {
        CapabilitySet::from_model_axes(self.console_model, self.operating_mode, self.host_platform)
    }

    pub fn apply_direct_boot_cartridge_header(&mut self, header: Option<&CartridgeHeader>) {
        if !self.startup_mode.uses_direct_boot_state() {
            return;
        }

        let cgb_flag = header.map_or(CgbFlag::None, |header| header.cgb_flag);
        self.operating_mode = self
            .compatibility
            .override_policy
            .forced_operating_mode
            .unwrap_or_else(|| {
                self.console_model
                    .direct_boot_operating_mode_for_cgb_flag_with_heuristic(
                        cgb_flag,
                        self.compatibility.heuristic_policy,
                    )
            });
    }

    pub const fn model_axes_are_coherent(&self) -> bool {
        self.console_model
            .supports_operating_mode(self.operating_mode)
            && self.console_model.supports_revision(self.revision)
            && self.sgb_profile_matches_host_platform()
    }

    pub const fn sgb_profile_matches_host_platform(&self) -> bool {
        match (self.host_platform, self.sgb_profile) {
            (HostPlatform::Handheld, None) => true,
            (HostPlatform::Handheld, Some(_)) => false,
            (HostPlatform::Sgb, Some(profile)) => {
                matches!(profile.host_platform(), HostPlatform::Sgb)
            }
            (HostPlatform::Sgb2, Some(profile)) => {
                matches!(profile.host_platform(), HostPlatform::Sgb2)
            }
            (_, None) => false,
        }
    }
}

impl Default for MachineConfig {
    fn default() -> Self {
        Self {
            console_model: ConsoleModel::GameBoy,
            operating_mode: ConsoleModel::GameBoy.default_operating_mode(),
            revision: ConsoleModel::GameBoy.default_revision(),
            host_platform: HostPlatform::Handheld,
            sgb_profile: None,
            startup_mode: StartupMode::SkipBoot,
            boot_rom_assets: BootRomAssets::none(),
            compatibility: CompatibilityPolicy::strict(),
        }
    }
}

#[cfg(test)]
mod tests;
