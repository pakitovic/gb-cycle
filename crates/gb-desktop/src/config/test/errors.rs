use super::*;

#[test]
fn desktop_config_error_helpers_cover_conversion_display_and_sources() {
    let boot_error = DesktopConfigError::from(BootRomAssetError::DirectoryNotFound {
        path: PathBuf::from("/tmp/missing-bootrom"),
    });
    assert!(boot_error.to_string().contains("/tmp/missing-bootrom"));
    assert!(boot_error.source().is_some());

    let save_key_error = CartridgeSaveKey::new("bad/key".to_string())
        .expect_err("invalid save keys should fail validation");
    let save_error = DesktopConfigError::from(save_key_error);
    assert!(save_error.source().is_some());
    assert!(!save_error.to_string().is_empty());

    let derived = DesktopConfigError::SaveKeyDerivationEmpty {
        rom_path: PathBuf::from("/tmp/roms/..."),
    };
    assert!(derived.to_string().contains("explicit save key"));
    assert!(derived.source().is_none());
}

#[test]
fn launch_option_helpers_cover_strict_and_experimental_compatibility_modes() {
    let mut cgb = LaunchOptions {
        console_model: DesktopConsoleModel::GameBoyColor,
        revision: HardwareRevision::CpuCgbE,
        ..LaunchOptions::default()
    };
    assert_eq!(cgb.effective_revision(), HardwareRevision::CpuCgbE);
    cgb.console_model = DesktopConsoleModel::GameBoy;
    cgb.normalize_revision_for_model();
    assert_eq!(cgb.revision, HardwareRevision::default());

    let strict = LaunchOptions {
        execution_mode: ExecutionMode::Strict,
        ..LaunchOptions::default()
    };
    assert_eq!(strict.compatibility_policy(), CompatibilityPolicy::strict());

    let experimental = LaunchOptions {
        execution_mode: ExecutionMode::Experimental,
        ..LaunchOptions::default()
    };
    assert_eq!(
        experimental.compatibility_policy(),
        CompatibilityPolicy::experimental()
    );
}
