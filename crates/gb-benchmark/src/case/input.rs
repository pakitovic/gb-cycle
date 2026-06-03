use gb_core::{DMG_T_CYCLES_PER_FRAME, JoypadButton};
use std::path::Path;

use crate::{BenchmarkStimulus, BenchmarkStimulusTime, target_frames_for_duration};

use super::parser::BenchmarkInputFile;
use super::{BenchmarkConfigError, DEFAULT_INPUT_HOLD_FRAMES};

pub(super) fn resolve_duration(
    path: &Path,
    id: &str,
    run_id: Option<&str>,
    duration_seconds: Option<u32>,
) -> Result<u32, BenchmarkConfigError> {
    match duration_seconds {
        Some(duration_seconds) if duration_seconds > 0 => Ok(duration_seconds),
        _ => Err(BenchmarkConfigError::ZeroDuration {
            path: path.to_path_buf(),
            id: id.to_string(),
            run_id: run_id.map(ToString::to_string),
        }),
    }
}

pub(super) fn expand_input(
    path: &Path,
    id: &str,
    run_id: &str,
    index: usize,
    duration_seconds: u32,
    input: BenchmarkInputFile,
    stimuli: &mut Vec<BenchmarkStimulus>,
) -> Result<(), BenchmarkConfigError> {
    let buttons = parse_input_buttons(path, id, run_id, index, input.button, input.buttons)?;
    let time = match (input.frame, input.second, input.tcycle) {
        (Some(frame), None, None) => BenchmarkStimulusTime::Frame(frame),
        (None, Some(second), None) => {
            BenchmarkStimulusTime::Frame(target_frames_for_duration(second))
        }
        (None, None, Some(t_cycle)) => BenchmarkStimulusTime::TCycle(t_cycle),
        _ => {
            return Err(invalid_input(
                path,
                id,
                run_id,
                index,
                "define exactly one of frame, second, or tcycle",
            ));
        }
    };
    let hold_frames = input.hold_frames.unwrap_or(DEFAULT_INPUT_HOLD_FRAMES);
    if hold_frames == 0 {
        return Err(invalid_input(
            path,
            id,
            run_id,
            index,
            "hold_frames must be greater than zero",
        ));
    }
    if matches!(time, BenchmarkStimulusTime::TCycle(_)) && input.repeat_every_frames.is_some() {
        return Err(invalid_input(
            path,
            id,
            run_id,
            index,
            "repeat_every_frames can only be used with frame or second timing",
        ));
    }
    if let Some(repeat_every_frames) = input.repeat_every_frames
        && (repeat_every_frames == 0 || repeat_every_frames <= hold_frames)
    {
        return Err(invalid_input(
            path,
            id,
            run_id,
            index,
            "repeat_every_frames must be greater than hold_frames",
        ));
    }

    match time {
        BenchmarkStimulusTime::Frame(frame) => expand_frame_input(
            frame,
            hold_frames,
            input.repeat_every_frames,
            duration_seconds,
            &buttons,
            stimuli,
        ),
        BenchmarkStimulusTime::TCycle(t_cycle) => {
            let release_t_cycle =
                t_cycle.saturating_add(u64::from(hold_frames) * DMG_T_CYCLES_PER_FRAME);
            push_button_pulse(
                stimuli,
                BenchmarkStimulusTime::TCycle(t_cycle),
                BenchmarkStimulusTime::TCycle(release_t_cycle),
                &buttons,
            );
        }
    }

    Ok(())
}

fn parse_input_buttons(
    path: &Path,
    id: &str,
    run_id: &str,
    index: usize,
    button: Option<String>,
    buttons: Option<Vec<String>>,
) -> Result<Vec<JoypadButton>, BenchmarkConfigError> {
    let button_names = match (button, buttons) {
        (Some(button), None) => vec![button],
        (None, Some(buttons)) if !buttons.is_empty() => buttons,
        _ => {
            return Err(invalid_input(
                path,
                id,
                run_id,
                index,
                "define exactly one of button or a non-empty buttons array",
            ));
        }
    };

    button_names
        .iter()
        .map(|button| {
            parse_joypad_button(button).ok_or_else(|| {
                invalid_input(path, id, run_id, index, "uses an unsupported joypad button")
            })
        })
        .collect()
}

fn expand_frame_input(
    start_frame: u32,
    hold_frames: u32,
    repeat_every_frames: Option<u32>,
    duration_seconds: u32,
    buttons: &[JoypadButton],
    stimuli: &mut Vec<BenchmarkStimulus>,
) {
    let target_frames = target_frames_for_duration(duration_seconds);
    let mut frame = start_frame;
    loop {
        if frame >= target_frames {
            break;
        }
        let release_frame = frame.saturating_add(hold_frames);
        push_button_pulse(
            stimuli,
            BenchmarkStimulusTime::Frame(frame),
            BenchmarkStimulusTime::Frame(release_frame),
            buttons,
        );
        let Some(repeat_every_frames) = repeat_every_frames else {
            break;
        };
        let Some(next_frame) = frame.checked_add(repeat_every_frames) else {
            break;
        };
        frame = next_frame;
    }
}

fn push_button_pulse(
    stimuli: &mut Vec<BenchmarkStimulus>,
    press_time: BenchmarkStimulusTime,
    release_time: BenchmarkStimulusTime,
    buttons: &[JoypadButton],
) {
    for button in buttons {
        stimuli.push(BenchmarkStimulus {
            when: press_time,
            button: *button,
            pressed: true,
        });
        stimuli.push(BenchmarkStimulus {
            when: release_time,
            button: *button,
            pressed: false,
        });
    }
}

fn invalid_input(
    path: &Path,
    id: &str,
    run_id: &str,
    index: usize,
    reason: &'static str,
) -> BenchmarkConfigError {
    BenchmarkConfigError::InvalidInput {
        path: path.to_path_buf(),
        id: id.to_string(),
        run_id: run_id.to_string(),
        index,
        reason,
    }
}

fn parse_joypad_button(button: &str) -> Option<JoypadButton> {
    match button.trim().to_ascii_lowercase().as_str() {
        "right" => Some(JoypadButton::Right),
        "left" => Some(JoypadButton::Left),
        "up" => Some(JoypadButton::Up),
        "down" => Some(JoypadButton::Down),
        "a" => Some(JoypadButton::A),
        "b" => Some(JoypadButton::B),
        "select" => Some(JoypadButton::Select),
        "start" => Some(JoypadButton::Start),
        _ => None,
    }
}
