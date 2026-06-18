use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

use crate::oracle::Oracle;

use super::super::manifest::{
    load_reports, load_selected_suite_families, load_selected_suites, parse_suite_manifest_for_test,
};
use super::super::model::{ReportModel, SuiteStimulusTime};
use super::common::{
    basic_manifest, unique_temp_dir, write_manifest, write_reports, write_source_manifest,
};

#[test]
fn reports_manifest_inherits_and_overrides_artifact_dir() {
    let workspace = unique_temp_dir("suite-report-artifacts");
    let reports_path = workspace.join(super::super::model::REPORTS_MANIFEST_PATH);
    fs::create_dir_all(reports_path.parent().expect("reports should have parent"))
        .expect("reports parent should be creatable");
    fs::write(
        &reports_path,
        r#"
status_dir = ".status"
artifact_dir = ".artifacts"

[[report]]
id = "default-artifacts"
store_dir = "default-artifacts"
sources = "default-artifacts/sources.report.toml"

[[report]]
id = "custom-artifacts"
store_dir = "custom-artifacts"
sources = "custom-artifacts/sources.report.toml"
artifact_dir = ".custom-artifacts"
"#,
    )
    .expect("reports should be writable");

    let reports = load_reports(&workspace).expect("reports should load");
    let default_report = reports
        .iter()
        .find(|report| report.id == "default-artifacts")
        .expect("default report should exist");
    assert_eq!(default_report.artifact_dir, PathBuf::from(".artifacts"));
    let custom_report = reports
        .iter()
        .find(|report| report.id == "custom-artifacts")
        .expect("custom report should exist");
    assert_eq!(
        custom_report.artifact_dir,
        PathBuf::from(".custom-artifacts")
    );

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn reports_manifest_rejects_unsafe_runtime_paths() {
    let cases = [
        (
            "suite-report-empty-default-status",
            r#"
status_dir = ""
artifact_dir = ".artifacts"

[[report]]
id = "sample-report"
store_dir = "sample-report"
sources = "sample-report/sources.report.toml"
"#,
            "report default status_dir must not be empty",
        ),
        (
            "suite-report-parent-store",
            r#"
status_dir = ".status"
artifact_dir = ".artifacts"

[[report]]
id = "sample-report"
store_dir = "../sample-report"
sources = "sample-report/sources.report.toml"
"#,
            "report store_dir ../sample-report must not contain parent components",
        ),
        (
            "suite-report-parent-artifacts",
            r#"
status_dir = ".status"
artifact_dir = ".artifacts"

[[report]]
id = "sample-report"
store_dir = "sample-report"
sources = "sample-report/sources.report.toml"
artifact_dir = "../artifacts"
"#,
            "report artifact_dir ../artifacts must not contain parent components",
        ),
        (
            "suite-report-current-status",
            r#"
status_dir = ".status"
artifact_dir = ".artifacts"

[[report]]
id = "sample-report"
store_dir = "sample-report"
sources = "sample-report/sources.report.toml"
status_dir = "."
"#,
            "report status_dir . must not contain current-directory components",
        ),
    ];

    for (workspace_name, reports_toml, expected_error) in cases {
        let workspace = unique_temp_dir(workspace_name);
        let reports_path = workspace.join(super::super::model::REPORTS_MANIFEST_PATH);
        fs::create_dir_all(reports_path.parent().expect("reports should have parent"))
            .expect("reports parent should be creatable");
        fs::write(&reports_path, reports_toml).expect("reports should be writable");

        assert!(
            load_reports(&workspace)
                .expect_err("unsafe runtime path should fail")
                .contains(expected_error)
        );

        fs::remove_dir_all(workspace).expect("workspace should be removable");
    }
}

#[test]
fn reports_manifest_loads_local_report_without_sources() {
    let workspace = unique_temp_dir("suite-local-report");
    let reports_path = workspace.join(super::super::model::REPORTS_MANIFEST_PATH);
    fs::create_dir_all(reports_path.parent().expect("reports should have parent"))
        .expect("reports parent should be creatable");
    fs::write(
        &reports_path,
        r#"
status_dir = ".status"
artifact_dir = ".artifacts"

[[report]]
id = "linked"
local = true
store_dir = "linked"
"#,
    )
    .expect("reports should be writable");

    let reports = load_reports(&workspace).expect("local report should load");
    let linked = reports.first().expect("linked report should exist");
    assert_eq!(linked.id, "linked");
    assert!(linked.local);
    assert_eq!(linked.store_dir, PathBuf::from("linked"));
    assert_eq!(linked.sources, None);

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn reports_manifest_rejects_local_report_with_sources() {
    let workspace = unique_temp_dir("suite-local-report-sources");
    let reports_path = workspace.join(super::super::model::REPORTS_MANIFEST_PATH);
    fs::create_dir_all(reports_path.parent().expect("reports should have parent"))
        .expect("reports parent should be creatable");
    fs::write(
        &reports_path,
        r#"
status_dir = ".status"
artifact_dir = ".artifacts"

[[report]]
id = "linked"
local = true
store_dir = "linked"
sources = "linked/sources.report.toml"
"#,
    )
    .expect("reports should be writable");

    assert!(
        load_reports(&workspace)
            .expect_err("local report with sources should fail")
            .contains("must not define sources")
    );

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn reports_manifest_rejects_non_local_report_without_sources() {
    let workspace = unique_temp_dir("suite-report-missing-sources");
    let reports_path = workspace.join(super::super::model::REPORTS_MANIFEST_PATH);
    fs::create_dir_all(reports_path.parent().expect("reports should have parent"))
        .expect("reports parent should be creatable");
    fs::write(
        &reports_path,
        r#"
status_dir = ".status"
artifact_dir = ".artifacts"

[[report]]
id = "sample-report"
store_dir = "sample-report"
"#,
    )
    .expect("reports should be writable");

    assert!(
        load_reports(&workspace)
            .expect_err("non-local report without sources should fail")
            .contains("must define sources unless local = true")
    );

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn local_report_can_load_regular_suite_manifest_without_sources() {
    let workspace = unique_temp_dir("suite-local-report-regular-suite");
    let reports_path = workspace.join(super::super::model::REPORTS_MANIFEST_PATH);
    fs::create_dir_all(reports_path.parent().expect("reports should have parent"))
        .expect("reports parent should be creatable");
    fs::write(
        &reports_path,
        r#"
status_dir = ".status"
artifact_dir = ".artifacts"

[[report]]
id = "linked"
local = true
store_dir = "linked"
"#,
    )
    .expect("reports should be writable");
    write_manifest(
        &workspace,
        "linked/local.suite.toml",
        &basic_manifest("linked", "local", "linked", "linked-local-case", "local.gb"),
    );

    let reports = load_reports(&workspace).expect("local report should load");
    let report = reports.first().expect("local report should exist");
    let suites = load_selected_suites(&workspace, report, Some("local"), None)
        .expect("regular suite manifest should load for local report");

    assert_eq!(suites.len(), 1);
    assert_eq!(suites[0].suite_name, "local");
    assert_eq!(suites[0].family, "linked");
    assert_eq!(suites[0].cases[0].target_root, PathBuf::from("linked"));

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn parses_manifest_defaults_for_serial_contains_cases() {
    let manifest = parse_suite_manifest_for_test(
        Path::new("blargg-cpu-instrs.toml"),
        "gb-emulator-shootout",
        &basic_manifest(
            "gb-emulator-shootout",
            "blargg-cpu-instrs",
            "blargg",
            "blargg-cpu-instrs-01-special",
            "cpu_instrs/01-special.gb",
        ),
    )
    .expect("manifest should parse");

    assert_eq!(manifest.suite_name, "blargg-cpu-instrs");
    assert_eq!(manifest.family, "blargg");
    assert_eq!(manifest.cases.len(), 1);
    assert_eq!(manifest.cases[0].family, "blargg");
    assert_eq!(
        manifest.cases[0].execution_mode,
        gb_core::ExecutionMode::Strict
    );
    assert_eq!(manifest.cases[0].timeout_frames, 2);
}

#[test]
fn report_is_required_and_must_match_selected_report() {
    let missing_report = basic_manifest(
        "gb-emulator-shootout",
        "blargg-cpu-instrs",
        "blargg",
        "blargg-cpu-instrs-01-special",
        "cpu_instrs/01-special.gb",
    )
    .replace("report = \"gb-emulator-shootout\"\n", "");
    assert!(
        parse_suite_manifest_for_test(
            Path::new("blargg-cpu-instrs.suite.toml"),
            "gb-emulator-shootout",
            &missing_report,
        )
        .expect_err("missing report should fail")
        .contains("must define report")
    );

    let mismatched_report = basic_manifest(
        "docboy",
        "blargg-cpu-instrs",
        "blargg",
        "blargg-cpu-instrs-01-special",
        "cpu_instrs/01-special.gb",
    );
    assert!(
        parse_suite_manifest_for_test(
            Path::new("blargg-cpu-instrs.suite.toml"),
            "gb-emulator-shootout",
            &mismatched_report,
        )
        .expect_err("mismatched report should fail")
        .contains("declares report")
    );
}

#[test]
fn disabled_cases_are_cataloged_with_comments_and_skipped() {
    let manifest = r#"
family = "docboy-dmg"
suite_name = "docboy-dmg"
report = "docboy"
model = "dmg"
timeout_frames = 2
oracle = { type = "memory-byte-equals", address = 65520, value = 1 }

[[case]]
id = "docboy-disabled"
rom = "disabled.gb"
disabled = true
comment = "Upstream marks this row disabled."
oracle = { type = "unknown-disabled-oracle" }

[[case]]
id = "docboy-enabled"
rom = "enabled.gb"
"#;
    let suite =
        parse_suite_manifest_for_test(Path::new("docboy-dmg.suite.toml"), "docboy", manifest)
            .expect("disabled row should be skipped after validating its comment");

    assert_eq!(suite.cases.len(), 1);
    assert_eq!(suite.cases[0].id, "docboy-enabled");

    let all_disabled = manifest.replace(
        r#"
[[case]]
id = "docboy-enabled"
rom = "enabled.gb"
"#,
        r#"
[[case]]
id = "docboy-enabled"
rom = "enabled.gb"
disabled = true
comment = "Temporarily disabled pending investigation."
"#,
    );
    let suite =
        parse_suite_manifest_for_test(Path::new("docboy-dmg.suite.toml"), "docboy", &all_disabled)
            .expect("suite with only disabled rows should still load");
    assert!(suite.cases.is_empty());

    let missing_comment = manifest.replace("comment = \"Upstream marks this row disabled.\"\n", "");
    assert!(
        parse_suite_manifest_for_test(
            Path::new("docboy-dmg.suite.toml"),
            "docboy",
            &missing_comment,
        )
        .expect_err("disabled row without comment should fail")
        .contains("must include a non-empty comment")
    );

    let blank_comment = manifest.replace(
        "comment = \"Upstream marks this row disabled.\"",
        "comment = \"   \"",
    );
    assert!(
        parse_suite_manifest_for_test(
            Path::new("docboy-dmg.suite.toml"),
            "docboy",
            &blank_comment,
        )
        .expect_err("disabled row with blank comment should fail")
        .contains("must include a non-empty comment")
    );
}

#[test]
fn parses_startup_modes_and_rejects_unsupported_startup() {
    let custom_boot = basic_manifest(
        "gbmicrotest",
        "gbmicrotest",
        "gbmicrotest",
        "gbmicrotest-case",
        "case.gb",
    )
    .replace(
        "rom = \"case.gb\"",
        "rom = \"case.gb\"\nstartup = \"custom-boot\"",
    );
    let manifest = parse_suite_manifest_for_test(
        Path::new("gbmicrotest.suite.toml"),
        "gbmicrotest",
        &custom_boot,
    )
    .expect("custom boot should parse");
    assert_eq!(
        manifest.cases[0].startup_mode,
        gb_core::StartupMode::CustomBoot
    );

    let real_boot = basic_manifest(
        "gbmicrotest",
        "gbmicrotest",
        "gbmicrotest",
        "gbmicrotest-case",
        "case.gb",
    )
    .replace(
        "rom = \"case.gb\"",
        "rom = \"case.gb\"\nstartup = \"real-boot\"",
    );
    let manifest = parse_suite_manifest_for_test(
        Path::new("gbmicrotest.suite.toml"),
        "gbmicrotest",
        &real_boot,
    )
    .expect("real boot should parse");
    assert_eq!(
        manifest.cases[0].startup_mode,
        gb_core::StartupMode::RealBoot
    );

    let unsupported = basic_manifest(
        "gbmicrotest",
        "gbmicrotest",
        "gbmicrotest",
        "gbmicrotest-case",
        "case.gb",
    )
    .replace(
        "rom = \"case.gb\"",
        "rom = \"case.gb\"\nstartup = \"warm-boot\"",
    );
    assert!(
        parse_suite_manifest_for_test(
            Path::new("gbmicrotest.suite.toml"),
            "gbmicrotest",
            &unsupported
        )
        .expect_err("unknown startup should fail")
        .contains("unsupported startup")
    );
}

#[test]
fn parses_joypad_stimuli_and_rejects_unsupported_stimulus_shape() {
    let manifest = r#"
family = "docboy-dmg"
suite_name = "docboy-dmg"
report = "docboy"
model = "dmg"
timeout_frames = 2
oracle = { type = "memory-byte-equals", address = 65520, value = 1 }

[[case]]
id = "docboy-interactive"
rom = "cpu/interactive.gb"

[[case.stimulus]]
tcycle = 8192
button = "up"
pressed = true

[[case.stimulus]]
frame = 1
button = "a"
pressed = false
"#;
    let suite =
        parse_suite_manifest_for_test(Path::new("docboy-dmg.suite.toml"), "docboy", manifest)
            .expect("stimuli should parse");

    assert_eq!(suite.cases[0].stimuli.len(), 2);
    assert_eq!(
        suite.cases[0].stimuli[0].when,
        SuiteStimulusTime::TCycle(8192)
    );
    assert_eq!(suite.cases[0].stimuli[0].button, gb_core::JoypadButton::Up);
    assert!(suite.cases[0].stimuli[0].pressed);
    assert_eq!(suite.cases[0].stimuli[1].when, SuiteStimulusTime::Frame(1));
    assert_eq!(suite.cases[0].stimuli[1].button, gb_core::JoypadButton::A);
    assert!(!suite.cases[0].stimuli[1].pressed);

    let both_time_fields = manifest.replace(
        "tcycle = 8192\nbutton = \"up\"",
        "tcycle = 8192\nframe = 1\nbutton = \"up\"",
    );
    assert!(
        parse_suite_manifest_for_test(
            Path::new("docboy-dmg.suite.toml"),
            "docboy",
            &both_time_fields,
        )
        .expect_err("stimulus with both time fields should fail")
        .contains("either tcycle or frame")
    );

    let unsupported_button = manifest.replace("button = \"up\"", "button = \"turbo\"");
    assert!(
        parse_suite_manifest_for_test(
            Path::new("docboy-dmg.suite.toml"),
            "docboy",
            &unsupported_button,
        )
        .expect_err("unsupported joypad button should fail")
        .contains("unsupported joypad button")
    );
}

#[test]
fn parses_model_profiles_and_rejects_unsupported_model_and_oracle() {
    let dmg0_model = basic_manifest(
        "gb-emulator-shootout",
        "acid",
        "acid",
        "acid-dmg0",
        "which.gb",
    )
    .replace(
        "model = \"dmg\"",
        "model = \"dmg\"\nrevision = \"dmg-cpu-0\"",
    );
    let dmg0_manifest = parse_suite_manifest_for_test(
        Path::new("acid.suite.toml"),
        "gb-emulator-shootout",
        &dmg0_model,
    )
    .expect("DMG0 should parse");
    assert_eq!(
        dmg0_manifest.cases[0].hardware_revision,
        gb_core::HardwareRevision::DmgCpu0
    );

    let mgb_model = basic_manifest(
        "gb-emulator-shootout",
        "acid",
        "acid",
        "acid-mgb",
        "which.gb",
    )
    .replace("model = \"dmg\"", "model = \"mgb\"");
    let mgb_manifest = parse_suite_manifest_for_test(
        Path::new("acid.suite.toml"),
        "gb-emulator-shootout",
        &mgb_model,
    )
    .expect("mgb should parse");
    assert_eq!(
        mgb_manifest.cases[0].console_model,
        gb_core::ConsoleModel::GameBoyPocket
    );
    assert_eq!(
        mgb_manifest.cases[0].hardware_revision,
        gb_core::HardwareRevision::CpuMgb
    );
    assert_eq!(
        mgb_manifest.cases[0].host_platform,
        gb_core::HostPlatform::Handheld
    );
    assert_eq!(mgb_manifest.cases[0].report_model, ReportModel::Mgb);

    let cgb_model = basic_manifest(
        "gb-emulator-shootout",
        "acid",
        "acid",
        "acid-cgb",
        "cgb-acid2.gbc",
    )
    .replace("model = \"dmg\"", "model = \"cgb\"");
    let cgb_manifest = parse_suite_manifest_for_test(
        Path::new("acid.suite.toml"),
        "gb-emulator-shootout",
        &cgb_model,
    )
    .expect("cgb should parse");
    assert_eq!(
        cgb_manifest.cases[0].console_model,
        gb_core::ConsoleModel::GameBoyColor
    );
    assert_eq!(
        cgb_manifest.cases[0].host_platform,
        gb_core::HostPlatform::Handheld
    );
    assert_eq!(cgb_manifest.cases[0].report_model, ReportModel::Cgb);

    let cgb0_model = cgb_model.replace(
        "model = \"cgb\"",
        "model = \"cgb\"\nrevision = \"cpu-cgb-0\"",
    );
    let cgb0_manifest = parse_suite_manifest_for_test(
        Path::new("acid.suite.toml"),
        "gb-emulator-shootout",
        &cgb0_model,
    )
    .expect("CGB0 should parse");
    assert_eq!(
        cgb0_manifest.cases[0].hardware_revision,
        gb_core::HardwareRevision::CpuCgb0
    );

    let agb_model = basic_manifest(
        "gb-emulator-shootout",
        "acid",
        "acid",
        "acid-agb",
        "cgb-acid2.gbc",
    )
    .replace("model = \"dmg\"", "model = \"agb\"");
    let agb_manifest = parse_suite_manifest_for_test(
        Path::new("acid.suite.toml"),
        "gb-emulator-shootout",
        &agb_model,
    )
    .expect("agb should parse");
    assert_eq!(
        agb_manifest.cases[0].console_model,
        gb_core::ConsoleModel::GameBoyAdvance
    );
    assert_eq!(
        agb_manifest.cases[0].hardware_revision,
        gb_core::HardwareRevision::CpuAgbA
    );
    let agb0_model = basic_manifest(
        "gb-emulator-shootout",
        "acid",
        "acid",
        "acid-agb0",
        "cgb-acid2.gbc",
    )
    .replace(
        "model = \"dmg\"",
        "model = \"agb\"\nrevision = \"cpu-agb-0\"",
    );
    let agb0_manifest = parse_suite_manifest_for_test(
        Path::new("acid.suite.toml"),
        "gb-emulator-shootout",
        &agb0_model,
    )
    .expect("agb0 should parse");
    assert_eq!(
        agb0_manifest.cases[0].hardware_revision,
        gb_core::HardwareRevision::CpuAgb0
    );
    assert_eq!(
        agb_manifest.cases[0].host_platform,
        gb_core::HostPlatform::Handheld
    );
    assert_eq!(agb_manifest.cases[0].report_model, ReportModel::Agb);

    let sgb_model = basic_manifest(
        "gb-emulator-shootout",
        "samesuite",
        "samesuite",
        "samesuite-sgb",
        "sgb/test.gb",
    )
    .replace("model = \"dmg\"", "model = \"sgb\"");
    let sgb_manifest = parse_suite_manifest_for_test(
        Path::new("samesuite.suite.toml"),
        "gb-emulator-shootout",
        &sgb_model,
    )
    .expect("sgb should parse");
    assert_eq!(
        sgb_manifest.cases[0].console_model,
        gb_core::ConsoleModel::GameBoy
    );
    assert_eq!(
        sgb_manifest.cases[0].host_platform,
        gb_core::HostPlatform::Sgb
    );
    assert_eq!(sgb_manifest.cases[0].report_model, ReportModel::Sgb);

    let sgb2_model = basic_manifest(
        "gb-emulator-shootout",
        "samesuite",
        "samesuite",
        "samesuite-sgb2",
        "sgb/test.gb",
    )
    .replace("model = \"dmg\"", "model = \"sgb2\"");
    let sgb2_manifest = parse_suite_manifest_for_test(
        Path::new("samesuite.suite.toml"),
        "gb-emulator-shootout",
        &sgb2_model,
    )
    .expect("sgb2 should parse");
    assert_eq!(
        sgb2_manifest.cases[0].console_model,
        gb_core::ConsoleModel::GameBoy
    );
    assert_eq!(
        sgb2_manifest.cases[0].host_platform,
        gb_core::HostPlatform::Sgb2
    );
    assert_eq!(sgb2_manifest.cases[0].report_model, ReportModel::Sgb2);

    let sgb_dmg0_model = sgb_model.replace(
        "model = \"sgb\"",
        "model = \"sgb\"\nrevision = \"dmg-cpu-0\"",
    );
    assert!(
        parse_suite_manifest_for_test(
            Path::new("samesuite.suite.toml"),
            "gb-emulator-shootout",
            &sgb_dmg0_model,
        )
        .expect_err("SGB should reject DMG0")
        .contains("does not support revision DmgCpu0")
    );

    let cgb_agb0_model = agb0_model.replace("model = \"agb\"", "model = \"cgb\"");
    assert!(
        parse_suite_manifest_for_test(
            Path::new("acid.suite.toml"),
            "gb-emulator-shootout",
            &cgb_agb0_model,
        )
        .expect_err("CGB should reject AGB0")
        .contains("does not support revision CpuAgb0")
    );

    let report_suffix = basic_manifest(
        "gb-emulator-shootout",
        "acid",
        "acid",
        "acid-which-dmg",
        "which.gb",
    )
    .replace(
        "rom = \"which.gb\"",
        "rom = \"which.gb\"\nreport_model_suffix = true",
    );
    let report_suffix_manifest = parse_suite_manifest_for_test(
        Path::new("acid.suite.toml"),
        "gb-emulator-shootout",
        &report_suffix,
    )
    .expect("case-level report model suffix should parse");
    assert_eq!(
        report_suffix_manifest.cases[0].report_model,
        ReportModel::Dmg
    );
    assert!(report_suffix_manifest.cases[0].report_model_suffix);
    assert_eq!(
        report_suffix_manifest.cases[0].report_rom(),
        "which.gb (DMG)"
    );

    let inherited_report_suffix = basic_manifest(
        "gb-emulator-shootout",
        "acid",
        "acid",
        "acid-which-dmg",
        "which.gb",
    )
    .replace(
        "model = \"dmg\"",
        "model = \"dmg\"\nreport_model_suffix = true",
    );
    let inherited_report_suffix_manifest = parse_suite_manifest_for_test(
        Path::new("acid.suite.toml"),
        "gb-emulator-shootout",
        &inherited_report_suffix,
    )
    .expect("header report model suffix should parse");
    assert!(inherited_report_suffix_manifest.cases[0].report_model_suffix);

    let inherited_revision_suffix = basic_manifest(
        "gb-emulator-shootout",
        "acid",
        "acid",
        "acid-which-dmg",
        "which.gb",
    )
    .replace(
        "model = \"dmg\"",
        "model = \"dmg\"\nreport_revision_suffix = true",
    );
    let inherited_revision_suffix_manifest = parse_suite_manifest_for_test(
        Path::new("acid.suite.toml"),
        "gb-emulator-shootout",
        &inherited_revision_suffix,
    )
    .expect("header report revision suffix should parse");
    assert!(inherited_revision_suffix_manifest.cases[0].report_revision_suffix);
    assert_eq!(
        inherited_revision_suffix_manifest.cases[0].report_rom(),
        "which.gb (DMG-CPU-C)"
    );

    let case_revision_suffix = basic_manifest(
        "gb-emulator-shootout",
        "acid",
        "acid",
        "acid-which-dmg",
        "which.gb",
    )
    .replace(
        "rom = \"which.gb\"",
        "rom = \"which.gb\"\nreport_revision_suffix = true",
    );
    let case_revision_suffix_manifest = parse_suite_manifest_for_test(
        Path::new("acid.suite.toml"),
        "gb-emulator-shootout",
        &case_revision_suffix,
    )
    .expect("case-level report revision suffix should parse");
    assert!(case_revision_suffix_manifest.cases[0].report_revision_suffix);

    let model_and_revision_suffix = basic_manifest(
        "gb-emulator-shootout",
        "acid",
        "acid",
        "acid-which-cgb-d",
        "which.gb",
    )
    .replace(
        "model = \"dmg\"",
        "model = \"cgb\"\nrevision = \"cpu-cgb-d\"\nreport_model_suffix = true\nreport_revision_suffix = true",
    );
    let model_and_revision_suffix_manifest = parse_suite_manifest_for_test(
        Path::new("acid.suite.toml"),
        "gb-emulator-shootout",
        &model_and_revision_suffix,
    )
    .expect("combined report suffixes should parse");
    assert_eq!(
        model_and_revision_suffix_manifest.cases[0].report_rom(),
        "which.gb (GBC) (CPU-CGB-D)"
    );

    let overridden_report_suffix = inherited_report_suffix.replace(
        "rom = \"which.gb\"",
        "rom = \"which.gb\"\nreport_model_suffix = false",
    );
    let overridden_report_suffix_manifest = parse_suite_manifest_for_test(
        Path::new("acid.suite.toml"),
        "gb-emulator-shootout",
        &overridden_report_suffix,
    )
    .expect("case-level report model suffix override should parse");
    assert!(!overridden_report_suffix_manifest.cases[0].report_model_suffix);

    let overridden_revision_suffix = inherited_revision_suffix.replace(
        "rom = \"which.gb\"",
        "rom = \"which.gb\"\nreport_revision_suffix = false",
    );
    let overridden_revision_suffix_manifest = parse_suite_manifest_for_test(
        Path::new("acid.suite.toml"),
        "gb-emulator-shootout",
        &overridden_revision_suffix,
    )
    .expect("case-level report revision suffix override should parse");
    assert!(!overridden_revision_suffix_manifest.cases[0].report_revision_suffix);
    assert_eq!(
        overridden_revision_suffix_manifest.cases[0].report_rom(),
        "which.gb"
    );

    let unsupported_alias = basic_manifest(
        "gb-emulator-shootout",
        "acid",
        "acid",
        "acid-gb",
        "which.gb",
    )
    .replace("model = \"dmg\"", "model = \"gb\"");
    assert!(
        parse_suite_manifest_for_test(
            Path::new("acid.suite.toml"),
            "gb-emulator-shootout",
            &unsupported_alias
        )
        .expect_err("gb alias should fail")
        .contains("unsupported model")
    );

    let unsupported_oracle = basic_manifest(
        "gb-emulator-shootout",
        "acid",
        "acid",
        "acid-which",
        "which.gb",
    )
    .replace("serial-contains", "info-framebuffer");
    assert!(
        parse_suite_manifest_for_test(
            Path::new("acid.suite.toml"),
            "gb-emulator-shootout",
            &unsupported_oracle
        )
        .expect_err("framebuffer should fail")
        .contains("unsupported suite oracle")
    );

    let unsupported_execution_mode = basic_manifest(
        "gb-emulator-shootout",
        "acid",
        "acid",
        "acid-which",
        "which.gb",
    )
    .replace(
        "timeout_frames = 2",
        "execution_mode = \"fast\"\ntimeout_frames = 2",
    );
    assert!(
        parse_suite_manifest_for_test(
            Path::new("acid.suite.toml"),
            "gb-emulator-shootout",
            &unsupported_execution_mode
        )
        .expect_err("execution mode should fail")
        .contains("unsupported execution_mode")
    );
}

#[test]
fn parser_rejects_unknown_manifest_keys() {
    let unknown_header_key = basic_manifest(
        "gb-emulator-shootout",
        "acid",
        "acid",
        "acid-which-dmg",
        "which.gb",
    )
    .replace("model = \"dmg\"", "model = \"dmg\"\nmodel_typo = \"dmg\"");
    assert!(
        parse_suite_manifest_for_test(
            Path::new("acid.suite.toml"),
            "gb-emulator-shootout",
            &unknown_header_key,
        )
        .expect_err("unknown header key should fail")
        .contains("uses unsupported key \"model_typo\"")
    );

    let unknown_revision_suffix_key = basic_manifest(
        "gb-emulator-shootout",
        "acid",
        "acid",
        "acid-which-dmg",
        "which.gb",
    )
    .replace(
        "model = \"dmg\"",
        "model = \"dmg\"\nreport_revision_extra = true",
    );
    assert!(
        parse_suite_manifest_for_test(
            Path::new("acid.suite.toml"),
            "gb-emulator-shootout",
            &unknown_revision_suffix_key,
        )
        .expect_err("unknown report revision suffix key should fail")
        .contains("uses unsupported key \"report_revision_extra\"")
    );

    let unknown_case_key = basic_manifest(
        "gb-emulator-shootout",
        "acid",
        "acid",
        "acid-which-dmg",
        "which.gb",
    )
    .replace("rom = \"which.gb\"", "rom = \"which.gb\"\ncase_typo = true");
    assert!(
        parse_suite_manifest_for_test(
            Path::new("acid.suite.toml"),
            "gb-emulator-shootout",
            &unknown_case_key,
        )
        .expect_err("unknown case key should fail")
        .contains("uses unsupported key \"case_typo\"")
    );
}

#[test]
fn framebuffer_fixtures_are_resolved_from_case_family_store_root() {
    let workspace = unique_temp_dir("suite-fixture-root");
    write_reports(
        &workspace,
        "gb-emulator-shootout",
        "gb-emulator-shootout/sources.report.toml",
    );
    write_manifest(
        &workspace,
        "gb-emulator-shootout/fixture-root.suite.toml",
        r#"
family = "blargg"
suite_name = "fixture-root"
report = "gb-emulator-shootout"
model = "dmg"
timeout_frames = 2
oracle = { type = "framebuffer", fixture = "screens/pass.png" }

[[case]]
id = "fixture-root-case"
rom = "screens/pass.gb"
"#,
    );
    write_png_fixture(
        &workspace
            .join("test")
            .join("gb-emulator-shootout")
            .join("blargg")
            .join("screens")
            .join("pass.png"),
    );

    let reports = load_reports(&workspace).expect("reports should load");
    let report = reports
        .iter()
        .find(|report| report.id == "gb-emulator-shootout")
        .expect("report should exist");
    let suites = load_selected_suites(&workspace, report, Some("fixture-root"), None)
        .expect("suite should load fixture relative to the family store root");

    assert_eq!(suites.len(), 1);
    assert!(matches!(&suites[0].cases[0].oracle, Oracle::Framebuffer(_)));

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn framebuffer_local_fixtures_are_resolved_from_report_data_dir() {
    let workspace = unique_temp_dir("suite-local-fixture-root");
    write_reports(
        &workspace,
        "gb-emulator-shootout",
        "gb-emulator-shootout/sources.report.toml",
    );
    write_manifest(
        &workspace,
        "gb-emulator-shootout/local-fixture-root.suite.toml",
        r#"
family = "cpp"
suite_name = "local-fixture-root"
report = "gb-emulator-shootout"
model = "sgb"
timeout_frames = 2
oracle = { type = "framebuffer", local = true, fixture = "fixtures/cpp/pass.png" }

[[case]]
id = "local-fixture-root-case"
rom = "sgb/pass.gb"
"#,
    );
    let local_fixture = workspace
        .join("crates")
        .join("gb-test-runner")
        .join("data")
        .join("gb-emulator-shootout")
        .join("fixtures")
        .join("cpp")
        .join("pass.png");
    write_png_fixture(&local_fixture);

    let reports = load_reports(&workspace).expect("reports should load");
    let report = reports
        .iter()
        .find(|report| report.id == "gb-emulator-shootout")
        .expect("report should exist");
    let suites = load_selected_suites(&workspace, report, Some("local-fixture-root"), None)
        .expect("suite should load fixture relative to report data dir");

    let descriptor = suites[0].cases[0]
        .oracle
        .framebuffer_artifact_descriptor()
        .expect("framebuffer descriptor should exist");
    assert_eq!(descriptor.fixtures, vec![local_fixture]);

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn framebuffer_local_fixture_flag_is_inherited_by_partial_case_oracle() {
    let workspace = unique_temp_dir("suite-local-fixture-inheritance");
    write_reports(
        &workspace,
        "gb-emulator-shootout",
        "gb-emulator-shootout/sources.report.toml",
    );
    write_manifest(
        &workspace,
        "gb-emulator-shootout/local-fixture-inheritance.suite.toml",
        r#"
family = "samesuite"
suite_name = "local-fixture-inheritance"
report = "gb-emulator-shootout"
model = "sgb"
timeout_frames = 2
oracle = { type = "framebuffer", local = true }

[[case]]
id = "local-fixture-inheritance-case"
rom = "sgb/pass.gb"
oracle = { fixture = "fixtures/samesuite/pass.png" }
"#,
    );
    let local_fixture = workspace
        .join("crates")
        .join("gb-test-runner")
        .join("data")
        .join("gb-emulator-shootout")
        .join("fixtures")
        .join("samesuite")
        .join("pass.png");
    write_png_fixture(&local_fixture);

    let reports = load_reports(&workspace).expect("reports should load");
    let report = reports
        .iter()
        .find(|report| report.id == "gb-emulator-shootout")
        .expect("report should exist");
    let suites = load_selected_suites(&workspace, report, Some("local-fixture-inheritance"), None)
        .expect("partial case oracle should inherit local fixture flag");

    let descriptor = suites[0].cases[0]
        .oracle
        .framebuffer_artifact_descriptor()
        .expect("framebuffer descriptor should exist");
    assert_eq!(descriptor.fixtures, vec![local_fixture]);

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn source_target_root_defines_report_local_rom_root() {
    let workspace = unique_temp_dir("suite-target-root");
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

[[source]]
id = "extra-docboy"

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
timeout_frames = 2
oracle = { type = "memory-byte-equals", address = 65520, value = 1 }

[[case]]
id = "docboy-memory"
rom = "memory/pass.gb"
"#,
    );

    let reports = load_reports(&workspace).expect("reports should load");
    let report = reports
        .iter()
        .find(|report| report.id == "docboy")
        .expect("report should exist");
    let suites = load_selected_suites(&workspace, report, Some("docboy-dmg"), None)
        .expect("suite should load target roots from report sources");

    assert_eq!(suites.len(), 1);
    assert_eq!(suites[0].cases[0].target_root, PathBuf::from("dmg"));

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn partial_case_oracle_inherits_global_type_and_parameters() {
    let manifest = r#"
family = "blargg"
suite_name = "oracle-inheritance"
report = "gb-emulator-shootout"
model = "dmg"
timeout_frames = 2
oracle = { type = "framebuffer", mode = "until-match", source = "invalid-source" }

[[case]]
id = "oracle-inheritance-case"
rom = "screens/pass.gb"
oracle = { fixture = "screens/pass.png" }
"#;

    assert!(
        parse_suite_manifest_for_test(
            Path::new("oracle-inheritance.suite.toml"),
            "gb-emulator-shootout",
            manifest
        )
        .expect_err("invalid inherited source should fail")
        .contains("unsupported framebuffer source")
    );
}

#[test]
fn partial_case_oracle_overrides_global_parameter_values() {
    let workspace = unique_temp_dir("suite-oracle-overrides");
    write_reports(
        &workspace,
        "gb-emulator-shootout",
        "gb-emulator-shootout/sources.report.toml",
    );
    write_manifest(
        &workspace,
        "gb-emulator-shootout/oracle-overrides.suite.toml",
        r#"
family = "blargg"
suite_name = "oracle-overrides"
report = "gb-emulator-shootout"
model = "dmg"
timeout_frames = 2
oracle = { type = "framebuffer", mode = "until-match", fixture = "screens/missing.png" }

[[case]]
id = "oracle-overrides-case"
rom = "screens/pass.gb"
oracle = { fixture = "screens/pass.png" }
"#,
    );
    write_png_fixture(
        &workspace
            .join("test")
            .join("gb-emulator-shootout")
            .join("blargg")
            .join("screens")
            .join("pass.png"),
    );

    let reports = load_reports(&workspace).expect("reports should load");
    let report = reports
        .iter()
        .find(|report| report.id == "gb-emulator-shootout")
        .expect("report should exist");
    let suites = load_selected_suites(&workspace, report, Some("oracle-overrides"), None)
        .expect("case fixture should override missing global fixture");

    assert_eq!(suites.len(), 1);
    assert!(matches!(&suites[0].cases[0].oracle, Oracle::Framebuffer(_)));

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn case_oracle_with_type_replaces_global_oracle() {
    let manifest = r#"
family = "blargg"
suite_name = "oracle-replacement"
report = "gb-emulator-shootout"
model = "dmg"
timeout_frames = 2
oracle = { type = "framebuffer", source = "invalid-source", fixture = "screens/missing.png" }

[[case]]
id = "oracle-replacement-case"
rom = "cpu_instrs/01-special.gb"
oracle = { type = "serial-contains", expected = "Passed" }
"#;
    let suite = parse_suite_manifest_for_test(
        Path::new("oracle-replacement.suite.toml"),
        "gb-emulator-shootout",
        manifest,
    )
    .expect("case oracle with type should replace invalid global oracle");

    assert!(matches!(&suite.cases[0].oracle, Oracle::SerialContains(_)));
}

#[test]
fn partial_case_oracle_requires_global_oracle_with_type() {
    let no_global = r#"
family = "blargg"
suite_name = "oracle-no-global"
report = "gb-emulator-shootout"
model = "dmg"
timeout_frames = 2

[[case]]
id = "oracle-no-global-case"
rom = "screens/pass.gb"
oracle = { fixture = "screens/pass.png" }
"#;
    assert!(
        parse_suite_manifest_for_test(
            Path::new("oracle-no-global.suite.toml"),
            "gb-emulator-shootout",
            no_global
        )
        .expect_err("partial oracle without global should fail")
        .contains("oracle override requires a global oracle with type")
    );

    let global_without_type = r#"
family = "blargg"
suite_name = "oracle-global-without-type"
report = "gb-emulator-shootout"
model = "dmg"
timeout_frames = 2
oracle = { fixture = "screens/pass.png" }

[[case]]
id = "oracle-global-without-type-case"
rom = "screens/pass.gb"
"#;
    assert!(
        parse_suite_manifest_for_test(
            Path::new("oracle-global-without-type.suite.toml"),
            "gb-emulator-shootout",
            global_without_type
        )
        .expect_err("global oracle without type should fail")
        .contains("global oracle must define type")
    );
}

#[test]
fn real_cpp_suite_manifest_loads_sgb_case() {
    let workspace = unique_temp_dir("cpp-suite-manifest");
    write_reports(
        &workspace,
        "gb-emulator-shootout",
        "gb-emulator-shootout/sources.report.toml",
    );
    write_manifest(
        &workspace,
        "gb-emulator-shootout/cpp.suite.toml",
        include_str!("../../../data/gb-emulator-shootout/cpp.suite.toml"),
    );
    write_fixture_placeholders(
        &workspace,
        &[
            "test/gb-emulator-shootout/cpp/rtc-invalid-banks-test.png",
            "test/gb-emulator-shootout/cpp/latch-rtc-test.png",
            "test/gb-emulator-shootout/cpp/ramg-mbc3-test.png",
            "test/gb-emulator-shootout/cpp/sgb-ext-test.png",
        ],
    );

    let reports = load_reports(&workspace).expect("reports should load");
    let report = reports
        .iter()
        .find(|report| report.id == "gb-emulator-shootout")
        .expect("report should exist");
    let suites = load_selected_suites(&workspace, report, Some("cpp"), None)
        .expect("real cpp manifest should load");

    assert_eq!(suites.len(), 1);
    let suite = &suites[0];
    assert_eq!(suite.suite_name, "cpp");
    assert_eq!(suite.family, "cpp");
    assert_eq!(suite.cases.len(), 4);
    assert!(
        suite
            .cases
            .iter()
            .all(|case| matches!(&case.oracle, Oracle::Framebuffer(_)))
    );

    let dmg_case = suite
        .cases
        .iter()
        .find(|case| case.id == "cpp-rtc-invalid-banks-test")
        .expect("DMG CPP case should exist");
    assert_eq!(dmg_case.console_model, gb_core::ConsoleModel::GameBoy);
    assert_eq!(dmg_case.host_platform, gb_core::HostPlatform::Handheld);
    assert_eq!(dmg_case.timeout_frames, 30);

    let sgb_case = suite
        .cases
        .iter()
        .find(|case| case.id == "cpp-sgb-ext-test")
        .expect("SGB CPP case should exist");
    assert_eq!(sgb_case.console_model, gb_core::ConsoleModel::GameBoy);
    assert_eq!(sgb_case.host_platform, gb_core::HostPlatform::Sgb);
    assert_eq!(sgb_case.timeout_frames, 240);

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn real_blargg_cpu_instrs_manifest_loads() {
    let manifest = parse_suite_manifest_for_test(
        Path::new("crates/gb-test-runner/data/gb-emulator-shootout/blargg-cpu-instrs.suite.toml"),
        "gb-emulator-shootout",
        include_str!("../../../data/gb-emulator-shootout/blargg-cpu-instrs.suite.toml"),
    )
    .expect("real blargg CPU instructions manifest should parse");

    assert_eq!(manifest.suite_name, "blargg-cpu-instrs");
    assert_eq!(manifest.family, "blargg");
    assert!(
        manifest
            .cases
            .iter()
            .any(|case| case.id == "blargg-cpu-instrs-01-special")
    );
}

#[test]
fn real_blargg_timing_memory_oam_manifest_loads_mixed_oracles() {
    let workspace = unique_temp_dir("blargg-timing-manifest");
    write_reports(
        &workspace,
        "gb-emulator-shootout",
        "gb-emulator-shootout/sources.report.toml",
    );
    write_manifest(
        &workspace,
        "gb-emulator-shootout/blargg-timing-memory-oam.suite.toml",
        include_str!("../../../data/gb-emulator-shootout/blargg-timing-memory-oam.suite.toml"),
    );
    write_blargg_timing_fixtures(&workspace);

    let reports = load_reports(&workspace).expect("reports should load");
    let report = reports
        .iter()
        .find(|report| report.id == "gb-emulator-shootout")
        .expect("report should exist");
    let suites = load_selected_suites(&workspace, report, Some("blargg-timing-memory-oam"), None)
        .expect("real blargg timing manifest should load");

    assert_eq!(suites.len(), 1);
    let suite = &suites[0];
    assert_eq!(suite.suite_name, "blargg-timing-memory-oam");
    assert_eq!(suite.family, "blargg");
    assert_eq!(suite.cases.len(), 16);

    let instr_timing = suite
        .cases
        .iter()
        .find(|case| case.id == "blargg-instr-timing")
        .expect("instr_timing should exist");
    assert!(matches!(&instr_timing.oracle, Oracle::SerialContains(_)));

    let halt_bug = suite
        .cases
        .iter()
        .find(|case| case.id == "blargg-halt-bug")
        .expect("halt_bug should exist");
    assert_eq!(halt_bug.execution_mode, gb_core::ExecutionMode::Permissive);
    assert!(matches!(&halt_bug.oracle, Oracle::Framebuffer(_)));

    let interrupt_time = suite
        .cases
        .iter()
        .find(|case| case.id == "blargg-interrupt-time")
        .expect("interrupt_time should exist");
    assert_eq!(
        interrupt_time.console_model,
        gb_core::ConsoleModel::GameBoyColor
    );
    assert_eq!(
        interrupt_time.execution_mode,
        gb_core::ExecutionMode::Permissive
    );
    assert_eq!(interrupt_time.timeout_frames, 1200);
    assert!(matches!(&interrupt_time.oracle, Oracle::Framebuffer(_)));

    let mem_timing_2 = suite
        .cases
        .iter()
        .find(|case| case.id == "blargg-mem-timing-2-01-read-timing")
        .expect("mem_timing-2 should exist");
    assert!(matches!(&mem_timing_2.oracle, Oracle::Framebuffer(_)));

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn real_blargg_sound_manifests_load_framebuffer_oracles() {
    let workspace = unique_temp_dir("blargg-sound-manifests");
    write_reports(
        &workspace,
        "gb-emulator-shootout",
        "gb-emulator-shootout/sources.report.toml",
    );
    write_manifest(
        &workspace,
        "gb-emulator-shootout/blargg-dmg-sound.suite.toml",
        include_str!("../../../data/gb-emulator-shootout/blargg-dmg-sound.suite.toml"),
    );
    write_manifest(
        &workspace,
        "gb-emulator-shootout/blargg-cgb-sound.suite.toml",
        include_str!("../../../data/gb-emulator-shootout/blargg-cgb-sound.suite.toml"),
    );
    write_blargg_sound_fixtures(&workspace);

    let reports = load_reports(&workspace).expect("reports should load");
    let report = reports
        .iter()
        .find(|report| report.id == "gb-emulator-shootout")
        .expect("report should exist");

    let dmg_suites = load_selected_suites(&workspace, report, Some("blargg-dmg-sound"), None)
        .expect("real blargg DMG sound manifest should load");
    assert_eq!(dmg_suites.len(), 1);
    let dmg_suite = &dmg_suites[0];
    assert_eq!(dmg_suite.suite_name, "blargg-dmg-sound");
    assert_eq!(dmg_suite.family, "blargg");
    assert_eq!(dmg_suite.cases.len(), 12);
    assert!(dmg_suite.cases.iter().all(|case| {
        case.console_model == gb_core::ConsoleModel::GameBoy
            && case.timeout_frames == 1200
            && matches!(&case.oracle, Oracle::Framebuffer(_))
    }));

    let cgb_suites = load_selected_suites(&workspace, report, Some("blargg-cgb-sound"), None)
        .expect("real blargg CGB sound manifest should load");
    assert_eq!(cgb_suites.len(), 1);
    let cgb_suite = &cgb_suites[0];
    assert_eq!(cgb_suite.suite_name, "blargg-cgb-sound");
    assert_eq!(cgb_suite.family, "blargg");
    assert_eq!(cgb_suite.cases.len(), 12);
    assert!(cgb_suite.cases.iter().all(|case| {
        case.console_model == gb_core::ConsoleModel::GameBoyColor
            && case.timeout_frames == 1200
            && matches!(&case.oracle, Oracle::Framebuffer(_))
    }));

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn real_mooneye_suite_manifests_load_fibonacci_result_oracles() {
    let workspace = unique_temp_dir("mooneye-suite-manifests");
    write_reports(
        &workspace,
        "gb-emulator-shootout",
        "gb-emulator-shootout/sources.report.toml",
    );
    write_manifest(
        &workspace,
        "gb-emulator-shootout/mooneye-acceptance-manual-misc.suite.toml",
        include_str!(
            "../../../data/gb-emulator-shootout/mooneye-acceptance-manual-misc.suite.toml"
        ),
    );
    write_manifest(
        &workspace,
        "gb-emulator-shootout/mooneye-emulator-mbc1-mbc5.suite.toml",
        include_str!("../../../data/gb-emulator-shootout/mooneye-emulator-mbc1-mbc5.suite.toml"),
    );
    write_manifest(
        &workspace,
        "gb-emulator-shootout/mooneye-emulator-mbc2.suite.toml",
        include_str!("../../../data/gb-emulator-shootout/mooneye-emulator-mbc2.suite.toml"),
    );
    write_fixture_placeholders(
        &workspace,
        &["test/gb-emulator-shootout/mooneye/manual-only/sprite_priority.png"],
    );

    let reports = load_reports(&workspace).expect("reports should load");
    let report = reports
        .iter()
        .find(|report| report.id == "gb-emulator-shootout")
        .expect("report should exist");

    let acceptance_suites = load_selected_suites(
        &workspace,
        report,
        Some("mooneye-acceptance-manual-misc"),
        None,
    )
    .expect("real mooneye acceptance manifest should load");
    assert_eq!(acceptance_suites.len(), 1);
    let acceptance = &acceptance_suites[0];
    assert_eq!(acceptance.suite_name, "mooneye-acceptance-manual-misc");
    assert_eq!(acceptance.family, "mooneye");
    assert_eq!(acceptance.cases.len(), 69);
    let sprite_priority = acceptance
        .cases
        .iter()
        .find(|case| case.id == "mooneye-manual-only-sprite-priority")
        .expect("manual sprite priority should exist");
    assert!(matches!(&sprite_priority.oracle, Oracle::Framebuffer(_)));
    let boot_regs_cgb = acceptance
        .cases
        .iter()
        .find(|case| case.id == "mooneye-misc-boot-regs-cgb")
        .expect("CGB boot_regs row should exist");
    assert_eq!(
        boot_regs_cgb.console_model,
        gb_core::ConsoleModel::GameBoyColor
    );
    assert!(matches!(&boot_regs_cgb.oracle, Oracle::FibonacciResult(_)));

    let mbc1_mbc5_suites =
        load_selected_suites(&workspace, report, Some("mooneye-emulator-mbc1-mbc5"), None)
            .expect("real mooneye MBC1/MBC5 manifest should load");
    assert_eq!(mbc1_mbc5_suites.len(), 1);
    let mbc1_mbc5 = &mbc1_mbc5_suites[0];
    assert_eq!(mbc1_mbc5.family, "mooneye");
    assert_eq!(mbc1_mbc5.cases.len(), 21);
    assert!(
        mbc1_mbc5
            .cases
            .iter()
            .all(|case| matches!(&case.oracle, Oracle::FibonacciResult(_)))
    );

    let mbc2_suites = load_selected_suites(&workspace, report, Some("mooneye-emulator-mbc2"), None)
        .expect("real mooneye MBC2 manifest should load");
    assert_eq!(mbc2_suites.len(), 1);
    let mbc2 = &mbc2_suites[0];
    assert_eq!(mbc2.family, "mooneye");
    assert_eq!(mbc2.cases.len(), 7);
    assert!(
        mbc2.cases
            .iter()
            .all(|case| matches!(&case.oracle, Oracle::FibonacciResult(_)))
    );

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn real_remaining_gb_emulator_shootout_suite_manifests_load_framebuffer_oracles() {
    let workspace = unique_temp_dir("gbemu-suite-manifests");
    write_reports(
        &workspace,
        "gb-emulator-shootout",
        "gb-emulator-shootout/sources.report.toml",
    );
    let manifests = [
        ("acid", "acid", 5),
        ("ashiepaws", "ashiepaws", 3),
        ("ax6", "ax6", 3),
        ("daid", "daid", 9),
        ("mealybug-tearoom-tests", "mealybug-tearoom-tests", 24),
        ("samesuite-apu", "samesuite", 61),
        ("samesuite", "samesuite", 7),
    ];
    for (suite_name, family, _) in manifests {
        let text = read_gbemu_suite_manifest(suite_name);
        write_manifest(
            &workspace,
            &format!("gb-emulator-shootout/{suite_name}.suite.toml"),
            &text,
        );
        write_manifest_fixture_placeholders(&workspace, "gb-emulator-shootout", family, &text);
    }

    let reports = load_reports(&workspace).expect("reports should load");
    let report = reports
        .iter()
        .find(|report| report.id == "gb-emulator-shootout")
        .expect("report should exist");

    for (suite_name, family, case_count) in manifests {
        let suites = load_selected_suites(&workspace, report, Some(suite_name), None)
            .unwrap_or_else(|error| panic!("real {suite_name} manifest should load: {error}"));
        assert_eq!(suites.len(), 1);
        let suite = &suites[0];
        assert_eq!(suite.suite_name, suite_name);
        assert_eq!(suite.family, family);
        assert_eq!(suite.cases.len(), case_count);
        assert!(
            suite
                .cases
                .iter()
                .all(|case| matches!(&case.oracle, Oracle::Framebuffer(_)))
        );
    }

    let acid = load_selected_suites(&workspace, report, Some("acid"), None)
        .expect("acid manifest should load");
    let acid_which_dmg = acid[0]
        .cases
        .iter()
        .find(|case| case.id == "acid-which-dmg")
        .expect("acid DMG info row should exist");
    assert_eq!(acid_which_dmg.console_model, gb_core::ConsoleModel::GameBoy);

    let samesuite = load_selected_suites(&workspace, report, Some("samesuite"), None)
        .expect("samesuite manifest should load");
    let sgb_case = samesuite[0]
        .cases
        .iter()
        .find(|case| case.id == "samesuite-sgb-command-mlt-req")
        .expect("SGB row should exist");
    assert_eq!(sgb_case.host_platform, gb_core::HostPlatform::Sgb);

    let ashiepaws = load_selected_suites(&workspace, report, Some("ashiepaws"), None)
        .expect("ashiepaws manifest should load");
    let bully_cgb = ashiepaws[0]
        .cases
        .iter()
        .find(|case| case.id == "ashiepaws-bully-cgb")
        .expect("CGB bully row should exist");
    assert_eq!(bully_cgb.startup_mode, gb_core::StartupMode::RealBoot);

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn real_rom_reports_pages_marks_real_boot_reports_with_boot_roms() {
    let workspace = crate::default_workspace_root();
    let pages = read_rom_reports_pages_for_test(&workspace);
    let reports = load_reports(&workspace).expect("reports should load");
    let mut missing_boot_roms = Vec::new();

    for page in pages {
        let report = reports
            .iter()
            .find(|report| report.id == page.name)
            .unwrap_or_else(|| panic!("report {:?} should exist", page.name));
        let real_boot_suites = real_boot_suite_names_for_report(report);
        if !real_boot_suites.is_empty() && !page.boot_roms {
            missing_boot_roms.push(format!("{} ({})", page.name, real_boot_suites.join(", ")));
        }
    }

    assert!(
        missing_boot_roms.is_empty(),
        "rom-reports-pages.json entries with RealBoot suite manifests must set boot_roms = true: {}",
        missing_boot_roms.join("; ")
    );
}

#[test]
fn real_standalone_extra_report_manifests_load_new_runner_oracles() {
    let report_specs = [
        (
            "mooneye",
            &[
                ("mooneye-acceptance", 75, "mooneye", "mooneye"),
                ("mooneye-emulator-only", 28, "mooneye", "mooneye"),
                ("mooneye-madness", 0, "mooneye", "mooneye"),
                ("mooneye-manual", 2, "mooneye", "mooneye"),
                ("mooneye-misc", 8, "mooneye", "mooneye"),
            ][..],
        ),
        (
            "little-things-gb",
            &[(
                "little-things-gb",
                4,
                "little-things-gb",
                "little-things-gb",
            )][..],
        ),
        ("magen", &[("magen", 8, "magen", "magen")][..]),
        (
            "nitro2k01",
            &[
                ("nitro2k01-whichboot", 8, "whichboot", "whichboot"),
                ("nitro2k01-windesync", 1, "windesync", "windesync"),
                (
                    "nitro2k01-double-halt-cancel",
                    3,
                    "double-halt-cancel",
                    "double-halt-cancel",
                ),
            ][..],
        ),
        (
            "mealybug-tearoom-tests",
            &[
                (
                    "mealybug-tearoom-tests-dma",
                    2,
                    "mealybug-tearoom-tests",
                    "mealybug-tearoom-tests",
                ),
                (
                    "mealybug-tearoom-tests-mbc",
                    1,
                    "mealybug-tearoom-tests",
                    "mealybug-tearoom-tests",
                ),
                (
                    "mealybug-tearoom-tests-ppu",
                    76,
                    "mealybug-tearoom-tests",
                    "mealybug-tearoom-tests",
                ),
            ][..],
        ),
        (
            "samesuite",
            &[
                ("samesuite-apu", 5, "samesuite", "samesuite"),
                ("samesuite-apu-channel-1", 20, "samesuite", "samesuite"),
                ("samesuite-apu-channel-2", 15, "samesuite", "samesuite"),
                ("samesuite-apu-channel-3", 15, "samesuite", "samesuite"),
                ("samesuite-apu-channel-4", 13, "samesuite", "samesuite"),
                ("samesuite-dma", 4, "samesuite", "samesuite"),
                ("samesuite-interrupt", 1, "samesuite", "samesuite"),
                ("samesuite-ppu", 1, "samesuite", "samesuite"),
                ("samesuite-sgb", 2, "samesuite", "samesuite"),
            ][..],
        ),
        (
            "rtc3test",
            &[
                ("rtc3test-basic-tests", 2, "basic-tests", "basic-tests"),
                ("rtc3test-range-tests", 2, "range-tests", "range-tests"),
                (
                    "rtc3test-sub-second-writes",
                    2,
                    "sub-second-writes",
                    "sub-second-writes",
                ),
            ][..],
        ),
    ];

    for (report_id, suites) in report_specs {
        let workspace = unique_temp_dir(&format!("{report_id}-suite-manifests"));
        let source_path = format!("{report_id}/sources.report.toml");
        write_reports(&workspace, report_id, &source_path);
        write_source_manifest(
            &workspace,
            &source_path,
            &read_report_source_manifest(report_id),
        );
        for (suite_name, _, _, target_root) in suites {
            let text = read_report_suite_manifest(report_id, suite_name);
            write_manifest(
                &workspace,
                &format!("{report_id}/{suite_name}.suite.toml"),
                &text,
            );
            write_manifest_fixture_placeholders(&workspace, report_id, target_root, &text);
        }

        let reports = load_reports(&workspace).expect("reports should load");
        let report = reports
            .iter()
            .find(|report| report.id == report_id)
            .expect("report should exist");
        for (suite_name, case_count, family, target_root) in suites {
            let loaded = load_selected_suites(&workspace, report, Some(suite_name), None)
                .unwrap_or_else(|error| panic!("{report_id}/{suite_name} should load: {error}"));
            assert_eq!(loaded.len(), 1);
            let suite = &loaded[0];
            assert_eq!(suite.suite_name, *suite_name);
            assert_eq!(suite.family, *family);
            assert_eq!(suite.cases.len(), *case_count);
            assert!(
                suite
                    .cases
                    .iter()
                    .all(|case| case.target_root == Path::new(target_root)),
                "{report_id}/{suite_name} should resolve cases below target root {target_root:?}"
            );
        }

        if report_id == "samesuite" {
            let apu = load_selected_suites(&workspace, report, Some("samesuite-apu"), None)
                .expect("samesuite APU suite should load");
            assert!(
                apu[0]
                    .cases
                    .iter()
                    .all(|case| matches!(&case.oracle, Oracle::FibonacciResult(_)))
            );
            assert!(
                apu[0]
                    .cases
                    .iter()
                    .any(|case| case.console_model == gb_core::ConsoleModel::GameBoy)
            );
            assert!(
                apu[0]
                    .cases
                    .iter()
                    .any(|case| case.hardware_revision == gb_core::HardwareRevision::CpuCgbC)
            );
            let channel_1 =
                load_selected_suites(&workspace, report, Some("samesuite-apu-channel-1"), None)
                    .expect("samesuite APU CH1 suite should load");
            assert!(
                channel_1[0]
                    .cases
                    .iter()
                    .all(|case| matches!(&case.oracle, Oracle::FibonacciResult(_)))
            );
            assert!(
                channel_1[0]
                    .cases
                    .iter()
                    .any(|case| case.hardware_revision == gb_core::HardwareRevision::CpuCgb0)
            );
            assert!(
                channel_1[0]
                    .cases
                    .iter()
                    .any(|case| case.hardware_revision == gb_core::HardwareRevision::CpuCgbD)
            );
            assert_eq!(
                channel_1[0]
                    .cases
                    .iter()
                    .find(|case| case.id == "samesuite-apu-channel-1-channel-1-volume-div")
                    .expect("CH1 volume DIV row should exist")
                    .timeout_frames,
                300
            );
            assert_eq!(
                channel_1[0]
                    .cases
                    .iter()
                    .find(|case| case.id == "samesuite-apu-channel-1-channel-1-nrx2-speed-change")
                    .expect("CH1 NRX2 speed row should exist")
                    .timeout_frames,
                420
            );
            let channel_2 =
                load_selected_suites(&workspace, report, Some("samesuite-apu-channel-2"), None)
                    .expect("samesuite APU CH2 suite should load");
            assert_eq!(
                channel_2[0]
                    .cases
                    .iter()
                    .find(|case| case.id == "samesuite-apu-channel-2-channel-2-nrx2-speed-change")
                    .expect("CH2 NRX2 speed row should exist")
                    .timeout_frames,
                420
            );
            let channel_4 =
                load_selected_suites(&workspace, report, Some("samesuite-apu-channel-4"), None)
                    .expect("samesuite APU CH4 suite should load");
            assert_eq!(
                channel_4[0]
                    .cases
                    .iter()
                    .find(|case| case.id == "samesuite-apu-channel-4-channel-4-volume-div")
                    .expect("CH4 volume DIV row should exist")
                    .timeout_frames,
                300
            );
        }
        if report_id == "magen" {
            let suites = load_selected_suites(&workspace, report, Some("magen"), None)
                .expect("magen suite should load");
            assert!(suites[0].cases.iter().all(|case| case.timeout_frames == 72));
        }
        if report_id == "nitro2k01" {
            let suites =
                load_selected_suites(&workspace, report, Some("nitro2k01-whichboot"), None)
                    .expect("nitro2k01 whichboot suite should load");
            assert!(
                suites[0]
                    .cases
                    .iter()
                    .all(|case| case.startup_mode == gb_core::StartupMode::RealBoot)
            );
        }
        if report_id == "rtc3test" {
            let suites =
                load_selected_suites(&workspace, report, Some("rtc3test-basic-tests"), None)
                    .expect("rtc3test basic suite should load");
            let basic = &suites[0];
            assert!(
                basic.cases.iter().all(|case| case.report_model_suffix
                    && matches!(&case.oracle, Oracle::Framebuffer(_)))
            );
            let dmg = basic
                .cases
                .iter()
                .find(|case| case.id == "rtc3test-dmg-basic-tests")
                .expect("DMG basic row should exist");
            assert_eq!(dmg.hardware_revision, gb_core::HardwareRevision::DmgCpuC);
            assert_eq!(dmg.report_rom(), "rtc3test.gb (DMG)");
            assert_eq!(dmg.stimuli[0].when, SuiteStimulusTime::Frame(30));
            let cgb = basic
                .cases
                .iter()
                .find(|case| case.id == "rtc3test-cgb-basic-tests")
                .expect("CGB basic row should exist");
            assert_eq!(cgb.hardware_revision, gb_core::HardwareRevision::CpuCgbE);
            assert_eq!(cgb.report_rom(), "rtc3test.gb (GBC)");
        }

        fs::remove_dir_all(workspace).expect("workspace should be removable");
    }
}

#[test]
fn real_little_things_report_selects_only_csp_family() {
    let workspace = unique_temp_dir("little-things-family-selection");
    let report_id = "little-things-gb";
    let source_path = "little-things-gb/sources.report.toml";
    write_reports(&workspace, report_id, source_path);
    write_source_manifest(
        &workspace,
        source_path,
        &read_report_source_manifest(report_id),
    );

    let text = read_report_suite_manifest(report_id, "little-things-gb");
    write_manifest(
        &workspace,
        "little-things-gb/little-things-gb.suite.toml",
        &text,
    );
    write_manifest_fixture_placeholders(&workspace, report_id, "little-things-gb", &text);

    let reports = load_reports(&workspace).expect("reports should load");
    let report = reports
        .iter()
        .find(|report| report.id == report_id)
        .expect("report should exist");

    assert_eq!(
        load_selected_suite_families(&workspace, report, Some("little-things-gb"), None)
            .expect("c-sp suite families should load"),
        vec!["little-things-gb".to_string()]
    );
    assert_eq!(
        load_selected_suite_families(
            &workspace,
            report,
            Some("little-things-gb"),
            Some("little-things-gb-dmg-firstwhite")
        )
        .expect("c-sp firstwhite case family should load"),
        vec!["little-things-gb".to_string()]
    );

    let csp_suite = load_selected_suites(&workspace, report, Some("little-things-gb"), None)
        .expect("c-sp suite should load");
    assert!(
        csp_suite[0]
            .cases
            .iter()
            .all(|case| case.target_root == Path::new("little-things-gb"))
    );

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn real_gbmicrotest_suite_manifest_loads_memory_byte_oracles() {
    let workspace = unique_temp_dir("gbmicrotest-suite-manifest");
    write_reports(&workspace, "gbmicrotest", "gbmicrotest/sources.report.toml");
    write_source_manifest(
        &workspace,
        "gbmicrotest/sources.report.toml",
        include_str!("../../../data/gbmicrotest/sources.report.toml"),
    );
    write_manifest(
        &workspace,
        "gbmicrotest/gbmicrotest.suite.toml",
        include_str!("../../../data/gbmicrotest/gbmicrotest.suite.toml"),
    );

    let reports = load_reports(&workspace).expect("reports should load");
    let report = reports
        .iter()
        .find(|report| report.id == "gbmicrotest")
        .expect("report should exist");
    let suites = load_selected_suites(&workspace, report, Some("gbmicrotest"), None)
        .expect("real gbmicrotest manifest should load");

    assert_eq!(suites.len(), 1);
    let suite = &suites[0];
    assert_eq!(suite.suite_name, "gbmicrotest");
    assert_eq!(suite.family, "gbmicrotest");
    assert_eq!(suite.cases.len(), 438);
    assert!(suite.cases.iter().all(|case| {
        case.target_root.as_os_str().is_empty()
            && case.console_model == gb_core::ConsoleModel::GameBoy
            && matches!(&case.oracle, Oracle::MemoryByteEquals(_))
    }));

    let real_boot_case = suite
        .cases
        .iter()
        .find(|case| case.id == "gbmicrotest-ppu-hblank-int-scx0-if-a")
        .expect("real boot case should exist");
    assert_eq!(real_boot_case.startup_mode, gb_core::StartupMode::RealBoot);

    let long_timeout_case = suite
        .cases
        .iter()
        .find(|case| case.id == "gbmicrotest-interrupts-is-if-set-during-ime0")
        .expect("long timeout case should exist");
    assert_eq!(long_timeout_case.timeout_frames, 30);

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn real_docboy_suite_manifests_load_memory_framebuffer_and_stimuli() {
    let workspace = unique_temp_dir("docboy-suite-manifests");
    write_reports(&workspace, "docboy", "docboy/sources.report.toml");
    write_source_manifest(
        &workspace,
        "docboy/sources.report.toml",
        include_str!("../../../data/docboy/sources.report.toml"),
    );
    let manifests = [
        ("docboy-dmg", "docboy-dmg", "dmg", 2326, 4),
        ("docboy-cgb", "docboy-cgb", "cgb", 6172, 642),
        ("docboy-cgb-dmg", "docboy-cgb-dmg", "cgb-dmg", 467, 0),
        (
            "docboy-cgb-dmg-ext",
            "docboy-cgb-dmg-ext",
            "cgb-dmg-ext",
            26,
            0,
        ),
    ];
    for (suite_name, _, target_root, _, disabled_count) in manifests {
        let text = read_docboy_suite_manifest(suite_name);
        assert_eq!(
            disabled_case_count(&text),
            disabled_count,
            "{suite_name} disabled row count should match migrated legacy comments"
        );
        write_manifest(
            &workspace,
            &format!("docboy/{suite_name}.suite.toml"),
            &text,
        );
        fs::create_dir_all(workspace.join("test").join("docboy").join(target_root))
            .expect("docboy fixture root should be creatable");
        write_manifest_fixture_placeholders(&workspace, "docboy", target_root, &text);
    }

    let reports = load_reports(&workspace).expect("reports should load");
    let report = reports
        .iter()
        .find(|report| report.id == "docboy")
        .expect("docboy report should exist");

    for (suite_name, family, target_root, case_count, _) in manifests {
        let suites = load_selected_suites(&workspace, report, Some(suite_name), None)
            .unwrap_or_else(|error| panic!("real {suite_name} manifest should load: {error}"));
        assert_eq!(suites.len(), 1);
        let suite = &suites[0];
        assert_eq!(suite.suite_name, suite_name);
        assert_eq!(suite.family, family);
        assert_eq!(suite.cases.len(), case_count);
        assert!(
            suite
                .cases
                .iter()
                .all(|case| case.target_root == Path::new(target_root))
        );
    }

    let dmg = load_selected_suites(&workspace, report, Some("docboy-dmg"), None)
        .expect("docboy DMG suite should load");
    let interactive = dmg[0]
        .cases
        .iter()
        .find(|case| {
            case.id == "docboy-cpu-interactive-stop-immediate-exit-joypad-interrupt-press-again"
        })
        .expect("interactive row should exist");
    assert_eq!(interactive.stimuli.len(), 3);
    assert!(matches!(&interactive.oracle, Oracle::MemoryByteEquals(_)));
    let visual = dmg[0]
        .cases
        .iter()
        .find(|case| case.id == "docboy-cpu-interactive-visual-stop-immediate-exit")
        .expect("interactive visual row should exist");
    assert!(matches!(&visual.oracle, Oracle::Framebuffer(_)));

    let cgb = load_selected_suites(&workspace, report, Some("docboy-cgb"), None)
        .expect("docboy CGB suite should load");
    assert!(
        cgb[0]
            .cases
            .iter()
            .any(|case| matches!(&case.oracle, Oracle::Framebuffer(_)))
    );
    assert!(
        cgb[0]
            .cases
            .iter()
            .any(|case| matches!(&case.oracle, Oracle::MemoryByteEquals(_)))
    );

    let cgb_dmg = load_selected_suites(&workspace, report, Some("docboy-cgb-dmg"), None)
        .expect("docboy CGB-DMG suite should load");
    assert!(
        cgb_dmg[0]
            .cases
            .iter()
            .any(|case| matches!(&case.oracle, Oracle::Framebuffer(_)))
    );
    assert!(
        cgb_dmg[0]
            .cases
            .iter()
            .any(|case| matches!(&case.oracle, Oracle::MemoryByteEquals(_)))
    );

    let cgb_dmg_ext = load_selected_suites(&workspace, report, Some("docboy-cgb-dmg-ext"), None)
        .expect("docboy CGB-DMG ext suite should load");
    assert!(
        cgb_dmg_ext[0]
            .cases
            .iter()
            .all(|case| matches!(&case.oracle, Oracle::MemoryByteEquals(_)))
    );

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn real_linked_draft_manifests_use_case_schema_and_repo_local_paths() {
    for suite_name in ["cgb-ir", "dmg04-contracts", "dmg04", "dmg07"] {
        let text = read_linked_suite_manifest(suite_name);
        assert!(
            !text.contains("[[session"),
            "{suite_name} should not use legacy session tables"
        );
        let manifest: LinkedDraftManifestFile = toml::from_str(&text)
            .unwrap_or_else(|error| panic!("{suite_name} should parse as linked draft: {error}"));
        assert_eq!(manifest.report, "linked");
        assert_eq!(manifest.suite_name, suite_name);
        assert!(!manifest.cases.is_empty());
        for case in manifest.cases {
            assert!(!case.id.is_empty());
            validate_linked_fixture_paths(&case.oracle);
            assert!(!case.participants.is_empty());
            for participant in case.participants {
                assert!(!participant.id.is_empty());
                validate_relative_repo_local_path(&participant.rom);
                assert!(matches!(participant.model.as_str(), "dmg" | "cgb"));
            }
        }
    }
}

fn write_blargg_timing_fixtures(workspace_root: &Path) {
    write_fixture_placeholders(
        workspace_root,
        &[
            "test/gb-emulator-shootout/blargg/halt_bug.png",
            "test/gb-emulator-shootout/blargg/interrupt_time.png",
            "test/gb-emulator-shootout/blargg/mem_timing-2/01-read_timing.png",
            "test/gb-emulator-shootout/blargg/mem_timing-2/02-write_timing.png",
            "test/gb-emulator-shootout/blargg/mem_timing-2/03-modify_timing.png",
            "test/gb-emulator-shootout/blargg/oam_bug/1-lcd_sync.png",
            "test/gb-emulator-shootout/blargg/oam_bug/2-causes.png",
            "test/gb-emulator-shootout/blargg/oam_bug/3-non_causes.png",
            "test/gb-emulator-shootout/blargg/oam_bug/4-scanline_timing.png",
            "test/gb-emulator-shootout/blargg/oam_bug/5-timing_bug.png",
            "test/gb-emulator-shootout/blargg/oam_bug/6-timing_no_bug.png",
            "test/gb-emulator-shootout/blargg/oam_bug/8-instr_effect.png",
        ],
    );
}

fn write_blargg_sound_fixtures(workspace_root: &Path) {
    write_fixture_placeholders(
        workspace_root,
        &[
            "test/gb-emulator-shootout/blargg/dmg_sound/01-registers.png",
            "test/gb-emulator-shootout/blargg/dmg_sound/02-len_ctr.png",
            "test/gb-emulator-shootout/blargg/dmg_sound/03-trigger.png",
            "test/gb-emulator-shootout/blargg/dmg_sound/04-sweep.png",
            "test/gb-emulator-shootout/blargg/dmg_sound/05-sweep_details.png",
            "test/gb-emulator-shootout/blargg/dmg_sound/06-overflow_on_trigger.png",
            "test/gb-emulator-shootout/blargg/dmg_sound/07-len_sweep_period_sync.png",
            "test/gb-emulator-shootout/blargg/dmg_sound/08-len_ctr_during_power.png",
            "test/gb-emulator-shootout/blargg/dmg_sound/09-wave_read_while_on.png",
            "test/gb-emulator-shootout/blargg/dmg_sound/10-wave_trigger_while_on.png",
            "test/gb-emulator-shootout/blargg/dmg_sound/11-regs_after_power.png",
            "test/gb-emulator-shootout/blargg/dmg_sound/12-wave_write_while_on.png",
            "test/gb-emulator-shootout/blargg/cgb_sound/01-registers.png",
            "test/gb-emulator-shootout/blargg/cgb_sound/02-len_ctr.png",
            "test/gb-emulator-shootout/blargg/cgb_sound/03-trigger.png",
            "test/gb-emulator-shootout/blargg/cgb_sound/04-sweep.png",
            "test/gb-emulator-shootout/blargg/cgb_sound/05-sweep_details.png",
            "test/gb-emulator-shootout/blargg/cgb_sound/06-overflow_on_trigger.png",
            "test/gb-emulator-shootout/blargg/cgb_sound/07-len_sweep_period_sync.png",
            "test/gb-emulator-shootout/blargg/cgb_sound/08-len_ctr_during_power.png",
            "test/gb-emulator-shootout/blargg/cgb_sound/09-wave_read_while_on.png",
            "test/gb-emulator-shootout/blargg/cgb_sound/10-wave_trigger_while_on.png",
            "test/gb-emulator-shootout/blargg/cgb_sound/11-regs_after_power.png",
            "test/gb-emulator-shootout/blargg/cgb_sound/12-wave.png",
        ],
    );
}

fn read_gbemu_suite_manifest(suite_name: &str) -> String {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data/gb-emulator-shootout")
            .join(format!("{suite_name}.suite.toml")),
    )
    .expect("suite manifest should be readable")
}

const ROM_REPORTS_PAGES_PATH_FOR_TEST: &str = "crates/gb-test-runner/data/rom-reports-pages.json";

#[derive(Debug, Deserialize)]
struct RomReportsPageEntryForTest {
    name: String,
    #[serde(default)]
    boot_roms: bool,
}

fn read_rom_reports_pages_for_test(workspace_root: &Path) -> Vec<RomReportsPageEntryForTest> {
    let path = workspace_root.join(ROM_REPORTS_PAGES_PATH_FOR_TEST);
    let text = fs::read_to_string(&path).expect("ROM reports Pages metadata should be readable");
    serde_json::from_str(&text).expect("ROM reports Pages metadata should parse")
}

fn real_boot_suite_names_for_report(report: &super::super::model::Report) -> Vec<String> {
    let report_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("data")
        .join(&report.store_dir);
    let mut suite_names = Vec::new();
    for entry in fs::read_dir(&report_root).unwrap_or_else(|error| {
        panic!(
            "report manifest directory {} should be readable: {error}",
            report_root.display()
        )
    }) {
        let path = entry
            .expect("suite manifest entry should be readable")
            .path();
        let Some(file_name) = path.file_name().and_then(|file_name| file_name.to_str()) else {
            continue;
        };
        if !file_name.ends_with(".suite.toml") || file_name.ends_with(".link.suite.toml") {
            continue;
        }
        if suite_manifest_declares_real_boot(&path) {
            suite_names.push(
                file_name
                    .strip_suffix(".suite.toml")
                    .expect("suite manifest suffix should be present")
                    .to_string(),
            );
        }
    }
    suite_names.sort();
    suite_names
}

fn suite_manifest_declares_real_boot(path: &Path) -> bool {
    let text = fs::read_to_string(path).unwrap_or_else(|error| {
        panic!(
            "suite manifest {} should be readable: {error}",
            path.display()
        )
    });
    let manifest: toml::Value = toml::from_str(&text)
        .unwrap_or_else(|error| panic!("suite manifest {} should parse: {error}", path.display()));
    let top_level_real_boot =
        manifest.get("startup").and_then(toml::Value::as_str) == Some("real-boot");
    let case_real_boot = manifest
        .get("case")
        .and_then(toml::Value::as_array)
        .map(|cases| {
            cases
                .iter()
                .any(|case| case.get("startup").and_then(toml::Value::as_str) == Some("real-boot"))
        })
        .unwrap_or(false);
    top_level_real_boot || case_real_boot
}

fn read_docboy_suite_manifest(suite_name: &str) -> String {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data/docboy")
            .join(format!("{suite_name}.suite.toml")),
    )
    .expect("suite manifest should be readable")
}

fn read_report_suite_manifest(report_id: &str, suite_name: &str) -> String {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join(report_id)
            .join(format!("{suite_name}.suite.toml")),
    )
    .expect("suite manifest should be readable")
}

fn read_linked_suite_manifest(suite_name: &str) -> String {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data/linked")
            .join(format!("{suite_name}.link.suite.toml")),
    )
    .expect("linked suite manifest should be readable")
}

#[derive(Debug, Deserialize)]
struct LinkedDraftManifestFile {
    report: String,
    suite_name: String,
    #[serde(rename = "case")]
    cases: Vec<LinkedDraftCaseFile>,
}

#[derive(Debug, Deserialize)]
struct LinkedDraftCaseFile {
    id: String,
    oracle: Option<toml::Value>,
    #[serde(default, rename = "participant")]
    participants: Vec<LinkedDraftParticipantFile>,
}

#[derive(Debug, Deserialize)]
struct LinkedDraftParticipantFile {
    id: String,
    rom: PathBuf,
    model: String,
}

fn validate_linked_fixture_paths(oracle: &Option<toml::Value>) {
    let Some(toml::Value::Table(oracle)) = oracle else {
        return;
    };
    let Some(fixture) = oracle.get("fixture") else {
        return;
    };
    match fixture {
        toml::Value::String(path) => validate_relative_repo_local_path(Path::new(path)),
        toml::Value::Array(paths) => {
            for path in paths {
                let toml::Value::String(path) = path else {
                    panic!("fixture arrays should contain string paths");
                };
                validate_relative_repo_local_path(Path::new(path));
            }
        }
        _ => panic!("fixture should be a string or an array of strings"),
    }
}

fn validate_relative_repo_local_path(path: &Path) {
    assert!(
        !path.is_absolute(),
        "linked local paths must be relative: {}",
        path.display()
    );
    assert!(
        path.components().all(|component| !matches!(
            component,
            Component::ParentDir | Component::CurDir | Component::RootDir | Component::Prefix(_)
        )),
        "linked local paths must be confined below data/linked: {}",
        path.display()
    );
}

fn read_report_source_manifest(report_id: &str) -> String {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join(report_id)
            .join("sources.report.toml"),
    )
    .expect("source manifest should be readable")
}

fn write_manifest_fixture_placeholders(
    workspace_root: &Path,
    report_id: &str,
    target_root: &str,
    manifest_text: &str,
) {
    fs::create_dir_all(
        workspace_root
            .join("test")
            .join(report_id)
            .join(target_root),
    )
    .expect("fixture root should be creatable");
    for (local, fixture) in fixture_specs_from_manifest(manifest_text) {
        let root = if local {
            workspace_root
                .join("crates")
                .join("gb-test-runner")
                .join("data")
                .join(report_id)
        } else {
            workspace_root
                .join("test")
                .join(report_id)
                .join(target_root)
        };
        let path = clean_path(&root.join(fixture));
        write_png_fixture(&path);
    }
}

fn fixture_specs_from_manifest(manifest_text: &str) -> Vec<(bool, PathBuf)> {
    let mut paths = Vec::new();
    let default_local = manifest_text
        .lines()
        .take_while(|line| line.trim() != "[[case]]")
        .any(|line| line.contains("oracle") && line.contains("local = true"));
    for line in manifest_text.lines() {
        let Some((_, value)) = line.split_once("fixture =") else {
            continue;
        };
        let local =
            line.contains("local = true") || (default_local && !line.contains("local = false"));
        for path in value
            .split('"')
            .enumerate()
            .filter_map(|(index, value)| (index % 2 == 1).then_some(value))
        {
            paths.push((local, PathBuf::from(path)));
        }
    }
    paths
}

fn disabled_case_count(manifest_text: &str) -> usize {
    manifest_text
        .lines()
        .filter(|line| line.trim() == "disabled = true")
        .count()
}

fn clean_path(path: &Path) -> PathBuf {
    let mut clean = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                clean.pop();
            }
            Component::Normal(component) => clean.push(component),
            Component::RootDir | Component::Prefix(_) => clean.push(component.as_os_str()),
        }
    }
    clean
}

fn write_fixture_placeholders(workspace_root: &Path, fixtures: &[&str]) {
    for fixture in fixtures {
        write_png_fixture(&workspace_root.join(fixture));
    }
}

fn write_png_fixture(path: &Path) {
    fs::create_dir_all(path.parent().expect("fixture should have parent"))
        .expect("fixture parent should be creatable");
    let file = fs::File::create(path).expect("fixture should be writable");
    let mut encoder = png::Encoder::new(file, 160, 144);
    encoder.set_color(png::ColorType::Grayscale);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .expect("PNG header should be writable");
    let pixels = vec![255; 160 * 144];
    writer
        .write_image_data(&pixels)
        .expect("PNG data should be writable");
}
