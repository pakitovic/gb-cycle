use super::*;

#[test]
fn console_models_keep_dmg_and_cgb_families_explicit() {
    assert!(ConsoleModel::Dmg0.is_dmg_family());
    assert!(ConsoleModel::Dmg.is_dmg_family());
    assert!(ConsoleModel::Mgb.is_dmg_family());
    assert!(ConsoleModel::Cgb.is_cgb_family());
    assert_eq!(
        ConsoleModel::Dmg.default_operating_mode(),
        OperatingMode::Dmg
    );
    assert_eq!(
        ConsoleModel::Cgb.default_operating_mode(),
        OperatingMode::Cgb
    );
    assert!(ConsoleModel::Cgb.supports_operating_mode(OperatingMode::CgbCompatibility));
    assert!(!ConsoleModel::Dmg.supports_operating_mode(OperatingMode::CgbCompatibility));
}

#[test]
fn operating_modes_and_host_platforms_keep_silicon_and_host_axes_separate() {
    assert!(OperatingMode::Dmg.uses_dmg_software_contract());
    assert!(OperatingMode::CgbCompatibility.uses_dmg_software_contract());
    assert!(!OperatingMode::Cgb.uses_dmg_software_contract());
    assert!(OperatingMode::Cgb.enables_cgb_extensions());
    assert!(!OperatingMode::CgbCompatibility.enables_cgb_extensions());
    assert!(HostPlatform::Sgb1.is_sgb());
    assert!(HostPlatform::Sgb2.is_sgb());
    assert!(!HostPlatform::Handheld.is_sgb());
}

#[test]
fn capability_sets_distinguish_cgb_compatibility_from_dmg_silicon() {
    let dmg = CapabilitySet::from_model_axes(
        ConsoleModel::Dmg,
        OperatingMode::Dmg,
        HostPlatform::Handheld,
    );
    let cgb_compat = CapabilitySet::from_model_axes(
        ConsoleModel::Cgb,
        OperatingMode::CgbCompatibility,
        HostPlatform::Handheld,
    );

    assert!(dmg.dmg_software_contract());
    assert!(cgb_compat.dmg_software_contract());
    assert!(dmg.dmg_family_quirks_enabled());
    assert!(!cgb_compat.dmg_family_quirks_enabled());
    assert!(!dmg.cgb_extensions_enabled());
    assert!(!cgb_compat.cgb_extensions_enabled());
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
        .with_host_platform(HostPlatform::Sgb2)
        .with_startup_mode(StartupMode::RealBoot)
        .with_execution_mode(ExecutionMode::Permissive);

    assert_eq!(config.console_model, ConsoleModel::Mgb);
    assert_eq!(config.operating_mode, OperatingMode::Dmg);
    assert_eq!(config.host_platform, HostPlatform::Sgb2);
    assert_eq!(config.startup_mode, StartupMode::RealBoot);
    assert_eq!(
        config.compatibility.execution_mode,
        ExecutionMode::Permissive
    );
    assert_eq!(
        config.compatibility.validation_policy,
        ValidationPolicy::Strict
    );
    assert!(config.boot_rom_assets.is_empty());
}

#[test]
fn with_console_model_preserves_an_explicit_operating_mode_override() {
    let config = MachineConfig::new(ConsoleModel::Dmg)
        .with_operating_mode(OperatingMode::CgbCompatibility)
        .with_console_model(ConsoleModel::Cgb);

    assert_eq!(config.console_model, ConsoleModel::Cgb);
    assert_eq!(config.operating_mode, OperatingMode::CgbCompatibility);
}

#[test]
fn machine_config_capability_set_tracks_the_three_model_axes() {
    let config = MachineConfig::new(ConsoleModel::Cgb)
        .with_operating_mode(OperatingMode::CgbCompatibility)
        .with_host_platform(HostPlatform::Sgb1);

    let capabilities = config.capability_set();

    assert!(config.model_axes_are_coherent());
    assert_eq!(capabilities.console_model(), ConsoleModel::Cgb);
    assert_eq!(capabilities.console_family(), ConsoleFamily::Cgb);
    assert_eq!(
        capabilities.operating_mode(),
        OperatingMode::CgbCompatibility
    );
    assert_eq!(capabilities.host_platform(), HostPlatform::Sgb1);
    assert!(capabilities.dmg_software_contract());
    assert!(!capabilities.cgb_extensions_enabled());
    assert!(!capabilities.dmg_family_quirks_enabled());
    assert!(capabilities.sgb_enhancements_enabled());
}

#[test]
fn incoherent_model_axes_stay_detectable_without_hiding_the_requested_shape() {
    let config = MachineConfig::new(ConsoleModel::Dmg).with_operating_mode(OperatingMode::Cgb);

    assert!(!config.model_axes_are_coherent());
    assert_eq!(config.operating_mode, OperatingMode::Cgb);
}
