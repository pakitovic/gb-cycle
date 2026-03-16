use gb_core::{
    BootRomAssets, BootRomKind, CompatibilityPolicy, ConsoleFamily, ConsoleModel, DiagnosticPolicy,
    ExecutionMode, HeuristicPolicy, MachineConfig, OverridePolicy, StartupMode, ValidationPolicy,
};

#[test]
fn default_machine_config_is_dmg_skip_boot_and_strict() {
    let config = MachineConfig::default();

    assert_eq!(config.console_model, ConsoleModel::Dmg);
    assert_eq!(config.startup_mode, StartupMode::SkipBoot);
    assert!(config.boot_rom_assets.is_empty());
    assert_eq!(config.compatibility.execution_mode, ExecutionMode::Strict);
    assert_eq!(
        config.compatibility.validation_policy,
        ValidationPolicy::Strict
    );
}

#[test]
fn public_model_api_exposes_dmg_and_cgb_families() {
    assert_eq!(ConsoleModel::Dmg0.family(), ConsoleFamily::Dmg);
    assert_eq!(ConsoleModel::Mgb.family(), ConsoleFamily::Dmg);
    assert_eq!(ConsoleModel::Cgb.family(), ConsoleFamily::Cgb);
}

#[test]
fn experimental_policy_keeps_future_cgb_extension_seams_explicit() {
    let config = MachineConfig::new(ConsoleModel::Cgb)
        .with_startup_mode(StartupMode::RealBoot)
        .with_compatibility(CompatibilityPolicy::experimental());

    assert_eq!(config.console_model, ConsoleModel::Cgb);
    assert_eq!(config.startup_mode, StartupMode::RealBoot);
    assert_eq!(
        config.compatibility.execution_mode,
        ExecutionMode::Experimental
    );
    assert_eq!(
        config.compatibility.heuristic_policy,
        HeuristicPolicy::AllowExperimental
    );
}

#[test]
fn startup_mode_and_execution_mode_expose_their_public_contracts() {
    assert!(!StartupMode::SkipBoot.requires_boot_rom());
    assert!(StartupMode::RealBoot.requires_boot_rom());

    assert!(ExecutionMode::Strict.is_oracle());
    assert!(!ExecutionMode::Permissive.is_oracle());
    assert!(!ExecutionMode::Experimental.is_oracle());
}

#[test]
fn override_policy_reports_when_explicit_overrides_exist() {
    assert!(!OverridePolicy::default().has_overrides());

    let model_override = OverridePolicy {
        forced_console_model: Some(ConsoleModel::Mgb),
        forced_startup_mode: None,
    };
    let startup_override = OverridePolicy {
        forced_console_model: None,
        forced_startup_mode: Some(StartupMode::RealBoot),
    };

    assert!(model_override.has_overrides());
    assert!(startup_override.has_overrides());
}

#[test]
fn default_compatibility_policy_matches_the_strict_preset() {
    assert_eq!(
        CompatibilityPolicy::default(),
        CompatibilityPolicy::strict()
    );
}

#[test]
fn machine_config_can_replace_the_full_compatibility_policy() {
    let compatibility = CompatibilityPolicy::experimental();
    let config = MachineConfig::new(ConsoleModel::Dmg0)
        .with_startup_mode(StartupMode::SkipBoot)
        .with_compatibility(compatibility.clone());

    assert_eq!(config.console_model, ConsoleModel::Dmg0);
    assert_eq!(config.startup_mode, StartupMode::SkipBoot);
    assert_eq!(
        config.compatibility.execution_mode,
        ExecutionMode::Experimental
    );
    assert_eq!(
        config.compatibility.validation_policy,
        ValidationPolicy::Warn
    );
    assert_eq!(
        config.compatibility.heuristic_policy,
        HeuristicPolicy::AllowExperimental
    );
    assert_eq!(
        config.compatibility.diagnostic_policy,
        DiagnosticPolicy::Verbose
    );
    assert_eq!(config.compatibility, compatibility);
}

#[test]
fn machine_config_can_carry_explicit_boot_rom_assets() {
    let boot_rom_assets = BootRomAssets::none()
        .with_bytes(BootRomKind::Dmg, vec![0xAA; 0x0100])
        .expect("dmg boot ROM asset should validate");
    let config = MachineConfig::new(ConsoleModel::Dmg).with_boot_rom_assets(boot_rom_assets);

    assert!(config.boot_rom_assets.has_image(BootRomKind::Dmg));
}
