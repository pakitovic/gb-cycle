use serde::Deserialize;

use super::super::{
    CPU_OBSERVATION_WINDOW_BYTES, CpuObservation, FramebufferObservation, Oracle, OracleConfig,
    OracleObservations, OracleOutcome, OracleStep,
};

const PASS_SIGNATURE: [u8; 6] = [3, 5, 8, 13, 21, 34];
const FAIL_SIGNATURE: [u8; 6] = [0x42; 6];

#[derive(Debug, Deserialize)]
struct OracleWrapper {
    oracle: OracleConfig,
}

fn parse_oracle_config(text: &str) -> OracleConfig {
    toml::from_str::<OracleWrapper>(text)
        .expect("oracle config should parse")
        .oracle
}

fn observations(
    signature: [u8; 6],
    current_opcode: Option<u8>,
    pc_window: [u8; CPU_OBSERVATION_WINDOW_BYTES],
) -> OracleObservations<'static> {
    OracleObservations {
        serial: b"",
        cpu: Some(CpuObservation {
            b: signature[0],
            c: signature[1],
            d: signature[2],
            e: signature[3],
            h: signature[4],
            l: signature[5],
            pc: 0x0154,
            current_opcode,
            pc_window,
        }),
        memory: &[],
        executed_tcycles: 0,
        framebuffer: FramebufferObservation::empty(),
        participants: &[],
        linked: None,
    }
}

fn no_halt_loop_window() -> [u8; CPU_OBSERVATION_WINDOW_BYTES] {
    [0xFF; CPU_OBSERVATION_WINDOW_BYTES]
}

fn halt_loop_window() -> [u8; CPU_OBSERVATION_WINDOW_BYTES] {
    let mut window = [0xFF; CPU_OBSERVATION_WINDOW_BYTES];
    window[2..6].copy_from_slice(&[0x40, 0x00, 0x18, 0xFD]);
    window
}

fn compact_terminal_loop_window() -> [u8; CPU_OBSERVATION_WINDOW_BYTES] {
    let mut window = [0xFF; CPU_OBSERVATION_WINDOW_BYTES];
    window[2..5].copy_from_slice(&[0x40, 0x18, 0xFE]);
    window
}

fn breakpoint_halt_window() -> [u8; CPU_OBSERVATION_WINDOW_BYTES] {
    let mut window = [0xFF; CPU_OBSERVATION_WINDOW_BYTES];
    window[2..4].copy_from_slice(&[0x40, 0x76]);
    window
}

fn fibonacci_oracle() -> Oracle {
    Oracle::from_manifest(&parse_oracle_config(
        "oracle = { type = \"fibonacci-result\" }",
    ))
    .expect("fibonacci-result oracle should parse")
}

fn legacy_fibonacci_oracle() -> Oracle {
    Oracle::from_manifest(&parse_oracle_config(
        "oracle = { type = \"fibonacci-result\", legacy = true }",
    ))
    .expect("legacy fibonacci-result oracle should parse")
}

fn terminal_non_pass_failure_oracle() -> Oracle {
    Oracle::from_manifest(&parse_oracle_config(
        "oracle = { type = \"fibonacci-result\", fail_on_terminal_non_pass = true }",
    ))
    .expect("terminal-failing fibonacci-result oracle should parse")
}

#[test]
fn catalog_builds_fibonacci_result_oracle_from_manifest_config() {
    assert!(matches!(fibonacci_oracle(), Oracle::FibonacciResult(_)));
    assert!(
        Oracle::from_manifest(&parse_oracle_config(
            "oracle = { type = \"fibonacci-result\", expected = \"Passed\" }"
        ))
        .expect_err("unknown parameter should fail")
        .contains("does not support parameter")
    );
}

#[test]
fn fibonacci_result_passes_on_fibonacci_signature_at_magic_breakpoint() {
    let mut oracle = fibonacci_oracle();
    assert_eq!(
        oracle
            .observe(observations(
                PASS_SIGNATURE,
                Some(0x40),
                no_halt_loop_window()
            ))
            .expect("oracle should observe"),
        OracleStep::Stop
    );
    assert_eq!(
        oracle
            .finish(observations(
                PASS_SIGNATURE,
                Some(0x40),
                no_halt_loop_window()
            ))
            .expect("oracle should finish"),
        OracleOutcome::Passed
    );
}

#[test]
fn fibonacci_result_fails_on_failure_signature_at_magic_breakpoint() {
    let mut oracle = fibonacci_oracle();
    assert_eq!(
        oracle
            .observe(observations(
                FAIL_SIGNATURE,
                Some(0x40),
                no_halt_loop_window()
            ))
            .expect("oracle should observe"),
        OracleStep::Stop
    );
    assert!(matches!(
        oracle
            .finish(observations(FAIL_SIGNATURE, Some(0x40), no_halt_loop_window()))
            .expect("oracle should finish"),
        OracleOutcome::Failed(message) if message.contains("failure signature")
    ));
}

#[test]
fn fibonacci_result_requires_terminal_signal_for_known_signature() {
    let mut oracle = fibonacci_oracle();
    assert_eq!(
        oracle
            .observe(observations(PASS_SIGNATURE, None, no_halt_loop_window()))
            .expect("oracle should observe"),
        OracleStep::Continue
    );
    assert!(matches!(
        oracle
            .finish(observations(PASS_SIGNATURE, None, no_halt_loop_window()))
            .expect("oracle should finish"),
        OracleOutcome::Failed(message) if message.contains("without terminal signal")
    ));
}

#[test]
fn fibonacci_result_legacy_mode_passes_on_legacy_breakpoint() {
    let mut oracle = legacy_fibonacci_oracle();
    assert_eq!(
        oracle
            .observe(observations(
                PASS_SIGNATURE,
                Some(0xED),
                no_halt_loop_window()
            ))
            .expect("oracle should observe"),
        OracleStep::Stop
    );
    assert_eq!(
        oracle
            .finish(observations(
                PASS_SIGNATURE,
                Some(0xED),
                no_halt_loop_window()
            ))
            .expect("oracle should finish"),
        OracleOutcome::Passed
    );
}

#[test]
fn fibonacci_result_legacy_mode_fails_immediately_without_pass_signature() {
    let mut oracle = legacy_fibonacci_oracle();
    assert_eq!(
        oracle
            .observe(observations([0; 6], Some(0xED), no_halt_loop_window()))
            .expect("oracle should observe"),
        OracleStep::Stop
    );
    assert!(matches!(
        oracle
            .finish(observations([0; 6], Some(0xED), no_halt_loop_window()))
            .expect("oracle should finish"),
        OracleOutcome::Failed(message)
            if message.contains("legacy fibonacci terminal reached without pass signature")
    ));
}

#[test]
fn fibonacci_result_default_mode_ignores_legacy_breakpoint_without_pass_signature() {
    let mut oracle = fibonacci_oracle();
    assert_eq!(
        oracle
            .observe(observations([0; 6], Some(0xED), no_halt_loop_window()))
            .expect("oracle should observe"),
        OracleStep::Continue
    );
    assert!(matches!(
        oracle
            .finish(observations([0; 6], Some(0xED), no_halt_loop_window()))
            .expect("oracle should finish"),
        OracleOutcome::Failed(message) if message.contains("was not reached")
    ));
}

#[test]
fn fibonacci_result_default_mode_ignores_magic_breakpoint_without_pass_signature() {
    let mut oracle = fibonacci_oracle();
    assert_eq!(
        oracle
            .observe(observations([0; 6], Some(0x40), no_halt_loop_window()))
            .expect("oracle should observe"),
        OracleStep::Continue
    );
    assert!(matches!(
        oracle
            .finish(observations([0; 6], Some(0x40), no_halt_loop_window()))
            .expect("oracle should finish"),
        OracleOutcome::Failed(message) if message.contains("was not reached")
    ));
}

#[test]
fn fibonacci_result_can_fail_magic_breakpoint_without_pass_signature() {
    let mut oracle = terminal_non_pass_failure_oracle();
    assert_eq!(
        oracle
            .observe(observations([0; 6], Some(0x40), no_halt_loop_window()))
            .expect("oracle should observe"),
        OracleStep::Stop
    );
    assert!(matches!(
        oracle
            .finish(observations([0; 6], Some(0x40), no_halt_loop_window()))
            .expect("oracle should finish"),
        OracleOutcome::Failed(message)
            if message.contains("fibonacci terminal reached without pass signature")
    ));
}

#[test]
fn fibonacci_result_default_mode_ignores_legacy_breakpoint() {
    let mut oracle = fibonacci_oracle();
    assert_eq!(
        oracle
            .observe(observations(
                PASS_SIGNATURE,
                Some(0xED),
                no_halt_loop_window()
            ))
            .expect("oracle should observe"),
        OracleStep::Continue
    );
    assert!(matches!(
        oracle
            .finish(observations(
                PASS_SIGNATURE,
                Some(0xED),
                no_halt_loop_window()
            ))
            .expect("oracle should finish"),
        OracleOutcome::Failed(message) if message.contains("without terminal signal")
    ));
}

#[test]
fn fibonacci_result_detects_post_breakpoint_halt_loop_near_pc() {
    let mut oracle = fibonacci_oracle();
    assert_eq!(
        oracle
            .observe(observations(PASS_SIGNATURE, None, halt_loop_window()))
            .expect("oracle should observe"),
        OracleStep::Stop
    );
    assert_eq!(
        oracle
            .finish(observations(PASS_SIGNATURE, None, halt_loop_window()))
            .expect("oracle should finish"),
        OracleOutcome::Passed
    );
}

#[test]
fn fibonacci_result_detects_compact_breakpoint_loop_near_pc() {
    let mut oracle = fibonacci_oracle();
    assert_eq!(
        oracle
            .observe(observations(
                PASS_SIGNATURE,
                None,
                compact_terminal_loop_window()
            ))
            .expect("oracle should observe"),
        OracleStep::Stop
    );
    assert_eq!(
        oracle
            .finish(observations(
                PASS_SIGNATURE,
                None,
                compact_terminal_loop_window()
            ))
            .expect("oracle should finish"),
        OracleOutcome::Passed
    );
}

#[test]
fn fibonacci_result_detects_breakpoint_halt_near_pc() {
    let mut oracle = fibonacci_oracle();
    assert_eq!(
        oracle
            .observe(observations(PASS_SIGNATURE, None, breakpoint_halt_window()))
            .expect("oracle should observe"),
        OracleStep::Stop
    );
    assert_eq!(
        oracle
            .finish(observations(PASS_SIGNATURE, None, breakpoint_halt_window()))
            .expect("oracle should finish"),
        OracleOutcome::Passed
    );
}

#[test]
fn fibonacci_result_fails_when_timeout_finishes_without_result() {
    let mut oracle = fibonacci_oracle();
    assert!(matches!(
        oracle
            .finish(observations([0; 6], None, no_halt_loop_window()))
            .expect("oracle should finish"),
        OracleOutcome::Failed(message) if message.contains("was not reached")
    ));
}
