use super::*;
use gb_core::JoypadButton;

#[test]
fn runtime_applies_each_stimulus_once() {
    let mut runtime = BenchmarkStimulusRuntime::new(vec![BenchmarkStimulus {
        when: BenchmarkStimulusTime::Frame(2),
        button: JoypadButton::A,
        pressed: true,
    }]);
    let mut applied = Vec::new();
    runtime.apply_due(0, 1, |button, pressed| applied.push((button, pressed)));
    runtime.apply_due(0, 2, |button, pressed| applied.push((button, pressed)));
    runtime.apply_due(1, 2, |button, pressed| applied.push((button, pressed)));

    assert_eq!(applied, vec![(JoypadButton::A, true)]);
}

#[test]
fn runtime_applies_due_events_in_source_order() {
    let mut runtime = BenchmarkStimulusRuntime::new(vec![
        BenchmarkStimulus {
            when: BenchmarkStimulusTime::Frame(2),
            button: JoypadButton::Start,
            pressed: true,
        },
        BenchmarkStimulus {
            when: BenchmarkStimulusTime::TCycle(8),
            button: JoypadButton::A,
            pressed: true,
        },
        BenchmarkStimulus {
            when: BenchmarkStimulusTime::Frame(2),
            button: JoypadButton::Start,
            pressed: false,
        },
        BenchmarkStimulus {
            when: BenchmarkStimulusTime::TCycle(8),
            button: JoypadButton::A,
            pressed: false,
        },
    ]);
    let mut applied = Vec::new();
    runtime.apply_due(7, 1, |button, pressed| applied.push((button, pressed)));
    runtime.apply_due(8, 2, |button, pressed| applied.push((button, pressed)));
    runtime.apply_due(9, 2, |button, pressed| applied.push((button, pressed)));

    assert_eq!(
        applied,
        vec![
            (JoypadButton::Start, true),
            (JoypadButton::A, true),
            (JoypadButton::Start, false),
            (JoypadButton::A, false),
        ]
    );
}
