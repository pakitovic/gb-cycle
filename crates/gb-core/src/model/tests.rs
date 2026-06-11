use super::*;

#[test]
fn console_models_keep_dmg_and_cgb_families_explicit() {
    assert!(ConsoleModel::GameBoy.is_dmg_family());
    assert!(ConsoleModel::GameBoyPocket.is_dmg_family());
    assert!(ConsoleModel::GameBoyLight.is_dmg_family());
    assert!(ConsoleModel::GameBoyColor.is_cgb_family());
    assert!(ConsoleModel::GameBoyAdvance.is_cgb_family());
    assert_eq!(
        ConsoleModel::GameBoy.default_operating_mode(),
        OperatingMode::Dmg
    );
    assert_eq!(
        ConsoleModel::GameBoyColor.default_operating_mode(),
        OperatingMode::Cgb
    );
    assert_eq!(
        ConsoleModel::GameBoyColor.default_revision(),
        HardwareRevision::CpuCgbE
    );
    assert_eq!(
        ConsoleModel::GameBoyAdvance.default_operating_mode(),
        OperatingMode::Cgb
    );
    assert_eq!(
        ConsoleModel::GameBoyAdvance.default_revision(),
        HardwareRevision::CpuAgbA
    );
    assert!(ConsoleModel::GameBoyColor.supports_operating_mode(OperatingMode::GbCompatible));
    assert!(ConsoleModel::GameBoyColor.supports_operating_mode(OperatingMode::CgbDmgExt));
    assert!(ConsoleModel::GameBoyAdvance.supports_operating_mode(OperatingMode::GbCompatible));
    assert!(ConsoleModel::GameBoyAdvance.supports_operating_mode(OperatingMode::CgbDmgExt));
    assert!(ConsoleModel::GameBoyColor.has_cgb_infrared_port());
    assert!(!ConsoleModel::GameBoyAdvance.has_cgb_infrared_port());
    assert!(!ConsoleModel::GameBoy.supports_operating_mode(OperatingMode::GbCompatible));
    assert!(!ConsoleModel::GameBoy.supports_operating_mode(OperatingMode::CgbDmgExt));
}

#[test]
fn console_models_publish_default_and_active_revisions() {
    assert_eq!(
        ConsoleModel::GameBoy.default_revision(),
        HardwareRevision::DmgCpuC
    );
    assert_eq!(
        ConsoleModel::GameBoy.active_revisions(),
        &[HardwareRevision::DmgCpu0, HardwareRevision::DmgCpuC]
    );
    assert_eq!(
        ConsoleModel::GameBoy.active_revisions_on_host(HostPlatform::Sgb),
        &[HardwareRevision::DmgCpuC]
    );
    assert_eq!(
        ConsoleModel::GameBoy.active_revisions_on_host(HostPlatform::Sgb2),
        &[HardwareRevision::DmgCpuC]
    );
    assert_eq!(
        ConsoleModel::GameBoyPocket.default_revision(),
        HardwareRevision::CpuMgb
    );
    assert_eq!(
        ConsoleModel::GameBoyLight.active_revisions(),
        &[HardwareRevision::CpuMgb]
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
    assert_eq!(
        ConsoleModel::GameBoyAdvance.active_revisions(),
        &[HardwareRevision::CpuAgb0, HardwareRevision::CpuAgbA]
    );
    assert!(ConsoleModel::GameBoy.supports_revision(HardwareRevision::DmgCpu0));
    assert!(
        !ConsoleModel::GameBoy
            .supports_revision_on_host(HostPlatform::Sgb, HardwareRevision::DmgCpu0)
    );
    assert!(
        ConsoleModel::GameBoy
            .supports_revision_on_host(HostPlatform::Sgb, HardwareRevision::DmgCpuC)
    );
    assert!(ConsoleModel::GameBoyColor.supports_revision(HardwareRevision::CpuCgb0));
    assert!(ConsoleModel::GameBoyColor.supports_revision(HardwareRevision::CpuCgbE));
    assert!(ConsoleModel::GameBoyAdvance.supports_revision(HardwareRevision::CpuAgb0));
    assert!(ConsoleModel::GameBoyAdvance.supports_revision(HardwareRevision::CpuAgbA));
    assert!(!ConsoleModel::GameBoy.supports_revision(HardwareRevision::CpuCgbE));
    assert!(!ConsoleModel::GameBoyColor.supports_revision(HardwareRevision::CpuAgbA));
}

#[test]
fn operating_modes_and_host_platforms_keep_silicon_and_host_axes_separate() {
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
fn experimental_cgb_header_policy_selects_dmg_ext_from_noncanonical_bit3_only() {
    assert_eq!(
        ConsoleModel::GameBoyColor.direct_boot_operating_mode_for_cgb_flag_with_heuristic(
            CgbFlag::SupportedNonCanonical(0x88),
            HeuristicPolicy::Disabled,
        ),
        OperatingMode::Cgb
    );

    for value in 0x88..=0x8F {
        assert_eq!(
            ConsoleModel::GameBoyColor.direct_boot_operating_mode_for_cgb_flag_with_heuristic(
                CgbFlag::SupportedNonCanonical(value),
                HeuristicPolicy::AllowExperimental,
            ),
            OperatingMode::CgbDmgExt,
            "header value {value:#04X} should request CGB DMG-ext experimentally"
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
        OperatingMode::CgbDmgExt
    );
    assert_eq!(
        ConsoleModel::GameBoy.direct_boot_operating_mode_for_cgb_flag_with_heuristic(
            CgbFlag::SupportedNonCanonical(0x88),
            HeuristicPolicy::AllowExperimental,
        ),
        OperatingMode::Dmg
    );
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
        OperatingMode::GbCompatible,
        HostPlatform::Handheld,
    );

    assert!(dmg.dmg_software_contract());
    assert!(cgb_compat.dmg_software_contract());
    assert!(dmg.dmg_family_quirks_enabled());
    assert!(!cgb_compat.dmg_family_quirks_enabled());
    assert!(!dmg.cgb_extensions_enabled());
    assert!(!cgb_compat.cgb_extensions_enabled());

    let cgb_dmg_ext = CapabilitySet::from_model_axes(
        ConsoleModel::GameBoyColor,
        OperatingMode::CgbDmgExt,
        HostPlatform::Handheld,
    );
    assert!(cgb_dmg_ext.dmg_software_contract());
    assert!(!cgb_dmg_ext.dmg_family_quirks_enabled());
    assert!(!cgb_dmg_ext.cgb_extensions_enabled());
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
    assert_eq!(config.revision, HardwareRevision::CpuMgb);
    assert_eq!(config.host_platform, HostPlatform::Sgb2);
    assert_eq!(config.sgb_profile, Some(SgbHostProfile::Sgb2Ntsc));
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
fn machine_config_tracks_revision_as_a_separate_axis() {
    let config = MachineConfig::new(ConsoleModel::GameBoyColor)
        .with_revision(HardwareRevision::CpuCgbE)
        .with_startup_mode(StartupMode::SkipBoot);

    assert_eq!(config.console_model, ConsoleModel::GameBoyColor);
    assert_eq!(config.revision, HardwareRevision::CpuCgbE);
    assert!(config.model_axes_are_coherent());
}

#[test]
fn hardware_revisions_derive_real_boot_images() {
    assert_eq!(
        HardwareRevision::DmgCpu0.boot_rom_filename(),
        "dmg0_boot.bin"
    );
    assert_eq!(
        HardwareRevision::DmgCpuC.boot_rom_filename(),
        "dmg_boot.bin"
    );
    assert_eq!(HardwareRevision::CpuMgb.boot_rom_filename(), "mgb_boot.bin");
    assert_eq!(
        HardwareRevision::CpuCgb0.boot_rom_filename(),
        "cgb0_boot.bin"
    );
    assert_eq!(
        HardwareRevision::CpuCgbD.boot_rom_filename(),
        "cgb_boot.bin"
    );
    assert_eq!(
        HardwareRevision::CpuCgbE.boot_rom_filename(),
        "cgbE_boot.bin"
    );
    assert_eq!(
        HardwareRevision::CpuAgb0.boot_rom_filename(),
        "cgb_agb0_boot.bin"
    );
    assert_eq!(
        HardwareRevision::CpuAgbA.boot_rom_filename(),
        "cgb_agb_boot.bin"
    );
    assert_eq!(HardwareRevision::DmgCpuC.boot_rom_expected_size(), 0x0100);
    assert_eq!(HardwareRevision::CpuCgbE.boot_rom_expected_size(), 0x0900);
    assert_eq!(HardwareRevision::CpuAgb0.boot_rom_expected_size(), 0x0900);
    assert_eq!(HardwareRevision::CpuAgbA.boot_rom_expected_size(), 0x0900);
    assert!(HardwareRevision::CpuAgb0.uses_cgb_boot_rom());
    assert!(HardwareRevision::CpuAgbA.uses_cgb_boot_rom());
}

#[test]
fn sgb_profiles_publish_timing_link_and_video_facts() {
    assert_eq!(SgbHostProfile::ALL.len(), 3);
    assert_eq!(
        SgbHostProfile::default_for_host_platform(HostPlatform::Sgb),
        Some(SgbHostProfile::SgbNtsc)
    );
    assert_eq!(
        SgbHostProfile::default_for_host_platform(HostPlatform::Sgb2),
        Some(SgbHostProfile::Sgb2Ntsc)
    );
    assert_eq!(
        SgbHostProfile::default_for_host_platform(HostPlatform::Handheld),
        None
    );

    let sgb_ntsc = SgbHostProfile::SgbNtsc.timing();
    assert_eq!(sgb_ntsc.source_master_clock_hz.rounded_hz(), 21_477_272);
    assert_eq!(sgb_ntsc.gb_clock_divisor, SGB_ICD2_CLOCK_DIVISOR);
    assert_eq!(sgb_ntsc.gb_master_clock_hz.rounded_hz(), 4_295_454);
    assert_eq!(sgb_ntsc.video_standard, SgbVideoStandard::Ntsc);
    assert!(!sgb_ntsc.corrected_clock);

    let sgb_pal = SgbHostProfile::SgbPal.timing();
    assert_eq!(sgb_pal.source_master_clock_hz.rounded_hz(), 21_281_370);
    assert_eq!(sgb_pal.gb_master_clock_hz.rounded_hz(), 4_256_274);
    assert_eq!(sgb_pal.video_standard, SgbVideoStandard::Pal);
    assert!(!SgbHostProfile::SgbPal.game_link_supported());

    let sgb2 = SgbHostProfile::Sgb2Ntsc.timing();
    assert_eq!(sgb2.source_master_clock_hz.rounded_hz(), 20_971_520);
    assert_eq!(sgb2.gb_master_clock_hz.rounded_hz(), DMG_MASTER_CLOCK_HZ);
    assert_eq!(sgb2.video_standard, SgbVideoStandard::Ntsc);
    assert!(sgb2.corrected_clock);
    assert!(SgbHostProfile::Sgb2Ntsc.game_link_supported());
}

#[test]
fn sgb_profile_selection_tracks_and_validates_host_platform() {
    let pal_sgb =
        MachineConfig::new(ConsoleModel::GameBoy).with_sgb_profile(SgbHostProfile::SgbPal);
    assert_eq!(pal_sgb.host_platform, HostPlatform::Sgb);
    assert_eq!(pal_sgb.sgb_profile, Some(SgbHostProfile::SgbPal));
    assert_eq!(pal_sgb.boot_rom_asset_kind(), BootRomAssetKind::Sgb);
    assert!(pal_sgb.model_axes_are_coherent());

    let sgb2 = MachineConfig::new(ConsoleModel::GameBoy).with_host_platform(HostPlatform::Sgb2);
    assert_eq!(sgb2.sgb_profile, Some(SgbHostProfile::Sgb2Ntsc));
    assert_eq!(sgb2.boot_rom_asset_kind(), BootRomAssetKind::Sgb2);
    assert!(sgb2.model_axes_are_coherent());

    let sgb_rejects_dmg0 = MachineConfig::new(ConsoleModel::GameBoy)
        .with_revision(HardwareRevision::DmgCpu0)
        .with_sgb_profile(SgbHostProfile::SgbNtsc);
    assert_eq!(sgb_rejects_dmg0.revision, HardwareRevision::DmgCpuC);
    assert!(!sgb_rejects_dmg0.uses_handheld_dmg0_revision());

    let mut manual_sgb_dmg0 = MachineConfig::new(ConsoleModel::GameBoy)
        .with_revision(HardwareRevision::DmgCpu0)
        .with_host_platform(HostPlatform::Sgb);
    manual_sgb_dmg0.revision = HardwareRevision::DmgCpu0;
    assert!(!manual_sgb_dmg0.model_axes_are_coherent());
    assert!(!manual_sgb_dmg0.uses_handheld_dmg0_revision());

    let mut impossible_pal_sgb2 = sgb2.clone();
    impossible_pal_sgb2.sgb_profile = Some(SgbHostProfile::SgbPal);
    assert!(!impossible_pal_sgb2.model_axes_are_coherent());

    let mut handheld_with_sgb_profile = MachineConfig::new(ConsoleModel::GameBoy);
    handheld_with_sgb_profile.sgb_profile = Some(SgbHostProfile::SgbNtsc);
    assert!(!handheld_with_sgb_profile.model_axes_are_coherent());
}

#[test]
fn with_console_model_preserves_an_explicit_operating_mode_override() {
    let config = MachineConfig::new(ConsoleModel::GameBoy)
        .with_operating_mode(OperatingMode::GbCompatible)
        .with_console_model(ConsoleModel::GameBoyColor);

    assert_eq!(config.console_model, ConsoleModel::GameBoyColor);
    assert_eq!(config.operating_mode, OperatingMode::GbCompatible);
}

#[test]
fn with_console_model_resets_axes_that_the_new_model_cannot_support() {
    let config = MachineConfig::new(ConsoleModel::GameBoyColor)
        .with_operating_mode(OperatingMode::Cgb)
        .with_revision(HardwareRevision::CpuCgbE)
        .with_console_model(ConsoleModel::GameBoy);

    assert_eq!(config.console_model, ConsoleModel::GameBoy);
    assert_eq!(config.operating_mode, OperatingMode::Dmg);
    assert_eq!(config.revision, HardwareRevision::DmgCpuC);
}

#[test]
fn machine_config_capability_set_tracks_the_three_model_axes() {
    let config = MachineConfig::new(ConsoleModel::GameBoyColor)
        .with_operating_mode(OperatingMode::GbCompatible)
        .with_host_platform(HostPlatform::Sgb);

    let capabilities = config.capability_set();

    assert!(config.model_axes_are_coherent());
    assert_eq!(capabilities.console_model(), ConsoleModel::GameBoyColor);
    assert_eq!(capabilities.console_family(), ConsoleFamily::Cgb);
    assert_eq!(capabilities.operating_mode(), OperatingMode::GbCompatible);
    assert_eq!(capabilities.host_platform(), HostPlatform::Sgb);
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
