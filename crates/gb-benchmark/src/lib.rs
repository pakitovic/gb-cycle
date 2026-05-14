use gb_core::{DMG_T_CYCLES_PER_FRAME, DMG_T_CYCLES_PER_SECOND, JoypadButton};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

pub const BENCHMARK_CASE_VERSION: u32 = 1;
pub const DEFAULT_INPUT_HOLD_FRAMES: u32 = 8;
pub const GB_CLI_FRONTEND: &str = "gb-cli";
pub const GB_DESKTOP_FRONTEND: &str = "gb-desktop";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchmarkSuite {
    pub source_path: PathBuf,
    pub id: String,
    pub rom: PathBuf,
    pub model: BenchmarkModel,
    pub startup: BenchmarkStartup,
    pub mode: BenchmarkMode,
    pub palette: Option<BenchmarkPalette>,
    pub cases: Vec<BenchmarkCase>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchmarkCase {
    pub source_path: PathBuf,
    pub id: String,
    pub run_id: Option<String>,
    pub run_label: Option<String>,
    pub artifact_id: String,
    pub rom: PathBuf,
    pub model: BenchmarkModel,
    pub startup: BenchmarkStartup,
    pub mode: BenchmarkMode,
    pub palette: Option<BenchmarkPalette>,
    pub duration_seconds: u32,
    pub screenshot: bool,
    pub stats: bool,
    pub stimuli: Vec<BenchmarkStimulus>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BenchmarkModel {
    #[serde(rename = "DMG", alias = "dmg")]
    Dmg,
    #[serde(rename = "MGB", alias = "mgb")]
    Mgb,
    #[serde(rename = "LGB", alias = "lgb")]
    Lgb,
    #[serde(rename = "CGB", alias = "cgb")]
    Cgb,
}

impl BenchmarkModel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dmg => "DMG",
            Self::Mgb => "MGB",
            Self::Lgb => "LGB",
            Self::Cgb => "CGB",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BenchmarkStartup {
    #[serde(rename = "skip-boot")]
    SkipBoot,
    #[serde(rename = "custom-boot")]
    CustomBoot,
    #[serde(rename = "real-boot")]
    RealBoot,
}

impl BenchmarkStartup {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SkipBoot => "skip-boot",
            Self::CustomBoot => "custom-boot",
            Self::RealBoot => "real-boot",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BenchmarkMode {
    #[serde(rename = "strict")]
    Strict,
    #[serde(rename = "permissive")]
    Permissive,
    #[serde(rename = "experimental")]
    Experimental,
}

impl BenchmarkMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Strict => "strict",
            Self::Permissive => "permissive",
            Self::Experimental => "experimental",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BenchmarkPalette {
    #[serde(rename = "grey")]
    Grey,
}

impl BenchmarkPalette {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Grey => "grey",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BenchmarkStimulusTime {
    TCycle(u64),
    Frame(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BenchmarkStimulus {
    pub when: BenchmarkStimulusTime,
    pub button: JoypadButton,
    pub pressed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchmarkStimulusRuntime {
    stimuli: Vec<BenchmarkStimulus>,
    applied: Vec<bool>,
}

impl BenchmarkStimulusRuntime {
    pub fn new(stimuli: Vec<BenchmarkStimulus>) -> Self {
        let applied = vec![false; stimuli.len()];
        Self { stimuli, applied }
    }

    pub fn apply_due<F>(&mut self, t_cycle: u64, completed_frames: u64, mut apply: F)
    where
        F: FnMut(JoypadButton, bool),
    {
        for (index, stimulus) in self.stimuli.iter().copied().enumerate() {
            if self.applied[index] {
                continue;
            }
            let due = match stimulus.when {
                BenchmarkStimulusTime::TCycle(stimulus_t_cycle) => stimulus_t_cycle == t_cycle,
                BenchmarkStimulusTime::Frame(frame) => u64::from(frame) == completed_frames,
            };
            if due {
                apply(stimulus.button, stimulus.pressed);
                self.applied[index] = true;
            }
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct BenchmarkStats {
    pub version: u32,
    pub frontend: String,
    pub id: String,
    pub artifact_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_label: Option<String>,
    pub rom: String,
    pub model: String,
    pub startup: String,
    pub mode: String,
    pub test_runner: bool,
    pub duration_seconds: u32,
    pub target_frames: u32,
    pub completed_frames: u64,
    pub elapsed_seconds: f64,
    pub fps: f64,
    pub speed_percent: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executed_tcycles: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot: Option<String>,
}

impl BenchmarkStats {
    pub fn new(
        frontend: &str,
        case: &BenchmarkCase,
        test_runner: bool,
        completed_frames: u64,
        elapsed_seconds: f64,
        executed_tcycles: Option<u64>,
        screenshot: Option<&Path>,
    ) -> Self {
        let elapsed_seconds = elapsed_seconds.max(f64::EPSILON);
        let fps = completed_frames as f64 / elapsed_seconds;
        Self {
            version: 1,
            frontend: frontend.to_string(),
            id: case.id.clone(),
            artifact_id: case.artifact_id.clone(),
            run_id: case.run_id.clone(),
            run_label: case.run_label.clone(),
            rom: case.rom.display().to_string(),
            model: case.model.as_str().to_string(),
            startup: case.startup.as_str().to_string(),
            mode: case.mode.as_str().to_string(),
            test_runner,
            duration_seconds: case.duration_seconds,
            target_frames: target_frames_for_duration(case.duration_seconds),
            completed_frames,
            elapsed_seconds,
            fps,
            speed_percent: fps / target_frame_rate_hz() * 100.0,
            executed_tcycles,
            screenshot: screenshot.map(|path| path.display().to_string()),
        }
    }
}

#[derive(Debug)]
pub enum BenchmarkConfigError {
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    UnsupportedVersion {
        path: PathBuf,
        version: u32,
    },
    EmptyId {
        path: PathBuf,
    },
    EmptyRunId {
        path: PathBuf,
        id: String,
        index: usize,
    },
    ZeroDuration {
        path: PathBuf,
        id: String,
        run_id: Option<String>,
    },
    MultipleRuns {
        path: PathBuf,
        id: String,
        count: usize,
    },
    InvalidStimulusTime {
        path: PathBuf,
        id: String,
        index: usize,
    },
    InvalidJoypadButton {
        path: PathBuf,
        id: String,
        index: usize,
        button: String,
    },
    InvalidInput {
        path: PathBuf,
        id: String,
        run_id: String,
        index: usize,
        reason: &'static str,
    },
}

impl fmt::Display for BenchmarkConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(
                    f,
                    "failed to read benchmark case {}: {source}",
                    path.display()
                )
            }
            Self::Parse { path, source } => {
                write!(
                    f,
                    "failed to parse benchmark case {}: {source}",
                    path.display()
                )
            }
            Self::UnsupportedVersion { path, version } => write!(
                f,
                "unsupported benchmark case version {version} in {}; expected {BENCHMARK_CASE_VERSION}",
                path.display()
            ),
            Self::EmptyId { path } => {
                write!(
                    f,
                    "benchmark case {} must define a non-empty id",
                    path.display()
                )
            }
            Self::EmptyRunId { path, id, index } => write!(
                f,
                "benchmark case {id:?} in {} run #{index} must define a non-empty id",
                path.display()
            ),
            Self::ZeroDuration { path, id, run_id } => {
                if let Some(run_id) = run_id {
                    write!(
                        f,
                        "benchmark case {id:?} run {run_id:?} in {} must use duration_seconds > 0",
                        path.display()
                    )
                } else {
                    write!(
                        f,
                        "benchmark case {id:?} in {} must use duration_seconds > 0",
                        path.display()
                    )
                }
            }
            Self::MultipleRuns { path, id, count } => write!(
                f,
                "benchmark case {id:?} in {} expands to {count} runs; use load_benchmark_cases for multi-run suites",
                path.display()
            ),
            Self::InvalidStimulusTime { path, id, index } => write!(
                f,
                "benchmark case {id:?} in {} stimulus #{index} must define exactly one of frame or tcycle",
                path.display()
            ),
            Self::InvalidJoypadButton {
                path,
                id,
                index,
                button,
            } => write!(
                f,
                "benchmark case {id:?} in {} stimulus #{index} uses unsupported joypad button {button:?}",
                path.display()
            ),
            Self::InvalidInput {
                path,
                id,
                run_id,
                index,
                reason,
            } => write!(
                f,
                "benchmark case {id:?} run {run_id:?} in {} input #{index} is invalid: {reason}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for BenchmarkConfigError {}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct BenchmarkCaseFile {
    version: u32,
    id: String,
    rom: PathBuf,
    model: BenchmarkModel,
    startup: BenchmarkStartup,
    mode: BenchmarkMode,
    palette: Option<BenchmarkPalette>,
    duration_seconds: Option<u32>,
    #[serde(default = "default_true")]
    screenshot: bool,
    #[serde(default = "default_true")]
    stats: bool,
    #[serde(rename = "stimulus", default)]
    stimuli: Vec<BenchmarkStimulusFile>,
    #[serde(rename = "run", default)]
    runs: Vec<BenchmarkRunFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct BenchmarkRunFile {
    id: Option<String>,
    label: Option<String>,
    duration_seconds: Option<u32>,
    screenshot: Option<bool>,
    stats: Option<bool>,
    #[serde(rename = "stimulus", default)]
    stimuli: Vec<BenchmarkStimulusFile>,
    #[serde(rename = "input", default)]
    inputs: Vec<BenchmarkInputFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct BenchmarkStimulusFile {
    frame: Option<u32>,
    tcycle: Option<u64>,
    button: String,
    pressed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct BenchmarkInputFile {
    frame: Option<u32>,
    second: Option<u32>,
    tcycle: Option<u64>,
    button: Option<String>,
    buttons: Option<Vec<String>>,
    hold_frames: Option<u32>,
    repeat_every_frames: Option<u32>,
}

const fn default_true() -> bool {
    true
}

pub fn load_benchmark_suite(
    path: impl AsRef<Path>,
) -> Result<BenchmarkSuite, BenchmarkConfigError> {
    let path = path.as_ref();
    let text = fs::read_to_string(path).map_err(|source| BenchmarkConfigError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    parse_benchmark_suite(path, &text)
}

pub fn load_benchmark_cases(
    path: impl AsRef<Path>,
) -> Result<Vec<BenchmarkCase>, BenchmarkConfigError> {
    load_benchmark_suite(path).map(|suite| suite.cases)
}

pub fn load_benchmark_case(path: impl AsRef<Path>) -> Result<BenchmarkCase, BenchmarkConfigError> {
    let suite = load_benchmark_suite(path)?;
    single_case_from_suite(suite)
}

pub fn parse_benchmark_suite(
    path: impl AsRef<Path>,
    text: &str,
) -> Result<BenchmarkSuite, BenchmarkConfigError> {
    let path = path.as_ref();
    let parsed = toml::from_str::<BenchmarkCaseFile>(text).map_err(|source| {
        BenchmarkConfigError::Parse {
            path: path.to_path_buf(),
            source,
        }
    })?;
    if parsed.version != BENCHMARK_CASE_VERSION {
        return Err(BenchmarkConfigError::UnsupportedVersion {
            path: path.to_path_buf(),
            version: parsed.version,
        });
    }
    let id = parsed.id.trim().to_string();
    if id.is_empty() {
        return Err(BenchmarkConfigError::EmptyId {
            path: path.to_path_buf(),
        });
    }

    let cases = if parsed.runs.is_empty() {
        vec![build_legacy_case(path, &id, &parsed)?]
    } else {
        parsed
            .runs
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, run)| build_run_case(path, &id, &parsed, index, run))
            .collect::<Result<Vec<_>, _>>()?
    };

    Ok(BenchmarkSuite {
        source_path: path.to_path_buf(),
        id,
        rom: parsed.rom,
        model: parsed.model,
        startup: parsed.startup,
        mode: parsed.mode,
        palette: parsed.palette,
        cases,
    })
}

pub fn parse_benchmark_cases(
    path: impl AsRef<Path>,
    text: &str,
) -> Result<Vec<BenchmarkCase>, BenchmarkConfigError> {
    parse_benchmark_suite(path, text).map(|suite| suite.cases)
}

pub fn parse_benchmark_case(
    path: impl AsRef<Path>,
    text: &str,
) -> Result<BenchmarkCase, BenchmarkConfigError> {
    let suite = parse_benchmark_suite(path, text)?;
    single_case_from_suite(suite)
}

fn single_case_from_suite(suite: BenchmarkSuite) -> Result<BenchmarkCase, BenchmarkConfigError> {
    let mut cases = suite.cases;
    if cases.len() == 1 {
        Ok(cases.remove(0))
    } else {
        Err(BenchmarkConfigError::MultipleRuns {
            path: suite.source_path,
            id: suite.id,
            count: cases.len(),
        })
    }
}

fn build_legacy_case(
    path: &Path,
    id: &str,
    parsed: &BenchmarkCaseFile,
) -> Result<BenchmarkCase, BenchmarkConfigError> {
    let duration_seconds = resolve_duration(path, id, None, parsed.duration_seconds)?;
    let stimuli = parsed
        .stimuli
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, stimulus)| parse_stimulus(path, id, index, stimulus))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(BenchmarkCase {
        source_path: path.to_path_buf(),
        id: id.to_string(),
        run_id: None,
        run_label: None,
        artifact_id: id.to_string(),
        rom: parsed.rom.clone(),
        model: parsed.model,
        startup: parsed.startup,
        mode: parsed.mode,
        palette: parsed.palette,
        duration_seconds,
        screenshot: parsed.screenshot,
        stats: parsed.stats,
        stimuli,
    })
}

fn build_run_case(
    path: &Path,
    id: &str,
    parsed: &BenchmarkCaseFile,
    run_index: usize,
    run: BenchmarkRunFile,
) -> Result<BenchmarkCase, BenchmarkConfigError> {
    let run_id = run
        .id
        .as_deref()
        .map(str::trim)
        .unwrap_or_default()
        .to_string();
    if run_id.is_empty() {
        return Err(BenchmarkConfigError::EmptyRunId {
            path: path.to_path_buf(),
            id: id.to_string(),
            index: run_index,
        });
    }
    let duration_seconds = resolve_duration(
        path,
        id,
        Some(&run_id),
        run.duration_seconds.or(parsed.duration_seconds),
    )?;
    let mut stimuli = run
        .stimuli
        .into_iter()
        .enumerate()
        .map(|(index, stimulus)| parse_stimulus(path, id, index, stimulus))
        .collect::<Result<Vec<_>, _>>()?;
    for (index, input) in run.inputs.into_iter().enumerate() {
        expand_input(
            path,
            id,
            &run_id,
            index,
            duration_seconds,
            input,
            &mut stimuli,
        )?;
    }

    Ok(BenchmarkCase {
        source_path: path.to_path_buf(),
        id: id.to_string(),
        run_id: Some(run_id.clone()),
        run_label: run.label,
        artifact_id: format!("{id}-{run_id}"),
        rom: parsed.rom.clone(),
        model: parsed.model,
        startup: parsed.startup,
        mode: parsed.mode,
        palette: parsed.palette,
        duration_seconds,
        screenshot: run.screenshot.unwrap_or(parsed.screenshot),
        stats: run.stats.unwrap_or(parsed.stats),
        stimuli,
    })
}

fn resolve_duration(
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

fn parse_stimulus(
    path: &Path,
    id: &str,
    index: usize,
    stimulus: BenchmarkStimulusFile,
) -> Result<BenchmarkStimulus, BenchmarkConfigError> {
    let when = match (stimulus.frame, stimulus.tcycle) {
        (Some(frame), None) => BenchmarkStimulusTime::Frame(frame),
        (None, Some(t_cycle)) => BenchmarkStimulusTime::TCycle(t_cycle),
        _ => {
            return Err(BenchmarkConfigError::InvalidStimulusTime {
                path: path.to_path_buf(),
                id: id.to_string(),
                index,
            });
        }
    };
    let button = parse_joypad_button(&stimulus.button).ok_or_else(|| {
        BenchmarkConfigError::InvalidJoypadButton {
            path: path.to_path_buf(),
            id: id.to_string(),
            index,
            button: stimulus.button.clone(),
        }
    })?;
    Ok(BenchmarkStimulus {
        when,
        button,
        pressed: stimulus.pressed,
    })
}

fn expand_input(
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

pub fn target_frame_rate_hz() -> f64 {
    DMG_T_CYCLES_PER_SECOND as f64 / DMG_T_CYCLES_PER_FRAME as f64
}

pub fn target_frames_for_duration(duration_seconds: u32) -> u32 {
    (f64::from(duration_seconds) * target_frame_rate_hz()).ceil() as u32
}

pub fn frontend_stats_path(frontend: &str, artifact_id: &str) -> PathBuf {
    PathBuf::from(frontend).join(format!("{artifact_id}-stats.toml"))
}

pub fn frontend_screenshot_path(frontend: &str, artifact_id: &str) -> PathBuf {
    PathBuf::from(frontend).join(format!("{artifact_id}.png"))
}

pub fn encode_stats_toml(stats: &BenchmarkStats) -> Result<String, toml::ser::Error> {
    toml::to_string_pretty(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_case_supports_frame_and_tcycle_stimuli() {
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
duration_seconds = 8

[[stimulus]]
frame = 30
button = "start"
pressed = true

[[stimulus]]
tcycle = 70224
button = "a"
pressed = false
"#,
        )
        .expect("benchmark case should parse");

        assert_eq!(case.id, "dr-mario");
        assert_eq!(case.artifact_id, "dr-mario");
        assert_eq!(case.run_id, None);
        assert_eq!(case.model, BenchmarkModel::Dmg);
        assert_eq!(case.stimuli.len(), 2);
        assert_eq!(case.stimuli[0].when, BenchmarkStimulusTime::Frame(30));
        assert_eq!(case.stimuli[0].button, JoypadButton::Start);
        assert_eq!(case.stimuli[1].when, BenchmarkStimulusTime::TCycle(70224));
        assert_eq!(case.stimuli[1].button, JoypadButton::A);
    }

    #[test]
    fn parse_suite_expands_multiple_fresh_runs() {
        let suite = parse_benchmark_suite(
            "case.toml",
            r#"
version = 1
id = "alone-in-the-dark"
rom = "alone.gbc"
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
        assert_eq!(suite.cases[0].id, "alone-in-the-dark");
        assert_eq!(suite.cases[0].artifact_id, "alone-in-the-dark-idle-40");
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
    fn parse_case_rejects_ambiguous_stimulus_timing() {
        let error = parse_benchmark_case(
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
tcycle = 2
button = "a"
pressed = true
"#,
        )
        .expect_err("ambiguous stimulus should fail");

        assert!(matches!(
            error,
            BenchmarkConfigError::InvalidStimulusTime { .. }
        ));
    }

    #[test]
    fn parse_suite_rejects_invalid_run_duration_and_inputs() {
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

    #[test]
    fn stimulus_runtime_applies_each_stimulus_once() {
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
}
