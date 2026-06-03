use super::*;

#[test]
fn machine_config_uses_the_same_execution_mode_presets_as_other_frontends() {
    let mut config = DesktopConfig::default();
    config.launch.execution_mode = ExecutionMode::Permissive;

    let machine_config = config
        .machine_config()
        .expect("skip-boot should not load firmware");

    assert_eq!(machine_config.console_model, ConsoleModel::GameBoy);
    assert_eq!(machine_config.revision, HardwareRevision::default());
    assert_eq!(machine_config.startup_mode, StartupMode::SkipBoot);
    assert_eq!(
        machine_config.compatibility.execution_mode,
        ExecutionMode::Permissive
    );
    assert_eq!(
        machine_config.compatibility.validation_policy,
        gb_core::ValidationPolicy::Warn
    );
}

#[test]
fn machine_config_applies_revision_for_selected_model() {
    let mut config = DesktopConfig::default();
    config.launch.console_model = DesktopConsoleModel::GameBoyColor;
    config.launch.revision = HardwareRevision::CpuCgbE;

    let cgb_config = config
        .machine_config()
        .expect("skip-boot should not load firmware");
    assert_eq!(cgb_config.console_model, ConsoleModel::GameBoyColor);
    assert_eq!(cgb_config.revision, HardwareRevision::CpuCgbE);

    config.launch.console_model = DesktopConsoleModel::GameBoy;
    let dmg_config = config
        .machine_config()
        .expect("skip-boot should not load firmware");
    assert_eq!(dmg_config.console_model, ConsoleModel::GameBoy);
    assert_eq!(dmg_config.revision, HardwareRevision::DmgCpuC);
}

#[test]
fn machine_config_maps_visible_sgb_models_to_dmg_core_plus_host_profile() {
    let mut config = DesktopConfig::default();
    config.launch.console_model = DesktopConsoleModel::SuperGameBoy;
    config.launch.revision = HardwareRevision::CpuCgbE;

    let sgb_config = config
        .machine_config()
        .expect("skip-boot should not load firmware");
    assert_eq!(sgb_config.console_model, ConsoleModel::GameBoy);
    assert_eq!(sgb_config.revision, HardwareRevision::DmgCpuC);
    assert_eq!(sgb_config.sgb_profile, Some(SgbHostProfile::SgbNtsc));
    assert_eq!(sgb_config.boot_rom_asset_kind(), BootRomAssetKind::Sgb);

    config.launch.sgb_video_standard = SgbVideoStandard::Pal;
    let sgb_pal_config = config
        .machine_config()
        .expect("skip-boot should not load firmware");
    assert_eq!(sgb_pal_config.sgb_profile, Some(SgbHostProfile::SgbPal));
    assert_eq!(sgb_pal_config.boot_rom_asset_kind(), BootRomAssetKind::Sgb);

    config.launch.console_model = DesktopConsoleModel::SuperGameBoy2;
    let sgb2_config = config
        .machine_config()
        .expect("skip-boot should not load firmware");
    assert_eq!(sgb2_config.console_model, ConsoleModel::GameBoy);
    assert_eq!(sgb2_config.revision, HardwareRevision::DmgCpuC);
    assert_eq!(sgb2_config.sgb_profile, Some(SgbHostProfile::Sgb2Ntsc));
    assert_eq!(sgb2_config.boot_rom_asset_kind(), BootRomAssetKind::Sgb2);
}
