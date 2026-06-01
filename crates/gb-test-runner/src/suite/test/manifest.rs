use std::fs;
use std::path::{Path, PathBuf};

use crate::oracle::Oracle;

use super::super::manifest::{load_reports, load_selected_suites, parse_suite_manifest_for_test};
use super::common::{
    basic_manifest, unique_temp_dir, write_manifest, write_reports, write_source_manifest,
};

#[test]
fn parses_manifest_defaults_for_serial_contains_cases() {
    let manifest = parse_suite_manifest_for_test(
        Path::new("blargg-cpu-instrs.toml"),
        "gb-emulator-shootout",
        &basic_manifest(
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
fn parses_startup_modes_and_rejects_unsupported_startup() {
    let custom_boot = basic_manifest("gbmicrotest", "gbmicrotest", "gbmicrotest-case", "case.gb")
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

    let real_boot = basic_manifest("gbmicrotest", "gbmicrotest", "gbmicrotest-case", "case.gb")
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

    let unsupported = basic_manifest("gbmicrotest", "gbmicrotest", "gbmicrotest-case", "case.gb")
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
fn parses_console_profiles_and_rejects_unsupported_console_and_oracle() {
    let cgb_console = basic_manifest("acid", "acid", "acid-cgb", "cgb-acid2.gbc")
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

    let sgb_console = basic_manifest("samesuite", "samesuite", "samesuite-sgb", "sgb/test.gb")
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

    let sgb2_console = basic_manifest("samesuite", "samesuite", "samesuite-sgb2", "sgb/test.gb")
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

    let unsupported_alias = basic_manifest("acid", "acid", "acid-gb", "which.gb")
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

    let unsupported_oracle = basic_manifest("acid", "acid", "acid-which", "which.gb")
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

    let unsupported_execution_mode = basic_manifest("acid", "acid", "acid-which", "which.gb")
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
"#,
    );
    write_manifest(
        &workspace,
        "docboy/docboy-dmg.suite.toml",
        r#"
family = "docboy-dmg"
suite_name = "docboy-dmg"
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
            "crates/gb-test-runner/data/gb-emulator-shootout/fixtures/cpp/sgb-ext-test.sgb.png",
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
