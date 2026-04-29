use crate::boot::BootRomAssets;
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
    Dmg0,
    #[default]
    Dmg,
    Mgb,
    Cgb,
}

impl ConsoleModel {
    pub const fn family(self) -> ConsoleFamily {
        match self {
            Self::Cgb => ConsoleFamily::Cgb,
            Self::Dmg0 | Self::Dmg | Self::Mgb => ConsoleFamily::Dmg,
        }
    }

    pub const fn is_dmg_family(self) -> bool {
        matches!(self.family(), ConsoleFamily::Dmg)
    }

    pub const fn is_cgb_family(self) -> bool {
        matches!(self.family(), ConsoleFamily::Cgb)
    }

    pub const fn default_operating_mode(self) -> OperatingMode {
        match self.family() {
            ConsoleFamily::Dmg => OperatingMode::Dmg,
            ConsoleFamily::Cgb => OperatingMode::Cgb,
        }
    }

    pub const fn supports_operating_mode(self, operating_mode: OperatingMode) -> bool {
        match self.family() {
            ConsoleFamily::Dmg => matches!(operating_mode, OperatingMode::Dmg),
            ConsoleFamily::Cgb => {
                matches!(
                    operating_mode,
                    OperatingMode::Cgb | OperatingMode::CgbCompatibility
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
                    OperatingMode::CgbCompatibility
                }
            }
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum OperatingMode {
    #[default]
    Dmg,
    Cgb,
    CgbCompatibility,
}

impl OperatingMode {
    pub const fn uses_dmg_software_contract(self) -> bool {
        matches!(self, Self::Dmg | Self::CgbCompatibility)
    }

    pub const fn enables_cgb_extensions(self) -> bool {
        matches!(self, Self::Cgb)
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum HostPlatform {
    #[default]
    Handheld,
    Sgb1,
    Sgb2,
}

impl HostPlatform {
    pub const fn is_sgb(self) -> bool {
        matches!(self, Self::Sgb1 | Self::Sgb2)
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
    RealBoot,
}

impl StartupMode {
    pub fn requires_boot_rom(self) -> bool {
        matches!(self, Self::RealBoot)
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
    pub host_platform: HostPlatform,
    pub startup_mode: StartupMode,
    pub boot_rom_assets: BootRomAssets,
    pub compatibility: CompatibilityPolicy,
}

impl MachineConfig {
    pub fn new(console_model: ConsoleModel) -> Self {
        Self {
            console_model,
            operating_mode: console_model.default_operating_mode(),
            ..Self::default()
        }
    }

    pub fn with_console_model(mut self, console_model: ConsoleModel) -> Self {
        self.console_model = console_model;
        self
    }

    pub fn with_operating_mode(mut self, operating_mode: OperatingMode) -> Self {
        self.operating_mode = operating_mode;
        self
    }

    pub fn with_host_platform(mut self, host_platform: HostPlatform) -> Self {
        self.host_platform = host_platform;
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
        if self.startup_mode != StartupMode::SkipBoot {
            return;
        }

        let cgb_flag = header.map_or(CgbFlag::None, |header| header.cgb_flag);
        self.operating_mode = self
            .compatibility
            .override_policy
            .forced_operating_mode
            .unwrap_or_else(|| {
                self.console_model
                    .direct_boot_operating_mode_for_cgb_flag(cgb_flag)
            });
    }

    pub const fn model_axes_are_coherent(&self) -> bool {
        self.console_model
            .supports_operating_mode(self.operating_mode)
    }
}

impl Default for MachineConfig {
    fn default() -> Self {
        Self {
            console_model: ConsoleModel::Dmg,
            operating_mode: ConsoleModel::Dmg.default_operating_mode(),
            host_platform: HostPlatform::Handheld,
            startup_mode: StartupMode::SkipBoot,
            boot_rom_assets: BootRomAssets::none(),
            compatibility: CompatibilityPolicy::strict(),
        }
    }
}

#[cfg(test)]
mod tests;
