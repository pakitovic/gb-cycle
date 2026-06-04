use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

use crate::oracle::Oracle;

use super::super::manifest::{load_reports, load_selected_suites, parse_suite_manifest_for_test};
use super::super::model::SuiteStimulusTime;
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
console = "dmg"
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
console = "dmg"
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
fn parses_console_profiles_and_rejects_unsupported_console_and_oracle() {
    let cgb_console = basic_manifest(
        "gb-emulator-shootout",
        "acid",
        "acid",
        "acid-cgb",
        "cgb-acid2.gbc",
    )
    .replace("console = \"dmg\"", "console = \"cgb\"");
    let cgb_manifest = parse_suite_manifest_for_test(
        Path::new("acid.suite.toml"),
        "gb-emulator-shootout",
        &cgb_console,
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

    let sgb_console = basic_manifest(
        "gb-emulator-shootout",
        "samesuite",
        "samesuite",
        "samesuite-sgb",
        "sgb/test.gb",
    )
    .replace("console = \"dmg\"", "console = \"sgb\"");
    let sgb_manifest = parse_suite_manifest_for_test(
        Path::new("samesuite.suite.toml"),
        "gb-emulator-shootout",
        &sgb_console,
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

    let sgb2_console = basic_manifest(
        "gb-emulator-shootout",
        "samesuite",
        "samesuite",
        "samesuite-sgb2",
        "sgb/test.gb",
    )
    .replace("console = \"dmg\"", "console = \"sgb2\"");
    let sgb2_manifest = parse_suite_manifest_for_test(
        Path::new("samesuite.suite.toml"),
        "gb-emulator-shootout",
        &sgb2_console,
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

    let unsupported_alias = basic_manifest(
        "gb-emulator-shootout",
        "acid",
        "acid",
        "acid-gb",
        "which.gb",
    )
    .replace("console = \"dmg\"", "console = \"gb\"");
    assert!(
        parse_suite_manifest_for_test(
            Path::new("acid.suite.toml"),
            "gb-emulator-shootout",
            &unsupported_alias
        )
        .expect_err("gb alias should fail")
        .contains("unsupported console")
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
console = "dmg"
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
console = "sgb"
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
console = "sgb"
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
console = "dmg"
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
console = "dmg"
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
console = "dmg"
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
console = "dmg"
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
console = "dmg"
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
console = "dmg"
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
    assert_eq!(bully_cgb.startup_mode, gb_core::StartupMode::CustomBoot);

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn real_standalone_extra_report_manifests_load_new_runner_oracles() {
    let report_specs = [
        (
            "mooneye",
            &[
                ("mooneye-cgb", 11, "mooneye"),
                ("mooneye-sgb", 2, "mooneye"),
            ][..],
        ),
        ("ax6", &[("ax6-dmg", 3, "ax6")][..]),
        (
            "little-things-gb",
            &[
                ("little-things-gb-dmg", 2, "little-things-gb"),
                ("little-things-gb-cgb", 1, "little-things-gb"),
            ][..],
        ),
        ("magen", &[("magen-cgb", 8, "magen")][..]),
        (
            "mealybug-tearoom-tests",
            &[("mealybug-tearoom-tests-cgb", 24, "mealybug-tearoom-tests")][..],
        ),
        (
            "samesuite",
            &[
                ("samesuite-dmg", 3, "samesuite"),
                ("samesuite-cgb", 9, "samesuite"),
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
        for (suite_name, _, target_root) in suites {
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
        for (suite_name, case_count, family) in suites {
            let loaded = load_selected_suites(&workspace, report, Some(suite_name), None)
                .unwrap_or_else(|error| panic!("{report_id}/{suite_name} should load: {error}"));
            assert_eq!(loaded.len(), 1);
            let suite = &loaded[0];
            assert_eq!(suite.suite_name, *suite_name);
            assert_eq!(suite.family, *family);
            assert_eq!(suite.cases.len(), *case_count);
        }

        if report_id == "mooneye" {
            let suites = load_selected_suites(&workspace, report, Some("mooneye-sgb"), None)
                .expect("mooneye SGB suite should load");
            assert!(
                suites[0]
                    .cases
                    .iter()
                    .any(|case| case.host_platform == gb_core::HostPlatform::Sgb2)
            );
            assert!(
                suites[0]
                    .cases
                    .iter()
                    .all(|case| matches!(&case.oracle, Oracle::FibonacciResult(_)))
            );
        }
        if report_id == "samesuite" {
            let suites = load_selected_suites(&workspace, report, Some("samesuite-cgb"), None)
                .expect("samesuite CGB suite should load");
            let cgb_d = suites[0]
                .cases
                .iter()
                .find(|case| {
                    case.id == "samesuite-cgb-apu-channel-1-channel-1-freq-change-timing-cgbde"
                })
                .expect("CGB-D row should exist");
            assert_eq!(cgb_d.hardware_revision, gb_core::HardwareRevision::CpuCgbD);
            assert!(
                suites[0]
                    .cases
                    .iter()
                    .any(|case| case.hardware_revision == gb_core::HardwareRevision::CpuCgbE)
            );
        }
        if report_id == "little-things-gb" {
            let suites =
                load_selected_suites(&workspace, report, Some("little-things-gb-cgb"), None)
                    .expect("little-things CGB suite should load");
            assert_eq!(
                suites[0].cases[0].startup_mode,
                gb_core::StartupMode::CustomBoot
            );
        }
        if report_id == "magen" {
            let suites = load_selected_suites(&workspace, report, Some("magen-cgb"), None)
                .expect("magen suite should load");
            assert!(suites[0].cases.iter().all(|case| case.timeout_frames == 72));
        }

        fs::remove_dir_all(workspace).expect("workspace should be removable");
    }
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

    let custom_boot_case = suite
        .cases
        .iter()
        .find(|case| case.id == "gbmicrotest-ppu-hblank-int-scx0-if-a")
        .expect("custom boot case should exist");
    assert_eq!(
        custom_boot_case.startup_mode,
        gb_core::StartupMode::CustomBoot
    );

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
        ("docboy-cgb", "docboy-cgb", "cgb", 6172, 643),
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
                assert!(matches!(participant.console.as_str(), "dmg" | "cgb"));
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
    console: String,
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
    for line in manifest_text.lines() {
        let Some((_, value)) = line.split_once("fixture =") else {
            continue;
        };
        let local = line.contains("local = true");
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
