use std::path::Path;

use super::super::manifest::parse_suite_manifest_for_test;
use super::common::basic_manifest;

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
    assert_eq!(manifest.cases[0].timeout_frames, 2);
}

#[test]
fn rejects_unsupported_console_and_oracle() {
    let unsupported_console = basic_manifest("acid", "acid", "acid-cgb", "cgb-acid2.gbc")
        .replace("console = \"dmg\"", "console = \"cgb\"");
    assert!(
        parse_suite_manifest_for_test(
            Path::new("acid.suite.toml"),
            "gb-emulator-shootout",
            &unsupported_console
        )
        .expect_err("cgb should fail")
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
