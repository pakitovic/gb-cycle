#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConsoleFamily {
    Dmg,
    Cgb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ConsoleModel {
    Dmg0,
    #[default]
    Dmg,
    Mgb,
    Cgb,
}

impl ConsoleModel {
    pub fn family(self) -> ConsoleFamily {
        match self {
            Self::Cgb => ConsoleFamily::Cgb,
            Self::Dmg0 | Self::Dmg | Self::Mgb => ConsoleFamily::Dmg,
        }
    }

    pub fn is_dmg_family(self) -> bool {
        self.family() == ConsoleFamily::Dmg
    }

    pub fn is_cgb_family(self) -> bool {
        self.family() == ConsoleFamily::Cgb
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ValidationPolicy {
    #[default]
    Strict,
    Warn,
    Ignore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum HeuristicPolicy {
    #[default]
    Disabled,
    AllowExperimental,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DiagnosticPolicy {
    Quiet,
    #[default]
    Standard,
    Verbose,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OverridePolicy {
    pub forced_console_model: Option<ConsoleModel>,
    pub forced_startup_mode: Option<StartupMode>,
}

impl OverridePolicy {
    pub fn has_overrides(&self) -> bool {
        self.forced_console_model.is_some() || self.forced_startup_mode.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineConfig {
    pub console_model: ConsoleModel,
    pub startup_mode: StartupMode,
    pub compatibility: CompatibilityPolicy,
}

impl MachineConfig {
    pub fn new(console_model: ConsoleModel) -> Self {
        Self {
            console_model,
            ..Self::default()
        }
    }

    pub fn with_console_model(mut self, console_model: ConsoleModel) -> Self {
        self.console_model = console_model;
        self
    }

    pub fn with_startup_mode(mut self, startup_mode: StartupMode) -> Self {
        self.startup_mode = startup_mode;
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
}

impl Default for MachineConfig {
    fn default() -> Self {
        Self {
            console_model: ConsoleModel::Dmg,
            startup_mode: StartupMode::SkipBoot,
            compatibility: CompatibilityPolicy::strict(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn console_models_keep_dmg_and_cgb_families_explicit() {
        assert!(ConsoleModel::Dmg0.is_dmg_family());
        assert!(ConsoleModel::Dmg.is_dmg_family());
        assert!(ConsoleModel::Mgb.is_dmg_family());
        assert!(ConsoleModel::Cgb.is_cgb_family());
    }

    #[test]
    fn compatibility_presets_keep_policy_choices_coherent() {
        assert_eq!(
            CompatibilityPolicy::strict().execution_mode,
            ExecutionMode::Strict
        );
        assert_eq!(
            CompatibilityPolicy::permissive().validation_policy,
            ValidationPolicy::Warn
        );
        assert_eq!(
            CompatibilityPolicy::experimental().heuristic_policy,
            HeuristicPolicy::AllowExperimental
        );
        assert_eq!(
            CompatibilityPolicy::experimental().diagnostic_policy,
            DiagnosticPolicy::Verbose
        );
    }

    #[test]
    fn machine_config_builder_methods_only_update_requested_fields() {
        let config = MachineConfig::default()
            .with_console_model(ConsoleModel::Mgb)
            .with_startup_mode(StartupMode::RealBoot)
            .with_execution_mode(ExecutionMode::Permissive);

        assert_eq!(config.console_model, ConsoleModel::Mgb);
        assert_eq!(config.startup_mode, StartupMode::RealBoot);
        assert_eq!(
            config.compatibility.execution_mode,
            ExecutionMode::Permissive
        );
        assert_eq!(
            config.compatibility.validation_policy,
            ValidationPolicy::Strict
        );
    }
}
