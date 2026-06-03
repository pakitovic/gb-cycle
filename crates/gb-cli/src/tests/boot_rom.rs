use super::*;

#[test]
fn boot_rom_path_resolution_and_verification_helpers_cover_host_side_paths() {
    let temp_dir = unique_temp_dir("boot-rom");
    let current_dir = temp_dir.join("cwd");
    let explicit_dir = temp_dir.join("explicit");
    let missing_dir = temp_dir.join("missing");
    let not_dir = temp_dir.join("not-a-dir");
    let short_dir = temp_dir.join("short");
    fs::create_dir_all(&current_dir).expect("cwd should be creatable");
    fs::create_dir_all(&explicit_dir).expect("explicit dir should be creatable");
    fs::create_dir_all(&short_dir).expect("short dir should be creatable");
    fs::write(&not_dir, b"file").expect("blocking file should be writable");
    write_fake_boot_rom(&explicit_dir, HardwareRevision::DmgCpuC, 0xAA);
    fs::write(
        short_dir.join(BootRomAssets::filename(HardwareRevision::DmgCpuC)),
        vec![0x00; 0x10],
    )
    .expect("short boot ROM image should be writable");

    let mut options = RunOptions::default_with_rom(PathBuf::from("demo.gb"));
    let mut stderr = Vec::new();
    let skip_boot = load_boot_rom_assets(&options, &current_dir, &mut stderr)
        .expect("skip-boot should not require assets");
    assert!(skip_boot.is_empty());
    assert!(stderr.is_empty());

    options.startup_mode = StartupMode::RealBoot;
    options.boot_rom_dir = Some(explicit_dir.clone());
    options.boot_rom_verify = BootRomVerificationMode::Warn;
    let warned_assets = load_boot_rom_assets(&options, &current_dir, &mut stderr)
        .expect("warn mode should still load assets");
    assert!(warned_assets.has_image(HardwareRevision::DmgCpuC));
    assert!(
        String::from_utf8(stderr.clone())
            .expect("stderr should be UTF-8")
            .contains("warning: boot ROM asset")
    );
    let mut failing_stderr = FailOnWrite {
        fail_on_write: Some(1),
        ..FailOnWrite::default()
    };
    let warning_write_error = load_boot_rom_assets(&options, &current_dir, &mut failing_stderr)
        .expect_err("warning write failures should surface");
    assert!(warning_write_error.contains("failed to write output"));

    stderr.clear();
    options.boot_rom_verify = BootRomVerificationMode::Off;
    let unchecked_assets = load_boot_rom_assets(&options, &current_dir, &mut stderr)
        .expect("off mode should skip verification");
    assert!(unchecked_assets.has_image(HardwareRevision::DmgCpuC));
    assert!(stderr.is_empty());

    write_fake_boot_rom_asset(&explicit_dir, BootRomAssetKind::Sgb, 0xBB);
    options.model = RunModel::SuperGameBoy;
    let sgb_assets = load_boot_rom_assets(&options, &current_dir, &mut stderr)
        .expect("SGB real-boot should resolve sgb_boot.bin");
    assert!(sgb_assets.has_asset(BootRomAssetKind::Sgb));

    write_fake_boot_rom_asset(&explicit_dir, BootRomAssetKind::Sgb2, 0xCC);
    options.model = RunModel::SuperGameBoy2;
    let sgb2_assets = load_boot_rom_assets(&options, &current_dir, &mut stderr)
        .expect("SGB2 real-boot should resolve sgb2_boot.bin");
    assert!(sgb2_assets.has_asset(BootRomAssetKind::Sgb2));
    options.model = RunModel::GameBoy;

    options.boot_rom_verify = BootRomVerificationMode::Strict;
    let strict_error = load_boot_rom_assets(&options, &current_dir, &mut Vec::new())
        .expect_err("strict verification should reject mismatched assets");
    assert!(strict_error.contains("unexpected sha256"));

    options.boot_rom_verify = BootRomVerificationMode::Off;
    options.boot_rom_dir = Some(missing_dir.clone());
    let missing_assets = load_boot_rom_assets(&options, &current_dir, &mut Vec::new())
        .expect("missing directories should resolve to no assets");
    assert!(missing_assets.is_empty());

    options.boot_rom_dir = Some(not_dir.clone());
    let directory_error = load_boot_rom_assets(&options, &current_dir, &mut Vec::new())
        .expect_err("file paths should fail directory validation");
    assert!(directory_error.contains("--boot-rom-dir expects a directory path"));

    options.boot_rom_dir = Some(short_dir.clone());
    let short_image_error = load_boot_rom_assets(&options, &current_dir, &mut Vec::new())
        .expect_err("short boot ROM images should fail directory loading");
    assert!(short_image_error.contains("failed to load boot ROM assets from"));
    assert!(short_image_error.contains("is too short"));

    assert_eq!(
        resolve_boot_rom_root(Some(Path::new("custom-assets")), &current_dir),
        Some(current_dir.join("custom-assets"))
    );
    assert_eq!(resolve_boot_rom_root(None, &current_dir), None);

    options.boot_rom_dir = None;
    options.boot_rom_verify = BootRomVerificationMode::Strict;
    let unconfigured_error = load_boot_rom_assets(&options, &current_dir, &mut Vec::new())
        .expect_err("strict real-boot should reject missing boot ROM configuration");
    assert_eq!(
        unconfigured_error,
        "boot ROM root is not configured; use --boot-rom-dir <dir>"
    );

    options.boot_rom_verify = BootRomVerificationMode::Off;
    let unconfigured_assets = load_boot_rom_assets(&options, &current_dir, &mut Vec::new())
        .expect("verification-off should allow unconfigured boot ROM roots");
    assert!(unconfigured_assets.is_empty());

    assert_eq!(
        resolve_path(&current_dir, Path::new("relative/demo.gb")),
        current_dir.join("relative/demo.gb")
    );
    assert_eq!(
        resolve_path(&current_dir, Path::new("/tmp/demo.gb")),
        PathBuf::from("/tmp/demo.gb")
    );
    validate_explicit_directory_input("--boot-rom-dir", None, &explicit_dir)
        .expect("missing explicit paths should be ignored");

    let missing_verify =
        verify_boot_rom_file(&temp_dir.join("missing.bin"), HardwareRevision::DmgCpuC)
            .expect_err("missing boot ROM files should fail");
    assert!(missing_verify.contains("failed to read boot ROM asset"));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}
