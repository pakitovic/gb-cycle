mod common;

use gb_core::{ConsoleModel, Machine, MachineConfig, SchedulerPhase, StartupMode, TCycle};

const FIXTURE_ACCEPT_ENV: &str = "GB_CYCLE_ACCEPT_MACHINE_FIXTURES";

#[test]
fn machine_uses_a_single_step_t_cycle_entry_point() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    let context = machine.step_t_cycle();

    assert_eq!(context.t_cycle(), TCycle::new(0));
    assert_eq!(context.phase(), SchedulerPhase::CpuWakeInterruptEvaluation);
    assert_eq!(machine.next_t_cycle(), TCycle::new(1));
}

#[test]
fn machine_trace_includes_phase_aligned_subsystem_hooks() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.step_t_cycle();

    let fixture_path = common::trace_fixtures_dir().join("machine_single_cycle_trace.txt");
    let expected = common::ensure_text_fixture(
        &fixture_path,
        &machine.tracer().sink().render_text(),
        FIXTURE_ACCEPT_ENV,
    );

    assert_eq!(machine.tracer().sink().render_text(), expected);
}

#[test]
fn two_identical_machines_produce_the_same_two_cycle_trace() {
    let config = MachineConfig::new(ConsoleModel::Dmg);
    let mut left = Machine::new(config.clone());
    let mut right = Machine::new(config);

    left.step_t_cycle();
    left.step_t_cycle();
    right.step_t_cycle();
    right.step_t_cycle();

    let fixture_path = common::trace_fixtures_dir().join("machine_two_cycle_trace.txt");
    let expected = common::ensure_text_fixture(
        &fixture_path,
        &left.tracer().sink().render_text(),
        FIXTURE_ACCEPT_ENV,
    );

    assert_eq!(left.tracer().sink().render_text(), expected);
    assert_eq!(right.tracer().sink().render_text(), expected);
    assert_eq!(left.next_t_cycle(), TCycle::new(2));
    assert_eq!(right.next_t_cycle(), TCycle::new(2));
}
