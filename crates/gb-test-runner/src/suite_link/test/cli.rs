use super::super::cli::parse_suite_link_arguments_for_test;

#[test]
fn help_mentions_supported_flags() {
    let help = super::super::cli::suite_link_help_text();

    assert!(help.contains("<report-id>"));
    assert!(help.contains("--suite <suite-name>"));
    assert!(help.contains("--case <case-id>"));
    assert!(help.contains("--threads <n>"));
    assert!(help.contains("--boot-rom-dir <dir>"));
}

#[test]
fn parse_accepts_suite_case_threads_and_boot_rom_dir() {
    let action = parse_suite_link_arguments_for_test([
        "linked",
        "--suite",
        "dmg04",
        "--case",
        "dmg04-basic-exchange",
        "--threads",
        "2",
        "--boot-rom-dir",
        "bootroms",
    ])
    .expect("arguments should parse");

    let debug = format!("{action:?}");
    assert!(debug.contains("report_id: Some(\"linked\")"));
    assert!(debug.contains("suite_name: Some(\"dmg04\")"));
    assert!(debug.contains("case_id: Some(\"dmg04-basic-exchange\")"));
    assert!(debug.contains("threads: Some(2)"));
    assert!(debug.contains("boot_rom_dir: Some"));
}

#[test]
fn parse_rejects_case_without_suite() {
    assert!(
        parse_suite_link_arguments_for_test(["linked", "--case", "one"])
            .expect_err("case without suite should fail")
            .contains("--case requires --suite")
    );
}

#[test]
fn parse_rejects_invalid_threads() {
    assert!(
        parse_suite_link_arguments_for_test(["linked", "--threads", "0"])
            .expect_err("zero threads should fail")
            .contains("greater than zero")
    );
    assert!(
        parse_suite_link_arguments_for_test(["linked", "--threads", "nan"])
            .expect_err("invalid threads should fail")
            .contains("invalid --threads value")
    );
}

#[test]
fn parse_rejects_missing_boot_rom_dir_value() {
    assert!(
        parse_suite_link_arguments_for_test(["linked", "--boot-rom-dir"])
            .expect_err("missing boot rom dir should fail")
            .contains("--boot-rom-dir requires a value")
    );
}
