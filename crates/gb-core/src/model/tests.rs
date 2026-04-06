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
    assert!(config.boot_rom_assets.is_empty());
}
