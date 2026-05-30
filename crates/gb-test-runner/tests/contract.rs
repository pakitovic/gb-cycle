use std::path::{Path, PathBuf};

use gb_core::{ConsoleModel, ExecutionMode, JoypadButton, StartupMode};
use gb_test_runner::{
    CaptureKind, CapturePlan, ExternalStimulus, ExternalStimulusAction, ExternalStimulusPlan,
    FailureArtifactPolicy, MemoryByteExpectation, MemoryTextOutputSpec, PassCondition,
    RomCaseValidationError, RomSuite, RomSuiteValidationError, RomTestCase, StimulusTime,
    TEST_ROM_STORE_DIR, Timeout, acid_suite, ashiepaws_suite, blargg_curated_suites,
    built_in_rom_suite_by_name, daid_suite, mealybug_tearoom_suite, mooneye_curated_suites,
    phase_2_cpu_timing_suite, phase_2_interrupt_timing_suite, phase_4_ppu_oam_corruption_suite,
    phase_6_cartridge_oracle_suite, phase_6_mbc6_oracle_suite,
};

fn rom_path_without_store_prefix(rom_path: &Path) -> &Path {
    let mut normalized_path = rom_path;
    if let Ok(stripped) = normalized_path.strip_prefix(TEST_ROM_STORE_DIR) {
        normalized_path = stripped;
    }
    if let Ok(stripped) = normalized_path.strip_prefix("gb-emulator-shootout") {
        normalized_path = stripped;
    }
    normalized_path
}

#[test]
fn new_rom_test_case_defaults_to_dmg_skip_boot_strict_with_debug_artifacts() {
    let case = RomTestCase::new(
        "blargg_cpu_instrs",
        PathBuf::from("crates/gb-core/tests/fixtures/roms/blargg.gb"),
        Timeout::Frames(600),
        PassCondition::SerialContains("Passed".to_string()),
    );

    assert_eq!(case.console_model, ConsoleModel::GameBoy);
    assert_eq!(case.startup_mode, StartupMode::SkipBoot);
    assert_eq!(case.execution_mode, ExecutionMode::Strict);
    assert!(case.external_stimuli.stimuli().is_empty());
    assert!(case.capture_plan.contains(CaptureKind::Serial));
    assert!(case.capture_plan.contains(CaptureKind::Trace));
    assert!(case.capture_plan.contains(CaptureKind::Snapshot));
    assert!(case.failure_artifacts.contains(CaptureKind::Serial));
    assert!(case.failure_artifacts.contains(CaptureKind::Trace));
    assert!(case.failure_artifacts.contains(CaptureKind::Snapshot));
    assert_eq!(case.validate(), Ok(()));
}

#[test]
fn rom_test_case_requires_capture_matching_the_pass_condition() {
    let case = RomTestCase::new(
        "dmg_acid2",
        PathBuf::from("crates/gb-core/tests/fixtures/roms/dmg-acid2.gb"),
        Timeout::Frames(1200),
        PassCondition::FramebufferFixture(PathBuf::from("expected/dmg-acid2.png")),
    )
    .with_capture_plan(CapturePlan::new().with_capture(CaptureKind::Trace))
    .with_failure_artifacts(
        FailureArtifactPolicy::new()
            .with_artifact(CaptureKind::Framebuffer)
            .with_artifact(CaptureKind::Trace),
    );

    assert_eq!(
        case.validate(),
        Err(RomCaseValidationError::MissingRequiredCapture(
            CaptureKind::Framebuffer
        ))
    );
}

#[test]
fn rom_test_case_requires_memory_text_capture_for_memory_text_output_conditions() {
    let case = RomTestCase::new(
        "mem-text-case",
        PathBuf::from("mem_text.gb"),
        Timeout::Frames(60),
        PassCondition::MemoryTextOutputContains {
            spec: MemoryTextOutputSpec::new(
                0xA000,
                0x80,
                0x00,
                0xA001,
                [0xDE, 0xB0, 0x61],
                0xA004,
                128,
            ),
            expected_substring: "Passed".to_string(),
        },
    )
    .with_capture_plan(CapturePlan::new().with_capture(CaptureKind::Snapshot))
    .with_failure_artifacts(
        FailureArtifactPolicy::new()
            .with_artifact(CaptureKind::MemoryTextOutput)
            .with_artifact(CaptureKind::Snapshot),
    );

    assert_eq!(
        case.validate(),
        Err(RomCaseValidationError::MissingRequiredCapture(
            CaptureKind::MemoryTextOutput
        ))
    );
}

#[test]
fn rom_test_case_requires_memory_bytes_capture_for_memory_byte_conditions() {
    let case = RomTestCase::new(
        "memory-byte-case",
        PathBuf::from("memory_byte.gb"),
        Timeout::TCycles(100_000),
        PassCondition::MemoryBytesEqual(vec![MemoryByteExpectation::new(0xFF82, 0x01)]),
    )
    .with_capture_plan(CapturePlan::new().with_capture(CaptureKind::Snapshot))
    .with_failure_artifacts(
        FailureArtifactPolicy::new()
            .with_artifact(CaptureKind::MemoryBytes)
            .with_artifact(CaptureKind::Snapshot),
    );

    assert_eq!(
        case.validate(),
        Err(RomCaseValidationError::MissingRequiredCapture(
            CaptureKind::MemoryBytes
        ))
    );
}

#[test]
fn rom_test_case_requires_blargg_console_capture_for_console_text_conditions() {
    let case = RomTestCase::new(
        "blargg-console-case",
        PathBuf::from("lcd.gb"),
        Timeout::Frames(60),
        PassCondition::BlarggConsoleTextContains("Passed".to_string()),
    )
    .with_capture_plan(CapturePlan::new().with_capture(CaptureKind::Snapshot))
    .with_failure_artifacts(
        FailureArtifactPolicy::new()
            .with_artifact(CaptureKind::BlarggConsoleText)
            .with_artifact(CaptureKind::Snapshot),
    );

    assert_eq!(
        case.validate(),
        Err(RomCaseValidationError::MissingRequiredCapture(
            CaptureKind::BlarggConsoleText
        ))
    );
}

#[test]
fn rom_test_case_requires_snapshot_capture_for_mooneye_result_conditions() {
    let case = RomTestCase::new(
        "mooneye-case",
        PathBuf::from("mooneye.gb"),
        Timeout::Frames(180),
        PassCondition::MooneyeResult,
    )
    .with_capture_plan(CapturePlan::new().with_capture(CaptureKind::Serial))
    .with_failure_artifacts(
        FailureArtifactPolicy::new()
            .with_artifact(CaptureKind::Snapshot)
            .with_artifact(CaptureKind::Serial),
    );

    assert_eq!(
        case.validate(),
        Err(RomCaseValidationError::MissingRequiredCapture(
            CaptureKind::Snapshot
        ))
    );
}

#[test]
fn rom_test_case_rejects_failure_artifacts_that_are_not_captured() {
    let case = RomTestCase::new(
        "mealybug_ly",
        PathBuf::from("crates/gb-core/tests/fixtures/roms/mealybug.gb"),
        Timeout::TCycles(4_194_304),
        PassCondition::TraceFixture(PathBuf::from("expected/ly.trace")),
    )
    .with_capture_plan(CapturePlan::new().with_capture(CaptureKind::Trace))
    .with_failure_artifacts(
        FailureArtifactPolicy::new()
            .with_artifact(CaptureKind::Trace)
            .with_artifact(CaptureKind::Snapshot),
    );

    assert_eq!(
        case.validate(),
        Err(RomCaseValidationError::ArtifactNotCaptured(
            CaptureKind::Snapshot
        ))
    );
}

#[test]
fn rom_test_case_rejects_duplicate_external_stimuli() {
    let duplicated = ExternalStimulus::at_t_cycle(
        380,
        ExternalStimulusAction::JoypadSetButton {
            button: JoypadButton::A,
            pressed: true,
        },
    );
    let case = RomTestCase::new(
        "phase2-halt-stop-and-halt-bug",
        PathBuf::from("phase2_halt_stop_and_halt_bug.gb"),
        Timeout::TCycles(512),
        PassCondition::TraceFixture(PathBuf::from("phase2_halt_stop_and_halt_bug.trace")),
    )
    .with_external_stimuli(
        ExternalStimulusPlan::new()
            .with_stimulus(duplicated)
            .with_stimulus(duplicated),
    );

    assert_eq!(
        case.validate(),
        Err(RomCaseValidationError::DuplicateExternalStimulus(
            duplicated
        ))
    );
}

#[test]
fn rom_suite_rejects_duplicate_case_ids() {
    let first = RomTestCase::new(
        "same-id",
        PathBuf::from("a.gb"),
        Timeout::Frames(10),
        PassCondition::SerialExact("Passed".to_string()),
    );
    let second = RomTestCase::new(
        "same-id",
        PathBuf::from("b.gb"),
        Timeout::Frames(10),
        PassCondition::SerialExact("Passed".to_string()),
    )
    .with_console_model(ConsoleModel::GameBoyPocket)
    .with_startup_mode(StartupMode::RealBoot)
    .with_execution_mode(ExecutionMode::Permissive);

    let suite = RomSuite::new("cpu").with_case(first).with_case(second);

    assert_eq!(
        suite.validate(),
        Err(RomSuiteValidationError::DuplicateCaseId(
            "same-id".to_string()
        ))
    );
}

#[test]
fn rom_suite_validates_a_scheduler_grouped_contract() {
    let suite = RomSuite::new("scheduler-ordering").with_case(RomTestCase::new(
        "phase-order-trace",
        PathBuf::from("synthetic/scheduler-order.gb"),
        Timeout::TCycles(1024),
        PassCondition::TraceFixture(PathBuf::from("expected/scheduler-order.trace")),
    ));

    assert_eq!(suite.validate(), Ok(()));
}

#[test]
fn rom_test_case_rejects_empty_id_empty_rom_path_and_zero_timeout() {
    let empty_id = RomTestCase::new(
        "",
        PathBuf::from("rom.gb"),
        Timeout::Frames(1),
        PassCondition::SerialExact("ok".to_string()),
    );
    let empty_path = RomTestCase::new(
        "valid-id",
        PathBuf::new(),
        Timeout::Frames(1),
        PassCondition::SerialExact("ok".to_string()),
    );
    let zero_timeout = RomTestCase::new(
        "valid-id",
        PathBuf::from("rom.gb"),
        Timeout::TCycles(0),
        PassCondition::SerialExact("ok".to_string()),
    );
    assert_eq!(
        empty_id.validate(),
        Err(RomCaseValidationError::EmptyCaseId)
    );
    assert_eq!(
        empty_path.validate(),
        Err(RomCaseValidationError::MissingRomPath)
    );
    assert_eq!(
        zero_timeout.validate(),
        Err(RomCaseValidationError::InvalidTimeout)
    );
}

#[test]
fn rom_test_case_requires_failure_artifacts_and_the_required_result_channel() {
    let missing_all_artifacts = RomTestCase::new(
        "serial-case",
        PathBuf::from("serial.gb"),
        Timeout::Frames(10),
        PassCondition::SerialContains("Passed".to_string()),
    )
    .with_failure_artifacts(FailureArtifactPolicy::new());

    let missing_required_artifact = RomTestCase::new(
        "framebuffer-case",
        PathBuf::from("lcd.gb"),
        Timeout::Frames(10),
        PassCondition::FramebufferFixture(PathBuf::from("expected/frame.png")),
    )
    .with_failure_artifacts(
        FailureArtifactPolicy::new()
            .with_artifact(CaptureKind::Trace)
            .with_artifact(CaptureKind::Snapshot),
    );

    assert_eq!(
        missing_all_artifacts.validate(),
        Err(RomCaseValidationError::MissingFailureArtifacts)
    );
    assert_eq!(
        missing_required_artifact.validate(),
        Err(RomCaseValidationError::MissingRequiredFailureArtifact(
            CaptureKind::Framebuffer
        ))
    );
}

#[test]
fn rom_suite_rejects_empty_name_and_invalid_cases() {
    let invalid_case = RomTestCase::new(
        "",
        PathBuf::from("rom.gb"),
        Timeout::Frames(1),
        PassCondition::SerialExact("Passed".to_string()),
    );
    let empty_name = RomSuite::new("");
    let invalid_suite = RomSuite::new("invalid-cases").with_case(invalid_case);

    assert_eq!(
        empty_name.validate(),
        Err(RomSuiteValidationError::EmptySuiteName)
    );
    assert_eq!(
        invalid_suite.validate(),
        Err(RomSuiteValidationError::InvalidCase {
            case_id: "".to_string(),
            error: RomCaseValidationError::EmptyCaseId,
        })
    );
}

#[test]
fn capture_and_artifact_builders_expose_their_registered_sets() {
    let capture_plan = CapturePlan::debugging_minimum_for(&PassCondition::TraceFixture(
        PathBuf::from("expected.trace"),
    ));
    let failure_artifacts = FailureArtifactPolicy::debugging_minimum_for(
        &PassCondition::SerialExact("Passed".to_string()),
    );

    assert_eq!(capture_plan.captures().len(), 2);
    assert!(capture_plan.contains(CaptureKind::Trace));
    assert!(capture_plan.contains(CaptureKind::Snapshot));

    assert_eq!(failure_artifacts.retained().len(), 3);
    assert!(failure_artifacts.contains(CaptureKind::Serial));
    assert!(failure_artifacts.contains(CaptureKind::Trace));
    assert!(failure_artifacts.contains(CaptureKind::Snapshot));
}

#[test]
fn external_stimulus_plan_builders_expose_the_registered_schedule() {
    let t_cycle_stimulus = ExternalStimulus::at_t_cycle(
        380,
        ExternalStimulusAction::JoypadSetButton {
            button: JoypadButton::A,
            pressed: true,
        },
    );
    let frame_stimulus = ExternalStimulus::at_frame(
        3,
        ExternalStimulusAction::JoypadSetButton {
            button: JoypadButton::Start,
            pressed: false,
        },
    );
    let plan = ExternalStimulusPlan::new()
        .with_stimulus(t_cycle_stimulus)
        .with_stimulus(frame_stimulus);

    assert_eq!(plan.stimuli().len(), 2);
    assert!(plan.contains(t_cycle_stimulus));
    assert!(plan.contains(frame_stimulus));
    assert_eq!(plan.stimuli()[0].when, StimulusTime::TCycle(380));
    assert_eq!(plan.stimuli()[1].when, StimulusTime::Frame(3));
}

#[test]
fn rom_suite_can_be_built_incrementally_with_push_case() {
    let mut suite = RomSuite::new("boot");
    suite.push_case(RomTestCase::new(
        "skip-boot-handshake",
        PathBuf::from("boot.gb"),
        Timeout::Frames(60),
        PassCondition::TraceFixture(PathBuf::from("boot.trace")),
    ));

    assert_eq!(suite.cases.len(), 1);
    assert_eq!(suite.validate(), Ok(()));
}

#[test]
fn phase_2_rom_automation_targets_validate_for_cpu_and_interrupt_timing() {
    let cpu_suite = phase_2_cpu_timing_suite();
    let interrupt_suite = phase_2_interrupt_timing_suite();

    assert_eq!(cpu_suite.validate(), Ok(()));
    assert_eq!(interrupt_suite.validate(), Ok(()));
    assert!(
        cpu_suite
            .cases
            .iter()
            .all(|case| case.execution_mode == ExecutionMode::Strict)
    );
    assert!(
        interrupt_suite
            .cases
            .iter()
            .all(|case| case.execution_mode == ExecutionMode::Strict)
    );
    assert!(
        cpu_suite
            .cases
            .iter()
            .chain(interrupt_suite.cases.iter())
            .all(|case| case
                .rom_path
                .starts_with(Path::new("crates/gb-core/tests/fixtures/roms/phase2")))
    );
    assert!(
        cpu_suite
            .cases
            .iter()
            .chain(interrupt_suite.cases.iter())
            .all(|case| trace_fixture_path(case)
                .starts_with(Path::new("crates/gb-core/tests/fixtures/traces/phase2")))
    );
    assert!(
        cpu_suite
            .cases
            .iter()
            .chain(interrupt_suite.cases.iter())
            .all(|case| case.capture_plan.contains(CaptureKind::Trace))
    );
    assert!(
        cpu_suite
            .cases
            .iter()
            .chain(interrupt_suite.cases.iter())
            .all(|case| case.failure_artifacts.contains(CaptureKind::Snapshot))
    );
    let halt_stop_case = interrupt_suite
        .cases
        .iter()
        .find(|case| case.id == "phase2-halt-stop-and-halt-bug")
        .expect("halt/stop case should exist");
    assert_eq!(halt_stop_case.external_stimuli.stimuli().len(), 2);
    assert_eq!(
        halt_stop_case.external_stimuli.stimuli()[0],
        ExternalStimulus::at_t_cycle(
            356,
            ExternalStimulusAction::JoypadSetButton {
                button: JoypadButton::A,
                pressed: true,
            }
        )
    );
    assert_eq!(
        halt_stop_case.external_stimuli.stimuli()[1],
        ExternalStimulus::at_t_cycle(
            357,
            ExternalStimulusAction::WriteMemory {
                address: 0xFF0F,
                value: 0x01,
            }
        )
    );
    assert!(
        interrupt_suite
            .cases
            .iter()
            .filter(|case| !case.external_stimuli.stimuli().is_empty())
            .all(|case| case.id == "phase2-halt-stop-and-halt-bug")
    );
}

#[test]
fn phase_4_rom_automation_targets_validate_for_ppu_oam_corruption() {
    let suite = phase_4_ppu_oam_corruption_suite();

    assert_eq!(suite.validate(), Ok(()));
    assert!(
        suite
            .cases
            .iter()
            .all(|case| case.execution_mode == ExecutionMode::Strict)
    );
    assert!(suite.cases.iter().all(|case| {
        case.rom_path
            .starts_with(Path::new("crates/gb-core/tests/fixtures/roms/phase4"))
    }));
    assert!(suite.cases.iter().all(|case| {
        trace_fixture_path(case)
            .starts_with(Path::new("crates/gb-core/tests/fixtures/traces/phase4"))
    }));
    assert!(
        suite
            .cases
            .iter()
            .all(|case| case.capture_plan.contains(CaptureKind::Trace))
    );
    assert!(
        suite
            .cases
            .iter()
            .all(|case| case.capture_plan.contains(CaptureKind::Snapshot))
    );
    assert!(
        suite
            .cases
            .iter()
            .all(|case| case.failure_artifacts.contains(CaptureKind::Trace))
    );
    assert!(
        suite
            .cases
            .iter()
            .all(|case| case.failure_artifacts.contains(CaptureKind::Snapshot))
    );
    assert!(
        suite
            .cases
            .iter()
            .any(|case| case.console_model == ConsoleModel::GameBoy)
    );
    assert!(
        suite
            .cases
            .iter()
            .any(|case| case.console_model == ConsoleModel::GameBoy)
    );
    assert!(
        suite
            .cases
            .iter()
            .any(|case| case.console_model == ConsoleModel::GameBoyPocket)
    );
    assert!(
        suite
            .cases
            .iter()
            .any(|case| case.console_model == ConsoleModel::GameBoyColor)
    );
}

#[test]
fn curated_blargg_suite_tracks_the_full_individual_shootout_list() {
    let split_suites = blargg_curated_suites();
    let cases = split_suites
        .iter()
        .flat_map(|suite| suite.cases.iter())
        .collect::<Vec<_>>();

    assert_eq!(
        split_suites
            .iter()
            .map(|suite| suite.name.as_str())
            .collect::<Vec<_>>(),
        vec![
            "blargg-cpu-instrs",
            "blargg-dmg-sound",
            "blargg-timing-memory-oam"
        ]
    );
    assert!(
        split_suites.iter().all(|suite| {
            suite.validate() == Ok(()) && suite.family.as_deref() == Some("blargg")
        })
    );
    assert_eq!(cases.len(), 39);
    assert!(
        cases
            .iter()
            .any(|case| case.id == "blargg-cpu-instrs-01-special")
    );
    assert!(
        cases
            .iter()
            .any(|case| case.id == "blargg-oam-bug-8-instr-effect")
    );
    assert!(cases.iter().any(|case| case.id == "blargg-instr-timing"));
    assert!(cases.iter().any(|case| case.id == "blargg-interrupt-time"));
    assert!(
        cases
            .iter()
            .any(|case| case.id == "blargg-dmg-sound-12-wave-write-while-on")
    );
}

#[test]
fn acid_suite_tracks_framebuffer_oracle_and_informational_cases() {
    let suite = acid_suite();

    assert_eq!(suite.validate(), Ok(()));
    assert_eq!(suite.family.as_deref(), Some("acid"));
    assert_eq!(suite.cases.len(), 5);

    let which_dmg_case = suite
        .cases
        .iter()
        .find(|case| case.id == "acid-which-dmg")
        .expect("acid suite should include which.gb DMG");
    assert_eq!(
        which_dmg_case.rom_path,
        PathBuf::from("test/gb-emulator-shootout/acid/which.gb")
    );
    assert!(
        which_dmg_case
            .capture_plan
            .contains(CaptureKind::Framebuffer)
    );
    assert!(which_dmg_case.capture_plan.contains(CaptureKind::Snapshot));
    assert!(
        which_dmg_case
            .failure_artifacts
            .contains(CaptureKind::Framebuffer)
    );
    assert!(matches!(
        which_dmg_case.pass_condition,
        PassCondition::Informational(CaptureKind::Framebuffer)
    ));

    let which_cgb_case = suite
        .cases
        .iter()
        .find(|case| case.id == "acid-which-cgb")
        .expect("acid suite should include which.gb CGB");
    assert_eq!(
        which_cgb_case.rom_path,
        PathBuf::from("test/gb-emulator-shootout/acid/which.gb")
    );
    assert!(
        which_cgb_case
            .capture_plan
            .contains(CaptureKind::Framebuffer)
    );
    assert!(which_cgb_case.capture_plan.contains(CaptureKind::Snapshot));
    assert!(
        which_cgb_case
            .failure_artifacts
            .contains(CaptureKind::Framebuffer)
    );
    assert!(matches!(
        which_cgb_case.pass_condition,
        PassCondition::Informational(CaptureKind::Framebuffer)
    ));

    let case = suite
        .cases
        .iter()
        .find(|case| case.id == "acid-dmg-acid2")
        .expect("acid suite should include acid-dmg-acid2");
    assert_eq!(
        case.rom_path,
        PathBuf::from("test/gb-emulator-shootout/acid/dmg-acid2.gb")
    );
    assert!(case.capture_plan.contains(CaptureKind::Framebuffer));
    assert!(case.capture_plan.contains(CaptureKind::Snapshot));
    assert!(case.failure_artifacts.contains(CaptureKind::Framebuffer));
    assert!(matches!(
        case.pass_condition,
        PassCondition::FramebufferFixture(_)
    ));

    let case = suite
        .cases
        .iter()
        .find(|case| case.id == "acid-cgb-acid2")
        .expect("acid suite should include acid-cgb-acid2");
    assert_eq!(
        case.rom_path,
        PathBuf::from("test/gb-emulator-shootout/acid/cgb-acid2.gbc")
    );
    assert!(case.capture_plan.contains(CaptureKind::Framebuffer));
    assert!(case.capture_plan.contains(CaptureKind::Snapshot));
    assert!(case.failure_artifacts.contains(CaptureKind::Framebuffer));
    assert!(matches!(
        case.pass_condition,
        PassCondition::FramebufferRgb555GrayscaleToleranceFixture(_)
    ));

    let case = suite
        .cases
        .iter()
        .find(|case| case.id == "acid-cgb-acid-hell")
        .expect("acid suite should include acid-cgb-acid-hell");
    assert_eq!(
        case.rom_path,
        PathBuf::from("test/gb-emulator-shootout/acid/cgb-acid-hell.gbc")
    );
    assert!(case.capture_plan.contains(CaptureKind::Framebuffer));
    assert!(case.capture_plan.contains(CaptureKind::Snapshot));
    assert!(case.failure_artifacts.contains(CaptureKind::Framebuffer));
    assert!(matches!(
        case.pass_condition,
        PassCondition::FramebufferRgb555Fixture(_)
    ));
}

#[test]
fn daid_suite_tracks_mixed_framebuffer_oracles() {
    let suite = daid_suite();

    assert_eq!(suite.validate(), Ok(()));
    assert_eq!(suite.family.as_deref(), Some("daid"));
    assert_eq!(suite.cases.len(), 9);

    let ppu_case = suite
        .cases
        .iter()
        .find(|case| case.id == "daid-ppu-scanline-bgp-dmg")
        .expect("daid suite should include ppu_scanline_bgp DMG");
    assert_eq!(
        rom_path_without_store_prefix(&ppu_case.rom_path),
        Path::new("daid/ppu_scanline_bgp.gb")
    );
    assert!(ppu_case.capture_plan.contains(CaptureKind::Framebuffer));
    assert!(ppu_case.capture_plan.contains(CaptureKind::Snapshot));
    assert!(matches!(
        ppu_case.pass_condition,
        PassCondition::FramebufferFixtureSet(_)
    ));

    let ppu_case = suite
        .cases
        .iter()
        .find(|case| case.id == "daid-ppu-scanline-bgp-gbc")
        .expect("daid suite should include ppu_scanline_bgp GBC");
    assert_eq!(
        rom_path_without_store_prefix(&ppu_case.rom_path),
        Path::new("daid/ppu_scanline_bgp.gb")
    );
    assert!(ppu_case.capture_plan.contains(CaptureKind::Framebuffer));
    assert!(ppu_case.capture_plan.contains(CaptureKind::Snapshot));
    assert!(matches!(
        ppu_case.pass_condition,
        PassCondition::FramebufferRgb555Fixture(_)
    ));

    let stop_case = suite
        .cases
        .iter()
        .find(|case| case.id == "daid-stop-instr-dmg")
        .expect("daid suite should include stop_instr DMG");
    assert_eq!(
        rom_path_without_store_prefix(&stop_case.rom_path),
        Path::new("daid/stop_instr.gb")
    );
    assert!(matches!(
        stop_case.pass_condition,
        PassCondition::FramebufferGrayscaleFixture(_)
    ));

    let info_case = suite
        .cases
        .iter()
        .find(|case| case.id == "daid-rom-and-ram")
        .expect("daid suite should include rom_and_ram");
    assert_eq!(
        rom_path_without_store_prefix(&info_case.rom_path),
        Path::new("daid/rom_and_ram.gb")
    );
    assert_eq!(info_case.execution_mode, ExecutionMode::Permissive);
    assert!(matches!(
        info_case.pass_condition,
        PassCondition::Informational(CaptureKind::Framebuffer)
    ));
}

#[test]
fn curated_mealybug_suite_uses_framebuffer_fixture_contracts() {
    let suite = mealybug_tearoom_suite();

    assert_eq!(suite.validate(), Ok(()));
    assert_eq!(suite.family.as_deref(), Some("mealybug-tearoom-tests"));
    assert_eq!(suite.cases.len(), 24);
    assert!(suite.cases.iter().all(|case| {
        case.rom_path.starts_with(Path::new(TEST_ROM_STORE_DIR))
            && case.capture_plan.contains(CaptureKind::Framebuffer)
            && case.capture_plan.contains(CaptureKind::Snapshot)
            && case.failure_artifacts.contains(CaptureKind::Framebuffer)
            && matches!(
                case.pass_condition,
                PassCondition::FramebufferFixture(_) | PassCondition::FramebufferFixtureSet(_)
            )
    }));
    assert!(
        suite
            .cases
            .iter()
            .any(|case| case.id == "mealybug-tearoom-tests-ppu-m2-win-en-toggle")
    );
    assert!(
        suite
            .cases
            .iter()
            .any(|case| case.id == "mealybug-tearoom-tests-ppu-m3-window-timing-wx-0")
    );
    assert!(
        suite
            .cases
            .iter()
            .any(|case| case.id == "mealybug-tearoom-tests-ppu-m3-lcdc-bg-en-change")
    );
    assert!(
        suite
            .cases
            .iter()
            .any(|case| case.id == "mealybug-tearoom-tests-ppu-m3-wx-6-change")
    );
    let obp0_change = suite
        .cases
        .iter()
        .find(|case| case.id == "mealybug-tearoom-tests-ppu-m3-obp0-change")
        .expect("curated mealybug suite should include m3_obp0_change");
    assert_eq!(obp0_change.startup_mode, StartupMode::CustomBoot);
    assert!(obp0_change.startup_memory_writes.is_empty());
    let bgp_change_sprites = suite
        .cases
        .iter()
        .find(|case| case.id == "mealybug-tearoom-tests-ppu-m3-bgp-change-sprites")
        .expect("curated mealybug suite should include m3_bgp_change_sprites");
    assert_eq!(bgp_change_sprites.startup_mode, StartupMode::CustomBoot);
    assert!(bgp_change_sprites.startup_memory_writes.is_empty());
    let scx_low_3_bits = suite
        .cases
        .iter()
        .find(|case| case.id == "mealybug-tearoom-tests-ppu-m3-scx-low-3-bits")
        .expect("curated mealybug suite should include m3_scx_low_3_bits");
    assert_eq!(scx_low_3_bits.startup_mode, StartupMode::CustomBoot);
    assert!(scx_low_3_bits.startup_memory_writes.is_empty());
}

#[test]
fn curated_ashiepaws_suite_tracks_the_active_framebuffer_cases() {
    let suite = ashiepaws_suite();

    assert_eq!(suite.validate(), Ok(()));
    assert_eq!(suite.family.as_deref(), Some("ashiepaws"));
    assert_eq!(suite.cases.len(), 3);
    assert!(suite.cases.iter().all(|case| {
        case.rom_path.starts_with(Path::new(TEST_ROM_STORE_DIR))
            && case.capture_plan.contains(CaptureKind::Framebuffer)
            && case.capture_plan.contains(CaptureKind::Snapshot)
            && case.failure_artifacts.contains(CaptureKind::Framebuffer)
            && matches!(
                case.pass_condition,
                PassCondition::FramebufferFixture(_) | PassCondition::FramebufferRgb555Fixture(_)
            )
    }));

    let bully = suite
        .cases
        .iter()
        .find(|case| case.id == "ashiepaws-bully-dmg")
        .expect("ashiepaws suite should include bully.gb DMG");
    assert_eq!(
        rom_path_without_store_prefix(&bully.rom_path),
        Path::new("ashiepaws/bully.gb")
    );
    assert_eq!(bully.console_model, ConsoleModel::GameBoy);
    assert_eq!(bully.startup_mode, StartupMode::SkipBoot);
    assert!(bully.startup_memory_writes.is_empty());
    assert!(matches!(
        bully.pass_condition,
        PassCondition::FramebufferFixture(_)
    ));

    let bully_cgb = suite
        .cases
        .iter()
        .find(|case| case.id == "ashiepaws-bully-cgb")
        .expect("ashiepaws suite should include bully.gb CGB");
    assert_eq!(
        rom_path_without_store_prefix(&bully_cgb.rom_path),
        Path::new("ashiepaws/bully.gb")
    );
    assert_eq!(bully_cgb.console_model, ConsoleModel::GameBoyColor);
    assert_eq!(bully_cgb.startup_mode, StartupMode::CustomBoot);
    assert!(bully_cgb.startup_memory_writes.is_empty());
    assert!(matches!(
        bully_cgb.pass_condition,
        PassCondition::FramebufferRgb555Fixture(_)
    ));

    let strikethrough = suite
        .cases
        .iter()
        .find(|case| case.id == "ashiepaws-strikethrough")
        .expect("ashiepaws suite should include strikethrough.gb");
    assert_eq!(
        rom_path_without_store_prefix(&strikethrough.rom_path),
        Path::new("ashiepaws/strikethrough.gb")
    );
}

#[test]
fn curated_mooneye_suite_matches_the_active_gbemu_dmg_list_and_keeps_case_specific_oracles() {
    let split_suites = mooneye_curated_suites();
    let cases = split_suites
        .iter()
        .flat_map(|suite| suite.cases.iter())
        .collect::<Vec<_>>();

    assert_eq!(
        split_suites
            .iter()
            .map(|suite| suite.name.as_str())
            .collect::<Vec<_>>(),
        vec![
            "mooneye-acceptance-manual",
            "mooneye-emulator-mbc1-mbc5",
            "mooneye-emulator-mbc2"
        ]
    );
    assert!(
        split_suites.iter().all(|suite| {
            suite.validate() == Ok(()) && suite.family.as_deref() == Some("mooneye")
        })
    );
    assert_eq!(cases.len(), 95);
    let expected_rom_paths = [
        "mooneye/acceptance/add_sp_e_timing.gb",
        "mooneye/acceptance/bits/mem_oam.gb",
        "mooneye/acceptance/bits/reg_f.gb",
        "mooneye/acceptance/bits/unused_hwio-GS.gb",
        "mooneye/acceptance/boot_div-dmgABCmgb.gb",
        "mooneye/acceptance/boot_hwio-dmgABCmgb.gb",
        "mooneye/acceptance/boot_regs-dmgABC.gb",
        "mooneye/acceptance/call_cc_timing.gb",
        "mooneye/acceptance/call_cc_timing2.gb",
        "mooneye/acceptance/call_timing.gb",
        "mooneye/acceptance/call_timing2.gb",
        "mooneye/acceptance/div_timing.gb",
        "mooneye/acceptance/di_timing-GS.gb",
        "mooneye/acceptance/ei_sequence.gb",
        "mooneye/acceptance/ei_timing.gb",
        "mooneye/acceptance/halt_ime0_ei.gb",
        "mooneye/acceptance/halt_ime0_nointr_timing.gb",
        "mooneye/acceptance/halt_ime1_timing.gb",
        "mooneye/acceptance/halt_ime1_timing2-GS.gb",
        "mooneye/acceptance/if_ie_registers.gb",
        "mooneye/acceptance/instr/daa.gb",
        "mooneye/acceptance/interrupts/ie_push.gb",
        "mooneye/acceptance/intr_timing.gb",
        "mooneye/acceptance/jp_cc_timing.gb",
        "mooneye/acceptance/jp_timing.gb",
        "mooneye/acceptance/ld_hl_sp_e_timing.gb",
        "mooneye/acceptance/oam_dma/basic.gb",
        "mooneye/acceptance/oam_dma/reg_read.gb",
        "mooneye/acceptance/oam_dma/sources-GS.gb",
        "mooneye/acceptance/oam_dma_restart.gb",
        "mooneye/acceptance/oam_dma_start.gb",
        "mooneye/acceptance/oam_dma_timing.gb",
        "mooneye/acceptance/pop_timing.gb",
        "mooneye/acceptance/ppu/hblank_ly_scx_timing-GS.gb",
        "mooneye/acceptance/ppu/intr_1_2_timing-GS.gb",
        "mooneye/acceptance/ppu/intr_2_0_timing.gb",
        "mooneye/acceptance/ppu/intr_2_mode0_timing.gb",
        "mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb",
        "mooneye/acceptance/ppu/intr_2_mode3_timing.gb",
        "mooneye/acceptance/ppu/intr_2_oam_ok_timing.gb",
        "mooneye/acceptance/ppu/lcdon_timing-GS.gb",
        "mooneye/acceptance/ppu/lcdon_write_timing-GS.gb",
        "mooneye/acceptance/ppu/stat_irq_blocking.gb",
        "mooneye/acceptance/ppu/stat_lyc_onoff.gb",
        "mooneye/acceptance/ppu/vblank_stat_intr-GS.gb",
        "mooneye/acceptance/push_timing.gb",
        "mooneye/acceptance/rapid_di_ei.gb",
        "mooneye/acceptance/reti_intr_timing.gb",
        "mooneye/acceptance/reti_timing.gb",
        "mooneye/acceptance/ret_cc_timing.gb",
        "mooneye/acceptance/ret_timing.gb",
        "mooneye/acceptance/rst_timing.gb",
        "mooneye/acceptance/serial/boot_sclk_align-dmgABCmgb.gb",
        "mooneye/acceptance/timer/div_write.gb",
        "mooneye/acceptance/timer/rapid_toggle.gb",
        "mooneye/acceptance/timer/tim00.gb",
        "mooneye/acceptance/timer/tim00_div_trigger.gb",
        "mooneye/acceptance/timer/tim01.gb",
        "mooneye/acceptance/timer/tim01_div_trigger.gb",
        "mooneye/acceptance/timer/tim10.gb",
        "mooneye/acceptance/timer/tim10_div_trigger.gb",
        "mooneye/acceptance/timer/tim11.gb",
        "mooneye/acceptance/timer/tim11_div_trigger.gb",
        "mooneye/acceptance/timer/tima_reload.gb",
        "mooneye/acceptance/timer/tima_write_reloading.gb",
        "mooneye/acceptance/timer/tma_write_reloading.gb",
        "mooneye/emulator-only/mbc1/bits_bank1.gb",
        "mooneye/emulator-only/mbc1/bits_bank2.gb",
        "mooneye/emulator-only/mbc1/bits_mode.gb",
        "mooneye/emulator-only/mbc1/bits_ramg.gb",
        "mooneye/emulator-only/mbc1/multicart_rom_8Mb.gb",
        "mooneye/emulator-only/mbc1/ram_256kb.gb",
        "mooneye/emulator-only/mbc1/ram_64kb.gb",
        "mooneye/emulator-only/mbc1/rom_16Mb.gb",
        "mooneye/emulator-only/mbc1/rom_1Mb.gb",
        "mooneye/emulator-only/mbc1/rom_2Mb.gb",
        "mooneye/emulator-only/mbc1/rom_4Mb.gb",
        "mooneye/emulator-only/mbc1/rom_512kb.gb",
        "mooneye/emulator-only/mbc1/rom_8Mb.gb",
        "mooneye/emulator-only/mbc2/bits_ramg.gb",
        "mooneye/emulator-only/mbc2/bits_romb.gb",
        "mooneye/emulator-only/mbc2/bits_unused.gb",
        "mooneye/emulator-only/mbc2/ram.gb",
        "mooneye/emulator-only/mbc2/rom_1Mb.gb",
        "mooneye/emulator-only/mbc2/rom_2Mb.gb",
        "mooneye/emulator-only/mbc2/rom_512kb.gb",
        "mooneye/emulator-only/mbc5/rom_16Mb.gb",
        "mooneye/emulator-only/mbc5/rom_1Mb.gb",
        "mooneye/emulator-only/mbc5/rom_2Mb.gb",
        "mooneye/emulator-only/mbc5/rom_32Mb.gb",
        "mooneye/emulator-only/mbc5/rom_4Mb.gb",
        "mooneye/emulator-only/mbc5/rom_512kb.gb",
        "mooneye/emulator-only/mbc5/rom_64Mb.gb",
        "mooneye/emulator-only/mbc5/rom_8Mb.gb",
        "mooneye/manual-only/sprite_priority.gb",
    ];
    let mut expected_rom_paths = expected_rom_paths
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let mut actual_rom_paths = cases
        .iter()
        .map(|case| {
            rom_path_without_store_prefix(&case.rom_path)
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();
    expected_rom_paths.sort();
    actual_rom_paths.sort();
    assert_eq!(actual_rom_paths, expected_rom_paths);
    assert!(cases.iter().all(|case| {
        case.console_model == ConsoleModel::GameBoy
            && case.rom_path.starts_with(Path::new(TEST_ROM_STORE_DIR))
            && rom_path_without_store_prefix(&case.rom_path).starts_with(Path::new("mooneye"))
    }));
    assert!(!cases.iter().any(|case| {
        let path = rom_path_without_store_prefix(&case.rom_path).to_string_lossy();
        path.contains("sgb")
            || path.contains("cgb")
            || path.contains("apu")
            || path.contains("sound")
    }));
    assert!(
        cases
            .iter()
            .any(|case| case.id == "mooneye-acceptance-boot-regs-dmgabc")
    );
    assert!(
        cases
            .iter()
            .any(|case| case.id == "mooneye-acceptance-timer-tma-write-reloading")
    );
    let mbc1_bits_ramg = cases
        .iter()
        .find(|case| case.id == "mooneye-emulator-only-mbc1-bits-ramg")
        .expect("mooneye suite should include emulator-only mbc1 bits_ramg");
    assert_eq!(mbc1_bits_ramg.timeout, Timeout::Frames(780));
    let mbc2_bits_ramg = cases
        .iter()
        .find(|case| case.id == "mooneye-emulator-only-mbc2-bits-ramg")
        .expect("mooneye suite should include emulator-only mbc2 bits_ramg");
    assert_eq!(mbc2_bits_ramg.timeout, Timeout::Frames(900));
    let mbc1_multicart = cases
        .iter()
        .find(|case| case.id == "mooneye-emulator-only-mbc1-multicart-rom-8mb")
        .expect("mooneye suite should include emulator-only mbc1 multicart_rom_8Mb");
    assert_eq!(mbc1_multicart.execution_mode, ExecutionMode::Strict);
    let sprite_priority = cases
        .iter()
        .find(|case| case.id == "mooneye-manual-only-sprite-priority")
        .expect("mooneye suite should include manual-only sprite_priority");
    assert_eq!(
        sprite_priority.capture_plan,
        CapturePlan::new()
            .with_capture(CaptureKind::Framebuffer)
            .with_capture(CaptureKind::Snapshot)
    );
    assert_eq!(
        sprite_priority.failure_artifacts,
        FailureArtifactPolicy::new()
            .with_artifact(CaptureKind::Framebuffer)
            .with_artifact(CaptureKind::Snapshot)
    );
    assert!(matches!(
        sprite_priority.pass_condition,
        PassCondition::FramebufferFixture(_)
    ));
    assert_eq!(sprite_priority.execution_mode, ExecutionMode::Strict);

    assert!(cases.iter().all(|case| {
        if case.id == "mooneye-manual-only-sprite-priority" {
            matches!(case.pass_condition, PassCondition::FramebufferFixture(_))
        } else {
            case.capture_plan
                == CapturePlan::new()
                    .with_capture(CaptureKind::Snapshot)
                    .with_capture(CaptureKind::Serial)
                && case.failure_artifacts
                    == FailureArtifactPolicy::new()
                        .with_artifact(CaptureKind::Snapshot)
                        .with_artifact(CaptureKind::Serial)
                && matches!(case.pass_condition, PassCondition::MooneyeResult)
        }
    }));
}

#[test]
fn phase_6_cartridge_oracle_suite_tracks_reserved_mapper_fixtures() {
    let suite = phase_6_cartridge_oracle_suite();

    assert_eq!(suite.validate(), Ok(()));
    assert_eq!(suite.cases.len(), 5);
    assert!(suite.cases.iter().all(|case| {
        case.capture_plan.contains(CaptureKind::SerialHex)
            && case.capture_plan.contains(CaptureKind::Snapshot)
            && case.failure_artifacts.contains(CaptureKind::SerialHex)
            && matches!(case.pass_condition, PassCondition::SerialHexExact(_))
    }));

    let mbc3 = suite
        .cases
        .iter()
        .find(|case| case.id == "phase6-mbc3-banking-ram-and-rtc")
        .expect("phase 6 suite should include the MBC3 RTC case");
    assert_eq!(
        mbc3.rom_path,
        PathBuf::from(
            "crates/gb-core/tests/fixtures/roms/phase6/phase6_mbc3_banking_ram_and_rtc.gb"
        )
    );
    assert_eq!(mbc3.startup_cartridge_rtc_seconds, Some(93_784));
}

#[test]
fn phase_6_mbc6_oracle_suite_tracks_the_dedicated_flash_fixture() {
    let suite = phase_6_mbc6_oracle_suite();

    assert_eq!(suite.name, "phase-6-mbc6-oracle");
    assert_eq!(suite.validate(), Ok(()));
    assert_eq!(suite.cases.len(), 1);

    let case = &suite.cases[0];
    assert_eq!(case.id, "phase6-mbc6-split-window-flash");
    assert_eq!(case.console_model, ConsoleModel::GameBoyColor);
    assert_eq!(
        case.rom_path,
        PathBuf::from(
            "crates/gb-core/tests/fixtures/roms/phase6/phase6_mbc6_split_window_flash.gb"
        )
    );
    assert_eq!(
        case.pass_condition,
        PassCondition::SerialHexExact("4D363A020304050011223344C281805A803C".to_string())
    );
    assert!(case.capture_plan.contains(CaptureKind::SerialHex));
    assert!(case.capture_plan.contains(CaptureKind::Snapshot));
    assert!(case.failure_artifacts.contains(CaptureKind::SerialHex));
}

#[test]
fn phase_6_mbc6_oracle_suite_stays_out_of_manual_rom_runner_catalog() {
    assert!(
        built_in_rom_suite_by_name("phase-6-mbc6-oracle").is_none(),
        "MBC6 synthetic oracle should remain cargo-test-only instead of a manual run_rom_suite target"
    );
}

fn trace_fixture_path(case: &RomTestCase) -> &Path {
    match &case.pass_condition {
        PassCondition::TraceFixture(path) => path.as_path(),
        _ => panic!("expected trace fixture pass condition"),
    }
}
