use super::*;

#[test]
fn console_models_keep_dmg_and_cgb_families_explicit() {
    assert!(ConsoleModel::GameBoy.is_dmg_family());
    assert!(ConsoleModel::GameBoyPocket.is_dmg_family());
    assert!(ConsoleModel::GameBoyLight.is_dmg_family());
    assert!(ConsoleModel::GameBoyColor.is_cgb_family());
    assert_eq!(
        ConsoleModel::GameBoy.default_operating_mode(),
        OperatingMode::Dmg
    );
    assert_eq!(
        ConsoleModel::GameBoyColor.default_operating_mode(),
        OperatingMode::Cgb
    );
    assert!(ConsoleModel::GameBoyColor.supports_operating_mode(OperatingMode::CgbCompatibility));
    assert!(!ConsoleModel::GameBoy.supports_operating_mode(OperatingMode::CgbCompatibility));
}

#[test]
fn console_models_publish_default_and_allowed_boot_rom_kinds() {
    assert_eq!(
        ConsoleModel::GameBoy.default_boot_rom_kind(),
        BootRomKind::Dmg
    );
    assert_eq!(
        ConsoleModel::GameBoy.allowed_boot_rom_kinds(),
        &[BootRomKind::Dmg0, BootRomKind::Dmg]
    );
    assert_eq!(
        ConsoleModel::GameBoyPocket.default_boot_rom_kind(),
        BootRomKind::Mgb
    );
    assert_eq!(
        ConsoleModel::GameBoyLight.default_boot_rom_kind(),
        BootRomKind::Mgb
    );
    assert_eq!(
        ConsoleModel::GameBoyColor.allowed_boot_rom_kinds(),
        &[BootRomKind::Cgb0, BootRomKind::Cgb, BootRomKind::CgbE]
    );
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
        ConsoleModel::GameBoy,
        OperatingMode::Dmg,
        HostPlatform::Handheld,
    );
    let cgb_compat = CapabilitySet::from_model_axes(
        ConsoleModel::GameBoyColor,
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
        .with_console_model(ConsoleModel::GameBoyPocket)
        .with_host_platform(HostPlatform::Sgb2)
        .with_startup_mode(StartupMode::RealBoot)
        .with_execution_mode(ExecutionMode::Permissive);

    assert_eq!(config.console_model, ConsoleModel::GameBoyPocket);
    assert_eq!(config.operating_mode, OperatingMode::Dmg);
    assert_eq!(config.host_platform, HostPlatform::Sgb2);
    assert_eq!(config.startup_mode, StartupMode::RealBoot);
    assert_eq!(config.boot_rom_kind, BootRomKind::Mgb);
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
fn machine_config_tracks_boot_rom_kind_as_a_separate_axis() {
    let config = MachineConfig::new(ConsoleModel::GameBoy)
        .with_boot_rom_kind(BootRomKind::Dmg0)
        .with_startup_mode(StartupMode::SkipBoot);

    assert_eq!(config.console_model, ConsoleModel::GameBoy);
    assert_eq!(config.boot_rom_kind, BootRomKind::Dmg0);
    assert!(config.model_axes_are_coherent());
}

#[test]
fn with_console_model_preserves_an_explicit_operating_mode_override() {
    let config = MachineConfig::new(ConsoleModel::GameBoy)
        .with_operating_mode(OperatingMode::CgbCompatibility)
        .with_console_model(ConsoleModel::GameBoyColor);

    assert_eq!(config.console_model, ConsoleModel::GameBoyColor);
    assert_eq!(config.operating_mode, OperatingMode::CgbCompatibility);
}

#[test]
fn machine_config_capability_set_tracks_the_three_model_axes() {
    let config = MachineConfig::new(ConsoleModel::GameBoyColor)
        .with_operating_mode(OperatingMode::CgbCompatibility)
        .with_host_platform(HostPlatform::Sgb1);

    let capabilities = config.capability_set();

    assert!(config.model_axes_are_coherent());
    assert_eq!(capabilities.console_model(), ConsoleModel::GameBoyColor);
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
    let config = MachineConfig::new(ConsoleModel::GameBoy).with_operating_mode(OperatingMode::Cgb);

    assert!(!config.model_axes_are_coherent());
    assert_eq!(config.operating_mode, OperatingMode::Cgb);
}
