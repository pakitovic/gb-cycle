use std::path::Path;

use gb_core::StartupMode;

use super::super::manifest::{
    load_reports, load_selected_link_suites_for_test, parse_link_suite_manifest_for_test,
};
use super::common::{linked_report, unique_temp_dir, write_reports};

#[test]
fn real_link_manifests_load() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let report = linked_report();

    let suites = load_selected_link_suites_for_test(&workspace, &report, None, None)
        .expect("real linked manifests should load");

    let suite_names = suites
        .iter()
        .map(|suite| suite.suite_name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(suite_names, ["cgb-ir", "dmg04-contracts", "dmg04", "dmg07"]);
}

#[test]
fn real_docboy_link_manifest_loads_from_report_sources() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let reports = load_reports(&workspace).expect("real reports should load");
    let report = reports
        .iter()
        .find(|report| report.id == "docboy")
        .expect("docboy report should exist");

    let suites =
        load_selected_link_suites_for_test(&workspace, report, Some("docboy-dmg-link"), None)
            .expect("docboy linked manifest should load");

    assert_eq!(suites.len(), 1);
    assert_eq!(suites[0].family, "docboy-dmg");
    assert_eq!(suites[0].cases.len(), 2);
    assert_eq!(suites[0].cases[0].target_root, Path::new("dmg"));
    assert_eq!(
        suites[0].cases[0].participants[0].rom,
        Path::new("serial/serial_two_players_basic_transfer_master.gb")
    );
}

#[test]
fn single_machine_suite_manifests_are_ignored() {
    let workspace = unique_temp_dir("ignore-single-machine");
    write_reports(&workspace);
    let linked_dir = workspace.join("crates/gb-test-runner/data/linked");
    std::fs::create_dir_all(&linked_dir).expect("linked dir should be created");
    std::fs::write(
        linked_dir.join("regular.suite.toml"),
        "report = \"linked\"\nsuite_name = \"regular\"\nfamily = \"linked\"\n",
    )
    .expect("regular suite manifest should be written");

    let suites = load_selected_link_suites_for_test(&workspace, &linked_report(), None, None)
        .expect("linked discovery should ignore regular suite manifests");

    assert!(suites.is_empty());
}

#[test]
fn parser_rejects_missing_or_mismatched_report() {
    assert!(
        parse_link_suite_manifest_for_test(
            Path::new("missing.link.suite.toml"),
            "linked",
            basic_manifest_without_report(),
        )
        .expect_err("missing report should fail")
        .contains("must define report")
    );
    assert!(
        parse_link_suite_manifest_for_test(
            Path::new("wrong.link.suite.toml"),
            "linked",
            &basic_manifest_with_report("other"),
        )
        .expect_err("mismatched report should fail")
        .contains("declares report")
    );
}

#[test]
fn parser_rejects_unknown_topology_and_escaped_paths() {
    let unknown_topology = basic_manifest_with_extra("topology = \"cable\"");
    assert!(
        parse_link_suite_manifest_for_test(
            Path::new("unknown.link.suite.toml"),
            "linked",
            &unknown_topology,
        )
        .expect_err("unknown topology should fail")
        .contains("unsupported topology")
    );

    let escaped_rom = basic_manifest_with_participant_rom("../left.gb");
    assert!(
        parse_link_suite_manifest_for_test(
            Path::new("escaped.link.suite.toml"),
            "linked",
            &escaped_rom,
        )
        .expect_err("escaped ROM should fail")
        .contains("must not contain '..'")
    );
}

#[test]
fn disabled_cases_require_comments_and_are_skipped() {
    let manifest = r#"report = "linked"
suite_name = "disabled"
family = "linked"
topology = "dmg04"
timeout_tcycles = 8
oracle = { type = "serial-hex-exact", target_participant = "left", expected = "" }

[[case]]
id = "disabled-case"
disabled = true
comment = "Known hardware investigation row."
oracle = { type = "unknown-disabled-oracle" }

[[case]]
id = "enabled-case"

  [[case.participant]]
  id = "left"
  rom = "left.gb"
  console = "dmg"

  [[case.participant]]
  id = "right"
  rom = "right.gb"
  console = "dmg"
"#;

    let suite = parse_link_suite_manifest_for_test(
        Path::new("disabled.link.suite.toml"),
        "linked",
        manifest,
    )
    .expect("disabled case should be skipped after validating comment");

    assert_eq!(suite.cases.len(), 1);
    assert_eq!(suite.cases[0].id, "enabled-case");

    let missing_comment = manifest.replace(
        r#"comment = "Known hardware investigation row."
"#,
        "",
    );
    assert!(
        parse_link_suite_manifest_for_test(
            Path::new("disabled.link.suite.toml"),
            "linked",
            &missing_comment,
        )
        .expect_err("disabled case without comment should fail")
        .contains("must include a non-empty comment")
    );
}

#[test]
fn parser_resolves_startup_precedence() {
    let manifest = r#"report = "linked"
suite_name = "startup"
family = "linked"
topology = "dmg04"
timeout_tcycles = 8
startup = "custom-boot"

[[case]]
id = "case"
startup = "real-boot"
oracle = { type = "serial-hex-exact", target_participant = "left", expected = "" }

  [[case.participant]]
  id = "left"
  rom = "left.gb"
  console = "dmg"

  [[case.participant]]
  id = "right"
  rom = "right.gb"
  console = "dmg"
  startup = "skip-boot"
"#;

    let suite = parse_link_suite_manifest_for_test(
        Path::new("startup.link.suite.toml"),
        "linked",
        manifest,
    )
    .expect("manifest should parse");

    assert_eq!(
        suite.cases[0].participants[0].startup_mode,
        StartupMode::RealBoot
    );
    assert_eq!(
        suite.cases[0].participants[1].startup_mode,
        StartupMode::SkipBoot
    );
}

#[test]
fn parser_accepts_agb_participant_profile() {
    let manifest =
        basic_manifest_with_report("linked").replacen("console = \"dmg\"", "console = \"agb\"", 1);
    let suite =
        parse_link_suite_manifest_for_test(Path::new("agb.link.suite.toml"), "linked", &manifest)
            .expect("AGB participant should parse");

    assert_eq!(
        suite.cases[0].participants[0].console_model,
        gb_core::ConsoleModel::GameBoyAdvance
    );
    assert_eq!(
        suite.cases[0].participants[0].hardware_revision,
        gb_core::HardwareRevision::CpuAgbA
    );
    assert_eq!(
        suite.cases[0].participants[0].host_platform,
        gb_core::HostPlatform::Handheld
    );
}

fn basic_manifest_without_report() -> &'static str {
    r#"suite_name = "basic"
family = "linked"
topology = "dmg04"
timeout_tcycles = 8

[[case]]
id = "case"
oracle = { type = "serial-hex-exact", target_participant = "left", expected = "" }

  [[case.participant]]
  id = "left"
  rom = "left.gb"
  console = "dmg"

  [[case.participant]]
  id = "right"
  rom = "right.gb"
  console = "dmg"
"#
}

fn basic_manifest_with_report(report: &str) -> String {
    format!("report = {report:?}\n{}", basic_manifest_without_report())
}

fn basic_manifest_with_extra(extra: &str) -> String {
    basic_manifest_with_report("linked").replacen("topology = \"dmg04\"", extra, 1)
}

fn basic_manifest_with_participant_rom(rom: &str) -> String {
    basic_manifest_with_report("linked").replacen("rom = \"left.gb\"", &format!("rom = {rom:?}"), 1)
}
