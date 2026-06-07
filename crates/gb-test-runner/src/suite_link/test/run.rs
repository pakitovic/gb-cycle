use std::fs;

use super::super::cli::run_suite_link_command_with_workspace_for_test;
use super::common::{
    copy_dmg04_basic_fixtures, unique_temp_dir, write_dmg04_manifest, write_reports,
};

#[test]
fn command_runs_selected_link_suite_and_writes_status() {
    let workspace = unique_temp_dir("run-pass");
    write_reports(&workspace);
    copy_dmg04_basic_fixtures(&workspace);
    write_dmg04_manifest(&workspace, "A5");
    let mut output = Vec::new();

    run_suite_link_command_with_workspace_for_test(
        ["linked", "--suite", "dmg04", "--threads", "2"],
        &workspace,
        &mut output,
    )
    .expect("suite should pass");

    let output = String::from_utf8(output).expect("output should be utf-8");
    assert!(output.contains("suite dmg04: running 1 cases"));
    assert!(output.contains("suite dmg04: 1/1 passed"));
    assert!(output.contains("case dmg04-basic-exchange: PASS after"));
    let status = fs::read_to_string(workspace.join("test/linked/.status/dmg04.toml"))
        .expect("status should be written");
    assert!(status.contains("suite_name = \"dmg04\""));
    assert!(status.contains("status = \"PASS\""));
}

#[test]
fn command_failure_generates_artifacts_and_pass_cleans_them() {
    let workspace = unique_temp_dir("failure-artifacts");
    write_reports(&workspace);
    copy_dmg04_basic_fixtures(&workspace);
    write_dmg04_manifest(&workspace, "00");
    let mut output = Vec::new();

    let error = run_suite_link_command_with_workspace_for_test(
        ["linked", "--suite", "dmg04"],
        &workspace,
        &mut output,
    )
    .expect_err("suite should fail");

    assert!(error.contains("one or more linked suite cases failed"));
    let artifact_dir = workspace.join("test/linked/.artifacts/dmg04/dmg04-basic-exchange");
    assert!(artifact_dir.join("failure.toml").is_file());
    assert!(artifact_dir.join("left/serial.hex.txt").is_file());
    let output = String::from_utf8(output).expect("output should be utf-8");
    assert!(output.contains("artifact_dir="));

    write_dmg04_manifest(&workspace, "A5");
    let mut output = Vec::new();
    run_suite_link_command_with_workspace_for_test(
        ["linked", "--suite", "dmg04"],
        &workspace,
        &mut output,
    )
    .expect("suite should pass after fixing expected serial");
    assert!(!artifact_dir.exists());
}

#[test]
fn command_rejects_manifest_real_boot_without_boot_rom_dir() {
    let workspace = unique_temp_dir("real-boot-missing-dir");
    write_reports(&workspace);
    copy_dmg04_basic_fixtures(&workspace);
    let linked_dir = workspace.join("crates/gb-test-runner/data/linked");
    fs::write(
        linked_dir.join("dmg04.link.suite.toml"),
        r#"report = "linked"
suite_name = "dmg04"
family = "linked"
topology = "dmg04"
timeout_tcycles = 8
startup = "real-boot"

[[case]]
id = "real-boot"
oracle = { type = "serial-hex-exact", target_participant = "left", expected = "" }

  [[case.participant]]
  id = "left"
  rom = "fixtures/dmg04/basic-left.gb"
  model = "dmg"

  [[case.participant]]
  id = "right"
  rom = "fixtures/dmg04/basic-right.gb"
  model = "dmg"
"#,
    )
    .expect("manifest should be written");
    let mut output = Vec::new();

    let error = run_suite_link_command_with_workspace_for_test(
        ["linked", "--suite", "dmg04"],
        &workspace,
        &mut output,
    )
    .expect_err("real boot without boot rom dir should fail");

    assert!(error.contains("pass --boot-rom-dir <dir>"));
}

#[test]
fn boot_rom_dir_forces_real_boot_and_validates_directory() {
    let workspace = unique_temp_dir("boot-rom-dir-forces-real-boot");
    write_reports(&workspace);
    copy_dmg04_basic_fixtures(&workspace);
    write_dmg04_manifest(&workspace, "A5");
    let mut output = Vec::new();

    let error = run_suite_link_command_with_workspace_for_test(
        [
            "linked",
            "--suite",
            "dmg04",
            "--boot-rom-dir",
            workspace
                .join("missing-boot-roms")
                .to_str()
                .expect("path should be utf-8"),
        ],
        &workspace,
        &mut output,
    )
    .expect_err("missing boot rom dir should fail");

    assert!(error.contains("failed to load boot ROM assets"));
    assert!(error.contains("does not exist"));
}

#[test]
fn boot_rom_dir_uses_participant_hardware_revision_for_asset_selection() {
    let workspace = unique_temp_dir("boot-rom-dir-cgb-d-revision");
    let boot_rom_dir = workspace.join("bootroms");
    fs::create_dir_all(&boot_rom_dir).expect("boot ROM dir should be creatable");
    write_reports(&workspace);
    let linked_dir = workspace.join("crates/gb-test-runner/data/linked");
    fs::create_dir_all(&linked_dir).expect("linked dir should be creatable");
    fs::write(
        linked_dir.join("cgb-ir.link.suite.toml"),
        r#"report = "linked"
suite_name = "cgb-ir"
family = "linked"
topology = "cgb-ir"
timeout_tcycles = 8

[[case]]
id = "cgb-ir-revision"
oracle = { type = "serial-hex-exact", target_participant = "receiver", expected = "" }

  [[case.participant]]
  id = "emitter"
  rom = "fixtures/cgb-ir/emitter.gbc"
  model = "cgb"
  revision = "cpu-cgb-d"

  [[case.participant]]
  id = "receiver"
  rom = "fixtures/cgb-ir/receiver.gbc"
  model = "cgb"
  revision = "cpu-cgb-e"
"#,
    )
    .expect("manifest should be written");
    let mut output = Vec::new();

    let error = run_suite_link_command_with_workspace_for_test(
        [
            "linked",
            "--suite",
            "cgb-ir",
            "--boot-rom-dir",
            boot_rom_dir.to_str().expect("path should be utf-8"),
        ],
        &workspace,
        &mut output,
    )
    .expect_err("forced real-boot should require the CGB-D boot ROM asset");

    assert!(error.contains("failed to load boot ROM assets"));
    assert!(error.contains("cgb_boot.bin"));

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}
