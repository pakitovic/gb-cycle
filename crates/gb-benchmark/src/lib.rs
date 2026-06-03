mod artifact;
pub mod bench;
mod case;
mod model;
mod stats;
mod stimulus;
mod timing;

pub use artifact::{
    GB_CLI_FRONTEND, GB_DESKTOP_FRONTEND, frontend_screenshot_path, frontend_stats_path,
};
pub use case::{
    BENCHMARK_CASE_VERSION, BenchmarkCase, BenchmarkConfigError, BenchmarkSuite,
    DEFAULT_INPUT_HOLD_FRAMES, load_benchmark_case, load_benchmark_cases, load_benchmark_suite,
    parse_benchmark_case, parse_benchmark_cases, parse_benchmark_suite,
};
pub use model::{BenchmarkMode, BenchmarkModel, BenchmarkPalette, BenchmarkStartup};
pub use stats::{BenchmarkStats, encode_stats_toml};
pub use stimulus::{BenchmarkStimulus, BenchmarkStimulusRuntime, BenchmarkStimulusTime};
pub use timing::{target_frame_rate_hz, target_frames_for_duration, target_tcycles_for_duration};
