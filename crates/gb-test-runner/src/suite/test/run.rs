use std::fs;

use super::super::cli::run_suite_command_with_workspace_for_test;
use super::common::{
    basic_manifest, build_fibonacci_result_rom, build_infinite_loop_rom, build_memory_write_rom,
    build_serial_text_rom, unique_temp_dir, write_manifest, write_reports, write_source_manifest,
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
            "blargg-cpu-instrs",
            "blargg",
            "blargg-cpu-instrs-01-special",
            "cpu_instrs/01-special.gb",
        ),
    );
    write_manifest(
        &workspace,
        "sample-report/acid.toml",
        &basic_manifest("acid", "acid", "acid-cgb-acid2", "cgb-acid2.gbc")
            .replace("console = \"dmg\"", "console = \"cgb\"")
            .replace("serial-contains", "framebuffer-fixture"),
    );
    let rom_path = workspace.join("test/sample-report/blargg/cpu_instrs/01-special.gb");
    fs::create_dir_all(rom_path.parent().expect("rom should have parent"))
        .expect("rom parent should be creatable");
    fs::write(&rom_path, build_serial_text_rom("Passed")).expect("rom should be writable");

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
    let status =
        fs::read_to_string(workspace.join("test/sample-report/.status/blargg-cpu-instrs.toml"))
            .expect("status should be written");
    assert!(status.contains("suite_name = \"blargg-cpu-instrs\""));
    assert!(status.contains("status = \"PASS\""));

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
console = "dmg"
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

    let status = fs::read_to_string(workspace.join("test/sample-report/.status/threaded.toml"))
        .expect("status should be written");
    let first = status
        .find("rom = \"threaded/first.gb\"")
        .expect("first row should exist");
    let second = status
        .find("rom = \"threaded/second.gb\"")
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
        fs::read_to_string(workspace.join("test/sample-report/.status/blargg-cpu-instrs.toml"))
            .expect("status should be written");
    assert!(status.contains("status = \"FAIL\""));

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
        &basic_manifest("acid", "acid", "acid-which-dmg", "which.gb")
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
    let status = fs::read_to_string(workspace.join("test/sample-report/.status/acid.toml"))
        .expect("status should be written");
    assert!(status.contains("status = \"PASS\""));

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
console = "dmg"
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
    let status = fs::read_to_string(workspace.join("test/sample-report/.status/mooneye.toml"))
        .expect("status should be written");
    assert!(status.contains("status = \"PASS\""));

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
console = "sgb"
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
    let status = fs::read_to_string(workspace.join("test/sample-report/.status/samesuite.toml"))
        .expect("status should be written");
    assert!(status.contains("status = \"PASS\""));

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
console = "dmg"
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
    let status = fs::read_to_string(workspace.join("test/gbmicrotest/.status/gbmicrotest.toml"))
        .expect("status should be written");
    assert!(status.contains("status = \"PASS\""));

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
console = "dmg"
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
    let status = fs::read_to_string(workspace.join("test/sample-report/.status/docboy-dmg.toml"))
        .expect("status should be written");
    assert!(status.contains("status = \"FAIL\""));

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
console = "dmg"
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
    let status = fs::read_to_string(workspace.join("test/sample-report/.status/docboy-dmg.toml"))
        .expect("status should be written");
    assert!(status.contains("status = \"FAIL\""));

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
console = "dmg"
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
    let status = fs::read_to_string(workspace.join("test/sample-report/.status/mooneye.toml"))
        .expect("status should be written");
    assert!(status.contains("status = \"FAIL\""));

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
console = "dmg"
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
