use std::fs;

use gb_core::{BootRomAssetKind, BootRomAssets};

use super::super::cli::run_suite_command_with_workspace_for_test;
use super::super::manifest::{load_reports, load_selected_suites};
use super::super::run::{SuiteRunConfig, run_suite_with_config};
use super::common::{
    basic_manifest, build_delayed_dmg_handoff_boot_rom, build_fibonacci_result_rom,
    build_infinite_loop_rom, build_joypad_a_pressed_memory_write_rom, build_mbc3_rtc_wait_rom,
    build_memory_write_rom, build_serial_text_rom, commit_upstream_repo, sha256_hex,
    unique_temp_dir, write_grayscale_png, write_manifest, write_materialized_source_manifest,
    write_reports, write_source_manifest,
};

#[test]
fn command_runs_serial_suite_and_writes_status() {
    let workspace = unique_temp_dir("serial-pass");
    write_reports(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
    );
    write_manifest(
        &workspace,
        "sample-report/blargg-cpu-instrs.suite.toml",
        &basic_manifest(
            "sample-report",
            "blargg-cpu-instrs",
            "blargg",
            "blargg-cpu-instrs-01-special",
            "cpu_instrs/01-special.gb",
        )
        .replace(
            "model = \"dmg\"",
            "model = \"dmg\"\nreport_model_suffix = true",
        ),
    );
    write_manifest(
        &workspace,
        "sample-report/acid.toml",
        &basic_manifest(
            "sample-report",
            "acid",
            "acid",
            "acid-cgb-acid2",
            "cgb-acid2.gbc",
        )
        .replace("model = \"dmg\"", "model = \"cgb\"")
        .replace("serial-contains", "framebuffer-fixture"),
    );
    let rom_path = workspace.join("test/sample-report/blargg/cpu_instrs/01-special.gb");
    fs::create_dir_all(rom_path.parent().expect("rom should have parent"))
        .expect("rom parent should be creatable");
    fs::write(&rom_path, build_serial_text_rom("Passed")).expect("rom should be writable");
    write_materialized_source_manifest(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
        &[("blargg", "blargg")],
    );

    let mut output = Vec::new();
    run_suite_command_with_workspace_for_test(
        [
            "sample-report",
            "--suite",
            "blargg-cpu-instrs",
            "--case",
            "blargg-cpu-instrs-01-special",
        ],
        &workspace,
        &mut output,
    )
    .expect("suite should pass");

    let output = String::from_utf8(output).expect("output should be utf-8");
    assert!(output.contains("suite blargg-cpu-instrs: 1/1 passed"));
    assert!(output.contains("case blargg-cpu-instrs-01-special: PASS after"));
    let status =
        fs::read_to_string(workspace.join("test/sample-report/.status/blargg-cpu-instrs.json"))
            .expect("status should be written");
    assert!(status.contains("\"suite_name\": \"blargg-cpu-instrs\""));
    assert!(status.contains("\"rom\": \"cpu_instrs/01-special.gb (DMG)\""));
    assert!(status.contains("\"status\": \"PASS\""));

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn command_clears_selected_suite_status_and_artifacts_before_running() {
    let workspace = unique_temp_dir("clean-selected-suite-runtime");
    write_reports(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
    );
    write_manifest(
        &workspace,
        "sample-report/blargg-cpu-instrs.suite.toml",
        &basic_manifest(
            "sample-report",
            "blargg-cpu-instrs",
            "blargg",
            "blargg-cpu-instrs-01-special",
            "cpu_instrs/01-special.gb",
        ),
    );
    write_manifest(
        &workspace,
        "sample-report/docboy-dmg-link.link.suite.toml",
        "this is intentionally not a single-machine suite manifest",
    );
    let rom_path = workspace.join("test/sample-report/blargg/cpu_instrs/01-special.gb");
    fs::create_dir_all(rom_path.parent().expect("rom should have parent"))
        .expect("rom parent should be creatable");
    fs::write(&rom_path, build_serial_text_rom("Passed")).expect("rom should be writable");
    write_materialized_source_manifest(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
        &[("blargg", "blargg")],
    );
    let selected_status = workspace.join("test/sample-report/.status/blargg-cpu-instrs.json");
    fs::create_dir_all(selected_status.parent().expect("status should have parent"))
        .expect("stale status parent should be creatable");
    fs::write(
        &selected_status,
        r#"{
  "suite_name": "blargg-cpu-instrs",
  "family": "stale",
  "cases": [
    {
      "rom": "stale.gb",
      "status": "PASS"
    }
  ]
}
"#,
    )
    .expect("stale status should be writable");
    let selected_artifact =
        workspace.join("test/sample-report/.artifacts/blargg-cpu-instrs/stale-case/old.txt");
    fs::create_dir_all(
        selected_artifact
            .parent()
            .expect("artifact should have parent"),
    )
    .expect("stale artifact parent should be creatable");
    fs::write(&selected_artifact, "stale").expect("stale artifact should be writable");
    let linked_status = workspace.join("test/sample-report/.status/docboy-dmg-link.json");
    fs::write(
        &linked_status,
        r#"{
  "suite_name": "docboy-dmg-link",
  "family": "docboy-dmg",
  "cases": [
    {
      "id": "linked-case",
      "status": "PASS"
    }
  ]
}
"#,
    )
    .expect("linked status should be writable");
    let linked_artifact =
        workspace.join("test/sample-report/.artifacts/docboy-dmg-link/linked-case/old.txt");
    fs::create_dir_all(
        linked_artifact
            .parent()
            .expect("linked artifact should have parent"),
    )
    .expect("linked artifact parent should be creatable");
    fs::write(&linked_artifact, "linked").expect("linked artifact should be writable");

    let mut output = Vec::new();
    run_suite_command_with_workspace_for_test(["sample-report"], &workspace, &mut output)
        .expect("suite should pass after clearing selected suite runtime dirs");

    let status = fs::read_to_string(&selected_status).expect("selected status should be rewritten");
    assert!(status.contains("\"rom\": \"cpu_instrs/01-special.gb\""));
    assert!(!status.contains("stale.gb"));
    assert!(!selected_artifact.exists());
    assert!(linked_status.is_file());
    assert!(linked_artifact.is_file());
    let output = String::from_utf8(output).expect("output should be utf-8");
    assert!(output.contains("suite blargg-cpu-instrs: 1/1 passed"));

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn command_preserves_report_status_and_artifacts_when_selection_is_invalid() {
    let workspace = unique_temp_dir("invalid-selection-preserves-runtime");
    write_reports(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
    );
    write_manifest(
        &workspace,
        "sample-report/blargg-cpu-instrs.suite.toml",
        &basic_manifest(
            "sample-report",
            "blargg-cpu-instrs",
            "blargg",
            "blargg-cpu-instrs-01-special",
            "cpu_instrs/01-special.gb",
        ),
    );
    let stale_status = workspace.join("test/sample-report/.status/stale-suite.json");
    fs::create_dir_all(stale_status.parent().expect("status should have parent"))
        .expect("stale status parent should be creatable");
    fs::write(
        &stale_status,
        r#"{
  "suite_name": "stale-suite",
  "family": "stale",
  "cases": [
    {
      "rom": "stale.gb",
      "status": "PASS"
    }
  ]
}
"#,
    )
    .expect("stale status should be writable");
    let stale_artifact =
        workspace.join("test/sample-report/.artifacts/stale-suite/stale-case/old.txt");
    fs::create_dir_all(
        stale_artifact
            .parent()
            .expect("artifact should have parent"),
    )
    .expect("stale artifact parent should be creatable");
    fs::write(&stale_artifact, "stale").expect("stale artifact should be writable");

    let mut output = Vec::new();
    let unknown_suite = run_suite_command_with_workspace_for_test(
        ["sample-report", "--suite", "missing-suite"],
        &workspace,
        &mut output,
    )
    .expect_err("unknown suite should fail before cleanup");
    assert!(unknown_suite.contains("unknown suite \"missing-suite\""));
    assert!(stale_status.is_file());
    assert!(stale_artifact.is_file());

    let mut output = Vec::new();
    let unknown_case = run_suite_command_with_workspace_for_test(
        [
            "sample-report",
            "--suite",
            "blargg-cpu-instrs",
            "--case",
            "missing-case",
        ],
        &workspace,
        &mut output,
    )
    .expect_err("unknown case should fail before cleanup");
    assert!(unknown_case.contains("unknown case \"missing-case\""));
    assert!(stale_status.is_file());
    assert!(stale_artifact.is_file());

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn command_rejects_unsafe_report_runtime_paths_before_cleanup() {
    let workspace = unique_temp_dir("unsafe-runtime-paths-preserve-store");
    let reports_path = workspace.join("crates/gb-test-runner/data/reports.toml");
    fs::create_dir_all(reports_path.parent().expect("reports should have parent"))
        .expect("reports parent should be creatable");
    fs::write(
        &reports_path,
        r#"status_dir = ""
artifact_dir = ".artifacts"

[[report]]
id = "sample-report"
store_dir = "sample-report"
sources = "sample-report/sources.report.toml"
"#,
    )
    .expect("reports should be writable");
    let materialized_rom = workspace.join("test/sample-report/blargg/cpu_instrs/01-special.gb");
    fs::create_dir_all(materialized_rom.parent().expect("rom should have parent"))
        .expect("rom parent should be creatable");
    fs::write(&materialized_rom, build_serial_text_rom("Passed")).expect("rom should be writable");

    let mut output = Vec::new();
    let error =
        run_suite_command_with_workspace_for_test(["sample-report"], &workspace, &mut output)
            .expect_err("unsafe report runtime path should fail before cleanup");

    assert!(error.contains("report default status_dir must not be empty"));
    assert!(materialized_rom.is_file());

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn command_local_report_ignores_link_suite_manifests_without_fetching() {
    let workspace = unique_temp_dir("local-report-link-manifests");
    write_local_report(&workspace, "linked");
    write_manifest(
        &workspace,
        "linked/dmg04.link.suite.toml",
        "this is intentionally invalid as a single-machine manifest",
    );

    let mut output = Vec::new();
    let error = run_suite_command_with_workspace_for_test(["linked"], &workspace, &mut output)
        .expect_err("link suite manifests should be ignored by rom-suite");

    assert!(output.is_empty());
    assert!(error.contains("does not contain suite manifests"));
    assert!(
        !workspace.join("test/linked").exists(),
        "local report without single-machine manifests should not materialize or run anything"
    );

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn command_auto_fetches_missing_suite_family_before_running() {
    let workspace = unique_temp_dir("auto-fetch-rom-workspace");
    let upstream = unique_temp_dir("auto-fetch-rom-upstream");
    let rom_bytes = build_serial_text_rom("Passed");
    let upstream_rom = upstream.join("roms/blargg/cpu_instrs/01-special.gb");
    fs::create_dir_all(
        upstream_rom
            .parent()
            .expect("upstream ROM should have parent"),
    )
    .expect("upstream ROM parent should be creatable");
    fs::write(&upstream_rom, &rom_bytes).expect("upstream ROM should be writable");
    let commit = commit_upstream_repo(&upstream);
    write_reports(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
    );
    write_source_manifest(
        &workspace,
        "sample-report/sources.report.toml",
        &format!(
            concat!(
                "[[source]]\n",
                "id = \"local-source\"\n",
                "git_url = {:?}\n",
                "git_rev = {:?}\n",
                "\n",
                "[[source.family]]\n",
                "id = \"blargg\"\n",
                "target_root = \"blargg\"\n",
                "sparse_paths = [\"roms/blargg\"]\n",
                "\n",
                "[[source.family.file]]\n",
                "path = \"roms/blargg/cpu_instrs/01-special.gb\"\n",
                "target = \"cpu_instrs/01-special.gb\"\n",
                "sha256 = {:?}\n",
            ),
            upstream.display().to_string(),
            commit,
            sha256_hex(&rom_bytes)
        ),
    );
    write_manifest(
        &workspace,
        "sample-report/blargg-cpu-instrs.suite.toml",
        &basic_manifest(
            "sample-report",
            "blargg-cpu-instrs",
            "blargg",
            "blargg-cpu-instrs-01-special",
            "cpu_instrs/01-special.gb",
        ),
    );

    let mut output = Vec::new();
    run_suite_command_with_workspace_for_test(
        ["sample-report", "--suite", "blargg-cpu-instrs"],
        &workspace,
        &mut output,
    )
    .expect("suite should fetch and pass");

    assert!(
        workspace
            .join("test/sample-report/blargg/cpu_instrs/01-special.gb")
            .exists()
    );
    let output = String::from_utf8(output).expect("output should be utf-8");
    assert!(output.contains("test ROM family blargg requires materialization"));
    assert!(output.contains("materialized test ROM families blargg"));
    assert!(output.contains("suite blargg-cpu-instrs: 1/1 passed"));

    fs::remove_dir_all(workspace).expect("workspace should be removable");
    fs::remove_dir_all(upstream).expect("upstream should be removable");
}

#[test]
fn command_auto_fetches_stale_suite_family_when_materialized_hash_changes() {
    let workspace = unique_temp_dir("auto-fetch-stale-workspace");
    let upstream = unique_temp_dir("auto-fetch-stale-upstream");
    let rom_bytes = build_serial_text_rom("Passed");
    let upstream_rom = upstream.join("roms/blargg/cpu_instrs/01-special.gb");
    fs::create_dir_all(
        upstream_rom
            .parent()
            .expect("upstream ROM should have parent"),
    )
    .expect("upstream ROM parent should be creatable");
    fs::write(&upstream_rom, &rom_bytes).expect("upstream ROM should be writable");
    let commit = commit_upstream_repo(&upstream);
    write_reports(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
    );
    write_source_manifest(
        &workspace,
        "sample-report/sources.report.toml",
        &format!(
            concat!(
                "[[source]]\n",
                "id = \"local-source\"\n",
                "git_url = {:?}\n",
                "git_rev = {:?}\n",
                "\n",
                "[[source.family]]\n",
                "id = \"blargg\"\n",
                "target_root = \"blargg\"\n",
                "sparse_paths = [\"roms/blargg\"]\n",
                "\n",
                "[[source.family.file]]\n",
                "path = \"roms/blargg/cpu_instrs/01-special.gb\"\n",
                "target = \"cpu_instrs/01-special.gb\"\n",
                "sha256 = {:?}\n",
            ),
            upstream.display().to_string(),
            commit,
            sha256_hex(&rom_bytes)
        ),
    );
    write_manifest(
        &workspace,
        "sample-report/blargg-cpu-instrs.suite.toml",
        &basic_manifest(
            "sample-report",
            "blargg-cpu-instrs",
            "blargg",
            "blargg-cpu-instrs-01-special",
            "cpu_instrs/01-special.gb",
        ),
    );
    let stale_rom = workspace.join("test/sample-report/blargg/cpu_instrs/01-special.gb");
    fs::create_dir_all(stale_rom.parent().expect("stale ROM should have parent"))
        .expect("stale ROM parent should be creatable");
    fs::write(&stale_rom, build_serial_text_rom("Failed")).expect("stale ROM should be writable");

    let mut output = Vec::new();
    run_suite_command_with_workspace_for_test(
        ["sample-report", "--suite", "blargg-cpu-instrs"],
        &workspace,
        &mut output,
    )
    .expect("suite should refetch stale ROM and pass");

    let output = String::from_utf8(output).expect("output should be utf-8");
    assert!(output.contains("hash mismatch"));
    assert!(output.contains("suite blargg-cpu-instrs: 1/1 passed"));

    fs::remove_dir_all(workspace).expect("workspace should be removable");
    fs::remove_dir_all(upstream).expect("upstream should be removable");
}

#[test]
fn command_auto_fetches_missing_framebuffer_fixture_before_manifest_oracle_load() {
    let workspace = unique_temp_dir("auto-fetch-fixture-workspace");
    let upstream = unique_temp_dir("auto-fetch-fixture-upstream");
    let upstream_root = upstream.join("roms/ax6");
    fs::create_dir_all(&upstream_root).expect("upstream root should be creatable");
    let rom_bytes = build_infinite_loop_rom();
    fs::write(upstream_root.join("rtc3test-1.gb"), &rom_bytes)
        .expect("upstream ROM should be writable");
    let mut fixture_pixels = vec![0; 160 * 144];
    for (index, pixel) in fixture_pixels.iter_mut().enumerate() {
        *pixel = if index.is_multiple_of(2) { 0 } else { 255 };
    }
    write_grayscale_png(&upstream_root.join("rtc3test-1.png"), &fixture_pixels);
    let fixture_bytes =
        fs::read(upstream_root.join("rtc3test-1.png")).expect("fixture should be readable");
    let commit = commit_upstream_repo(&upstream);
    write_reports(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
    );
    write_source_manifest(
        &workspace,
        "sample-report/sources.report.toml",
        &format!(
            concat!(
                "[[source]]\n",
                "id = \"local-source\"\n",
                "git_url = {:?}\n",
                "git_rev = {:?}\n",
                "\n",
                "[[source.family]]\n",
                "id = \"ax6\"\n",
                "target_root = \"ax6\"\n",
                "sparse_paths = [\"roms/ax6\"]\n",
                "\n",
                "[[source.family.file]]\n",
                "path = \"roms/ax6/rtc3test-1.gb\"\n",
                "target = \"rtc3test-1.gb\"\n",
                "sha256 = {:?}\n",
                "\n",
                "[[source.family.file]]\n",
                "path = \"roms/ax6/rtc3test-1.png\"\n",
                "target = \"rtc3test-1.png\"\n",
                "sha256 = {:?}\n",
            ),
            upstream.display().to_string(),
            commit,
            sha256_hex(&rom_bytes),
            sha256_hex(&fixture_bytes)
        ),
    );
    write_manifest(
        &workspace,
        "sample-report/ax6.suite.toml",
        r#"
family = "ax6"
suite_name = "ax6"
report = "sample-report"
model = "cgb"
timeout_frames = 1
oracle = { type = "framebuffer", source = "cgb", projection = "grayscale", fixture = "rtc3test-1.png" }

[[case]]
id = "ax6-rtc3test-1"
rom = "rtc3test-1.gb"
"#,
    );

    let mut output = Vec::new();
    let error = run_suite_command_with_workspace_for_test(
        ["sample-report", "--suite", "ax6"],
        &workspace,
        &mut output,
    )
    .expect_err("framebuffer mismatch should fail after fixture materialization");

    assert!(error.contains("one or more suite cases failed"));
    assert!(
        workspace
            .join("test/sample-report/ax6/rtc3test-1.png")
            .exists()
    );
    let output = String::from_utf8(output).expect("output should be utf-8");
    assert!(output.contains("test ROM family ax6 requires materialization"));
    assert!(!output.contains("failed to read framebuffer fixture"));

    fs::remove_dir_all(workspace).expect("workspace should be removable");
    fs::remove_dir_all(upstream).expect("upstream should be removable");
}

#[test]
fn command_case_selection_auto_fetches_only_selected_case_family() {
    let workspace = unique_temp_dir("auto-fetch-case-workspace");
    let upstream = unique_temp_dir("auto-fetch-case-upstream");
    let rom_bytes = build_serial_text_rom("Passed");
    let upstream_rom = upstream.join("roms/blargg/pass.gb");
    fs::create_dir_all(
        upstream_rom
            .parent()
            .expect("upstream ROM should have parent"),
    )
    .expect("upstream ROM parent should be creatable");
    fs::write(&upstream_rom, &rom_bytes).expect("upstream ROM should be writable");
    let commit = commit_upstream_repo(&upstream);
    write_reports(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
    );
    write_source_manifest(
        &workspace,
        "sample-report/sources.report.toml",
        &format!(
            concat!(
                "[[source]]\n",
                "id = \"local-source\"\n",
                "git_url = {:?}\n",
                "git_rev = {:?}\n",
                "\n",
                "[[source.family]]\n",
                "id = \"blargg\"\n",
                "target_root = \"blargg\"\n",
                "sparse_paths = [\"roms/blargg\"]\n",
                "\n",
                "[[source.family.file]]\n",
                "path = \"roms/blargg/pass.gb\"\n",
                "target = \"pass.gb\"\n",
                "sha256 = {:?}\n",
                "\n",
                "[[source.family]]\n",
                "id = \"acid\"\n",
                "target_root = \"acid\"\n",
                "sparse_paths = [\"roms/acid\"]\n",
                "\n",
                "[[source.family.file]]\n",
                "path = \"roms/acid/missing.gb\"\n",
                "target = \"missing.gb\"\n",
                "sha256 = \"0000000000000000000000000000000000000000000000000000000000000000\"\n",
            ),
            upstream.display().to_string(),
            commit,
            sha256_hex(&rom_bytes)
        ),
    );
    write_manifest(
        &workspace,
        "sample-report/mixed.suite.toml",
        r#"
family = "blargg"
suite_name = "mixed"
report = "sample-report"
model = "dmg"
timeout_frames = 2
oracle = { type = "serial-contains", expected = "Passed" }

[[case]]
id = "selected-blargg"
rom = "pass.gb"

[[case]]
family = "acid"
id = "unselected-acid"
rom = "missing.gb"
"#,
    );

    let mut output = Vec::new();
    run_suite_command_with_workspace_for_test(
        [
            "sample-report",
            "--suite",
            "mixed",
            "--case",
            "selected-blargg",
        ],
        &workspace,
        &mut output,
    )
    .expect("selected case should fetch only its family and pass");

    let output = String::from_utf8(output).expect("output should be utf-8");
    assert!(output.contains("test ROM family blargg requires materialization"));
    assert!(!output.contains("test ROM family acid requires materialization"));
    assert!(workspace.join("test/sample-report/blargg/pass.gb").exists());
    assert!(
        !workspace
            .join("test/sample-report/acid/missing.gb")
            .exists()
    );

    fs::remove_dir_all(workspace).expect("workspace should be removable");
    fs::remove_dir_all(upstream).expect("upstream should be removable");
}

#[test]
fn command_unknown_suite_fails_before_auto_fetch() {
    let workspace = unique_temp_dir("auto-fetch-unknown-suite");
    write_reports(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
    );
    write_source_manifest(
        &workspace,
        "sample-report/sources.report.toml",
        "not a valid fetch source manifest",
    );
    write_manifest(
        &workspace,
        "sample-report/blargg-cpu-instrs.suite.toml",
        &basic_manifest(
            "sample-report",
            "blargg-cpu-instrs",
            "blargg",
            "blargg-cpu-instrs-01-special",
            "cpu_instrs/01-special.gb",
        ),
    );

    let mut output = Vec::new();
    let error = run_suite_command_with_workspace_for_test(
        ["sample-report", "--suite", "missing-suite"],
        &workspace,
        &mut output,
    )
    .expect_err("unknown suite should fail before source validation");

    assert!(error.contains("unknown suite"));
    assert!(!error.contains("source manifest"));
    assert!(output.is_empty());

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn command_runs_with_explicit_threads_and_preserves_status_order() {
    let workspace = unique_temp_dir("serial-threads");
    write_reports(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
    );
    write_manifest(
        &workspace,
        "sample-report/threaded.suite.toml",
        r#"
family = "blargg"
suite_name = "threaded"
report = "sample-report"
model = "dmg"
timeout_frames = 2
oracle = { type = "serial-contains", expected = "Passed" }

[[case]]
id = "threaded-first"
rom = "threaded/first.gb"

[[case]]
id = "threaded-second"
rom = "threaded/second.gb"
"#,
    );
    for rom in ["first.gb", "second.gb"] {
        let rom_path = workspace
            .join("test/sample-report/blargg/threaded")
            .join(rom);
        fs::create_dir_all(rom_path.parent().expect("rom should have parent"))
            .expect("rom parent should be creatable");
        fs::write(&rom_path, build_serial_text_rom("Passed")).expect("rom should be writable");
    }
    write_materialized_source_manifest(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
        &[("blargg", "blargg")],
    );

    for threads in ["2", "1"] {
        let mut output = Vec::new();
        run_suite_command_with_workspace_for_test(
            ["sample-report", "--suite", "threaded", "--threads", threads],
            &workspace,
            &mut output,
        )
        .expect("threaded suite should pass");
        let output = String::from_utf8(output).expect("output should be utf-8");
        assert!(output.contains("suite threaded: 2/2 passed"));
    }

    let status = fs::read_to_string(workspace.join("test/sample-report/.status/threaded.json"))
        .expect("status should be written");
    let first = status
        .find("\"rom\": \"threaded/first.gb\"")
        .expect("first row should exist");
    let second = status
        .find("\"rom\": \"threaded/second.gb\"")
        .expect("second row should exist");
    assert!(first < second);

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn command_reports_failed_cases_and_rejects_unknown_case() {
    let workspace = unique_temp_dir("serial-fail");
    write_reports(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
    );
    write_manifest(
        &workspace,
        "sample-report/blargg-cpu-instrs.suite.toml",
        &basic_manifest(
            "sample-report",
            "blargg-cpu-instrs",
            "blargg",
            "blargg-cpu-instrs-01-special",
            "cpu_instrs/01-special.gb",
        ),
    );
    let rom_path = workspace.join("test/sample-report/blargg/cpu_instrs/01-special.gb");
    fs::create_dir_all(rom_path.parent().expect("rom should have parent"))
        .expect("rom parent should be creatable");
    fs::write(&rom_path, build_serial_text_rom("Failed")).expect("rom should be writable");
    write_materialized_source_manifest(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
        &[("blargg", "blargg")],
    );

    let mut output = Vec::new();
    let error = run_suite_command_with_workspace_for_test(
        ["sample-report", "--suite", "blargg-cpu-instrs"],
        &workspace,
        &mut output,
    )
    .expect_err("suite should fail");
    assert!(error.contains("one or more suite cases failed"));
    let output = String::from_utf8(output).expect("output should be utf-8");
    assert!(output.contains("case blargg-cpu-instrs-01-special: FAIL"));
    let status =
        fs::read_to_string(workspace.join("test/sample-report/.status/blargg-cpu-instrs.json"))
            .expect("status should be written");
    assert!(status.contains("\"status\": \"FAIL\""));

    let mut output = Vec::new();
    assert!(
        run_suite_command_with_workspace_for_test(
            [
                "sample-report",
                "--suite",
                "blargg-cpu-instrs",
                "--case",
                "missing-case",
            ],
            &workspace,
            &mut output,
        )
        .expect_err("unknown case should fail")
        .contains("unknown case")
    );

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn command_rejects_manifest_real_boot_without_boot_rom_dir() {
    let workspace = unique_temp_dir("real-boot-without-assets");
    write_reports(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
    );
    write_manifest(
        &workspace,
        "sample-report/real-boot.suite.toml",
        &basic_manifest(
            "sample-report",
            "real-boot",
            "blargg",
            "real-boot-case",
            "case.gb",
        )
        .replace(
            "rom = \"case.gb\"",
            "rom = \"case.gb\"\nstartup = \"real-boot\"",
        ),
    );
    let rom_path = workspace.join("test/sample-report/blargg/case.gb");
    fs::create_dir_all(rom_path.parent().expect("rom should have parent"))
        .expect("rom parent should be creatable");
    fs::write(&rom_path, build_infinite_loop_rom()).expect("rom should be writable");
    write_materialized_source_manifest(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
        &[("blargg", "blargg")],
    );

    let mut output = Vec::new();
    let error =
        run_suite_command_with_workspace_for_test(["sample-report"], &workspace, &mut output)
            .expect_err("manifest real-boot should require boot ROM dir");
    assert!(error.contains("startup = \"real-boot\""));
    assert!(error.contains("--boot-rom-dir <dir>"));

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn command_boot_rom_dir_without_manifest_real_boot_does_not_validate_assets() {
    let workspace = unique_temp_dir("boot-rom-dir-without-real-boot");
    let boot_rom_dir = workspace.join("missing-bootroms");
    write_reports(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
    );
    write_manifest(
        &workspace,
        "sample-report/skip-boot.suite.toml",
        &basic_manifest(
            "sample-report",
            "skip-boot",
            "blargg",
            "skip-boot-case",
            "case.gb",
        ),
    );
    let rom_path = workspace.join("test/sample-report/blargg/case.gb");
    fs::create_dir_all(rom_path.parent().expect("rom should have parent"))
        .expect("rom parent should be creatable");
    fs::write(&rom_path, build_serial_text_rom("Passed")).expect("rom should be writable");
    write_materialized_source_manifest(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
        &[("blargg", "blargg")],
    );

    let mut output = Vec::new();
    run_suite_command_with_workspace_for_test(
        [
            "sample-report",
            "--suite",
            "skip-boot",
            "--boot-rom-dir",
            boot_rom_dir.to_str().expect("path should be utf-8"),
        ],
        &workspace,
        &mut output,
    )
    .expect("plain boot ROM dir should not validate assets for skip-boot cases");

    let output = String::from_utf8(output).expect("output should be utf-8");
    assert!(output.contains("suite skip-boot: 1/1 passed"));
    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn command_boot_rom_dir_validates_manifest_real_boot_assets() {
    let workspace = unique_temp_dir("boot-rom-dir-manifest-real-boot");
    let boot_rom_dir = workspace.join("bootroms");
    fs::create_dir_all(&boot_rom_dir).expect("boot ROM dir should be creatable");
    write_reports(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
    );
    write_manifest(
        &workspace,
        "sample-report/real-boot.suite.toml",
        &basic_manifest(
            "sample-report",
            "real-boot",
            "blargg",
            "real-boot-case",
            "case.gb",
        )
        .replace(
            "rom = \"case.gb\"",
            "rom = \"case.gb\"\nstartup = \"real-boot\"",
        ),
    );
    let rom_path = workspace.join("test/sample-report/blargg/case.gb");
    fs::create_dir_all(rom_path.parent().expect("rom should have parent"))
        .expect("rom parent should be creatable");
    fs::write(&rom_path, build_infinite_loop_rom()).expect("rom should be writable");
    write_materialized_source_manifest(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
        &[("blargg", "blargg")],
    );

    let mut output = Vec::new();
    let error = run_suite_command_with_workspace_for_test(
        [
            "sample-report",
            "--suite",
            "real-boot",
            "--boot-rom-dir",
            boot_rom_dir.to_str().expect("path should be utf-8"),
        ],
        &workspace,
        &mut output,
    )
    .expect_err("manifest real-boot should require verified assets");
    assert!(error.contains("failed to load boot ROM assets"));
    assert!(error.contains("dmg_boot.bin"));

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn command_force_real_boot_requires_verified_assets() {
    let workspace = unique_temp_dir("force-real-boot-requires-assets");
    let boot_rom_dir = workspace.join("bootroms");
    fs::create_dir_all(&boot_rom_dir).expect("boot ROM dir should be creatable");
    write_reports(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
    );
    write_manifest(
        &workspace,
        "sample-report/skip-boot.suite.toml",
        &basic_manifest(
            "sample-report",
            "skip-boot",
            "blargg",
            "skip-boot-case",
            "case.gb",
        ),
    );
    let rom_path = workspace.join("test/sample-report/blargg/case.gb");
    fs::create_dir_all(rom_path.parent().expect("rom should have parent"))
        .expect("rom parent should be creatable");
    fs::write(&rom_path, build_infinite_loop_rom()).expect("rom should be writable");
    write_materialized_source_manifest(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
        &[("blargg", "blargg")],
    );

    let mut output = Vec::new();
    let error = run_suite_command_with_workspace_for_test(
        [
            "sample-report",
            "--suite",
            "skip-boot",
            "--boot-rom-dir",
            boot_rom_dir.to_str().expect("path should be utf-8"),
            "--force-real-boot",
        ],
        &workspace,
        &mut output,
    )
    .expect_err("forced real-boot should require verified assets");
    assert!(error.contains("failed to load boot ROM assets"));
    assert!(error.contains("dmg_boot.bin"));

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn command_force_real_boot_uses_case_hardware_revision_for_asset_selection() {
    let workspace = unique_temp_dir("boot-rom-dir-cgb-d-revision");
    let boot_rom_dir = workspace.join("bootroms");
    fs::create_dir_all(&boot_rom_dir).expect("boot ROM dir should be creatable");
    write_reports(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
    );
    write_manifest(
        &workspace,
        "sample-report/cgb-d.suite.toml",
        &basic_manifest(
            "sample-report",
            "cgb-d",
            "samesuite",
            "cgb-d-case",
            "case.gbc",
        )
        .replace(
            "model = \"dmg\"",
            "model = \"cgb\"\nrevision = \"cpu-cgb-d\"",
        ),
    );
    let rom_path = workspace.join("test/sample-report/samesuite/case.gbc");
    fs::create_dir_all(rom_path.parent().expect("rom should have parent"))
        .expect("rom parent should be creatable");
    fs::write(&rom_path, build_infinite_loop_rom()).expect("rom should be writable");
    write_materialized_source_manifest(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
        &[("samesuite", "samesuite")],
    );

    let mut output = Vec::new();
    let error = run_suite_command_with_workspace_for_test(
        [
            "sample-report",
            "--suite",
            "cgb-d",
            "--boot-rom-dir",
            boot_rom_dir.to_str().expect("path should be utf-8"),
            "--force-real-boot",
        ],
        &workspace,
        &mut output,
    )
    .expect_err("forced real-boot should require the CGB-D boot ROM asset");
    assert!(error.contains("failed to load boot ROM assets"));
    assert!(error.contains("cgb_boot.bin"));
    assert!(!error.contains("cgbE_boot.bin"));

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn command_force_real_boot_uses_dmg0_asset_for_dmg0_revision() {
    let workspace = unique_temp_dir("boot-rom-dir-dmg0-revision");
    let boot_rom_dir = workspace.join("bootroms");
    fs::create_dir_all(&boot_rom_dir).expect("boot ROM dir should be creatable");
    write_reports(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
    );
    write_manifest(
        &workspace,
        "sample-report/dmg0.suite.toml",
        &basic_manifest("sample-report", "dmg0", "blargg", "dmg0-case", "case.gb").replace(
            "model = \"dmg\"",
            "model = \"dmg\"\nrevision = \"dmg-cpu-0\"",
        ),
    );
    let rom_path = workspace.join("test/sample-report/blargg/case.gb");
    fs::create_dir_all(rom_path.parent().expect("rom should have parent"))
        .expect("rom parent should be creatable");
    fs::write(&rom_path, build_infinite_loop_rom()).expect("rom should be writable");
    write_materialized_source_manifest(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
        &[("blargg", "blargg")],
    );

    let mut output = Vec::new();
    let error = run_suite_command_with_workspace_for_test(
        [
            "sample-report",
            "--suite",
            "dmg0",
            "--boot-rom-dir",
            boot_rom_dir.to_str().expect("path should be utf-8"),
            "--force-real-boot",
        ],
        &workspace,
        &mut output,
    )
    .expect_err("forced real-boot should require the DMG0 boot ROM asset");
    assert!(error.contains("failed to load boot ROM assets"));
    assert!(error.contains("dmg0_boot.bin"));
    assert!(!error.contains("dmg_boot.bin"));

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn command_force_real_boot_uses_cgb0_asset_for_cgb0_revision() {
    let workspace = unique_temp_dir("boot-rom-dir-cgb0-revision");
    let boot_rom_dir = workspace.join("bootroms");
    fs::create_dir_all(&boot_rom_dir).expect("boot ROM dir should be creatable");
    write_reports(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
    );
    write_manifest(
        &workspace,
        "sample-report/cgb0.suite.toml",
        &basic_manifest(
            "sample-report",
            "cgb0",
            "samesuite",
            "cgb0-case",
            "case.gbc",
        )
        .replace(
            "model = \"dmg\"",
            "model = \"cgb\"\nrevision = \"cpu-cgb-0\"",
        ),
    );
    let rom_path = workspace.join("test/sample-report/samesuite/case.gbc");
    fs::create_dir_all(rom_path.parent().expect("rom should have parent"))
        .expect("rom parent should be creatable");
    fs::write(&rom_path, build_infinite_loop_rom()).expect("rom should be writable");
    write_materialized_source_manifest(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
        &[("samesuite", "samesuite")],
    );

    let mut output = Vec::new();
    let error = run_suite_command_with_workspace_for_test(
        [
            "sample-report",
            "--suite",
            "cgb0",
            "--boot-rom-dir",
            boot_rom_dir.to_str().expect("path should be utf-8"),
            "--force-real-boot",
        ],
        &workspace,
        &mut output,
    )
    .expect_err("forced real-boot should require the CGB0 boot ROM asset");
    assert!(error.contains("failed to load boot ROM assets"));
    assert!(error.contains("cgb0_boot.bin"));
    assert!(!error.contains("cgb_boot.bin"));
    assert!(!error.contains("cgbE_boot.bin"));

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn command_force_real_boot_uses_agb0_asset_for_agb0_revision() {
    let workspace = unique_temp_dir("boot-rom-dir-agb0-revision");
    let boot_rom_dir = workspace.join("bootroms");
    fs::create_dir_all(&boot_rom_dir).expect("boot ROM dir should be creatable");
    write_reports(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
    );
    write_manifest(
        &workspace,
        "sample-report/agb0.suite.toml",
        &basic_manifest(
            "sample-report",
            "agb0",
            "samesuite",
            "agb0-case",
            "case.gbc",
        )
        .replace(
            "model = \"dmg\"",
            "model = \"agb\"\nrevision = \"cpu-agb-0\"",
        ),
    );
    let rom_path = workspace.join("test/sample-report/samesuite/case.gbc");
    fs::create_dir_all(rom_path.parent().expect("rom should have parent"))
        .expect("rom parent should be creatable");
    fs::write(&rom_path, build_infinite_loop_rom()).expect("rom should be writable");
    write_materialized_source_manifest(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
        &[("samesuite", "samesuite")],
    );

    let mut output = Vec::new();
    let error = run_suite_command_with_workspace_for_test(
        [
            "sample-report",
            "--suite",
            "agb0",
            "--boot-rom-dir",
            boot_rom_dir.to_str().expect("path should be utf-8"),
            "--force-real-boot",
        ],
        &workspace,
        &mut output,
    )
    .expect_err("forced real-boot should require the AGB0 boot ROM asset");
    assert!(error.contains("failed to load boot ROM assets"));
    assert!(error.contains("cgb_agb0_boot.bin"));
    assert!(!error.contains("cgb_agb_boot.bin"));
    assert!(!error.contains("cgbE_boot.bin"));

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn run_suite_real_boot_handoff_does_not_consume_case_timeout() {
    let workspace = unique_temp_dir("real-boot-handoff-budget");
    write_reports(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
    );
    write_manifest(
        &workspace,
        "sample-report/real-boot.suite.toml",
        r#"
family = "blargg"
suite_name = "real-boot"
report = "sample-report"
model = "dmg"
startup = "real-boot"
timeout_frames = 1
oracle = { type = "memory-byte-equals", address = 49152, value = 1 }

[[case]]
id = "real-boot-memory-pass"
rom = "case.gb"
"#,
    );
    let rom_path = workspace.join("test/sample-report/blargg/case.gb");
    fs::create_dir_all(rom_path.parent().expect("rom should have parent"))
        .expect("rom parent should be creatable");
    fs::write(&rom_path, build_memory_write_rom(0xC000, 1)).expect("rom should be writable");
    let boot_rom_assets = BootRomAssets::none()
        .with_asset_bytes(BootRomAssetKind::Dmg, build_delayed_dmg_handoff_boot_rom())
        .expect("synthetic boot ROM should load");
    let reports = load_reports(&workspace).expect("reports should load");
    let report = reports
        .iter()
        .find(|report| report.id == "sample-report")
        .expect("sample report should exist");
    let suites = load_selected_suites(&workspace, report, Some("real-boot"), None)
        .expect("suite should load");

    let suite_report = run_suite_with_config(
        &workspace,
        report,
        &suites[0],
        &SuiteRunConfig {
            boot_rom_assets: Some(boot_rom_assets),
        },
    );

    assert!(suite_report.all_passed());
    assert!(suite_report.cases[0].executed_tcycles < gb_core::DMG_T_CYCLES_PER_FRAME);

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn command_advances_mbc3_rtc_during_suite_execution() {
    let workspace = unique_temp_dir("mbc3-rtc-suite");
    write_reports(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
    );
    write_manifest(
        &workspace,
        "sample-report/rtc.suite.toml",
        r#"
family = "blargg"
suite_name = "rtc"
report = "sample-report"
model = "cgb"
execution_mode = "permissive"
timeout_frames = 80
oracle = { type = "memory-byte-equals", address = 49152, value = 1 }

[[case]]
id = "rtc-waits-for-seconds-register"
rom = "rtc/wait.gb"
"#,
    );
    let rom_path = workspace.join("test/sample-report/blargg/rtc/wait.gb");
    fs::create_dir_all(rom_path.parent().expect("rom should have parent"))
        .expect("rom parent should be creatable");
    fs::write(&rom_path, build_mbc3_rtc_wait_rom(0xC000, 1)).expect("rom should be writable");
    write_materialized_source_manifest(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
        &[("blargg", "blargg")],
    );

    let mut output = Vec::new();
    run_suite_command_with_workspace_for_test(
        [
            "sample-report",
            "--suite",
            "rtc",
            "--case",
            "rtc-waits-for-seconds-register",
        ],
        &workspace,
        &mut output,
    )
    .expect("RTC-backed suite should pass");

    let output = String::from_utf8(output).expect("output should be utf-8");
    assert!(output.contains("suite rtc: 1/1 passed"));

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn command_writes_framebuffer_failure_artifacts() {
    let workspace = unique_temp_dir("framebuffer-failure-artifacts");
    write_reports(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
    );
    write_manifest(
        &workspace,
        "sample-report/ax6.suite.toml",
        r#"
family = "ax6"
suite_name = "ax6"
report = "sample-report"
model = "cgb"
timeout_frames = 1
oracle = { type = "framebuffer", source = "cgb", projection = "grayscale", fixture = "rtc3test-1.png" }

[[case]]
id = "ax6-rtc3test-1"
rom = "rtc3test-1.gb"
"#,
    );
    let fixture_path = workspace.join("test/sample-report/ax6/rtc3test-1.png");
    let mut fixture_pixels = vec![0; 160 * 144];
    for (index, pixel) in fixture_pixels.iter_mut().enumerate() {
        *pixel = if index.is_multiple_of(2) { 0 } else { 255 };
    }
    write_grayscale_png(&fixture_path, &fixture_pixels);
    let rom_path = workspace.join("test/sample-report/ax6/rtc3test-1.gb");
    fs::create_dir_all(rom_path.parent().expect("rom should have parent"))
        .expect("rom parent should be creatable");
    fs::write(&rom_path, build_infinite_loop_rom()).expect("rom should be writable");
    write_materialized_source_manifest(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
        &[("ax6", "ax6")],
    );

    let mut output = Vec::new();
    run_suite_command_with_workspace_for_test(
        [
            "sample-report",
            "--suite",
            "ax6",
            "--case",
            "ax6-rtc3test-1",
        ],
        &workspace,
        &mut output,
    )
    .expect_err("framebuffer mismatch should fail");

    let artifact_dir = workspace.join("test/sample-report/.artifacts/ax6/ax6-rtc3test-1");
    let output = String::from_utf8(output).expect("output should be utf-8");
    assert!(output.contains(&format!("artifact_dir={}", artifact_dir.display())));
    assert!(artifact_dir.join("actual.png").exists());
    assert!(artifact_dir.join("expected-0.png").exists());
    assert!(artifact_dir.join("snapshot.txt").exists());
    let metadata =
        fs::read_to_string(artifact_dir.join("failure.toml")).expect("metadata should exist");
    assert!(metadata.contains("source = \"cgb\""));
    assert!(metadata.contains("actual.png"));
    assert!(metadata.contains("expected-0.png"));
    assert!(metadata.contains("rtc3test-1.png"));

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn command_writes_non_framebuffer_failure_artifacts_and_cleans_them_on_pass() {
    let workspace = unique_temp_dir("serial-failure-artifacts");
    write_reports(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
    );
    write_manifest(
        &workspace,
        "sample-report/blargg-cpu-instrs.suite.toml",
        &basic_manifest(
            "sample-report",
            "blargg-cpu-instrs",
            "blargg",
            "blargg-cpu-instrs-01-special",
            "cpu_instrs/01-special.gb",
        ),
    );
    let rom_path = workspace.join("test/sample-report/blargg/cpu_instrs/01-special.gb");
    fs::create_dir_all(rom_path.parent().expect("rom should have parent"))
        .expect("rom parent should be creatable");
    fs::write(&rom_path, build_serial_text_rom("Failed")).expect("rom should be writable");
    write_materialized_source_manifest(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
        &[("blargg", "blargg")],
    );

    let mut output = Vec::new();
    run_suite_command_with_workspace_for_test(
        ["sample-report", "--suite", "blargg-cpu-instrs"],
        &workspace,
        &mut output,
    )
    .expect_err("serial mismatch should fail");

    let artifact_dir = workspace
        .join("test/sample-report/.artifacts/blargg-cpu-instrs/blargg-cpu-instrs-01-special");
    assert!(artifact_dir.join("failure.toml").exists());
    assert!(artifact_dir.join("snapshot.txt").exists());
    assert!(artifact_dir.join("serial.txt").exists());
    assert!(!artifact_dir.join("actual.png").exists());

    fs::write(&rom_path, build_serial_text_rom("Passed")).expect("rom should be writable");
    write_materialized_source_manifest(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
        &[("blargg", "blargg")],
    );
    let mut output = Vec::new();
    run_suite_command_with_workspace_for_test(
        ["sample-report", "--suite", "blargg-cpu-instrs"],
        &workspace,
        &mut output,
    )
    .expect("serial match should pass");
    assert!(!artifact_dir.exists());

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn command_treats_info_framebuffer_as_pass_for_ci() {
    let workspace = unique_temp_dir("framebuffer-info-pass");
    write_reports(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
    );
    write_manifest(
        &workspace,
        "sample-report/acid.suite.toml",
        &basic_manifest(
            "sample-report",
            "acid",
            "acid",
            "acid-which-dmg",
            "which.gb",
        )
        .replace(
            "oracle = { type = \"serial-contains\", expected = \"Passed\" }",
            "oracle = { type = \"framebuffer\", mode = \"info\" }",
        )
        .replace("timeout_frames = 2", "timeout_frames = 1"),
    );
    let rom_path = workspace.join("test/sample-report/acid/which.gb");
    fs::create_dir_all(rom_path.parent().expect("rom should have parent"))
        .expect("rom parent should be creatable");
    fs::write(&rom_path, build_serial_text_rom("")).expect("rom should be writable");
    write_materialized_source_manifest(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
        &[("acid", "acid")],
    );

    let mut output = Vec::new();
    run_suite_command_with_workspace_for_test(
        [
            "sample-report",
            "--suite",
            "acid",
            "--case",
            "acid-which-dmg",
        ],
        &workspace,
        &mut output,
    )
    .expect("info framebuffer should pass");

    let output = String::from_utf8(output).expect("output should be utf-8");
    assert!(output.contains("suite acid: 1/1 passed"));
    assert!(output.contains("case acid-which-dmg: Informational after"));
    let status = fs::read_to_string(workspace.join("test/sample-report/.status/acid.json"))
        .expect("status should be written");
    assert!(status.contains("\"status\": \"INFO\""));

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn command_runs_fibonacci_result_suite_as_ci_friendly_pass() {
    let workspace = unique_temp_dir("fibonacci-pass");
    write_reports(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
    );
    write_manifest(
        &workspace,
        "sample-report/mooneye.suite.toml",
        r#"
family = "mooneye"
suite_name = "mooneye"
report = "sample-report"
model = "dmg"
timeout_frames = 2
oracle = { type = "fibonacci-result" }

[[case]]
id = "mooneye-pass"
rom = "acceptance/pass.gb"
"#,
    );
    let rom_path = workspace.join("test/sample-report/mooneye/acceptance/pass.gb");
    fs::create_dir_all(rom_path.parent().expect("rom should have parent"))
        .expect("rom parent should be creatable");
    fs::write(&rom_path, build_fibonacci_result_rom([3, 5, 8, 13, 21, 34]))
        .expect("rom should be writable");
    write_materialized_source_manifest(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
        &[("mooneye", "mooneye")],
    );

    let mut output = Vec::new();
    run_suite_command_with_workspace_for_test(
        [
            "sample-report",
            "--suite",
            "mooneye",
            "--case",
            "mooneye-pass",
        ],
        &workspace,
        &mut output,
    )
    .expect("fibonacci result suite should pass");

    let output = String::from_utf8(output).expect("output should be utf-8");
    assert!(output.contains("suite mooneye: 1/1 passed"));
    let status = fs::read_to_string(workspace.join("test/sample-report/.status/mooneye.json"))
        .expect("status should be written");
    assert!(status.contains("\"status\": \"PASS\""));

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn command_runs_sgb_suite_with_handheld_core_and_sgb_host() {
    let workspace = unique_temp_dir("sgb-pass");
    write_reports(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
    );
    write_manifest(
        &workspace,
        "sample-report/samesuite.suite.toml",
        r#"
family = "samesuite"
suite_name = "samesuite"
report = "sample-report"
model = "sgb"
timeout_frames = 1
oracle = { type = "framebuffer", mode = "info" }

[[case]]
id = "samesuite-sgb-smoke"
rom = "sgb/smoke.gb"
"#,
    );
    let rom_path = workspace.join("test/sample-report/samesuite/sgb/smoke.gb");
    fs::create_dir_all(rom_path.parent().expect("rom should have parent"))
        .expect("rom parent should be creatable");
    fs::write(&rom_path, build_infinite_loop_rom()).expect("rom should be writable");
    write_materialized_source_manifest(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
        &[("samesuite", "samesuite")],
    );

    let mut output = Vec::new();
    run_suite_command_with_workspace_for_test(
        [
            "sample-report",
            "--suite",
            "samesuite",
            "--case",
            "samesuite-sgb-smoke",
        ],
        &workspace,
        &mut output,
    )
    .expect("sgb suite should pass");

    let output = String::from_utf8(output).expect("output should be utf-8");
    assert!(output.contains("suite samesuite: 1/1 passed"));
    assert!(output.contains("case samesuite-sgb-smoke: Informational after"));
    let status = fs::read_to_string(workspace.join("test/sample-report/.status/samesuite.json"))
        .expect("status should be written");
    assert!(status.contains("\"status\": \"INFO\""));

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn command_runs_memory_byte_equals_suite_as_ci_friendly_pass() {
    let workspace = unique_temp_dir("memory-byte-pass");
    write_reports(&workspace, "gbmicrotest", "gbmicrotest/sources.report.toml");
    write_source_manifest(
        &workspace,
        "gbmicrotest/sources.report.toml",
        r#"
[[source]]
id = "gbmicrotest"

[[source.family]]
id = "gbmicrotest"
target_root = ""
"#,
    );
    write_manifest(
        &workspace,
        "gbmicrotest/gbmicrotest.suite.toml",
        r#"
family = "gbmicrotest"
suite_name = "gbmicrotest"
report = "gbmicrotest"
model = "dmg"
timeout_frames = 1
oracle = { type = "memory-byte-equals", address = 65410, value = 1 }

[[case]]
id = "gbmicrotest-pass"
rom = "memory/pass.gb"
"#,
    );
    let rom_path = workspace.join("test/gbmicrotest/memory/pass.gb");
    fs::create_dir_all(rom_path.parent().expect("rom should have parent"))
        .expect("rom parent should be creatable");
    fs::write(&rom_path, build_memory_write_rom(0xFF82, 1)).expect("rom should be writable");
    write_materialized_source_manifest(
        &workspace,
        "gbmicrotest",
        "gbmicrotest/sources.report.toml",
        &[("gbmicrotest", "")],
    );

    let mut output = Vec::new();
    run_suite_command_with_workspace_for_test(
        [
            "gbmicrotest",
            "--suite",
            "gbmicrotest",
            "--case",
            "gbmicrotest-pass",
        ],
        &workspace,
        &mut output,
    )
    .expect("memory byte suite should pass");

    let output = String::from_utf8(output).expect("output should be utf-8");
    assert!(output.contains("suite gbmicrotest: 1/1 passed"));
    let status = fs::read_to_string(workspace.join("test/gbmicrotest/.status/gbmicrotest.json"))
        .expect("status should be written");
    assert!(status.contains("\"status\": \"PASS\""));

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn command_applies_case_joypad_stimuli() {
    let workspace = unique_temp_dir("joypad-stimulus-pass");
    write_reports(&workspace, "docboy", "docboy/sources.report.toml");
    write_source_manifest(
        &workspace,
        "docboy/sources.report.toml",
        r#"
[[source]]
id = "docboy"

[[source.family]]
id = "docboy-dmg"
target_root = "dmg"
"#,
    );
    write_manifest(
        &workspace,
        "docboy/docboy-dmg.suite.toml",
        r#"
family = "docboy-dmg"
suite_name = "docboy-dmg"
report = "docboy"
model = "dmg"
timeout_frames = 1
oracle = { type = "memory-byte-equals", address = 49152, value = 1 }

[[case]]
id = "docboy-joypad-stimulus"
rom = "interactive/a.gb"

[[case.stimulus]]
tcycle = 0
button = "a"
pressed = true
"#,
    );
    let rom_path = workspace.join("test/docboy/dmg/interactive/a.gb");
    fs::create_dir_all(rom_path.parent().expect("rom should have parent"))
        .expect("rom parent should be creatable");
    fs::write(
        &rom_path,
        build_joypad_a_pressed_memory_write_rom(0xC000, 1),
    )
    .expect("rom should be writable");
    write_materialized_source_manifest(
        &workspace,
        "docboy",
        "docboy/sources.report.toml",
        &[("docboy-dmg", "dmg")],
    );

    let mut output = Vec::new();
    run_suite_command_with_workspace_for_test(
        [
            "docboy",
            "--suite",
            "docboy-dmg",
            "--case",
            "docboy-joypad-stimulus",
        ],
        &workspace,
        &mut output,
    )
    .expect("joypad stimulus suite should pass");

    let output = String::from_utf8(output).expect("output should be utf-8");
    assert!(output.contains("suite docboy-dmg: 1/1 passed"));

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn command_reports_memory_byte_fail_value_as_failed_case() {
    let workspace = unique_temp_dir("memory-byte-fail-value");
    write_reports(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
    );
    write_manifest(
        &workspace,
        "sample-report/docboy.suite.toml",
        r#"
family = "docboy-dmg"
suite_name = "docboy-dmg"
report = "sample-report"
model = "dmg"
timeout_frames = 1
oracle = { type = "memory-byte-equals", address = 65520, value = 1, fail_value = 2 }

[[case]]
id = "docboy-fail-value"
rom = "memory/fail.gb"
"#,
    );
    let rom_path = workspace.join("test/sample-report/docboy-dmg/memory/fail.gb");
    fs::create_dir_all(rom_path.parent().expect("rom should have parent"))
        .expect("rom parent should be creatable");
    fs::write(&rom_path, build_memory_write_rom(0xFFF0, 2)).expect("rom should be writable");
    write_materialized_source_manifest(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
        &[("docboy-dmg", "docboy-dmg")],
    );

    let mut output = Vec::new();
    let error = run_suite_command_with_workspace_for_test(
        [
            "sample-report",
            "--suite",
            "docboy-dmg",
            "--case",
            "docboy-fail-value",
        ],
        &workspace,
        &mut output,
    )
    .expect_err("memory byte fail value should fail");
    assert!(error.contains("one or more suite cases failed"));
    let output = String::from_utf8(output).expect("output should be utf-8");
    assert!(output.contains("case docboy-fail-value: FAIL"));
    assert!(output.contains("fail_value 0x02"));
    let status = fs::read_to_string(workspace.join("test/sample-report/.status/docboy-dmg.json"))
        .expect("status should be written");
    assert!(status.contains("\"status\": \"FAIL\""));

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn command_reports_memory_byte_timeout_as_failed_case() {
    let workspace = unique_temp_dir("memory-byte-timeout");
    write_reports(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
    );
    write_manifest(
        &workspace,
        "sample-report/docboy.suite.toml",
        r#"
family = "docboy-dmg"
suite_name = "docboy-dmg"
report = "sample-report"
model = "dmg"
timeout_frames = 1
oracle = { type = "memory-byte-equals", address = 65520, value = 1, fail_value = 2 }

[[case]]
id = "docboy-timeout"
rom = "memory/timeout.gb"
"#,
    );
    let rom_path = workspace.join("test/sample-report/docboy-dmg/memory/timeout.gb");
    fs::create_dir_all(rom_path.parent().expect("rom should have parent"))
        .expect("rom parent should be creatable");
    fs::write(&rom_path, build_infinite_loop_rom()).expect("rom should be writable");
    write_materialized_source_manifest(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
        &[("docboy-dmg", "docboy-dmg")],
    );

    let mut output = Vec::new();
    run_suite_command_with_workspace_for_test(
        [
            "sample-report",
            "--suite",
            "docboy-dmg",
            "--case",
            "docboy-timeout",
        ],
        &workspace,
        &mut output,
    )
    .expect_err("memory byte timeout should fail");
    let output = String::from_utf8(output).expect("output should be utf-8");
    assert!(output.contains("case docboy-timeout: FAIL"));
    assert!(output.contains("memory byte mismatch"));
    let status = fs::read_to_string(workspace.join("test/sample-report/.status/docboy-dmg.json"))
        .expect("status should be written");
    assert!(status.contains("\"status\": \"FAIL\""));

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn command_reports_fibonacci_failure_signature_as_failed_case() {
    let workspace = unique_temp_dir("fibonacci-fail");
    write_reports(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
    );
    write_manifest(
        &workspace,
        "sample-report/mooneye.suite.toml",
        r#"
family = "mooneye"
suite_name = "mooneye"
report = "sample-report"
model = "dmg"
timeout_frames = 2
oracle = { type = "fibonacci-result" }

[[case]]
id = "mooneye-fail"
rom = "acceptance/fail.gb"
"#,
    );
    let rom_path = workspace.join("test/sample-report/mooneye/acceptance/fail.gb");
    fs::create_dir_all(rom_path.parent().expect("rom should have parent"))
        .expect("rom parent should be creatable");
    fs::write(&rom_path, build_fibonacci_result_rom([0x42; 6])).expect("rom should be writable");
    write_materialized_source_manifest(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
        &[("mooneye", "mooneye")],
    );

    let mut output = Vec::new();
    let error = run_suite_command_with_workspace_for_test(
        [
            "sample-report",
            "--suite",
            "mooneye",
            "--case",
            "mooneye-fail",
        ],
        &workspace,
        &mut output,
    )
    .expect_err("fibonacci failure signature should fail");
    assert!(error.contains("one or more suite cases failed"));
    let output = String::from_utf8(output).expect("output should be utf-8");
    assert!(output.contains("case mooneye-fail: FAIL"));
    assert!(output.contains("failure signature"));
    let status = fs::read_to_string(workspace.join("test/sample-report/.status/mooneye.json"))
        .expect("status should be written");
    assert!(status.contains("\"status\": \"FAIL\""));

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn command_reports_fibonacci_timeout_without_result_as_failed_case() {
    let workspace = unique_temp_dir("fibonacci-timeout");
    write_reports(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
    );
    write_manifest(
        &workspace,
        "sample-report/mooneye.suite.toml",
        r#"
family = "mooneye"
suite_name = "mooneye"
report = "sample-report"
model = "dmg"
timeout_frames = 1
oracle = { type = "fibonacci-result" }

[[case]]
id = "mooneye-timeout"
rom = "acceptance/timeout.gb"
"#,
    );
    let rom_path = workspace.join("test/sample-report/mooneye/acceptance/timeout.gb");
    fs::create_dir_all(rom_path.parent().expect("rom should have parent"))
        .expect("rom parent should be creatable");
    fs::write(&rom_path, build_infinite_loop_rom()).expect("rom should be writable");
    write_materialized_source_manifest(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
        &[("mooneye", "mooneye")],
    );

    let mut output = Vec::new();
    run_suite_command_with_workspace_for_test(
        [
            "sample-report",
            "--suite",
            "mooneye",
            "--case",
            "mooneye-timeout",
        ],
        &workspace,
        &mut output,
    )
    .expect_err("missing fibonacci result should fail");
    let output = String::from_utf8(output).expect("output should be utf-8");
    assert!(output.contains("case mooneye-timeout: FAIL"));
    assert!(output.contains("fibonacci result was not reached"));

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

fn write_local_report(workspace: &std::path::Path, report_id: &str) {
    let path = workspace.join(super::super::model::REPORTS_MANIFEST_PATH);
    fs::create_dir_all(path.parent().expect("reports should have parent"))
        .expect("reports parent should be creatable");
    fs::write(
        path,
        format!(
            concat!(
                "status_dir = \".status\"\n",
                "artifact_dir = \".artifacts\"\n",
                "report_file = \"test-report.md\"\n",
                "\n",
                "[[report]]\n",
                "id = {:?}\n",
                "local = true\n",
                "store_dir = {:?}\n",
            ),
            report_id, report_id
        ),
    )
    .expect("reports manifest should be writable");
}
