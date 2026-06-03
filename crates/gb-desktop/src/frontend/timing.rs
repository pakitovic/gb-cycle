#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum AudioQueuePacingCorrectionPolicy {
    #[default]
    Enabled,
    Disabled,
}

impl AudioQueuePacingCorrectionPolicy {
    fn from_env() -> Self {
        Self::from_env_value(
            env::var_os(DESKTOP_AUDIO_DISABLE_PACING_CORRECTION_ENV_VAR).as_deref(),
        )
    }

    fn from_env_value(value: Option<&OsStr>) -> Self {
        let Some(value) = value else {
            return Self::Enabled;
        };

        let value = value.to_string_lossy();
        if value.is_empty()
            || value == "1"
            || value.eq_ignore_ascii_case("true")
            || value.eq_ignore_ascii_case("on")
            || value.eq_ignore_ascii_case("yes")
            || value.eq_ignore_ascii_case("disable")
            || value.eq_ignore_ascii_case("disabled")
        {
            Self::Disabled
        } else {
            Self::Enabled
        }
    }

    fn correction_enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

struct FramePacer {
    next_frame_start: Instant,
    frame_duration: Duration,
    audio_queue_pacing_correction_enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct FramePacingSample {
    pacing_duration: Duration,
    sleep_target_duration: Duration,
    audio_correction_duration: Duration,
    late_duration: Duration,
    oversleep_duration: Duration,
}

const HOST_RTC_NANOS_PER_SECOND: u128 = 1_000_000_000;
const MBC3_RTC_CLOCK_TICKS_PER_SECOND: u128 = 32_768;
const MBC3_RTC_CLOCK_HALF_NORMAL_T_CYCLES: u16 = 256;

#[derive(Debug, Clone)]
struct HostRtcSync {
    // Cartridge RTCs track real wall time, including host suspend intervals.
    // Keep this separate from Instant-based frame pacing/profiling clocks.
    last_host_wall_time: SystemTime,
    // HuC-3 persists a seconds/minutes RTC shape, so desktop feeds it only
    // after host elapsed time crosses a whole-second boundary.
    huc3_second_nanos_remainder: u128,
    // MBC3 exposes subsecond writes and halt/resume phase, so desktop converts
    // host elapsed nanoseconds into the cartridge's 32.768 kHz clock domain.
    mbc3_clock_tick_nanos_remainder: u128,
    // Host elapsed time grants MBC3 RTC ticks, but active emulation releases
    // them on the cartridge clock cadence instead of batching them at host
    // event/frame boundaries where CPU-visible RTC writes can observe them.
    pending_mbc3_clock_ticks: u64,
    mbc3_half_normal_t_cycle_remainder: u16,
}

impl HostRtcSync {
    fn new(last_host_wall_time: SystemTime) -> Self {
        Self {
            last_host_wall_time,
            huc3_second_nanos_remainder: 0,
            mbc3_clock_tick_nanos_remainder: 0,
            pending_mbc3_clock_ticks: 0,
            mbc3_half_normal_t_cycle_remainder: 0,
        }
    }

    fn from_host_clock() -> Self {
        Self::new(SystemTime::now())
    }

    fn resync_to_host_clock(&mut self) {
        *self = Self::from_host_clock();
    }

    fn apply_to_machine(&mut self, machine: &mut Machine<TraceSummaryBuffer>) {
        self.sync_host_elapsed_to_machine(machine);
        self.flush_pending_mbc3_clock_ticks(machine);
    }

    fn sync_host_elapsed_to_machine(&mut self, machine: &mut Machine<TraceSummaryBuffer>) {
        self.sync_host_elapsed_to_machine_at(machine, SystemTime::now());
    }

    fn sync_host_elapsed_to_machine_at(
        &mut self,
        machine: &mut Machine<TraceSummaryBuffer>,
        now: SystemTime,
    ) {
        let elapsed = now
            .duration_since(self.last_host_wall_time)
            .unwrap_or(Duration::ZERO);
        self.apply_elapsed_to_machine(machine, elapsed);
        self.last_host_wall_time = now;
    }

    fn apply_elapsed_to_machine(
        &mut self,
        machine: &mut Machine<TraceSummaryBuffer>,
        elapsed: Duration,
    ) {
        if elapsed.is_zero() {
            return;
        }

        let elapsed_nanos = elapsed.as_nanos();
        let huc3_total_nanos = self
            .huc3_second_nanos_remainder
            .saturating_add(elapsed_nanos);
        let huc3_seconds = huc3_total_nanos / HOST_RTC_NANOS_PER_SECOND;
        self.huc3_second_nanos_remainder = huc3_total_nanos % HOST_RTC_NANOS_PER_SECOND;
        if huc3_seconds != 0 {
            machine.advance_huc3_cartridge_rtc_seconds(u128_to_u64_saturating(huc3_seconds));
        }

        let mbc3_total_clock_ticks = self
            .mbc3_clock_tick_nanos_remainder
            .saturating_add(elapsed_nanos.saturating_mul(MBC3_RTC_CLOCK_TICKS_PER_SECOND));
        let mbc3_clock_ticks = mbc3_total_clock_ticks / HOST_RTC_NANOS_PER_SECOND;
        self.mbc3_clock_tick_nanos_remainder = mbc3_total_clock_ticks % HOST_RTC_NANOS_PER_SECOND;
        if mbc3_clock_ticks != 0 {
            self.pending_mbc3_clock_ticks = self
                .pending_mbc3_clock_ticks
                .saturating_add(u128_to_u64_saturating(mbc3_clock_ticks));
        }
    }

    fn tick_mbc3_for_emulated_t_cycle(&mut self, machine: &mut Machine<TraceSummaryBuffer>) {
        self.mbc3_half_normal_t_cycle_remainder += match machine.speed().current_speed() {
            CgbSpeedMode::Normal => 2,
            CgbSpeedMode::Double => 1,
        };

        if self.mbc3_half_normal_t_cycle_remainder >= MBC3_RTC_CLOCK_HALF_NORMAL_T_CYCLES {
            self.mbc3_half_normal_t_cycle_remainder -= MBC3_RTC_CLOCK_HALF_NORMAL_T_CYCLES;
            if self.pending_mbc3_clock_ticks != 0 {
                self.pending_mbc3_clock_ticks -= 1;
                machine.advance_mbc3_cartridge_rtc_clock_ticks(1);
            }
        }
    }

    fn flush_pending_mbc3_clock_ticks(&mut self, machine: &mut Machine<TraceSummaryBuffer>) {
        if self.pending_mbc3_clock_ticks == 0 {
            return;
        }

        let ticks = self.pending_mbc3_clock_ticks;
        self.pending_mbc3_clock_ticks = 0;
        machine.advance_mbc3_cartridge_rtc_clock_ticks(ticks);
    }
}

fn u128_to_u64_saturating(value: u128) -> u64 {
    value.min(u128::from(u64::MAX)) as u64
}

impl FramePacer {
    fn new(_vsync_enabled: bool, frame_duration: Duration) -> Self {
        Self {
            next_frame_start: Instant::now(),
            frame_duration,
            audio_queue_pacing_correction_enabled: AudioQueuePacingCorrectionPolicy::from_env()
                .correction_enabled(),
        }
    }

    fn set_frame_duration(&mut self, frame_duration: Duration) {
        if self.frame_duration == frame_duration {
            return;
        }
        self.frame_duration = frame_duration;
        self.reset_host_pacing();
    }

    fn wait_until_next_frame(&mut self, audio_queue_ms: Option<f64>) -> FramePacingSample {
        let audio_correction = audio_queue_pacing_correction_with_policy(
            audio_queue_ms,
            self.audio_queue_pacing_correction_enabled,
        );
        self.next_frame_start += self.frame_duration + audio_correction;
        let now = Instant::now();
        if now < self.next_frame_start {
            let sleep_target_duration = self.next_frame_start - now;
            thread::sleep(sleep_target_duration);
            let oversleep_duration =
                Instant::now().saturating_duration_since(self.next_frame_start);
            FramePacingSample {
                pacing_duration: sleep_target_duration,
                sleep_target_duration,
                audio_correction_duration: audio_correction,
                late_duration: Duration::ZERO,
                oversleep_duration,
            }
        } else {
            let late_duration = now - self.next_frame_start;
            self.next_frame_start = now;
            FramePacingSample {
                pacing_duration: Duration::ZERO,
                sleep_target_duration: Duration::ZERO,
                audio_correction_duration: audio_correction,
                late_duration,
                oversleep_duration: Duration::ZERO,
            }
        }
    }

    fn set_vsync_enabled(&mut self, _vsync_enabled: bool) {
        self.next_frame_start = Instant::now();
    }

    fn reset_host_pacing(&mut self) {
        self.next_frame_start = Instant::now();
    }
}

fn audio_queue_pacing_correction_with_policy(
    audio_queue_ms: Option<f64>,
    correction_enabled: bool,
) -> Duration {
    if !correction_enabled {
        return Duration::ZERO;
    }

    let Some(audio_queue_ms) = audio_queue_ms else {
        return Duration::ZERO;
    };

    let excess_ms = audio_queue_ms - (AUDIO_QUEUE_TARGET_MS + AUDIO_QUEUE_DEADBAND_MS);
    if excess_ms <= 0.0 {
        return Duration::ZERO;
    }

    let correction_ms = (excess_ms * AUDIO_QUEUE_PACING_GAIN).min(AUDIO_QUEUE_MAX_CORRECTION_MS);
    Duration::from_secs_f64(correction_ms / 1_000.0)
}

fn host_audio_capture_due_for_t_cycle(
    speed_mode: CgbSpeedMode,
    scheduler_t_cycle: u64,
    cpu_execution_state: CpuExecutionState,
) -> bool {
    !matches!(
        cpu_execution_state,
        CpuExecutionState::Stopped
            | CpuExecutionState::ZombieStopped
            | CpuExecutionState::SpeedSwitchPause { .. }
    ) && speed_mode.apu_tick_due_at_scheduler_t_cycle(scheduler_t_cycle)
}

#[derive(Debug)]
struct FramePerformanceSample {
    session_kind: EmulationProfileSessionKind,
    emulation_duration: Duration,
    emulation_profile_request: Option<EmulationProfileRequest>,
    render_duration: Duration,
    present_duration: Duration,
    pacing_duration: Duration,
    pacing_sleep_target_duration: Duration,
    pacing_audio_correction_duration: Duration,
    pacing_late_duration: Duration,
    pacing_oversleep_duration: Duration,
    audio_submit_sample_count: Option<usize>,
    audio_submit_t_cycles: Option<usize>,
    audio_submit_queue_before_ms: Option<f64>,
    audio_submit_enqueued_ms: Option<f64>,
    audio_submit_queue_after_ms: Option<f64>,
    audio_queue_before_pacing_ms: Option<f64>,
    audio_queue_after_pacing_ms: Option<f64>,
    speed_mode: Option<CgbSpeedMode>,
    frame_step_t_cycles: Option<usize>,
    frame_video_dots: Option<usize>,
    frame_start_ly: Option<u8>,
    frame_start_dot: Option<u16>,
    frame_end_ly: Option<u8>,
    frame_end_dot: Option<u16>,
    frame_origin_crossings: Option<u8>,
    scanline_transitions: Option<u16>,
    scanlines_over_456: Option<u16>,
    max_scanline_t_cycles: Option<usize>,
    max_scanline_ly: Option<u8>,
    max_mode0_start_dot: Option<u16>,
    max_mode0_start_dot_ly: Option<u8>,
    ly_153_to_0_transitions: Option<u8>,
    ly_153_to_0_startup_mode0: Option<u8>,
    ly_153_to_0_blank_frame: Option<u8>,
    ly_0_self_wraps: Option<u8>,
    ly_0_self_wrap_startup_mode0: Option<u8>,
    ly_0_self_wrap_blank_frame: Option<u8>,
    ly_0_to_1_transitions: Option<u8>,
    ly_0_scanline_t_cycles: Option<usize>,
    ly_0_max_mode0_start_dot: Option<u16>,
    ly_0_stall_t_cycles: Option<usize>,
    ly_0_stall_hblank_t_cycles: Option<usize>,
    ly_0_stall_oam_t_cycles: Option<usize>,
    ly_0_stall_drawing_t_cycles: Option<usize>,
    ly_0_stall_startup_mode0_t_cycles: Option<usize>,
    ly_0_stall_blank_frame_t_cycles: Option<usize>,
    ly_0_stall_runs: Option<u16>,
    ly_0_max_stall_run_t_cycles: Option<usize>,
    ly_0_max_stall_dot: Option<u16>,
    ly_0_max_stall_mode_dot: Option<u16>,
    cpu_stop_t_cycles: Option<usize>,
    cpu_zombie_stop_t_cycles: Option<usize>,
    ly_0_cpu_stop_t_cycles: Option<usize>,
    ly_0_cpu_zombie_stop_t_cycles: Option<usize>,
    ly_0_stall_cpu_stop_t_cycles: Option<usize>,
    ly_0_stall_cpu_zombie_stop_t_cycles: Option<usize>,
    lcd_disabled_t_cycles: Option<usize>,
    lcd_disable_transitions: Option<u8>,
    lcd_enable_transitions: Option<u8>,
    ly_0_lcd_disabled_t_cycles: Option<usize>,
    ly_0_stall_lcd_disabled_t_cycles: Option<usize>,
}

struct PerformanceCounter {
    base_title: String,
    target_frame_rate_hz: f64,
    emulation_profile_mode: EmulationProfileMode,
    emulation_profile_worker: Option<AsyncEmulationProfileWorker>,
    emulation_profile_request_in_flight: bool,
    sample_session_kind: EmulationProfileSessionKind,
    presented_frames_total: u64,
    sample_started_at: Instant,
    frames_in_sample: u32,
    sample_emulation_duration: Duration,
    sample_profiled_frames: u32,
    sample_profiled_emulation_duration: Duration,
    sample_profiled_emulation_breakdown: EmulationBreakdownSample,
    sample_render_duration: Duration,
    sample_present_duration: Duration,
    sample_pacing_duration: Duration,
    sample_pacing_sleep_target_duration: Duration,
    sample_pacing_audio_correction_duration: Duration,
    sample_pacing_late_duration: Duration,
    sample_pacing_oversleep_duration: Duration,
    sample_audio_submit_sample_count: u64,
    sample_audio_submit_sample_count_observations: u32,
    sample_audio_submit_t_cycles: u64,
    sample_audio_submit_t_cycles_observations: u32,
    sample_audio_submit_queue_before_ms: f64,
    sample_audio_submit_queue_before_observations: u32,
    sample_audio_submit_enqueued_ms: f64,
    sample_audio_submit_enqueued_observations: u32,
    sample_audio_submit_queue_after_ms: f64,
    sample_audio_submit_queue_after_observations: u32,
    sample_audio_queue_before_pacing_ms: f64,
    sample_audio_queue_before_pacing_observations: u32,
    sample_audio_queue_after_pacing_ms: f64,
    sample_audio_queue_after_pacing_observations: u32,
    sample_speed_mode_normal_frames: u32,
    sample_speed_mode_double_frames: u32,
    sample_frame_step_t_cycles: u64,
    sample_frame_step_t_cycles_observations: u32,
    sample_frame_video_dots: u64,
    sample_frame_video_dots_observations: u32,
    sample_frame_start_ly: u64,
    sample_frame_start_ly_observations: u32,
    sample_frame_start_dot: u64,
    sample_frame_start_dot_observations: u32,
    sample_frame_end_ly: u64,
    sample_frame_end_ly_observations: u32,
    sample_frame_end_dot: u64,
    sample_frame_end_dot_observations: u32,
    sample_frame_origin_crossings: u64,
    sample_frame_origin_crossings_observations: u32,
    sample_scanline_transitions: u64,
    sample_scanline_transitions_observations: u32,
    sample_scanlines_over_456: u64,
    sample_scanlines_over_456_observations: u32,
    sample_max_scanline_t_cycles: u64,
    sample_max_scanline_t_cycles_observations: u32,
    sample_max_scanline_ly: u64,
    sample_max_scanline_ly_observations: u32,
    sample_max_mode0_start_dot: u64,
    sample_max_mode0_start_dot_observations: u32,
    sample_max_mode0_start_dot_ly: u64,
    sample_max_mode0_start_dot_ly_observations: u32,
    sample_ly_153_to_0_transitions: u64,
    sample_ly_153_to_0_transitions_observations: u32,
    sample_ly_153_to_0_startup_mode0: u64,
    sample_ly_153_to_0_startup_mode0_observations: u32,
    sample_ly_153_to_0_blank_frame: u64,
    sample_ly_153_to_0_blank_frame_observations: u32,
    sample_ly_0_self_wraps: u64,
    sample_ly_0_self_wraps_observations: u32,
    sample_ly_0_self_wrap_startup_mode0: u64,
    sample_ly_0_self_wrap_startup_mode0_observations: u32,
    sample_ly_0_self_wrap_blank_frame: u64,
    sample_ly_0_self_wrap_blank_frame_observations: u32,
    sample_ly_0_to_1_transitions: u64,
    sample_ly_0_to_1_transitions_observations: u32,
    sample_ly_0_scanline_t_cycles: u64,
    sample_ly_0_scanline_t_cycles_observations: u32,
    sample_ly_0_max_mode0_start_dot: u64,
    sample_ly_0_max_mode0_start_dot_observations: u32,
    sample_ly_0_stall_t_cycles: u64,
    sample_ly_0_stall_t_cycles_observations: u32,
    sample_ly_0_stall_hblank_t_cycles: u64,
    sample_ly_0_stall_hblank_t_cycles_observations: u32,
    sample_ly_0_stall_oam_t_cycles: u64,
    sample_ly_0_stall_oam_t_cycles_observations: u32,
    sample_ly_0_stall_drawing_t_cycles: u64,
    sample_ly_0_stall_drawing_t_cycles_observations: u32,
    sample_ly_0_stall_startup_mode0_t_cycles: u64,
    sample_ly_0_stall_startup_mode0_t_cycles_observations: u32,
    sample_ly_0_stall_blank_frame_t_cycles: u64,
    sample_ly_0_stall_blank_frame_t_cycles_observations: u32,
    sample_ly_0_stall_runs: u64,
    sample_ly_0_stall_runs_observations: u32,
    sample_ly_0_max_stall_run_t_cycles: u64,
    sample_ly_0_max_stall_run_t_cycles_observations: u32,
    sample_ly_0_max_stall_dot: u64,
    sample_ly_0_max_stall_dot_observations: u32,
    sample_ly_0_max_stall_mode_dot: u64,
    sample_ly_0_max_stall_mode_dot_observations: u32,
    sample_cpu_stop_t_cycles: u64,
    sample_cpu_stop_t_cycles_observations: u32,
    sample_cpu_zombie_stop_t_cycles: u64,
    sample_cpu_zombie_stop_t_cycles_observations: u32,
    sample_ly_0_cpu_stop_t_cycles: u64,
    sample_ly_0_cpu_stop_t_cycles_observations: u32,
    sample_ly_0_cpu_zombie_stop_t_cycles: u64,
    sample_ly_0_cpu_zombie_stop_t_cycles_observations: u32,
    sample_ly_0_stall_cpu_stop_t_cycles: u64,
    sample_ly_0_stall_cpu_stop_t_cycles_observations: u32,
    sample_ly_0_stall_cpu_zombie_stop_t_cycles: u64,
    sample_ly_0_stall_cpu_zombie_stop_t_cycles_observations: u32,
    sample_lcd_disabled_t_cycles: u64,
    sample_lcd_disabled_t_cycles_observations: u32,
    sample_lcd_disable_transitions: u64,
    sample_lcd_disable_transitions_observations: u32,
    sample_lcd_enable_transitions: u64,
    sample_lcd_enable_transitions_observations: u32,
    sample_ly_0_lcd_disabled_t_cycles: u64,
    sample_ly_0_lcd_disabled_t_cycles_observations: u32,
    sample_ly_0_stall_lcd_disabled_t_cycles: u64,
    sample_ly_0_stall_lcd_disabled_t_cycles_observations: u32,
    hud_snapshot: Option<PerformanceHudSnapshot>,
}

impl PerformanceCounter {
    fn new(base_title: String) -> Self {
        Self::new_with_emulation_profile_mode(base_title, EmulationProfileMode::from_env())
    }

    fn new_with_emulation_profile_mode(
        base_title: String,
        emulation_profile_mode: EmulationProfileMode,
    ) -> Self {
        Self {
            base_title,
            target_frame_rate_hz: target_frame_rate_hz(),
            emulation_profile_mode,
            emulation_profile_worker: emulation_profile_mode
                .enabled()
                .then(AsyncEmulationProfileWorker::new),
            emulation_profile_request_in_flight: false,
            sample_session_kind: EmulationProfileSessionKind::Single,
            presented_frames_total: 0,
            sample_started_at: Instant::now(),
            frames_in_sample: 0,
            sample_emulation_duration: Duration::ZERO,
            sample_profiled_frames: 0,
            sample_profiled_emulation_duration: Duration::ZERO,
            sample_profiled_emulation_breakdown: EmulationBreakdownSample::default(),
            sample_render_duration: Duration::ZERO,
            sample_present_duration: Duration::ZERO,
            sample_pacing_duration: Duration::ZERO,
            sample_pacing_sleep_target_duration: Duration::ZERO,
            sample_pacing_audio_correction_duration: Duration::ZERO,
            sample_pacing_late_duration: Duration::ZERO,
            sample_pacing_oversleep_duration: Duration::ZERO,
            sample_audio_submit_sample_count: 0,
            sample_audio_submit_sample_count_observations: 0,
            sample_audio_submit_t_cycles: 0,
            sample_audio_submit_t_cycles_observations: 0,
            sample_audio_submit_queue_before_ms: 0.0,
            sample_audio_submit_queue_before_observations: 0,
            sample_audio_submit_enqueued_ms: 0.0,
            sample_audio_submit_enqueued_observations: 0,
            sample_audio_submit_queue_after_ms: 0.0,
            sample_audio_submit_queue_after_observations: 0,
            sample_audio_queue_before_pacing_ms: 0.0,
            sample_audio_queue_before_pacing_observations: 0,
            sample_audio_queue_after_pacing_ms: 0.0,
            sample_audio_queue_after_pacing_observations: 0,
            sample_speed_mode_normal_frames: 0,
            sample_speed_mode_double_frames: 0,
            sample_frame_step_t_cycles: 0,
            sample_frame_step_t_cycles_observations: 0,
            sample_frame_video_dots: 0,
            sample_frame_video_dots_observations: 0,
            sample_frame_start_ly: 0,
            sample_frame_start_ly_observations: 0,
            sample_frame_start_dot: 0,
            sample_frame_start_dot_observations: 0,
            sample_frame_end_ly: 0,
            sample_frame_end_ly_observations: 0,
            sample_frame_end_dot: 0,
            sample_frame_end_dot_observations: 0,
            sample_frame_origin_crossings: 0,
            sample_frame_origin_crossings_observations: 0,
            sample_scanline_transitions: 0,
            sample_scanline_transitions_observations: 0,
            sample_scanlines_over_456: 0,
            sample_scanlines_over_456_observations: 0,
            sample_max_scanline_t_cycles: 0,
            sample_max_scanline_t_cycles_observations: 0,
            sample_max_scanline_ly: 0,
            sample_max_scanline_ly_observations: 0,
            sample_max_mode0_start_dot: 0,
            sample_max_mode0_start_dot_observations: 0,
            sample_max_mode0_start_dot_ly: 0,
            sample_max_mode0_start_dot_ly_observations: 0,
            sample_ly_153_to_0_transitions: 0,
            sample_ly_153_to_0_transitions_observations: 0,
            sample_ly_153_to_0_startup_mode0: 0,
            sample_ly_153_to_0_startup_mode0_observations: 0,
            sample_ly_153_to_0_blank_frame: 0,
            sample_ly_153_to_0_blank_frame_observations: 0,
            sample_ly_0_self_wraps: 0,
            sample_ly_0_self_wraps_observations: 0,
            sample_ly_0_self_wrap_startup_mode0: 0,
            sample_ly_0_self_wrap_startup_mode0_observations: 0,
            sample_ly_0_self_wrap_blank_frame: 0,
            sample_ly_0_self_wrap_blank_frame_observations: 0,
            sample_ly_0_to_1_transitions: 0,
            sample_ly_0_to_1_transitions_observations: 0,
            sample_ly_0_scanline_t_cycles: 0,
            sample_ly_0_scanline_t_cycles_observations: 0,
            sample_ly_0_max_mode0_start_dot: 0,
            sample_ly_0_max_mode0_start_dot_observations: 0,
            sample_ly_0_stall_t_cycles: 0,
            sample_ly_0_stall_t_cycles_observations: 0,
            sample_ly_0_stall_hblank_t_cycles: 0,
            sample_ly_0_stall_hblank_t_cycles_observations: 0,
            sample_ly_0_stall_oam_t_cycles: 0,
            sample_ly_0_stall_oam_t_cycles_observations: 0,
            sample_ly_0_stall_drawing_t_cycles: 0,
            sample_ly_0_stall_drawing_t_cycles_observations: 0,
            sample_ly_0_stall_startup_mode0_t_cycles: 0,
            sample_ly_0_stall_startup_mode0_t_cycles_observations: 0,
            sample_ly_0_stall_blank_frame_t_cycles: 0,
            sample_ly_0_stall_blank_frame_t_cycles_observations: 0,
            sample_ly_0_stall_runs: 0,
            sample_ly_0_stall_runs_observations: 0,
            sample_ly_0_max_stall_run_t_cycles: 0,
            sample_ly_0_max_stall_run_t_cycles_observations: 0,
            sample_ly_0_max_stall_dot: 0,
            sample_ly_0_max_stall_dot_observations: 0,
            sample_ly_0_max_stall_mode_dot: 0,
            sample_ly_0_max_stall_mode_dot_observations: 0,
            sample_cpu_stop_t_cycles: 0,
            sample_cpu_stop_t_cycles_observations: 0,
            sample_cpu_zombie_stop_t_cycles: 0,
            sample_cpu_zombie_stop_t_cycles_observations: 0,
            sample_ly_0_cpu_stop_t_cycles: 0,
            sample_ly_0_cpu_stop_t_cycles_observations: 0,
            sample_ly_0_cpu_zombie_stop_t_cycles: 0,
            sample_ly_0_cpu_zombie_stop_t_cycles_observations: 0,
            sample_ly_0_stall_cpu_stop_t_cycles: 0,
            sample_ly_0_stall_cpu_stop_t_cycles_observations: 0,
            sample_ly_0_stall_cpu_zombie_stop_t_cycles: 0,
            sample_ly_0_stall_cpu_zombie_stop_t_cycles_observations: 0,
            sample_lcd_disabled_t_cycles: 0,
            sample_lcd_disabled_t_cycles_observations: 0,
            sample_lcd_disable_transitions: 0,
            sample_lcd_disable_transitions_observations: 0,
            sample_lcd_enable_transitions: 0,
            sample_lcd_enable_transitions_observations: 0,
            sample_ly_0_lcd_disabled_t_cycles: 0,
            sample_ly_0_lcd_disabled_t_cycles_observations: 0,
            sample_ly_0_stall_lcd_disabled_t_cycles: 0,
            sample_ly_0_stall_lcd_disabled_t_cycles_observations: 0,
            hud_snapshot: None,
        }
    }

    fn set_target_frame_rate_hz(&mut self, target_frame_rate_hz: f64) {
        self.target_frame_rate_hz = target_frame_rate_hz;
    }

    fn record_presented_frame(
        &mut self,
        window: &mut Window,
        sample: FramePerformanceSample,
    ) -> Result<(), String> {
        self.presented_frames_total = self.presented_frames_total.saturating_add(1);
        self.collect_emulation_profile_results();
        self.submit_emulation_profile_request(
            sample.emulation_profile_request,
            sample.emulation_duration,
        );

        self.sample_session_kind = sample.session_kind;
        self.frames_in_sample += 1;
        self.sample_emulation_duration += sample.emulation_duration;
        self.sample_render_duration += sample.render_duration;
        self.sample_present_duration += sample.present_duration;
        self.sample_pacing_duration += sample.pacing_duration;
        self.sample_pacing_sleep_target_duration += sample.pacing_sleep_target_duration;
        self.sample_pacing_audio_correction_duration += sample.pacing_audio_correction_duration;
        self.sample_pacing_late_duration += sample.pacing_late_duration;
        self.sample_pacing_oversleep_duration += sample.pacing_oversleep_duration;
        if let Some(sample_count) = sample.audio_submit_sample_count {
            self.sample_audio_submit_sample_count += sample_count as u64;
            self.sample_audio_submit_sample_count_observations += 1;
        }
        if let Some(t_cycles) = sample.audio_submit_t_cycles {
            self.sample_audio_submit_t_cycles += t_cycles as u64;
            self.sample_audio_submit_t_cycles_observations += 1;
        }
        if let Some(audio_queue_ms) = sample.audio_submit_queue_before_ms {
            self.sample_audio_submit_queue_before_ms += audio_queue_ms;
            self.sample_audio_submit_queue_before_observations += 1;
        }
        if let Some(audio_queue_ms) = sample.audio_submit_enqueued_ms {
            self.sample_audio_submit_enqueued_ms += audio_queue_ms;
            self.sample_audio_submit_enqueued_observations += 1;
        }
        if let Some(audio_queue_ms) = sample.audio_submit_queue_after_ms {
            self.sample_audio_submit_queue_after_ms += audio_queue_ms;
            self.sample_audio_submit_queue_after_observations += 1;
        }
        if let Some(audio_queue_ms) = sample.audio_queue_before_pacing_ms {
            self.sample_audio_queue_before_pacing_ms += audio_queue_ms;
            self.sample_audio_queue_before_pacing_observations += 1;
        }
        if let Some(audio_queue_ms) = sample.audio_queue_after_pacing_ms {
            self.sample_audio_queue_after_pacing_ms += audio_queue_ms;
            self.sample_audio_queue_after_pacing_observations += 1;
        }
        match sample.speed_mode {
            Some(CgbSpeedMode::Normal) => {
                self.sample_speed_mode_normal_frames =
                    self.sample_speed_mode_normal_frames.saturating_add(1);
            }
            Some(CgbSpeedMode::Double) => {
                self.sample_speed_mode_double_frames =
                    self.sample_speed_mode_double_frames.saturating_add(1);
            }
            None => {}
        }
        if let Some(t_cycles) = sample.frame_step_t_cycles {
            self.sample_frame_step_t_cycles += t_cycles as u64;
            self.sample_frame_step_t_cycles_observations += 1;
        }
        if let Some(video_dots) = sample.frame_video_dots {
            self.sample_frame_video_dots += video_dots as u64;
            self.sample_frame_video_dots_observations += 1;
        }
        if let Some(start_ly) = sample.frame_start_ly {
            self.sample_frame_start_ly += u64::from(start_ly);
            self.sample_frame_start_ly_observations += 1;
        }
        if let Some(start_dot) = sample.frame_start_dot {
            self.sample_frame_start_dot += u64::from(start_dot);
            self.sample_frame_start_dot_observations += 1;
        }
        if let Some(end_ly) = sample.frame_end_ly {
            self.sample_frame_end_ly += u64::from(end_ly);
            self.sample_frame_end_ly_observations += 1;
        }
        if let Some(end_dot) = sample.frame_end_dot {
            self.sample_frame_end_dot += u64::from(end_dot);
            self.sample_frame_end_dot_observations += 1;
        }
        if let Some(frame_origin_crossings) = sample.frame_origin_crossings {
            self.sample_frame_origin_crossings += u64::from(frame_origin_crossings);
            self.sample_frame_origin_crossings_observations += 1;
        }
        if let Some(scanline_transitions) = sample.scanline_transitions {
            self.sample_scanline_transitions += u64::from(scanline_transitions);
            self.sample_scanline_transitions_observations += 1;
        }
        if let Some(scanlines_over_456) = sample.scanlines_over_456 {
            self.sample_scanlines_over_456 += u64::from(scanlines_over_456);
            self.sample_scanlines_over_456_observations += 1;
        }
        if let Some(max_scanline_t_cycles) = sample.max_scanline_t_cycles {
            self.sample_max_scanline_t_cycles += max_scanline_t_cycles as u64;
            self.sample_max_scanline_t_cycles_observations += 1;
        }
        if let Some(max_scanline_ly) = sample.max_scanline_ly {
            self.sample_max_scanline_ly += u64::from(max_scanline_ly);
            self.sample_max_scanline_ly_observations += 1;
        }
        if let Some(max_mode0_start_dot) = sample.max_mode0_start_dot {
            self.sample_max_mode0_start_dot += u64::from(max_mode0_start_dot);
            self.sample_max_mode0_start_dot_observations += 1;
        }
        if let Some(max_mode0_start_dot_ly) = sample.max_mode0_start_dot_ly {
            self.sample_max_mode0_start_dot_ly += u64::from(max_mode0_start_dot_ly);
            self.sample_max_mode0_start_dot_ly_observations += 1;
        }
        if let Some(transitions) = sample.ly_153_to_0_transitions {
            self.sample_ly_153_to_0_transitions += u64::from(transitions);
            self.sample_ly_153_to_0_transitions_observations += 1;
        }
        if let Some(transitions) = sample.ly_153_to_0_startup_mode0 {
            self.sample_ly_153_to_0_startup_mode0 += u64::from(transitions);
            self.sample_ly_153_to_0_startup_mode0_observations += 1;
        }
        if let Some(transitions) = sample.ly_153_to_0_blank_frame {
            self.sample_ly_153_to_0_blank_frame += u64::from(transitions);
            self.sample_ly_153_to_0_blank_frame_observations += 1;
        }
        if let Some(wraps) = sample.ly_0_self_wraps {
            self.sample_ly_0_self_wraps += u64::from(wraps);
            self.sample_ly_0_self_wraps_observations += 1;
        }
        if let Some(wraps) = sample.ly_0_self_wrap_startup_mode0 {
            self.sample_ly_0_self_wrap_startup_mode0 += u64::from(wraps);
            self.sample_ly_0_self_wrap_startup_mode0_observations += 1;
        }
        if let Some(wraps) = sample.ly_0_self_wrap_blank_frame {
            self.sample_ly_0_self_wrap_blank_frame += u64::from(wraps);
            self.sample_ly_0_self_wrap_blank_frame_observations += 1;
        }
        if let Some(transitions) = sample.ly_0_to_1_transitions {
            self.sample_ly_0_to_1_transitions += u64::from(transitions);
            self.sample_ly_0_to_1_transitions_observations += 1;
        }
        if let Some(t_cycles) = sample.ly_0_scanline_t_cycles {
            self.sample_ly_0_scanline_t_cycles += t_cycles as u64;
            self.sample_ly_0_scanline_t_cycles_observations += 1;
        }
        if let Some(mode0_start_dot) = sample.ly_0_max_mode0_start_dot {
            self.sample_ly_0_max_mode0_start_dot += u64::from(mode0_start_dot);
            self.sample_ly_0_max_mode0_start_dot_observations += 1;
        }
        if let Some(t_cycles) = sample.ly_0_stall_t_cycles {
            self.sample_ly_0_stall_t_cycles += t_cycles as u64;
            self.sample_ly_0_stall_t_cycles_observations += 1;
        }
        if let Some(t_cycles) = sample.ly_0_stall_hblank_t_cycles {
            self.sample_ly_0_stall_hblank_t_cycles += t_cycles as u64;
            self.sample_ly_0_stall_hblank_t_cycles_observations += 1;
        }
        if let Some(t_cycles) = sample.ly_0_stall_oam_t_cycles {
            self.sample_ly_0_stall_oam_t_cycles += t_cycles as u64;
            self.sample_ly_0_stall_oam_t_cycles_observations += 1;
        }
        if let Some(t_cycles) = sample.ly_0_stall_drawing_t_cycles {
            self.sample_ly_0_stall_drawing_t_cycles += t_cycles as u64;
            self.sample_ly_0_stall_drawing_t_cycles_observations += 1;
        }
        if let Some(t_cycles) = sample.ly_0_stall_startup_mode0_t_cycles {
            self.sample_ly_0_stall_startup_mode0_t_cycles += t_cycles as u64;
            self.sample_ly_0_stall_startup_mode0_t_cycles_observations += 1;
        }
        if let Some(t_cycles) = sample.ly_0_stall_blank_frame_t_cycles {
            self.sample_ly_0_stall_blank_frame_t_cycles += t_cycles as u64;
            self.sample_ly_0_stall_blank_frame_t_cycles_observations += 1;
        }
        if let Some(stall_runs) = sample.ly_0_stall_runs {
            self.sample_ly_0_stall_runs += u64::from(stall_runs);
            self.sample_ly_0_stall_runs_observations += 1;
        }
        if let Some(t_cycles) = sample.ly_0_max_stall_run_t_cycles {
            self.sample_ly_0_max_stall_run_t_cycles += t_cycles as u64;
            self.sample_ly_0_max_stall_run_t_cycles_observations += 1;
        }
        if let Some(dot) = sample.ly_0_max_stall_dot {
            self.sample_ly_0_max_stall_dot += u64::from(dot);
            self.sample_ly_0_max_stall_dot_observations += 1;
        }
        if let Some(mode_dot) = sample.ly_0_max_stall_mode_dot {
            self.sample_ly_0_max_stall_mode_dot += u64::from(mode_dot);
            self.sample_ly_0_max_stall_mode_dot_observations += 1;
        }
        if let Some(t_cycles) = sample.cpu_stop_t_cycles {
            self.sample_cpu_stop_t_cycles += t_cycles as u64;
            self.sample_cpu_stop_t_cycles_observations += 1;
        }
        if let Some(t_cycles) = sample.cpu_zombie_stop_t_cycles {
            self.sample_cpu_zombie_stop_t_cycles += t_cycles as u64;
            self.sample_cpu_zombie_stop_t_cycles_observations += 1;
        }
        if let Some(t_cycles) = sample.ly_0_cpu_stop_t_cycles {
            self.sample_ly_0_cpu_stop_t_cycles += t_cycles as u64;
            self.sample_ly_0_cpu_stop_t_cycles_observations += 1;
        }
        if let Some(t_cycles) = sample.ly_0_cpu_zombie_stop_t_cycles {
            self.sample_ly_0_cpu_zombie_stop_t_cycles += t_cycles as u64;
            self.sample_ly_0_cpu_zombie_stop_t_cycles_observations += 1;
        }
        if let Some(t_cycles) = sample.ly_0_stall_cpu_stop_t_cycles {
            self.sample_ly_0_stall_cpu_stop_t_cycles += t_cycles as u64;
            self.sample_ly_0_stall_cpu_stop_t_cycles_observations += 1;
        }
        if let Some(t_cycles) = sample.ly_0_stall_cpu_zombie_stop_t_cycles {
            self.sample_ly_0_stall_cpu_zombie_stop_t_cycles += t_cycles as u64;
            self.sample_ly_0_stall_cpu_zombie_stop_t_cycles_observations += 1;
        }
        if let Some(t_cycles) = sample.lcd_disabled_t_cycles {
            self.sample_lcd_disabled_t_cycles += t_cycles as u64;
            self.sample_lcd_disabled_t_cycles_observations += 1;
        }
        if let Some(transitions) = sample.lcd_disable_transitions {
            self.sample_lcd_disable_transitions += u64::from(transitions);
            self.sample_lcd_disable_transitions_observations += 1;
        }
        if let Some(transitions) = sample.lcd_enable_transitions {
            self.sample_lcd_enable_transitions += u64::from(transitions);
            self.sample_lcd_enable_transitions_observations += 1;
        }
        if let Some(t_cycles) = sample.ly_0_lcd_disabled_t_cycles {
            self.sample_ly_0_lcd_disabled_t_cycles += t_cycles as u64;
            self.sample_ly_0_lcd_disabled_t_cycles_observations += 1;
        }
        if let Some(t_cycles) = sample.ly_0_stall_lcd_disabled_t_cycles {
            self.sample_ly_0_stall_lcd_disabled_t_cycles += t_cycles as u64;
            self.sample_ly_0_stall_lcd_disabled_t_cycles_observations += 1;
        }

        let elapsed = self.sample_started_at.elapsed();
        self.hud_snapshot = Some(self.snapshot_from_elapsed(elapsed));
        if elapsed < PERFORMANCE_SAMPLE_INTERVAL {
            return Ok(());
        }

        let snapshot = self
            .hud_snapshot
            .expect("performance HUD snapshot should exist after at least one frame");
        map_display_result(
            window.set_title(&performance_window_title(&self.base_title, snapshot)),
            "failed to update SDL3 window title",
        )?;
        if let Some(summary) = self.emulation_profile_summary(elapsed, snapshot) {
            eprintln!("{summary}");
        }

        self.reset_sample();

        Ok(())
    }

    fn reset_base_title(&mut self, window: &mut Window, base_title: String) -> Result<(), String> {
        self.base_title = base_title;
        self.hud_snapshot = None;
        self.reset_sample();
        map_display_result(
            window.set_title(&self.base_title),
            "failed to update SDL3 window title",
        )
    }

    fn hud_snapshot(&self) -> Option<PerformanceHudSnapshot> {
        self.hud_snapshot
    }

    fn emulation_profile_enabled(&self) -> bool {
        self.emulation_profile_mode.enabled()
    }

    fn emulation_profile_detail(&self) -> Option<EmulationProfileDetail> {
        self.emulation_profile_mode.detail()
    }

    fn should_profile_next_frame(&mut self) -> bool {
        self.collect_emulation_profile_results();
        let Some(sample_every_frames) = self.emulation_profile_mode.sample_every_frames() else {
            return false;
        };
        !self.emulation_profile_request_in_flight
            && self.presented_frames_total > 0
            && (self.presented_frames_total + 1).is_multiple_of(u64::from(sample_every_frames))
    }

    fn snapshot_from_elapsed(&self, elapsed: Duration) -> PerformanceHudSnapshot {
        let frames = self.frames_in_sample.max(1);
        let frames_f64 = f64::from(frames);
        let elapsed_secs = elapsed.as_secs_f64().max(f64::EPSILON);
        let fps = frames_f64 / elapsed_secs;

        PerformanceHudSnapshot {
            fps,
            speed_percent: fps / self.target_frame_rate_hz * 100.0,
            frame_time_ms: elapsed_secs * 1_000.0 / frames_f64,
            emulation_time_ms: self.sample_emulation_duration.as_secs_f64() * 1_000.0 / frames_f64,
            render_time_ms: self.sample_render_duration.as_secs_f64() * 1_000.0 / frames_f64,
            pacing_time_ms: self.sample_pacing_duration.as_secs_f64() * 1_000.0 / frames_f64,
            audio_queue_ms: (self.sample_audio_queue_after_pacing_observations > 0).then_some(
                self.sample_audio_queue_after_pacing_ms
                    / f64::from(self.sample_audio_queue_after_pacing_observations),
            ),
            rewind: RewindHudSnapshot::default(),
        }
    }

    fn emulation_profile_summary(
        &self,
        elapsed: Duration,
        snapshot: PerformanceHudSnapshot,
    ) -> Option<String> {
        if !self.emulation_profile_enabled() || self.frames_in_sample == 0 {
            return None;
        }
        if self.sample_profiled_frames == 0 {
            return None;
        }

        let profiled_frames_f64 = f64::from(self.sample_profiled_frames.max(1));
        let frames_f64 = f64::from(self.frames_in_sample.max(1));
        let breakdown = self.sample_profiled_emulation_breakdown;
        let sampled_emu_ms =
            average_duration_ms(self.sample_profiled_emulation_duration, profiled_frames_f64);
        let estimated_core_duration = self
            .sample_profiled_emulation_duration
            .saturating_sub(breakdown.host_duration());
        let core_ms = average_duration_ms(estimated_core_duration, profiled_frames_f64);
        let host_ms = average_duration_ms(breakdown.host_duration(), profiled_frames_f64);
        let sample_every_frames = self
            .emulation_profile_mode
            .sample_every_frames()
            .expect("sampled emulation profile mode should provide a frame stride");
        let profile_detail_label = self
            .emulation_profile_mode
            .detail()
            .expect("sampled emulation profile mode should provide a detail mode")
            .label();
        let profile_base_ms =
            average_duration_ms(breakdown.profile_base_duration, profiled_frames_f64);
        let profile_core_ms =
            average_duration_ms(breakdown.profile_core_duration, profiled_frames_f64);
        let profile_full_ms =
            average_duration_ms(breakdown.profile_full_duration, profiled_frames_f64);
        let profile_core_overhead_ms = average_duration_ms(
            breakdown.profile_core_overhead_duration,
            profiled_frames_f64,
        );
        let profile_ppu_observer_overhead_ms = average_duration_ms(
            breakdown.profile_ppu_observer_overhead_duration,
            profiled_frames_f64,
        );
        let serial_active_t_cycles =
            average_counter_value(breakdown.serial_active_t_cycles, profiled_frames_f64);
        let serial_internal_ticks =
            average_counter_value(breakdown.serial_internal_ticks, profiled_frames_f64);
        let serial_external_ticks =
            average_counter_value(breakdown.serial_external_ticks, profiled_frames_f64);
        let serial_external_wait_ticks =
            average_counter_value(breakdown.serial_external_wait_ticks, profiled_frames_f64);
        let serial_shift_edges =
            average_counter_value(breakdown.serial_shift_edges, profiled_frames_f64);
        let serial_completed_bytes =
            average_counter_value(breakdown.serial_completed_bytes, profiled_frames_f64);
        let serial_external_port_ticks =
            average_counter_value(breakdown.serial_external_port_ticks, profiled_frames_f64);
        let audio_submit_samples = if self.sample_audio_submit_sample_count_observations > 0 {
            format!(
                "{:.2}",
                self.sample_audio_submit_sample_count as f64
                    / f64::from(self.sample_audio_submit_sample_count_observations)
            )
        } else {
            "off".to_string()
        };
        let audio_submit_t_cycles = if self.sample_audio_submit_t_cycles_observations > 0 {
            format!(
                "{:.2}",
                self.sample_audio_submit_t_cycles as f64
                    / f64::from(self.sample_audio_submit_t_cycles_observations)
            )
        } else {
            "off".to_string()
        };
        let audio_submit_queue_before_ms = if self.sample_audio_submit_queue_before_observations > 0
        {
            format!(
                "{:.2}",
                self.sample_audio_submit_queue_before_ms
                    / f64::from(self.sample_audio_submit_queue_before_observations)
            )
        } else {
            "off".to_string()
        };
        let audio_submit_enqueued_ms = if self.sample_audio_submit_enqueued_observations > 0 {
            format!(
                "{:.2}",
                self.sample_audio_submit_enqueued_ms
                    / f64::from(self.sample_audio_submit_enqueued_observations)
            )
        } else {
            "off".to_string()
        };
        let audio_submit_queue_after_ms = if self.sample_audio_submit_queue_after_observations > 0 {
            format!(
                "{:.2}",
                self.sample_audio_submit_queue_after_ms
                    / f64::from(self.sample_audio_submit_queue_after_observations)
            )
        } else {
            "off".to_string()
        };
        let audio_queue_before_pacing_ms = if self.sample_audio_queue_before_pacing_observations > 0
        {
            format!(
                "{:.2}",
                self.sample_audio_queue_before_pacing_ms
                    / f64::from(self.sample_audio_queue_before_pacing_observations)
            )
        } else {
            "off".to_string()
        };
        let audio_queue_after_pacing_ms = if self.sample_audio_queue_after_pacing_observations > 0 {
            format!(
                "{:.2}",
                self.sample_audio_queue_after_pacing_ms
                    / f64::from(self.sample_audio_queue_after_pacing_observations)
            )
        } else {
            "off".to_string()
        };
        let speed_mode = match (
            self.sample_speed_mode_normal_frames,
            self.sample_speed_mode_double_frames,
        ) {
            (0, 0) => "off",
            (_, 0) => "normal",
            (0, _) => "double",
            _ => "mixed",
        };
        let frame_step_t_cycles = if self.sample_frame_step_t_cycles_observations > 0 {
            format!(
                "{:.2}",
                self.sample_frame_step_t_cycles as f64
                    / f64::from(self.sample_frame_step_t_cycles_observations)
            )
        } else {
            "off".to_string()
        };
        let frame_video_dots = if self.sample_frame_video_dots_observations > 0 {
            format!(
                "{:.2}",
                self.sample_frame_video_dots as f64
                    / f64::from(self.sample_frame_video_dots_observations)
            )
        } else {
            "off".to_string()
        };
        let frame_start_ly = if self.sample_frame_start_ly_observations > 0 {
            format!(
                "{:.2}",
                self.sample_frame_start_ly as f64
                    / f64::from(self.sample_frame_start_ly_observations)
            )
        } else {
            "off".to_string()
        };
        let frame_start_dot = if self.sample_frame_start_dot_observations > 0 {
            format!(
                "{:.2}",
                self.sample_frame_start_dot as f64
                    / f64::from(self.sample_frame_start_dot_observations)
            )
        } else {
            "off".to_string()
        };
        let frame_end_ly = if self.sample_frame_end_ly_observations > 0 {
            format!(
                "{:.2}",
                self.sample_frame_end_ly as f64 / f64::from(self.sample_frame_end_ly_observations)
            )
        } else {
            "off".to_string()
        };
        let frame_end_dot = if self.sample_frame_end_dot_observations > 0 {
            format!(
                "{:.2}",
                self.sample_frame_end_dot as f64
                    / f64::from(self.sample_frame_end_dot_observations)
            )
        } else {
            "off".to_string()
        };
        let frame_origin_crossings = if self.sample_frame_origin_crossings_observations > 0 {
            format!(
                "{:.2}",
                self.sample_frame_origin_crossings as f64
                    / f64::from(self.sample_frame_origin_crossings_observations)
            )
        } else {
            "off".to_string()
        };
        let scanline_transitions = if self.sample_scanline_transitions_observations > 0 {
            format!(
                "{:.2}",
                self.sample_scanline_transitions as f64
                    / f64::from(self.sample_scanline_transitions_observations)
            )
        } else {
            "off".to_string()
        };
        let scanlines_over_456 = if self.sample_scanlines_over_456_observations > 0 {
            format!(
                "{:.2}",
                self.sample_scanlines_over_456 as f64
                    / f64::from(self.sample_scanlines_over_456_observations)
            )
        } else {
            "off".to_string()
        };
        let max_scanline_t_cycles = if self.sample_max_scanline_t_cycles_observations > 0 {
            format!(
                "{:.2}",
                self.sample_max_scanline_t_cycles as f64
                    / f64::from(self.sample_max_scanline_t_cycles_observations)
            )
        } else {
            "off".to_string()
        };
        let max_scanline_ly = if self.sample_max_scanline_ly_observations > 0 {
            format!(
                "{:.2}",
                self.sample_max_scanline_ly as f64
                    / f64::from(self.sample_max_scanline_ly_observations)
            )
        } else {
            "off".to_string()
        };
        let max_mode0_start_dot = if self.sample_max_mode0_start_dot_observations > 0 {
            format!(
                "{:.2}",
                self.sample_max_mode0_start_dot as f64
                    / f64::from(self.sample_max_mode0_start_dot_observations)
            )
        } else {
            "off".to_string()
        };
        let max_mode0_start_dot_ly = if self.sample_max_mode0_start_dot_ly_observations > 0 {
            format!(
                "{:.2}",
                self.sample_max_mode0_start_dot_ly as f64
                    / f64::from(self.sample_max_mode0_start_dot_ly_observations)
            )
        } else {
            "off".to_string()
        };
        let ly_153_to_0_transitions = if self.sample_ly_153_to_0_transitions_observations > 0 {
            format!(
                "{:.2}",
                self.sample_ly_153_to_0_transitions as f64
                    / f64::from(self.sample_ly_153_to_0_transitions_observations)
            )
        } else {
            "off".to_string()
        };
        let ly_153_to_0_startup_mode0 = if self.sample_ly_153_to_0_startup_mode0_observations > 0 {
            format!(
                "{:.2}",
                self.sample_ly_153_to_0_startup_mode0 as f64
                    / f64::from(self.sample_ly_153_to_0_startup_mode0_observations)
            )
        } else {
            "off".to_string()
        };
        let ly_153_to_0_blank_frame = if self.sample_ly_153_to_0_blank_frame_observations > 0 {
            format!(
                "{:.2}",
                self.sample_ly_153_to_0_blank_frame as f64
                    / f64::from(self.sample_ly_153_to_0_blank_frame_observations)
            )
        } else {
            "off".to_string()
        };
        let ly_0_self_wraps = if self.sample_ly_0_self_wraps_observations > 0 {
            format!(
                "{:.2}",
                self.sample_ly_0_self_wraps as f64
                    / f64::from(self.sample_ly_0_self_wraps_observations)
            )
        } else {
            "off".to_string()
        };
        let ly_0_self_wrap_startup_mode0 =
            if self.sample_ly_0_self_wrap_startup_mode0_observations > 0 {
                format!(
                    "{:.2}",
                    self.sample_ly_0_self_wrap_startup_mode0 as f64
                        / f64::from(self.sample_ly_0_self_wrap_startup_mode0_observations)
                )
            } else {
                "off".to_string()
            };
        let ly_0_self_wrap_blank_frame = if self.sample_ly_0_self_wrap_blank_frame_observations > 0
        {
            format!(
                "{:.2}",
                self.sample_ly_0_self_wrap_blank_frame as f64
                    / f64::from(self.sample_ly_0_self_wrap_blank_frame_observations)
            )
        } else {
            "off".to_string()
        };
        let ly_0_to_1_transitions = if self.sample_ly_0_to_1_transitions_observations > 0 {
            format!(
                "{:.2}",
                self.sample_ly_0_to_1_transitions as f64
                    / f64::from(self.sample_ly_0_to_1_transitions_observations)
            )
        } else {
            "off".to_string()
        };
        let ly_0_scanline_t_cycles = if self.sample_ly_0_scanline_t_cycles_observations > 0 {
            format!(
                "{:.2}",
                self.sample_ly_0_scanline_t_cycles as f64
                    / f64::from(self.sample_ly_0_scanline_t_cycles_observations)
            )
        } else {
            "off".to_string()
        };
        let ly_0_max_mode0_start_dot = if self.sample_ly_0_max_mode0_start_dot_observations > 0 {
            format!(
                "{:.2}",
                self.sample_ly_0_max_mode0_start_dot as f64
                    / f64::from(self.sample_ly_0_max_mode0_start_dot_observations)
            )
        } else {
            "off".to_string()
        };
        let ly_0_stall_t_cycles = if self.sample_ly_0_stall_t_cycles_observations > 0 {
            format!(
                "{:.2}",
                self.sample_ly_0_stall_t_cycles as f64
                    / f64::from(self.sample_ly_0_stall_t_cycles_observations)
            )
        } else {
            "off".to_string()
        };
        let ly_0_stall_hblank_t_cycles = if self.sample_ly_0_stall_hblank_t_cycles_observations > 0
        {
            format!(
                "{:.2}",
                self.sample_ly_0_stall_hblank_t_cycles as f64
                    / f64::from(self.sample_ly_0_stall_hblank_t_cycles_observations)
            )
        } else {
            "off".to_string()
        };
        let ly_0_stall_oam_t_cycles = if self.sample_ly_0_stall_oam_t_cycles_observations > 0 {
            format!(
                "{:.2}",
                self.sample_ly_0_stall_oam_t_cycles as f64
                    / f64::from(self.sample_ly_0_stall_oam_t_cycles_observations)
            )
        } else {
            "off".to_string()
        };
        let ly_0_stall_drawing_t_cycles =
            if self.sample_ly_0_stall_drawing_t_cycles_observations > 0 {
                format!(
                    "{:.2}",
                    self.sample_ly_0_stall_drawing_t_cycles as f64
                        / f64::from(self.sample_ly_0_stall_drawing_t_cycles_observations)
                )
            } else {
                "off".to_string()
            };
        let ly_0_stall_startup_mode0_t_cycles =
            if self.sample_ly_0_stall_startup_mode0_t_cycles_observations > 0 {
                format!(
                    "{:.2}",
                    self.sample_ly_0_stall_startup_mode0_t_cycles as f64
                        / f64::from(self.sample_ly_0_stall_startup_mode0_t_cycles_observations)
                )
            } else {
                "off".to_string()
            };
        let ly_0_stall_blank_frame_t_cycles =
            if self.sample_ly_0_stall_blank_frame_t_cycles_observations > 0 {
                format!(
                    "{:.2}",
                    self.sample_ly_0_stall_blank_frame_t_cycles as f64
                        / f64::from(self.sample_ly_0_stall_blank_frame_t_cycles_observations)
                )
            } else {
                "off".to_string()
            };
        let ly_0_stall_runs = if self.sample_ly_0_stall_runs_observations > 0 {
            format!(
                "{:.2}",
                self.sample_ly_0_stall_runs as f64
                    / f64::from(self.sample_ly_0_stall_runs_observations)
            )
        } else {
            "off".to_string()
        };
        let ly_0_max_stall_run_t_cycles =
            if self.sample_ly_0_max_stall_run_t_cycles_observations > 0 {
                format!(
                    "{:.2}",
                    self.sample_ly_0_max_stall_run_t_cycles as f64
                        / f64::from(self.sample_ly_0_max_stall_run_t_cycles_observations)
                )
            } else {
                "off".to_string()
            };
        let ly_0_max_stall_dot = if self.sample_ly_0_max_stall_dot_observations > 0 {
            format!(
                "{:.2}",
                self.sample_ly_0_max_stall_dot as f64
                    / f64::from(self.sample_ly_0_max_stall_dot_observations)
            )
        } else {
            "off".to_string()
        };
        let ly_0_max_stall_mode_dot = if self.sample_ly_0_max_stall_mode_dot_observations > 0 {
            format!(
                "{:.2}",
                self.sample_ly_0_max_stall_mode_dot as f64
                    / f64::from(self.sample_ly_0_max_stall_mode_dot_observations)
            )
        } else {
            "off".to_string()
        };
        let cpu_stop_t_cycles = if self.sample_cpu_stop_t_cycles_observations > 0 {
            format!(
                "{:.2}",
                self.sample_cpu_stop_t_cycles as f64
                    / f64::from(self.sample_cpu_stop_t_cycles_observations)
            )
        } else {
            "off".to_string()
        };
        let cpu_zombie_stop_t_cycles = if self.sample_cpu_zombie_stop_t_cycles_observations > 0 {
            format!(
                "{:.2}",
                self.sample_cpu_zombie_stop_t_cycles as f64
                    / f64::from(self.sample_cpu_zombie_stop_t_cycles_observations)
            )
        } else {
            "off".to_string()
        };
        let ly_0_cpu_stop_t_cycles = if self.sample_ly_0_cpu_stop_t_cycles_observations > 0 {
            format!(
                "{:.2}",
                self.sample_ly_0_cpu_stop_t_cycles as f64
                    / f64::from(self.sample_ly_0_cpu_stop_t_cycles_observations)
            )
        } else {
            "off".to_string()
        };
        let ly_0_cpu_zombie_stop_t_cycles =
            if self.sample_ly_0_cpu_zombie_stop_t_cycles_observations > 0 {
                format!(
                    "{:.2}",
                    self.sample_ly_0_cpu_zombie_stop_t_cycles as f64
                        / f64::from(self.sample_ly_0_cpu_zombie_stop_t_cycles_observations)
                )
            } else {
                "off".to_string()
            };
        let ly_0_stall_cpu_stop_t_cycles =
            if self.sample_ly_0_stall_cpu_stop_t_cycles_observations > 0 {
                format!(
                    "{:.2}",
                    self.sample_ly_0_stall_cpu_stop_t_cycles as f64
                        / f64::from(self.sample_ly_0_stall_cpu_stop_t_cycles_observations)
                )
            } else {
                "off".to_string()
            };
        let ly_0_stall_cpu_zombie_stop_t_cycles =
            if self.sample_ly_0_stall_cpu_zombie_stop_t_cycles_observations > 0 {
                format!(
                    "{:.2}",
                    self.sample_ly_0_stall_cpu_zombie_stop_t_cycles as f64
                        / f64::from(self.sample_ly_0_stall_cpu_zombie_stop_t_cycles_observations)
                )
            } else {
                "off".to_string()
            };
        let lcd_disabled_t_cycles = if self.sample_lcd_disabled_t_cycles_observations > 0 {
            format!(
                "{:.2}",
                self.sample_lcd_disabled_t_cycles as f64
                    / f64::from(self.sample_lcd_disabled_t_cycles_observations)
            )
        } else {
            "off".to_string()
        };
        let lcd_disable_transitions = if self.sample_lcd_disable_transitions_observations > 0 {
            format!(
                "{:.2}",
                self.sample_lcd_disable_transitions as f64
                    / f64::from(self.sample_lcd_disable_transitions_observations)
            )
        } else {
            "off".to_string()
        };
        let lcd_enable_transitions = if self.sample_lcd_enable_transitions_observations > 0 {
            format!(
                "{:.2}",
                self.sample_lcd_enable_transitions as f64
                    / f64::from(self.sample_lcd_enable_transitions_observations)
            )
        } else {
            "off".to_string()
        };
        let ly_0_lcd_disabled_t_cycles = if self.sample_ly_0_lcd_disabled_t_cycles_observations > 0
        {
            format!(
                "{:.2}",
                self.sample_ly_0_lcd_disabled_t_cycles as f64
                    / f64::from(self.sample_ly_0_lcd_disabled_t_cycles_observations)
            )
        } else {
            "off".to_string()
        };
        let ly_0_stall_lcd_disabled_t_cycles =
            if self.sample_ly_0_stall_lcd_disabled_t_cycles_observations > 0 {
                format!(
                    "{:.2}",
                    self.sample_ly_0_stall_lcd_disabled_t_cycles as f64
                        / f64::from(self.sample_ly_0_stall_lcd_disabled_t_cycles_observations)
                )
            } else {
                "off".to_string()
            };

        Some(format!(
            "gb-desktop emu-profile session={} fps={:.1} speed={:.0}% frame_ms={:.2} emu_ms={:.2} sampled_frames={} sample_every={} profile_detail={profile_detail_label} sampled_emu_ms={sampled_emu_ms:.2} core_est_ms={core_ms:.2} profile_base_ms={profile_base_ms:.2} profile_core_ms={profile_core_ms:.2} profile_full_ms={profile_full_ms:.2} profile_core_overhead_ms={profile_core_overhead_ms:.2} profile_ppu_observer_overhead_ms={profile_ppu_observer_overhead_ms:.2} ppu_ms={:.2} cpu_ms={:.2} core_other_ms={:.2} ext_ms={:.2} timer_ms={:.2} apu_ms={:.2} dma_ms={:.2} serial_ms={:.2} serial_active_tcycles={serial_active_t_cycles:.2} serial_internal_ticks={serial_internal_ticks:.2} serial_external_ticks={serial_external_ticks:.2} serial_wait_external_ticks={serial_external_wait_ticks:.2} serial_shift_edges={serial_shift_edges:.2} serial_completed_bytes={serial_completed_bytes:.2} serial_ext_port_ticks={serial_external_port_ticks:.2} irq_ms={:.2} ppu_mode0_1_ms={:.2} ppu_mode2_ms={:.2} ppu_mode3_startup_ms={:.2} ppu_bg_ms={:.2} ppu_win_ms={:.2} ppu_push_ms={:.2} ppu_obj_ms={:.2} ppu_px_ms={:.2} ppu_bus_ms={:.2} ppu_busstate_ms={:.2} ppu_busview_ms={:.2} ppu_snapshot_ms={:.2} ppu_pub_ms={:.2} ppu_tick_ms={:.2} ppu_mode3_ctrl_ms={:.2} ppu_bg_edge_ms={:.2} ppu_win_edge_ms={:.2} ppu_obj_edge_ms={:.2} ppu_raster_pub_ms={:.2} ppu_mode_ms={:.2} ppu_raster_ms={:.2} ppu_stat_ms={:.2} ppu_visible_ms={:.2} ppu_misc_ms={:.2} ppu_other_ms={:.2} ppu_unbucketed_ms={:.2} ppu_profile_gap_ms={:.2} host_ms={host_ms:.2} poll_ms={:.2} audsubmit_ms={:.2} save_ms={:.2} frame_tcycles={frame_step_t_cycles} scheduler_tcycles={frame_step_t_cycles} video_dots={frame_video_dots} speed_mode={speed_mode} frame_start_ly={frame_start_ly} frame_start_dot={frame_start_dot} frame_end_ly={frame_end_ly} frame_end_dot={frame_end_dot} frame_crossings={frame_origin_crossings} scanline_transitions={scanline_transitions} scanlines_over_456={scanlines_over_456} max_scanline_tcycles={max_scanline_t_cycles} max_scanline_ly={max_scanline_ly} max_mode0_start_dot={max_mode0_start_dot} max_mode0_start_dot_ly={max_mode0_start_dot_ly} ly153_to0={ly_153_to_0_transitions} ly153_to0_startup={ly_153_to_0_startup_mode0} ly153_to0_blank={ly_153_to_0_blank_frame} ly0_self_wraps={ly_0_self_wraps} ly0_self_wrap_startup={ly_0_self_wrap_startup_mode0} ly0_self_wrap_blank={ly_0_self_wrap_blank_frame} ly0_to1={ly_0_to_1_transitions} ly0_tcycles={ly_0_scanline_t_cycles} ly0_max_mode0_start_dot={ly_0_max_mode0_start_dot} ly0_stall_tcycles={ly_0_stall_t_cycles} ly0_stall_hb_tcycles={ly_0_stall_hblank_t_cycles} ly0_stall_oam_tcycles={ly_0_stall_oam_t_cycles} ly0_stall_draw_tcycles={ly_0_stall_drawing_t_cycles} ly0_stall_startup_tcycles={ly_0_stall_startup_mode0_t_cycles} ly0_stall_blank_tcycles={ly_0_stall_blank_frame_t_cycles} ly0_stall_runs={ly_0_stall_runs} ly0_max_stall_tcycles={ly_0_max_stall_run_t_cycles} ly0_max_stall_dot={ly_0_max_stall_dot} ly0_max_stall_mode_dot={ly_0_max_stall_mode_dot} cpu_stop_tcycles={cpu_stop_t_cycles} cpu_zstop_tcycles={cpu_zombie_stop_t_cycles} ly0_stop_tcycles={ly_0_cpu_stop_t_cycles} ly0_zstop_tcycles={ly_0_cpu_zombie_stop_t_cycles} ly0_stall_stop_tcycles={ly_0_stall_cpu_stop_t_cycles} ly0_stall_zstop_tcycles={ly_0_stall_cpu_zombie_stop_t_cycles} lcdoff_tcycles={lcd_disabled_t_cycles} lcdoff_transitions={lcd_disable_transitions} lcdon_transitions={lcd_enable_transitions} ly0_lcdoff_tcycles={ly_0_lcd_disabled_t_cycles} ly0_stall_lcdoff_tcycles={ly_0_stall_lcd_disabled_t_cycles} submit_samples={audio_submit_samples} submit_tcycles={audio_submit_t_cycles} submit_queue_before_ms={audio_submit_queue_before_ms} submit_enqueued_ms={audio_submit_enqueued_ms} submit_queue_after_ms={audio_submit_queue_after_ms} audio_queue_before_ms={audio_queue_before_pacing_ms} audio_queue_after_ms={audio_queue_after_pacing_ms} present_ms={:.2} pac_ms={:.2} sleep_target_ms={:.2} audio_corr_ms={:.2} late_ms={:.2} oversleep_ms={:.2} sample_secs={:.2}",
            self.sample_session_kind.label(),
            snapshot.fps,
            snapshot.speed_percent,
            snapshot.frame_time_ms,
            snapshot.emulation_time_ms,
            self.sample_profiled_frames,
            sample_every_frames,
            scaled_average_duration_ms(
                breakdown.core_ppu_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_cpu_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_other_duration(),
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_external_events_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_timer_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_apu_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_dma_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_serial_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_interrupts_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_ppu_mode0_1_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_ppu_mode2_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_ppu_mode3_startup_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_ppu_bg_fetch_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_ppu_window_fetch_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_ppu_push_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_ppu_obj_fetch_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_ppu_pixel_transfer_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_ppu_bus_sync_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_ppu_bus_state_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_ppu_bus_view_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_ppu_bus_snapshot_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_ppu_published_access_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_ppu_tick_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_ppu_mode3_control_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_ppu_bg_edge_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_ppu_window_edge_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_ppu_obj_edge_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_ppu_raster_publication_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_ppu_mode_timing_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_ppu_raster_advance_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_ppu_stat_irq_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_ppu_visible_prep_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.core_ppu_misc_duration,
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.ppu_other_duration(),
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.ppu_other_duration(),
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            scaled_average_duration_ms(
                breakdown.ppu_other_duration(),
                breakdown.core_duration(),
                estimated_core_duration,
                profiled_frames_f64,
            ),
            average_duration_ms(breakdown.host_event_poll_duration, profiled_frames_f64),
            average_duration_ms(breakdown.host_audio_submit_duration, profiled_frames_f64),
            average_duration_ms(breakdown.host_save_flush_duration, profiled_frames_f64),
            average_duration_ms(self.sample_present_duration, frames_f64),
            average_duration_ms(self.sample_pacing_duration, frames_f64),
            average_duration_ms(self.sample_pacing_sleep_target_duration, frames_f64),
            average_duration_ms(self.sample_pacing_audio_correction_duration, frames_f64),
            average_duration_ms(self.sample_pacing_late_duration, frames_f64),
            average_duration_ms(self.sample_pacing_oversleep_duration, frames_f64),
            elapsed.as_secs_f64(),
        ))
    }

    fn reset_sample(&mut self) {
        self.sample_started_at = Instant::now();
        self.frames_in_sample = 0;
        self.sample_emulation_duration = Duration::ZERO;
        self.sample_profiled_frames = 0;
        self.sample_profiled_emulation_duration = Duration::ZERO;
        self.sample_profiled_emulation_breakdown = EmulationBreakdownSample::default();
        self.sample_render_duration = Duration::ZERO;
        self.sample_present_duration = Duration::ZERO;
        self.sample_pacing_duration = Duration::ZERO;
        self.sample_pacing_sleep_target_duration = Duration::ZERO;
        self.sample_pacing_audio_correction_duration = Duration::ZERO;
        self.sample_pacing_late_duration = Duration::ZERO;
        self.sample_pacing_oversleep_duration = Duration::ZERO;
        self.sample_audio_submit_sample_count = 0;
        self.sample_audio_submit_sample_count_observations = 0;
        self.sample_audio_submit_t_cycles = 0;
        self.sample_audio_submit_t_cycles_observations = 0;
        self.sample_audio_submit_queue_before_ms = 0.0;
        self.sample_audio_submit_queue_before_observations = 0;
        self.sample_audio_submit_enqueued_ms = 0.0;
        self.sample_audio_submit_enqueued_observations = 0;
        self.sample_audio_submit_queue_after_ms = 0.0;
        self.sample_audio_submit_queue_after_observations = 0;
        self.sample_audio_queue_before_pacing_ms = 0.0;
        self.sample_audio_queue_before_pacing_observations = 0;
        self.sample_audio_queue_after_pacing_ms = 0.0;
        self.sample_audio_queue_after_pacing_observations = 0;
        self.sample_speed_mode_normal_frames = 0;
        self.sample_speed_mode_double_frames = 0;
        self.sample_frame_step_t_cycles = 0;
        self.sample_frame_step_t_cycles_observations = 0;
        self.sample_frame_video_dots = 0;
        self.sample_frame_video_dots_observations = 0;
        self.sample_frame_start_ly = 0;
        self.sample_frame_start_ly_observations = 0;
        self.sample_frame_start_dot = 0;
        self.sample_frame_start_dot_observations = 0;
        self.sample_frame_end_ly = 0;
        self.sample_frame_end_ly_observations = 0;
        self.sample_frame_end_dot = 0;
        self.sample_frame_end_dot_observations = 0;
        self.sample_frame_origin_crossings = 0;
        self.sample_frame_origin_crossings_observations = 0;
        self.sample_scanline_transitions = 0;
        self.sample_scanline_transitions_observations = 0;
        self.sample_scanlines_over_456 = 0;
        self.sample_scanlines_over_456_observations = 0;
        self.sample_max_scanline_t_cycles = 0;
        self.sample_max_scanline_t_cycles_observations = 0;
        self.sample_max_scanline_ly = 0;
        self.sample_max_scanline_ly_observations = 0;
        self.sample_max_mode0_start_dot = 0;
        self.sample_max_mode0_start_dot_observations = 0;
        self.sample_max_mode0_start_dot_ly = 0;
        self.sample_max_mode0_start_dot_ly_observations = 0;
        self.sample_ly_153_to_0_transitions = 0;
        self.sample_ly_153_to_0_transitions_observations = 0;
        self.sample_ly_153_to_0_startup_mode0 = 0;
        self.sample_ly_153_to_0_startup_mode0_observations = 0;
        self.sample_ly_153_to_0_blank_frame = 0;
        self.sample_ly_153_to_0_blank_frame_observations = 0;
        self.sample_ly_0_self_wraps = 0;
        self.sample_ly_0_self_wraps_observations = 0;
        self.sample_ly_0_self_wrap_startup_mode0 = 0;
        self.sample_ly_0_self_wrap_startup_mode0_observations = 0;
        self.sample_ly_0_self_wrap_blank_frame = 0;
        self.sample_ly_0_self_wrap_blank_frame_observations = 0;
        self.sample_ly_0_to_1_transitions = 0;
        self.sample_ly_0_to_1_transitions_observations = 0;
        self.sample_ly_0_scanline_t_cycles = 0;
        self.sample_ly_0_scanline_t_cycles_observations = 0;
        self.sample_ly_0_max_mode0_start_dot = 0;
        self.sample_ly_0_max_mode0_start_dot_observations = 0;
        self.sample_ly_0_stall_t_cycles = 0;
        self.sample_ly_0_stall_t_cycles_observations = 0;
        self.sample_ly_0_stall_hblank_t_cycles = 0;
        self.sample_ly_0_stall_hblank_t_cycles_observations = 0;
        self.sample_ly_0_stall_oam_t_cycles = 0;
        self.sample_ly_0_stall_oam_t_cycles_observations = 0;
        self.sample_ly_0_stall_drawing_t_cycles = 0;
        self.sample_ly_0_stall_drawing_t_cycles_observations = 0;
        self.sample_ly_0_stall_startup_mode0_t_cycles = 0;
        self.sample_ly_0_stall_startup_mode0_t_cycles_observations = 0;
        self.sample_ly_0_stall_blank_frame_t_cycles = 0;
        self.sample_ly_0_stall_blank_frame_t_cycles_observations = 0;
        self.sample_ly_0_stall_runs = 0;
        self.sample_ly_0_stall_runs_observations = 0;
        self.sample_ly_0_max_stall_run_t_cycles = 0;
        self.sample_ly_0_max_stall_run_t_cycles_observations = 0;
        self.sample_ly_0_max_stall_dot = 0;
        self.sample_ly_0_max_stall_dot_observations = 0;
        self.sample_ly_0_max_stall_mode_dot = 0;
        self.sample_ly_0_max_stall_mode_dot_observations = 0;
        self.sample_cpu_stop_t_cycles = 0;
        self.sample_cpu_stop_t_cycles_observations = 0;
        self.sample_cpu_zombie_stop_t_cycles = 0;
        self.sample_cpu_zombie_stop_t_cycles_observations = 0;
        self.sample_ly_0_cpu_stop_t_cycles = 0;
        self.sample_ly_0_cpu_stop_t_cycles_observations = 0;
        self.sample_ly_0_cpu_zombie_stop_t_cycles = 0;
        self.sample_ly_0_cpu_zombie_stop_t_cycles_observations = 0;
        self.sample_ly_0_stall_cpu_stop_t_cycles = 0;
        self.sample_ly_0_stall_cpu_stop_t_cycles_observations = 0;
        self.sample_ly_0_stall_cpu_zombie_stop_t_cycles = 0;
        self.sample_ly_0_stall_cpu_zombie_stop_t_cycles_observations = 0;
        self.sample_lcd_disabled_t_cycles = 0;
        self.sample_lcd_disabled_t_cycles_observations = 0;
        self.sample_lcd_disable_transitions = 0;
        self.sample_lcd_disable_transitions_observations = 0;
        self.sample_lcd_enable_transitions = 0;
        self.sample_lcd_enable_transitions_observations = 0;
        self.sample_ly_0_lcd_disabled_t_cycles = 0;
        self.sample_ly_0_lcd_disabled_t_cycles_observations = 0;
        self.sample_ly_0_stall_lcd_disabled_t_cycles = 0;
        self.sample_ly_0_stall_lcd_disabled_t_cycles_observations = 0;
    }

    fn collect_emulation_profile_results(&mut self) {
        let Some(worker) = self.emulation_profile_worker.as_ref() else {
            return;
        };
        worker.collect_completed(&mut |result| {
            self.emulation_profile_request_in_flight = false;
            self.sample_profiled_frames += 1;
            self.sample_profiled_emulation_duration += result.emulation_duration;
            self.sample_profiled_emulation_breakdown
                .accumulate(result.breakdown);
        });
    }

    fn submit_emulation_profile_request(
        &mut self,
        request: Option<EmulationProfileRequest>,
        emulation_duration: Duration,
    ) {
        let Some(request) = request else {
            return;
        };
        let Some(worker) = self.emulation_profile_worker.as_ref() else {
            return;
        };
        self.emulation_profile_request_in_flight =
            worker.try_submit(request.into_work_item(emulation_duration));
    }
}

fn average_duration_ms(duration: Duration, frames_f64: f64) -> f64 {
    duration.as_secs_f64() * 1_000.0 / frames_f64.max(f64::EPSILON)
}

fn average_counter_value(value: u64, frames_f64: f64) -> f64 {
    value as f64 / frames_f64.max(f64::EPSILON)
}

fn scaled_average_duration_ms(
    observed_duration: Duration,
    observed_total: Duration,
    scaled_total: Duration,
    frames_f64: f64,
) -> f64 {
    let observed_total_secs = observed_total.as_secs_f64();
    if observed_total_secs <= f64::EPSILON {
        return 0.0;
    }

    average_duration_ms(observed_duration, frames_f64)
        * (scaled_total.as_secs_f64() / observed_total_secs)
}

fn performance_window_title(base_title: &str, snapshot: PerformanceHudSnapshot) -> String {
    let audio = match snapshot.audio_queue_ms {
        Some(audio_queue_ms) => format!("{audio_queue_ms:.1} ms"),
        None => "off".to_string(),
    };
    format!(
        "{base_title} | {:.1} FPS | {:.2} ms | {:.0}% speed | emu {:.2} | render {:.2} | pacing {:.2} | audio {audio}",
        snapshot.fps,
        snapshot.frame_time_ms,
        snapshot.speed_percent,
        snapshot.emulation_time_ms,
        snapshot.render_time_ms,
        snapshot.pacing_time_ms,
    )
}

fn desktop_gb_master_clock_rate_for_config(config: &DesktopConfig) -> SgbClockRate {
    match config
        .launch
        .console_model
        .sgb_profile_for_standard(config.launch.effective_sgb_video_standard())
    {
        Some(profile) => profile.timing().gb_master_clock_hz,
        None => SgbClockRate::from_hz(DMG_T_CYCLES_PER_SECOND as u32),
    }
}

fn frame_duration_for_gb_master_clock(clock_rate: SgbClockRate) -> Duration {
    let numerator = u128::from(DMG_T_CYCLES_PER_FRAME)
        .saturating_mul(u128::from(clock_rate.denominator))
        .saturating_mul(1_000_000_000);
    let denominator = clock_rate.numerator_hz.max(1) as u128;
    let rounded_nanos = numerator
        .saturating_add(denominator / 2)
        .saturating_div(denominator);
    Duration::from_nanos(u64::try_from(rounded_nanos).unwrap_or(u64::MAX))
}

fn frame_duration_for_config(config: &DesktopConfig) -> Duration {
    frame_duration_for_gb_master_clock(desktop_gb_master_clock_rate_for_config(config))
}

fn target_frame_rate_hz_for_config(config: &DesktopConfig) -> f64 {
    let clock_rate = desktop_gb_master_clock_rate_for_config(config);
    clock_rate.numerator_hz as f64
        / f64::from(clock_rate.denominator)
        / DMG_T_CYCLES_PER_FRAME as f64
}

fn target_frame_rate_hz() -> f64 {
    target_frame_rate_hz_for_config(&DesktopConfig::default())
}
