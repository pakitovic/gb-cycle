use gb_core::{
    BootRomAssets, CapabilitySet, CgbFlag, CompatibilityPolicy, ConsoleFamily, ConsoleModel,
    DiagnosticPolicy, ExecutionMode, HardwareRevision, HeuristicPolicy, HostPlatform,
    MachineConfig, OperatingMode, OverridePolicy, StartupMode, ValidationPolicy,
};

#[test]
fn default_machine_config_is_dmg_skip_boot_and_strict() {
    let config = MachineConfig::default();

    assert_eq!(config.console_model, ConsoleModel::GameBoy);
    assert_eq!(config.operating_mode, OperatingMode::Dmg);
    assert_eq!(config.revision, HardwareRevision::DmgCpuC);
    assert_eq!(config.host_platform, HostPlatform::Handheld);
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
    assert_eq!(ConsoleModel::GameBoy.family(), ConsoleFamily::Dmg);
    assert_eq!(ConsoleModel::GameBoyPocket.family(), ConsoleFamily::Dmg);
    assert_eq!(ConsoleModel::GameBoyLight.family(), ConsoleFamily::Dmg);
    assert_eq!(ConsoleModel::GameBoyColor.family(), ConsoleFamily::Cgb);
    assert_eq!(
        ConsoleModel::GameBoy.default_operating_mode(),
        OperatingMode::Dmg
    );
    assert_eq!(
        ConsoleModel::GameBoyColor.default_operating_mode(),
        OperatingMode::Cgb
    );
}

#[test]
fn public_model_api_exposes_revision_defaults_and_active_sets() {
    assert_eq!(
        ConsoleModel::GameBoy.default_revision(),
        HardwareRevision::DmgCpuC
    );
    assert_eq!(
        ConsoleModel::GameBoyPocket.active_revisions(),
        &[HardwareRevision::CpuMgb]
    );
    assert_eq!(
        ConsoleModel::GameBoyLight.default_revision(),
        HardwareRevision::CpuMgb
    );
    assert_eq!(
        ConsoleModel::GameBoyColor.default_revision(),
        HardwareRevision::CpuCgbE
    );
    assert_eq!(
        ConsoleModel::GameBoyColor.active_revisions(),
        &[
            HardwareRevision::CpuCgb0,
            HardwareRevision::CpuCgbC,
            HardwareRevision::CpuCgbD,
            HardwareRevision::CpuCgbE
        ]
    );
    assert!(ConsoleModel::GameBoyColor.supports_revision(HardwareRevision::CpuCgb0));
    assert!(ConsoleModel::GameBoyColor.supports_revision(HardwareRevision::CpuCgbE));
    assert_eq!(
        ConsoleModel::GameBoyAdvance.default_revision(),
        HardwareRevision::CpuAgbA
    );
    assert_eq!(
        ConsoleModel::GameBoyAdvance.active_revisions(),
        &[HardwareRevision::CpuAgb0, HardwareRevision::CpuAgbA]
    );
    assert!(ConsoleModel::GameBoyAdvance.supports_revision(HardwareRevision::CpuAgb0));
    assert!(ConsoleModel::GameBoyAdvance.supports_revision(HardwareRevision::CpuAgbA));
    assert!(!ConsoleModel::GameBoy.supports_revision(HardwareRevision::CpuCgbE));
    assert_eq!(
        HardwareRevision::CpuCgbE.boot_rom_filename(),
        "cgbE_boot.bin"
    );
    assert_eq!(
        HardwareRevision::CpuAgb0.boot_rom_filename(),
        "cgb_agb0_boot.bin"
    );
}

#[test]
fn direct_boot_cgb_header_policy_keeps_model_and_mode_axes_separate() {
    assert_eq!(
        ConsoleModel::GameBoyColor.direct_boot_operating_mode_for_cgb_flag(CgbFlag::None),
        OperatingMode::GbCompatible
    );
    assert_eq!(
        ConsoleModel::GameBoyColor.direct_boot_operating_mode_for_cgb_flag(CgbFlag::Unknown(0x42)),
        OperatingMode::GbCompatible
    );
    assert_eq!(
        ConsoleModel::GameBoyColor.direct_boot_operating_mode_for_cgb_flag(CgbFlag::Supported),
        OperatingMode::Cgb
    );
    assert_eq!(
        ConsoleModel::GameBoyColor.direct_boot_operating_mode_for_cgb_flag(CgbFlag::Only),
        OperatingMode::Cgb
    );
    assert_eq!(
        ConsoleModel::GameBoyColor
            .direct_boot_operating_mode_for_cgb_flag(CgbFlag::SupportedNonCanonical(0x88)),
        OperatingMode::Cgb
    );
    assert_eq!(
        ConsoleModel::GameBoyColor
            .direct_boot_operating_mode_for_cgb_flag(CgbFlag::SupportedNonCanonical(0x8C)),
        OperatingMode::Cgb
    );
    assert_eq!(
        ConsoleModel::GameBoy.direct_boot_operating_mode_for_cgb_flag(CgbFlag::Supported),
        OperatingMode::Dmg
    );
}

#[test]
fn experimental_cgb_header_policy_maps_dmg_ext_without_changing_strict_policy() {
    for value in 0x88..=0x8F {
        assert_eq!(
            ConsoleModel::GameBoyColor.direct_boot_operating_mode_for_cgb_flag_with_heuristic(
                CgbFlag::SupportedNonCanonical(value),
                HeuristicPolicy::AllowExperimental,
            ),
            OperatingMode::CgbDmgExt,
            "experimental header value {value:#04X} should request CGB DMG-ext"
        );
        assert_eq!(
            ConsoleModel::GameBoyColor.direct_boot_operating_mode_for_cgb_flag_with_heuristic(
                CgbFlag::SupportedNonCanonical(value),
                HeuristicPolicy::Disabled,
            ),
            OperatingMode::Cgb,
            "strict header value {value:#04X} must keep the current native-CGB fallback"
        );
    }

    assert_eq!(
        ConsoleModel::GameBoyColor.direct_boot_operating_mode_for_cgb_flag_with_heuristic(
            CgbFlag::SupportedNonCanonical(0x84),
            HeuristicPolicy::AllowExperimental,
        ),
        OperatingMode::GbCompatible
    );
    assert_eq!(
        ConsoleModel::GameBoyColor.direct_boot_operating_mode_for_cgb_flag_with_heuristic(
            CgbFlag::SupportedNonCanonical(0x8C),
            HeuristicPolicy::AllowExperimental,
        ),
        OperatingMode::CgbDmgExt,
        "bit 3 wins over bit 2 under the experimental policy"
    );
}

#[test]
fn cgb_flag_reports_native_mode_request_without_implying_special_hardware() {
    assert!(!CgbFlag::None.enables_cgb_native_mode());
    assert!(!CgbFlag::Unknown(0x42).enables_cgb_native_mode());
    assert!(CgbFlag::Supported.enables_cgb_native_mode());
    assert!(CgbFlag::Only.enables_cgb_native_mode());
    assert!(CgbFlag::SupportedNonCanonical(0xA0).enables_cgb_native_mode());
    assert!(CgbFlag::Only.is_cgb_only());
    assert!(!CgbFlag::SupportedNonCanonical(0xA0).is_cgb_only());
    assert!(CgbFlag::SupportedNonCanonical(0x88).requests_cgb_dmg_ext_mode());
    assert!(!CgbFlag::SupportedNonCanonical(0x84).requests_cgb_dmg_ext_mode());
    assert!(CgbFlag::SupportedNonCanonical(0x84).requests_cgb_dmg_compatibility_mode());
    assert!(!CgbFlag::SupportedNonCanonical(0x88).requests_cgb_dmg_compatibility_mode());
}

#[test]
fn public_model_api_exposes_operating_modes_and_host_platforms() {
    assert!(OperatingMode::Dmg.uses_dmg_software_contract());
    assert!(OperatingMode::GbCompatible.uses_dmg_software_contract());
    assert!(OperatingMode::CgbDmgExt.uses_dmg_software_contract());
    assert!(!OperatingMode::Cgb.uses_dmg_software_contract());
    assert!(OperatingMode::Cgb.enables_cgb_extensions());
    assert!(!OperatingMode::GbCompatible.enables_cgb_extensions());
    assert!(!OperatingMode::CgbDmgExt.enables_cgb_extensions());
    assert!(OperatingMode::CgbDmgExt.enables_cgb_speed_switch());
    assert!(OperatingMode::CgbDmgExt.enables_cgb_high_speed_serial());
    assert!(OperatingMode::CgbDmgExt.enables_cgb_infrared_register());
    assert!(HostPlatform::Sgb.is_sgb());
    assert!(HostPlatform::Sgb2.is_sgb());
    assert!(!HostPlatform::Handheld.is_sgb());
}

#[test]
fn experimental_policy_keeps_future_cgb_extension_seams_explicit() {
    let config = MachineConfig::new(ConsoleModel::GameBoyColor)
        .with_operating_mode(OperatingMode::GbCompatible)
        .with_host_platform(HostPlatform::Sgb)
        .with_startup_mode(StartupMode::RealBoot)
        .with_compatibility(CompatibilityPolicy::experimental());

    assert_eq!(config.console_model, ConsoleModel::GameBoyColor);
    assert_eq!(config.operating_mode, OperatingMode::GbCompatible);
    assert_eq!(config.host_platform, HostPlatform::Sgb);
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
    assert!(!StartupMode::CustomBoot.requires_boot_rom());
    assert!(StartupMode::RealBoot.requires_boot_rom());
    assert!(StartupMode::SkipBoot.uses_direct_boot_state());
    assert!(StartupMode::CustomBoot.uses_direct_boot_state());
    assert!(!StartupMode::RealBoot.uses_direct_boot_state());

    assert!(ExecutionMode::Strict.is_oracle());
    assert!(!ExecutionMode::Permissive.is_oracle());
    assert!(!ExecutionMode::Experimental.is_oracle());
}

#[test]
fn override_policy_reports_when_explicit_overrides_exist() {
    assert!(!OverridePolicy::default().has_overrides());

    let model_override = OverridePolicy {
        forced_console_model: Some(ConsoleModel::GameBoyPocket),
        forced_operating_mode: None,
        forced_host_platform: None,
        forced_startup_mode: None,
    };
    let startup_override = OverridePolicy {
        forced_console_model: None,
        forced_operating_mode: None,
        forced_host_platform: None,
        forced_startup_mode: Some(StartupMode::RealBoot),
    };
    let operating_mode_override = OverridePolicy {
        forced_console_model: None,
        forced_operating_mode: Some(OperatingMode::GbCompatible),
        forced_host_platform: None,
        forced_startup_mode: None,
    };
    let host_platform_override = OverridePolicy {
        forced_console_model: None,
        forced_operating_mode: None,
        forced_host_platform: Some(HostPlatform::Sgb2),
        forced_startup_mode: None,
    };

    assert!(model_override.has_overrides());
    assert!(startup_override.has_overrides());
    assert!(operating_mode_override.has_overrides());
    assert!(host_platform_override.has_overrides());
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
    let config = MachineConfig::new(ConsoleModel::GameBoy)
        .with_host_platform(HostPlatform::Sgb)
        .with_startup_mode(StartupMode::SkipBoot)
        .with_compatibility(compatibility.clone());

    assert_eq!(config.console_model, ConsoleModel::GameBoy);
    assert_eq!(config.operating_mode, OperatingMode::Dmg);
    assert_eq!(config.host_platform, HostPlatform::Sgb);
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
        .with_bytes(HardwareRevision::DmgCpuC, vec![0xAA; 0x0100])
        .expect("dmg boot ROM asset should validate");
    let config = MachineConfig::new(ConsoleModel::GameBoy).with_boot_rom_assets(boot_rom_assets);

    assert!(config.boot_rom_assets.has_image(HardwareRevision::DmgCpuC));
}

#[test]
fn capability_sets_keep_silicon_mode_and_host_axes_distinct() {
    let dmg = CapabilitySet::from_model_axes(
        ConsoleModel::GameBoy,
        OperatingMode::Dmg,
        HostPlatform::Handheld,
    );
    let cgb_compat = CapabilitySet::from_model_axes(
        ConsoleModel::GameBoyColor,
        OperatingMode::GbCompatible,
        HostPlatform::Sgb2,
    );

    assert_eq!(dmg.console_family(), ConsoleFamily::Dmg);
    assert_eq!(cgb_compat.console_family(), ConsoleFamily::Cgb);
    assert!(dmg.dmg_software_contract());
    assert!(cgb_compat.dmg_software_contract());
    assert!(dmg.dmg_family_quirks_enabled());
    assert!(!cgb_compat.dmg_family_quirks_enabled());
    assert!(!dmg.cgb_extensions_enabled());
    assert!(!cgb_compat.cgb_extensions_enabled());
    assert!(!dmg.sgb_enhancements_enabled());
    assert!(cgb_compat.sgb_enhancements_enabled());
}

#[test]
fn machine_config_reports_when_requested_axes_are_incoherent() {
    let config = MachineConfig::new(ConsoleModel::GameBoy).with_operating_mode(OperatingMode::Cgb);

    assert!(!config.model_axes_are_coherent());
    assert_eq!(config.operating_mode, OperatingMode::Cgb);
}
