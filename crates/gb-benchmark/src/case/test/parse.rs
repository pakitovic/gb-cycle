use std::path::PathBuf;

use gb_core::JoypadButton;

use crate::{
    BenchmarkConfigError, BenchmarkModel, BenchmarkStimulusTime, parse_benchmark_case,
    parse_benchmark_cases, parse_benchmark_suite, target_frames_for_duration,
};

#[test]
fn parse_case_supports_single_run_inputs() {
    let case = parse_benchmark_case(
        "case.toml",
        r#"
version = 1
id = "dr-mario"
rom = "Dr. Mario.gb"
model = "DMG"
startup = "custom-boot"
mode = "permissive"
palette = "grey"

[[run]]
id = "inputs"
duration_seconds = 8

[[run.input]]
frame = 30
button = "start"
hold_frames = 8

[[run.input]]
second = 2
buttons = ["a", "b"]
hold_frames = 4

[[run.input]]
tcycle = 70224
button = "select"
"#,
    )
    .expect("benchmark case should parse");

    assert_eq!(case.id, "dr-mario");
    assert_eq!(case.artifact_id, "dr-mario-inputs");
    assert_eq!(case.rom, PathBuf::from("Dr. Mario.gb"));
    assert_eq!(case.run_id.as_deref(), Some("inputs"));
    assert_eq!(case.model, BenchmarkModel::Dmg);
    assert_eq!(case.stimuli.len(), 8);
    assert_eq!(case.stimuli[0].when, BenchmarkStimulusTime::Frame(30));
    assert_eq!(case.stimuli[0].button, JoypadButton::Start);
    assert!(case.stimuli[0].pressed);
    assert_eq!(case.stimuli[1].when, BenchmarkStimulusTime::Frame(38));
    assert_eq!(case.stimuli[1].button, JoypadButton::Start);
    assert!(!case.stimuli[1].pressed);
    assert!(case.stimuli.iter().any(|stimulus| {
        stimulus.when == BenchmarkStimulusTime::Frame(target_frames_for_duration(2))
            && stimulus.button == JoypadButton::A
            && stimulus.pressed
    }));
    assert!(case.stimuli.iter().any(|stimulus| {
        stimulus.when == BenchmarkStimulusTime::TCycle(70224)
            && stimulus.button == JoypadButton::Select
            && stimulus.pressed
    }));
}

#[test]
fn parse_suite_expands_multiple_fresh_runs() {
    let suite = parse_benchmark_suite(
        "benchmark/test/case.toml",
        r#"
version = 1
id = "alone-in-the-dark"
rom = "roms/alone.gbc"
model = "CGB"
startup = "custom-boot"
mode = "permissive"
screenshot = true
stats = true

[[run]]
id = "idle-40"
label = "40s idle"
duration_seconds = 40

[[run]]
id = "start-a-120"
label = "120s Start/A"
duration_seconds = 120

[[run.input]]
frame = 30
button = "start"
hold_frames = 8
repeat_every_frames = 60

[[run.input]]
second = 2
buttons = ["start", "a"]
hold_frames = 4
"#,
    )
    .expect("benchmark suite should parse");

    assert_eq!(suite.cases.len(), 2);
    assert_eq!(suite.rom, PathBuf::from("benchmark/test/roms/alone.gbc"));
    assert_eq!(suite.cases[0].id, "alone-in-the-dark");
    assert_eq!(suite.cases[0].artifact_id, "alone-in-the-dark-idle-40");
    assert_eq!(
        suite.cases[0].rom,
        PathBuf::from("benchmark/test/roms/alone.gbc")
    );
    assert_eq!(suite.cases[0].duration_seconds, 40);
    assert_eq!(suite.cases[0].stimuli, Vec::new());

    let active = &suite.cases[1];
    assert_eq!(active.run_id.as_deref(), Some("start-a-120"));
    assert_eq!(active.run_label.as_deref(), Some("120s Start/A"));
    assert_eq!(active.artifact_id, "alone-in-the-dark-start-a-120");
    assert_eq!(active.stimuli[0].when, BenchmarkStimulusTime::Frame(30));
    assert_eq!(active.stimuli[0].button, JoypadButton::Start);
    assert!(active.stimuli[0].pressed);
    assert_eq!(active.stimuli[1].when, BenchmarkStimulusTime::Frame(38));
    assert!(!active.stimuli[1].pressed);
    assert!(active.stimuli.iter().any(|stimulus| {
        stimulus.when == BenchmarkStimulusTime::Frame(target_frames_for_duration(2))
            && stimulus.button == JoypadButton::A
            && stimulus.pressed
    }));
}

#[test]
fn input_pulses_repeat_until_the_run_ends() {
    let cases = parse_benchmark_cases(
        "case.toml",
        r#"
version = 1
id = "repeat"
rom = "repeat.gb"
model = "DMG"
startup = "custom-boot"
mode = "permissive"

[[run]]
id = "tap"
duration_seconds = 1

[[run.input]]
frame = 2
button = "a"
hold_frames = 3
repeat_every_frames = 10
"#,
    )
    .expect("repeating input should parse");

    let stimuli = &cases[0].stimuli;
    assert_eq!(stimuli.len(), 12);
    assert_eq!(stimuli[0].when, BenchmarkStimulusTime::Frame(2));
    assert!(stimuli[0].pressed);
    assert_eq!(stimuli[1].when, BenchmarkStimulusTime::Frame(5));
    assert!(!stimuli[1].pressed);
    assert_eq!(stimuli[2].when, BenchmarkStimulusTime::Frame(12));
    assert!(stimuli[2].pressed);
}

#[test]
fn parse_suite_rejects_removed_legacy_format_and_missing_runs() {
    let legacy = parse_benchmark_cases(
        "case.toml",
        r#"
version = 1
id = "bad"
rom = "bad.gb"
model = "DMG"
startup = "custom-boot"
mode = "permissive"
duration_seconds = 8

[[stimulus]]
frame = 1
button = "a"
pressed = true
"#,
    )
    .expect_err("legacy format should fail");

    assert!(matches!(
        legacy,
        BenchmarkConfigError::DeprecatedLegacyFormat { .. }
    ));

    let missing_runs = parse_benchmark_cases(
        "case.toml",
        r#"
version = 1
id = "bad"
rom = "bad.gb"
model = "DMG"
startup = "custom-boot"
mode = "permissive"
"#,
    )
    .expect_err("suite without runs should fail");

    assert!(matches!(
        missing_runs,
        BenchmarkConfigError::MissingRuns { .. }
    ));

    let run_stimulus = parse_benchmark_cases(
        "case.toml",
        r#"
version = 1
id = "bad"
rom = "bad.gb"
model = "DMG"
startup = "custom-boot"
mode = "permissive"

[[run]]
id = "raw"
duration_seconds = 1

[[run.stimulus]]
frame = 1
button = "a"
pressed = true
"#,
    )
    .expect_err("run stimulus format should fail");

    assert!(matches!(
        run_stimulus,
        BenchmarkConfigError::DeprecatedLegacyFormat { .. }
    ));
}

#[test]
fn parse_suite_rejects_invalid_run_duration_and_inputs() {
    let invalid_case_id = parse_benchmark_cases(
        "case.toml",
        r#"
version = 1
id = "../bad"
rom = "bad.gb"
model = "DMG"
startup = "custom-boot"
mode = "permissive"

[[run]]
id = "safe"
duration_seconds = 1
"#,
    )
    .expect_err("unsafe case id should fail");
    assert!(matches!(
        invalid_case_id,
        BenchmarkConfigError::InvalidArtifactId { .. }
    ));

    let invalid_run_id = parse_benchmark_cases(
        "case.toml",
        r#"
version = 1
id = "safe"
rom = "bad.gb"
model = "DMG"
startup = "custom-boot"
mode = "permissive"

[[run]]
id = "bad/run"
duration_seconds = 1
"#,
    )
    .expect_err("unsafe run id should fail");
    assert!(matches!(
        invalid_run_id,
        BenchmarkConfigError::InvalidArtifactId { .. }
    ));

    let zero_duration = parse_benchmark_cases(
        "case.toml",
        r#"
version = 1
id = "bad"
rom = "bad.gb"
model = "DMG"
startup = "custom-boot"
mode = "permissive"

[[run]]
id = "zero"
duration_seconds = 0
"#,
    )
    .expect_err("zero duration should fail");
    assert!(matches!(
        zero_duration,
        BenchmarkConfigError::ZeroDuration { .. }
    ));

    let ambiguous_time = parse_benchmark_cases(
        "case.toml",
        r#"
version = 1
id = "bad"
rom = "bad.gb"
model = "DMG"
startup = "custom-boot"
mode = "permissive"

[[run]]
id = "ambiguous"
duration_seconds = 1

[[run.input]]
frame = 1
second = 1
button = "a"
"#,
    )
    .expect_err("ambiguous input timing should fail");
    assert!(matches!(
        ambiguous_time,
        BenchmarkConfigError::InvalidInput { .. }
    ));

    let invalid_button = parse_benchmark_cases(
        "case.toml",
        r#"
version = 1
id = "bad"
rom = "bad.gb"
model = "DMG"
startup = "custom-boot"
mode = "permissive"

[[run]]
id = "button"
duration_seconds = 1

[[run.input]]
frame = 1
button = "coin"
"#,
    )
    .expect_err("unsupported button should fail");
    assert!(matches!(
        invalid_button,
        BenchmarkConfigError::InvalidInput { .. }
    ));

    let invalid_repeat = parse_benchmark_cases(
        "case.toml",
        r#"
version = 1
id = "bad"
rom = "bad.gb"
model = "DMG"
startup = "custom-boot"
mode = "permissive"

[[run]]
id = "repeat"
duration_seconds = 1

[[run.input]]
frame = 1
button = "a"
hold_frames = 8
repeat_every_frames = 8
"#,
    )
    .expect_err("invalid repeat interval should fail");
    assert!(matches!(
        invalid_repeat,
        BenchmarkConfigError::InvalidInput { .. }
    ));
}
