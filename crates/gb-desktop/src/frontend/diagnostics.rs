impl EmulationProfileMode {
    fn from_env() -> Self {
        Self::from_env_value(env::var_os(DESKTOP_EMU_PROFILE_ENV_VAR).as_deref())
    }

    fn from_env_value(value: Option<&OsStr>) -> Self {
        let Some(value) = value else {
            return Self::Disabled;
        };

        let value = value.to_string_lossy();
        let value = value.trim();
        if value.is_empty()
            || value == "0"
            || value.eq_ignore_ascii_case("false")
            || value.eq_ignore_ascii_case("off")
            || value.eq_ignore_ascii_case("no")
            || value.eq_ignore_ascii_case("disabled")
        {
            Self::Disabled
        } else {
            let normalized = value.trim().to_ascii_lowercase();
            let (detail, sample_every_frames) =
                parse_emulation_profile_detail_and_sample_stride(&normalized);
            Self::SampledSummary {
                sample_every_frames,
                detail,
            }
        }
    }

    fn enabled(self) -> bool {
        !matches!(self, Self::Disabled)
    }

    fn sample_every_frames(self) -> Option<u32> {
        match self {
            Self::Disabled => None,
            Self::SampledSummary {
                sample_every_frames,
                ..
            } => Some(sample_every_frames),
        }
    }

    fn detail(self) -> Option<EmulationProfileDetail> {
        match self {
            Self::Disabled => None,
            Self::SampledSummary { detail, .. } => Some(detail),
        }
    }
}

impl EmulationProfileDetail {
    const fn label(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::CoreOnly => "core",
            Self::Overhead => "overhead",
        }
    }

    const fn records_ppu_regions(self) -> bool {
        matches!(self, Self::Full | Self::Overhead)
    }
}

fn parse_emulation_profile_detail_and_sample_stride(
    normalized: &str,
) -> (EmulationProfileDetail, u32) {
    for (prefix, detail) in [
        ("summary-lite:", EmulationProfileDetail::CoreOnly),
        ("lite:", EmulationProfileDetail::CoreOnly),
        ("summary-overhead:", EmulationProfileDetail::Overhead),
        ("overhead:", EmulationProfileDetail::Overhead),
        ("summary:", EmulationProfileDetail::Full),
        ("sampled:", EmulationProfileDetail::Full),
        ("every:", EmulationProfileDetail::Full),
        ("stride:", EmulationProfileDetail::Full),
    ] {
        if let Some(rest) = normalized.strip_prefix(prefix) {
            let sample_every_frames = rest
                .parse::<u32>()
                .ok()
                .filter(|sample_every| *sample_every > 0)
                .unwrap_or(DEFAULT_EMU_PROFILE_SAMPLE_EVERY_FRAMES);
            return (detail, sample_every_frames);
        }
    }

    let detail = match normalized {
        "summary-lite" | "lite" => EmulationProfileDetail::CoreOnly,
        "summary-overhead" | "overhead" => EmulationProfileDetail::Overhead,
        _ => EmulationProfileDetail::Full,
    };
    (detail, DEFAULT_EMU_PROFILE_SAMPLE_EVERY_FRAMES)
}

impl EmulationBreakdownSample {
    fn add_core_region_duration(&mut self, region: MachineStepRegion, duration: Duration) {
        match region {
            MachineStepRegion::ExternalEvents => self.core_external_events_duration += duration,
            MachineStepRegion::Timer => self.core_timer_duration += duration,
            MachineStepRegion::Apu => self.core_apu_duration += duration,
            MachineStepRegion::Dma => self.core_dma_duration += duration,
            MachineStepRegion::Ppu => self.core_ppu_duration += duration,
            MachineStepRegion::Serial => self.core_serial_duration += duration,
            MachineStepRegion::Cpu => self.core_cpu_duration += duration,
            MachineStepRegion::Interrupts => self.core_interrupts_duration += duration,
        }
    }

    fn add_host_event_poll_duration(&mut self, duration: Duration) {
        self.host_event_poll_duration += duration;
    }

    fn add_host_audio_submit_duration(&mut self, duration: Duration) {
        self.host_audio_submit_duration += duration;
    }

    fn add_ppu_region_duration(&mut self, region: PpuStepRegion, duration: Duration) {
        match region {
            PpuStepRegion::Other => self.core_ppu_misc_duration += duration,
            PpuStepRegion::BusSync => self.core_ppu_bus_sync_duration += duration,
            PpuStepRegion::BusState => self.core_ppu_bus_state_duration += duration,
            PpuStepRegion::BusView => self.core_ppu_bus_view_duration += duration,
            PpuStepRegion::BusSnapshot => self.core_ppu_bus_snapshot_duration += duration,
            PpuStepRegion::PublishedAccess => self.core_ppu_published_access_duration += duration,
            PpuStepRegion::Tick => self.core_ppu_tick_duration += duration,
            PpuStepRegion::ModeTiming => self.core_ppu_mode_timing_duration += duration,
            PpuStepRegion::RasterAdvance => self.core_ppu_raster_advance_duration += duration,
            PpuStepRegion::RasterPublication => {
                self.core_ppu_raster_publication_duration += duration;
            }
            PpuStepRegion::StatIrq => self.core_ppu_stat_irq_duration += duration,
            PpuStepRegion::VisiblePrep => self.core_ppu_visible_prep_duration += duration,
            PpuStepRegion::Mode0Or1 => self.core_ppu_mode0_1_duration += duration,
            PpuStepRegion::Mode2Scan => self.core_ppu_mode2_duration += duration,
            PpuStepRegion::Mode3Control => self.core_ppu_mode3_control_duration += duration,
            PpuStepRegion::Mode3Startup => self.core_ppu_mode3_startup_duration += duration,
            PpuStepRegion::Mode3BgFetch => self.core_ppu_bg_fetch_duration += duration,
            PpuStepRegion::Mode3BgEdge => self.core_ppu_bg_edge_duration += duration,
            PpuStepRegion::Mode3WindowFetch => self.core_ppu_window_fetch_duration += duration,
            PpuStepRegion::Mode3WindowEdge => self.core_ppu_window_edge_duration += duration,
            PpuStepRegion::Mode3Push => self.core_ppu_push_duration += duration,
            PpuStepRegion::Mode3ObjEdge => self.core_ppu_obj_edge_duration += duration,
            PpuStepRegion::Mode3ObjFetch => self.core_ppu_obj_fetch_duration += duration,
            PpuStepRegion::Mode3PixelTransfer => {
                self.core_ppu_pixel_transfer_duration += duration;
            }
        }
    }

    fn add_host_save_flush_duration(&mut self, duration: Duration) {
        self.host_save_flush_duration += duration;
    }

    fn add_serial_telemetry(&mut self, telemetry: SerialTickTelemetry) {
        self.serial_active_t_cycles = self
            .serial_active_t_cycles
            .saturating_add(telemetry.active_t_cycles);
        self.serial_internal_ticks = self
            .serial_internal_ticks
            .saturating_add(telemetry.internal_ticks);
        self.serial_external_ticks = self
            .serial_external_ticks
            .saturating_add(telemetry.external_ticks);
        self.serial_external_wait_ticks = self
            .serial_external_wait_ticks
            .saturating_add(telemetry.external_wait_ticks);
        self.serial_shift_edges = self
            .serial_shift_edges
            .saturating_add(telemetry.shift_edges);
        self.serial_completed_bytes = self
            .serial_completed_bytes
            .saturating_add(telemetry.completed_bytes);
        self.serial_external_port_ticks = self
            .serial_external_port_ticks
            .saturating_add(telemetry.external_port_ticks);
    }

    fn add_profile_replay_durations(
        &mut self,
        base_duration: Duration,
        core_duration: Duration,
        full_duration: Duration,
    ) {
        self.profile_base_duration += base_duration;
        self.profile_core_duration += core_duration;
        self.profile_full_duration += full_duration;
        self.profile_core_overhead_duration += core_duration.saturating_sub(base_duration);
        self.profile_ppu_observer_overhead_duration += full_duration.saturating_sub(core_duration);
    }

    fn accumulate(&mut self, other: Self) {
        self.core_external_events_duration += other.core_external_events_duration;
        self.core_timer_duration += other.core_timer_duration;
        self.core_apu_duration += other.core_apu_duration;
        self.core_dma_duration += other.core_dma_duration;
        self.core_ppu_duration += other.core_ppu_duration;
        self.core_ppu_bus_sync_duration += other.core_ppu_bus_sync_duration;
        self.core_ppu_bus_state_duration += other.core_ppu_bus_state_duration;
        self.core_ppu_bus_view_duration += other.core_ppu_bus_view_duration;
        self.core_ppu_bus_snapshot_duration += other.core_ppu_bus_snapshot_duration;
        self.core_ppu_published_access_duration += other.core_ppu_published_access_duration;
        self.core_ppu_tick_duration += other.core_ppu_tick_duration;
        self.core_ppu_misc_duration += other.core_ppu_misc_duration;
        self.core_ppu_mode_timing_duration += other.core_ppu_mode_timing_duration;
        self.core_ppu_raster_advance_duration += other.core_ppu_raster_advance_duration;
        self.core_ppu_raster_publication_duration += other.core_ppu_raster_publication_duration;
        self.core_ppu_stat_irq_duration += other.core_ppu_stat_irq_duration;
        self.core_ppu_visible_prep_duration += other.core_ppu_visible_prep_duration;
        self.core_ppu_mode0_1_duration += other.core_ppu_mode0_1_duration;
        self.core_ppu_mode2_duration += other.core_ppu_mode2_duration;
        self.core_ppu_mode3_control_duration += other.core_ppu_mode3_control_duration;
        self.core_ppu_mode3_startup_duration += other.core_ppu_mode3_startup_duration;
        self.core_ppu_bg_fetch_duration += other.core_ppu_bg_fetch_duration;
        self.core_ppu_bg_edge_duration += other.core_ppu_bg_edge_duration;
        self.core_ppu_window_fetch_duration += other.core_ppu_window_fetch_duration;
        self.core_ppu_window_edge_duration += other.core_ppu_window_edge_duration;
        self.core_ppu_push_duration += other.core_ppu_push_duration;
        self.core_ppu_obj_edge_duration += other.core_ppu_obj_edge_duration;
        self.core_ppu_obj_fetch_duration += other.core_ppu_obj_fetch_duration;
        self.core_ppu_pixel_transfer_duration += other.core_ppu_pixel_transfer_duration;
        self.core_serial_duration += other.core_serial_duration;
        self.serial_active_t_cycles = self
            .serial_active_t_cycles
            .saturating_add(other.serial_active_t_cycles);
        self.serial_internal_ticks = self
            .serial_internal_ticks
            .saturating_add(other.serial_internal_ticks);
        self.serial_external_ticks = self
            .serial_external_ticks
            .saturating_add(other.serial_external_ticks);
        self.serial_external_wait_ticks = self
            .serial_external_wait_ticks
            .saturating_add(other.serial_external_wait_ticks);
        self.serial_shift_edges = self
            .serial_shift_edges
            .saturating_add(other.serial_shift_edges);
        self.serial_completed_bytes = self
            .serial_completed_bytes
            .saturating_add(other.serial_completed_bytes);
        self.serial_external_port_ticks = self
            .serial_external_port_ticks
            .saturating_add(other.serial_external_port_ticks);
        self.core_cpu_duration += other.core_cpu_duration;
        self.core_interrupts_duration += other.core_interrupts_duration;
        self.host_event_poll_duration += other.host_event_poll_duration;
        self.host_audio_submit_duration += other.host_audio_submit_duration;
        self.host_save_flush_duration += other.host_save_flush_duration;
        self.profile_base_duration += other.profile_base_duration;
        self.profile_core_duration += other.profile_core_duration;
        self.profile_full_duration += other.profile_full_duration;
        self.profile_core_overhead_duration += other.profile_core_overhead_duration;
        self.profile_ppu_observer_overhead_duration += other.profile_ppu_observer_overhead_duration;
    }

    fn core_duration(self) -> Duration {
        self.core_external_events_duration
            + self.core_timer_duration
            + self.core_apu_duration
            + self.core_dma_duration
            + self.core_ppu_duration
            + self.core_serial_duration
            + self.core_cpu_duration
            + self.core_interrupts_duration
    }

    fn host_duration(self) -> Duration {
        self.host_event_poll_duration
            + self.host_audio_submit_duration
            + self.host_save_flush_duration
    }

    fn core_other_duration(self) -> Duration {
        self.core_duration()
            .saturating_sub(self.core_ppu_duration + self.core_cpu_duration)
    }

    fn ppu_profiled_duration(self) -> Duration {
        self.core_ppu_mode0_1_duration
            + self.core_ppu_bus_sync_duration
            + self.core_ppu_bus_state_duration
            + self.core_ppu_bus_view_duration
            + self.core_ppu_bus_snapshot_duration
            + self.core_ppu_published_access_duration
            + self.core_ppu_tick_duration
            + self.core_ppu_misc_duration
            + self.core_ppu_mode_timing_duration
            + self.core_ppu_raster_advance_duration
            + self.core_ppu_raster_publication_duration
            + self.core_ppu_stat_irq_duration
            + self.core_ppu_visible_prep_duration
            + self.core_ppu_mode2_duration
            + self.core_ppu_mode3_control_duration
            + self.core_ppu_mode3_startup_duration
            + self.core_ppu_bg_fetch_duration
            + self.core_ppu_bg_edge_duration
            + self.core_ppu_window_fetch_duration
            + self.core_ppu_window_edge_duration
            + self.core_ppu_push_duration
            + self.core_ppu_obj_edge_duration
            + self.core_ppu_obj_fetch_duration
            + self.core_ppu_pixel_transfer_duration
    }

    fn ppu_other_duration(self) -> Duration {
        self.core_ppu_duration
            .saturating_sub(self.ppu_profiled_duration())
    }
}

impl EmulationProfileRequest {
    #[cfg(test)]
    fn new(machine: DesktopEmulationSession) -> Self {
        Self::new_with_detail(machine, EmulationProfileDetail::Full)
    }

    fn new_with_detail(machine: DesktopEmulationSession, detail: EmulationProfileDetail) -> Self {
        Self {
            machine,
            detail,
            breakdown: EmulationBreakdownSample::default(),
        }
    }

    fn record_host_event_poll_duration(&mut self, duration: Duration) {
        self.breakdown.add_host_event_poll_duration(duration);
    }

    fn record_host_audio_submit_duration(&mut self, duration: Duration) {
        self.breakdown.add_host_audio_submit_duration(duration);
    }

    fn record_host_save_flush_duration(&mut self, duration: Duration) {
        self.breakdown.add_host_save_flush_duration(duration);
    }

    fn into_work_item(self, emulation_duration: Duration) -> EmulationProfileWorkItem {
        EmulationProfileWorkItem {
            machine: self.machine,
            detail: self.detail,
            emulation_duration,
            breakdown: self.breakdown,
        }
    }
}

impl ReplayFrameCoreProfiler {
    fn new(records_ppu_regions: bool) -> Self {
        Self {
            records_ppu_regions,
            ..Default::default()
        }
    }

    fn finish(self) -> EmulationBreakdownSample {
        debug_assert!(self.active_region.is_none());
        debug_assert!(self.active_ppu_region.is_none());
        self.sample
    }
}

impl Default for ReplayFrameCoreProfiler {
    fn default() -> Self {
        Self {
            sample: EmulationBreakdownSample::default(),
            records_ppu_regions: true,
            active_region: None,
            active_ppu_region: None,
        }
    }
}

impl MachineStepObserver for ReplayFrameCoreProfiler {
    fn records_ppu_regions(&self) -> bool {
        self.records_ppu_regions
    }

    fn begin_region(&mut self, region: MachineStepRegion) {
        debug_assert!(self.active_region.is_none());
        self.active_region = Some((region, Instant::now()));
    }

    fn end_region(&mut self, region: MachineStepRegion) {
        let (active_region, started_at) = self
            .active_region
            .take()
            .expect("machine-step profiler region should have started before it ends");
        debug_assert_eq!(active_region, region);
        self.sample
            .add_core_region_duration(active_region, started_at.elapsed());
    }

    fn begin_ppu_region(&mut self, region: PpuStepRegion) {
        debug_assert!(self.active_ppu_region.is_none());
        self.active_ppu_region = Some((region, Instant::now()));
    }

    fn end_ppu_region(&mut self, region: PpuStepRegion) {
        let (active_region, started_at) = self
            .active_ppu_region
            .take()
            .expect("ppu-step profiler region should have started before it ends");
        debug_assert_eq!(active_region, region);
        self.sample
            .add_ppu_region_duration(active_region, started_at.elapsed());
    }

    fn record_serial_tick(&mut self, telemetry: SerialTickTelemetry) {
        self.sample.add_serial_telemetry(telemetry);
    }
}

impl AsyncEmulationProfileWorker {
    fn new() -> Self {
        let (request_sender, request_receiver) = mpsc::sync_channel(1);
        let (result_sender, result_receiver) = mpsc::channel();
        let worker_handle = thread::spawn(move || {
            while let Ok(work_item) = request_receiver.recv() {
                let result = profile_emulation_work_item(work_item);
                if result_sender.send(result).is_err() {
                    break;
                }
            }
        });
        Self {
            request_sender: Some(request_sender),
            result_receiver,
            worker_handle: Some(worker_handle),
        }
    }

    fn try_submit(&self, work_item: EmulationProfileWorkItem) -> bool {
        self.request_sender
            .as_ref()
            .expect("emulation profile worker sender should exist while the worker is alive")
            .try_send(work_item)
            .is_ok()
    }

    fn collect_completed(&self, completed: &mut impl FnMut(CompletedEmulationProfileSample)) {
        while let Ok(result) = self.result_receiver.try_recv() {
            completed(result);
        }
    }
}

impl Drop for AsyncEmulationProfileWorker {
    fn drop(&mut self) {
        self.request_sender.take();
        if let Some(worker_handle) = self.worker_handle.take() {
            let _ = worker_handle.join();
        }
    }
}

fn profile_emulation_work_item(
    mut work_item: EmulationProfileWorkItem,
) -> CompletedEmulationProfileSample {
    match work_item.detail {
        EmulationProfileDetail::Full | EmulationProfileDetail::CoreOnly => {
            let mut profiler = ReplayFrameCoreProfiler::new(work_item.detail.records_ppu_regions());
            step_profile_replay_frame_with_observer(&mut work_item.machine, &mut profiler);
            work_item.breakdown.accumulate(profiler.finish());
        }
        EmulationProfileDetail::Overhead => {
            let starting_machine = work_item.machine;
            let mut base_machine = starting_machine.clone();
            let mut core_machine = starting_machine.clone();
            let mut full_machine = starting_machine;

            let base_started_at = Instant::now();
            step_profile_replay_frame_unobserved(&mut base_machine);
            let base_duration = base_started_at.elapsed();

            let mut core_profiler = ReplayFrameCoreProfiler::new(false);
            let core_started_at = Instant::now();
            step_profile_replay_frame_with_observer(&mut core_machine, &mut core_profiler);
            let core_duration = core_started_at.elapsed();

            let mut full_profiler = ReplayFrameCoreProfiler::new(true);
            let full_started_at = Instant::now();
            step_profile_replay_frame_with_observer(&mut full_machine, &mut full_profiler);
            let full_duration = full_started_at.elapsed();

            debug_assert_profile_replay_equivalent(&base_machine, &core_machine);
            debug_assert_profile_replay_equivalent(&base_machine, &full_machine);

            work_item.breakdown.add_profile_replay_durations(
                base_duration,
                core_duration,
                full_duration,
            );
            work_item.breakdown.accumulate(full_profiler.finish());
        }
    }

    CompletedEmulationProfileSample {
        emulation_duration: work_item.emulation_duration,
        breakdown: work_item.breakdown,
    }
}

fn step_profile_replay_frame_unobserved(machine: &mut DesktopEmulationSession) {
    step_profile_replay_frame(machine, |machine| machine.step_t_cycle());
}

fn step_profile_replay_frame_with_observer(
    machine: &mut DesktopEmulationSession,
    observer: &mut impl MachineStepObserver,
) {
    step_profile_replay_frame(machine, |machine| {
        machine.step_t_cycle_with_observer(observer);
    });
}

fn step_profile_replay_frame(
    machine: &mut DesktopEmulationSession,
    mut step_t_cycle: impl FnMut(&mut DesktopEmulationSession),
) {
    let mut at_frame_origin = machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0;

    loop {
        step_t_cycle(machine);
        let now_at_frame_origin = machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0;
        if now_at_frame_origin && !at_frame_origin {
            break;
        }
        at_frame_origin = now_at_frame_origin;
    }
}

#[cfg(debug_assertions)]
fn debug_assert_profile_replay_equivalent(
    expected: &DesktopEmulationSession,
    actual: &DesktopEmulationSession,
) {
    for slot in PlayerSlot::ALL {
        match (
            expected.machine_for_player_slot(slot),
            actual.machine_for_player_slot(slot),
        ) {
            (Some(expected), Some(actual)) => {
                debug_assert_eq!(expected.snapshot(), actual.snapshot())
            }
            (None, None) => {}
            _ => debug_assert!(
                false,
                "profile replay paths should preserve the same linked session shape"
            ),
        }
    }
}

#[cfg(not(debug_assertions))]
fn debug_assert_profile_replay_equivalent(
    _expected: &DesktopEmulationSession,
    _actual: &DesktopEmulationSession,
) {
}

impl DesktopTraceCapture {
    fn from_env() -> Result<Self, String> {
        let output_path = env::var_os(DESKTOP_TRACE_PATH_ENV_VAR).map(PathBuf::from);
        let max_t_cycles = if output_path.is_some() {
            parse_trace_capture_t_cycles(env::var_os(DESKTOP_TRACE_T_CYCLES_ENV_VAR).as_deref())?
        } else {
            DEFAULT_TRACE_CAPTURE_T_CYCLES
        };
        Ok(Self {
            enabled: output_path.is_some() && max_t_cycles > 0,
            output_path,
            max_t_cycles,
            records: VecDeque::new(),
        })
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn record_t_cycle(&mut self, machine: &Machine<TraceSummaryBuffer>) {
        debug_assert!(self.enabled);

        if self.records.len() == self.max_t_cycles {
            self.records.pop_front();
        }
        self.records.push_back(DesktopTraceRecord {
            t_cycle: machine.next_t_cycle().get().saturating_sub(1),
            cpu: machine.cpu().snapshot(),
            apu: machine.apu().snapshot(),
            interrupts: machine.interrupts().snapshot(),
            joypad: machine.joypad().snapshot(),
            cartridge_trace: machine.cartridge().trace_summary(),
        });
    }

    fn write_artifact(&self) -> Result<(), String> {
        let Some(path) = self.output_path.as_ref() else {
            return Ok(());
        };

        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(|error| {
                format!("failed to create desktop trace artifact directory {parent:?}: {error}")
            })?;
        }

        let mut rendered = String::new();
        for record in &self.records {
            rendered.push_str(&render_desktop_trace_record(record));
            rendered.push('\n');
        }
        fs::write(path, rendered)
            .map_err(|error| format!("failed to write desktop trace artifact {path:?}: {error}"))
    }
}

impl DesktopWatchTraceCapture {
    fn from_env() -> Result<Self, String> {
        let output_path = env::var_os(DESKTOP_WATCH_TRACE_PATH_ENV_VAR).map(PathBuf::from);
        let watched_addresses = if output_path.is_some() {
            parse_watch_trace_addresses(
                env::var_os(DESKTOP_WATCH_TRACE_ADDRESSES_ENV_VAR).as_deref(),
            )?
        } else {
            BTreeSet::new()
        };
        let max_records = if output_path.is_some() {
            parse_watch_trace_event_count(
                env::var_os(DESKTOP_WATCH_TRACE_EVENTS_ENV_VAR).as_deref(),
            )?
        } else {
            DEFAULT_WATCH_TRACE_EVENTS
        };
        Ok(Self {
            output_path,
            watched_addresses,
            max_records,
            records: VecDeque::new(),
        })
    }

    fn is_enabled(&self) -> bool {
        self.output_path.is_some()
    }

    fn record_t_cycle(&mut self, machine: &Machine<TraceSummaryBuffer>) {
        if !self.is_enabled() {
            return;
        }

        let cpu = machine.cpu().snapshot();
        let matched_addresses = watched_cpu_addresses(&cpu, &self.watched_addresses);
        if matched_addresses.is_empty() {
            return;
        }

        if self.records.len() == self.max_records {
            self.records.pop_front();
        }

        self.records.push_back(DesktopWatchTraceRecord {
            t_cycle: machine.next_t_cycle().get().saturating_sub(1),
            matched_addresses,
            cpu,
            interrupts: machine.interrupts().snapshot(),
            joypad: machine.joypad().snapshot(),
            ppu_mode: machine.ppu().access_mode(),
            ppu_ly: machine.ppu().ly(),
            ppu_line_dot: machine.ppu().line_dot(),
            cartridge_trace: machine.cartridge().trace_summary(),
        });
    }

    fn write_artifact(&self) -> Result<(), String> {
        let Some(path) = self.output_path.as_ref() else {
            return Ok(());
        };

        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create desktop watch trace artifact directory {parent:?}: {error}"
                )
            })?;
        }

        let mut rendered = String::new();
        for record in &self.records {
            rendered.push_str(&render_desktop_watch_trace_record(record));
            rendered.push('\n');
        }
        fs::write(path, rendered).map_err(|error| {
            format!("failed to write desktop watch trace artifact {path:?}: {error}")
        })
    }
}

impl PcWatchRange {
    fn new(start: u16, end: u16) -> Result<Self, String> {
        if end < start {
            return Err(format!(
                "{DESKTOP_PC_WATCH_TRACE_RANGES_ENV_VAR} range end {end:#06X} is below start {start:#06X}"
            ));
        }
        Ok(Self { start, end })
    }

    fn contains(self, pc: u16) -> bool {
        self.start <= pc && pc <= self.end
    }
}

impl DesktopPcWatchTraceCapture {
    fn from_env() -> Result<Self, String> {
        let output_path = env::var_os(DESKTOP_PC_WATCH_TRACE_PATH_ENV_VAR).map(PathBuf::from);
        let watched_ranges = if output_path.is_some() {
            parse_pc_watch_trace_ranges(
                env::var_os(DESKTOP_PC_WATCH_TRACE_RANGES_ENV_VAR).as_deref(),
            )?
        } else {
            Vec::new()
        };
        let max_records = if output_path.is_some() {
            parse_pc_watch_trace_event_count(
                env::var_os(DESKTOP_PC_WATCH_TRACE_EVENTS_ENV_VAR).as_deref(),
            )?
        } else {
            DEFAULT_PC_WATCH_TRACE_EVENTS
        };
        Ok(Self {
            output_path,
            watched_ranges,
            max_records,
            records: VecDeque::new(),
        })
    }

    fn is_enabled(&self) -> bool {
        self.output_path.is_some()
    }

    fn record_t_cycle(&mut self, machine: &Machine<TraceSummaryBuffer>) {
        if !self.is_enabled() {
            return;
        }

        let cpu = machine.cpu().snapshot();
        let matched_ranges = watched_pc_ranges(&cpu, &self.watched_ranges);
        if matched_ranges.is_empty() {
            return;
        }

        if self.records.len() == self.max_records {
            self.records.pop_front();
        }

        self.records.push_back(DesktopPcWatchTraceRecord {
            t_cycle: machine.next_t_cycle().get().saturating_sub(1),
            matched_ranges,
            cpu,
            interrupts: machine.interrupts().snapshot(),
            joypad: machine.joypad().snapshot(),
            ppu_mode: machine.ppu().access_mode(),
            ppu_ly: machine.ppu().ly(),
            ppu_line_dot: machine.ppu().line_dot(),
            cartridge_trace: machine.cartridge().trace_summary(),
        });
    }

    fn write_artifact(&self) -> Result<(), String> {
        let Some(path) = self.output_path.as_ref() else {
            return Ok(());
        };

        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create desktop PC watch trace artifact directory {parent:?}: {error}"
                )
            })?;
        }

        let mut rendered = String::new();
        for record in &self.records {
            rendered.push_str(&render_desktop_pc_watch_trace_record(record));
            rendered.push('\n');
        }
        fs::write(path, rendered).map_err(|error| {
            format!("failed to write desktop PC watch trace artifact {path:?}: {error}")
        })
    }
}

impl DesktopEdgeTraceCapture {
    fn from_env() -> Result<Self, String> {
        let output_path = env::var_os(DESKTOP_EDGE_TRACE_PATH_ENV_VAR).map(PathBuf::from);
        let watched_addresses = if output_path.is_some() {
            parse_edge_trace_addresses(
                env::var_os(DESKTOP_EDGE_TRACE_ADDRESSES_ENV_VAR).as_deref(),
            )?
        } else {
            BTreeSet::new()
        };
        let watched_pc_ranges = if output_path.is_some() {
            parse_edge_trace_pc_ranges(
                env::var_os(DESKTOP_EDGE_TRACE_PC_RANGES_ENV_VAR).as_deref(),
            )?
        } else {
            Vec::new()
        };
        let max_records = if output_path.is_some() {
            parse_edge_trace_event_count(env::var_os(DESKTOP_EDGE_TRACE_EVENTS_ENV_VAR).as_deref())?
        } else {
            DEFAULT_EDGE_TRACE_EVENTS
        };
        if output_path.is_some() && watched_addresses.is_empty() && watched_pc_ranges.is_empty() {
            return Err(format!(
                "{DESKTOP_EDGE_TRACE_PATH_ENV_VAR} requires at least one watched address or PC range via {DESKTOP_EDGE_TRACE_ADDRESSES_ENV_VAR} / {DESKTOP_EDGE_TRACE_PC_RANGES_ENV_VAR}"
            ));
        }
        Ok(Self {
            output_path,
            watched_addresses,
            watched_pc_ranges,
            active_pc_ranges: BTreeSet::new(),
            last_observed_values: BTreeMap::new(),
            max_records,
            records: VecDeque::new(),
        })
    }

    fn is_enabled(&self) -> bool {
        self.output_path.is_some()
    }

    fn record_t_cycle(&mut self, machine: &Machine<TraceSummaryBuffer>) {
        if !self.is_enabled() {
            return;
        }

        let cpu = machine.cpu().snapshot();
        let current_pc_ranges = watched_pc_ranges(&cpu, &self.watched_pc_ranges);
        let mut triggers = Vec::new();
        triggers.extend(
            entered_pc_ranges(&current_pc_ranges, &self.active_pc_ranges)
                .into_iter()
                .map(DesktopEdgeTraceTrigger::EnteredPcRange),
        );
        if let Some(trigger) = watched_bus_value_change(
            cpu.last_bus_activity,
            &self.watched_addresses,
            &self.last_observed_values,
        ) {
            triggers.push(trigger);
        }
        self.active_pc_ranges = current_pc_ranges.iter().copied().collect();
        if let Some(activity) = cpu.last_bus_activity
            && self.watched_addresses.contains(&activity.address)
        {
            self.last_observed_values
                .insert(activity.address, activity.value);
        }
        if triggers.is_empty() {
            return;
        }

        if self.records.len() == self.max_records {
            self.records.pop_front();
        }

        self.records.push_back(DesktopEdgeTraceRecord {
            t_cycle: machine.next_t_cycle().get().saturating_sub(1),
            current_pc_ranges,
            triggers,
            cpu,
            interrupts: machine.interrupts().snapshot(),
            joypad: machine.joypad().snapshot(),
            ppu_mode: machine.ppu().access_mode(),
            ppu_ly: machine.ppu().ly(),
            ppu_line_dot: machine.ppu().line_dot(),
            cartridge_trace: machine.cartridge().trace_summary(),
        });
    }

    fn write_artifact(&self) -> Result<(), String> {
        let Some(path) = self.output_path.as_ref() else {
            return Ok(());
        };

        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create desktop edge trace artifact directory {parent:?}: {error}"
                )
            })?;
        }

        let mut rendered = String::new();
        for record in &self.records {
            rendered.push_str(&render_desktop_edge_trace_record(record));
            rendered.push('\n');
        }
        fs::write(path, rendered).map_err(|error| {
            format!("failed to write desktop edge trace artifact {path:?}: {error}")
        })
    }
}

impl DesktopCgbIrTraceCapture {
    fn from_env() -> Result<Self, String> {
        let output_path = env::var_os(DESKTOP_CGB_IR_TRACE_PATH_ENV_VAR).map(PathBuf::from);
        let watched_addresses = if output_path.is_some() {
            parse_cgb_ir_trace_watch_addresses(
                env::var_os(DESKTOP_CGB_IR_TRACE_WATCH_ADDRESSES_ENV_VAR).as_deref(),
            )?
        } else {
            BTreeSet::new()
        };
        let watched_trigger_addresses = if output_path.is_some() {
            match env::var_os(DESKTOP_CGB_IR_TRACE_TRIGGER_ADDRESSES_ENV_VAR) {
                Some(value) => parse_cgb_ir_trace_trigger_addresses(Some(value.as_os_str()))?,
                None => watched_addresses.clone(),
            }
        } else {
            BTreeSet::new()
        };
        let max_records = if output_path.is_some() {
            parse_cgb_ir_trace_event_count(
                env::var_os(DESKTOP_CGB_IR_TRACE_EVENTS_ENV_VAR).as_deref(),
            )?
        } else {
            DEFAULT_CGB_IR_TRACE_EVENTS
        };
        Ok(Self {
            output_path,
            watched_addresses,
            watched_trigger_addresses,
            max_records,
            records: VecDeque::new(),
            last_p1_status: None,
            last_p2_status: None,
            last_p1_pressed_mask: None,
            last_p2_pressed_mask: None,
        })
    }

    fn is_enabled(&self) -> bool {
        self.output_path.is_some()
    }

    fn record_t_cycle(&mut self, machine: &DesktopEmulationSession) {
        if !self.is_enabled() {
            return;
        }
        if !machine.is_linked_cgb_infrared_two_player() {
            self.reset_observed_state();
            return;
        }

        let Some(p1) = cgb_ir_trace_participant(machine, PlayerSlot::P1, &self.watched_addresses)
        else {
            return;
        };
        let Some(p2) = cgb_ir_trace_participant(machine, PlayerSlot::P2, &self.watched_addresses)
        else {
            return;
        };

        let mut triggers = Vec::new();
        collect_cgb_ir_status_trigger(
            &mut triggers,
            PlayerSlot::P1,
            self.last_p1_status,
            p1.status,
        );
        collect_cgb_ir_status_trigger(
            &mut triggers,
            PlayerSlot::P2,
            self.last_p2_status,
            p2.status,
        );
        collect_cgb_ir_rp_bus_trigger(&mut triggers, PlayerSlot::P1, p1.cpu.last_bus_activity);
        collect_cgb_ir_rp_bus_trigger(&mut triggers, PlayerSlot::P2, p2.cpu.last_bus_activity);
        collect_cgb_ir_watched_bus_trigger(
            &mut triggers,
            PlayerSlot::P1,
            p1.cpu.last_bus_activity,
            &self.watched_trigger_addresses,
        );
        collect_cgb_ir_watched_bus_trigger(
            &mut triggers,
            PlayerSlot::P2,
            p2.cpu.last_bus_activity,
            &self.watched_trigger_addresses,
        );
        collect_cgb_ir_joypad_trigger(
            &mut triggers,
            PlayerSlot::P1,
            self.last_p1_pressed_mask,
            p1.joypad.pressed_mask,
        );
        collect_cgb_ir_joypad_trigger(
            &mut triggers,
            PlayerSlot::P2,
            self.last_p2_pressed_mask,
            p2.joypad.pressed_mask,
        );

        self.last_p1_status = Some(p1.status);
        self.last_p2_status = Some(p2.status);
        self.last_p1_pressed_mask = Some(p1.joypad.pressed_mask);
        self.last_p2_pressed_mask = Some(p2.joypad.pressed_mask);

        if triggers.is_empty() {
            return;
        }
        if self.records.len() == self.max_records {
            self.records.pop_front();
        }
        self.records.push_back(DesktopCgbIrTraceRecord {
            t_cycle: machine
                .primary_machine()
                .next_t_cycle()
                .get()
                .saturating_sub(1),
            triggers,
            p1,
            p2,
        });
    }

    fn reset_observed_state(&mut self) {
        self.last_p1_status = None;
        self.last_p2_status = None;
        self.last_p1_pressed_mask = None;
        self.last_p2_pressed_mask = None;
    }

    fn write_artifact(&self) -> Result<(), String> {
        let Some(path) = self.output_path.as_ref() else {
            return Ok(());
        };

        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create desktop CGB IR trace artifact directory {parent:?}: {error}"
                )
            })?;
        }

        let mut rendered = String::new();
        for record in &self.records {
            rendered.push_str(&render_desktop_cgb_ir_trace_record(record));
            rendered.push('\n');
        }
        fs::write(path, rendered).map_err(|error| {
            format!("failed to write desktop CGB IR trace artifact {path:?}: {error}")
        })
    }
}

fn cgb_ir_trace_participant(
    machine: &DesktopEmulationSession,
    slot: PlayerSlot,
    watched_addresses: &BTreeSet<u16>,
) -> Option<DesktopCgbIrTraceParticipantRecord> {
    let participant = machine.machine_for_player_slot(slot)?;
    let cpu = participant.cpu().snapshot();
    let rom_window = participant.cartridge().mapped_rom_window(cpu.registers.pc);
    let watched_values = cgb_ir_trace_watched_values(participant, watched_addresses);
    Some(DesktopCgbIrTraceParticipantRecord {
        status: participant.cgb_infrared_status()?,
        cpu,
        joypad: participant.joypad().snapshot(),
        rom_window,
        watched_values,
    })
}

fn cgb_ir_trace_watched_values(
    machine: &Machine<TraceSummaryBuffer>,
    watched_addresses: &BTreeSet<u16>,
) -> Vec<DesktopCgbIrTraceWatchedValue> {
    watched_addresses
        .iter()
        .copied()
        .map(|address| cgb_ir_trace_watched_value(machine, address))
        .collect()
}

fn cgb_ir_trace_watched_value(
    machine: &Machine<TraceSummaryBuffer>,
    address: u16,
) -> DesktopCgbIrTraceWatchedValue {
    if let Some(sample) = machine.debug_wram_address_sample(address) {
        return DesktopCgbIrTraceWatchedValue::Wram(sample);
    }

    if let Some(offset) = cgb_ir_trace_hram_offset(address) {
        return DesktopCgbIrTraceWatchedValue::Hram {
            address,
            offset,
            value: machine.debug_hram_bytes()[usize::from(offset)],
        };
    }

    DesktopCgbIrTraceWatchedValue::Unsupported { address }
}

fn cgb_ir_trace_hram_offset(address: u16) -> Option<u8> {
    (0xFF80..=0xFFFE)
        .contains(&address)
        .then(|| (address - 0xFF80) as u8)
}

fn collect_cgb_ir_status_trigger(
    triggers: &mut Vec<DesktopCgbIrTraceTrigger>,
    slot: PlayerSlot,
    previous: Option<CgbInfraredStatus>,
    current: CgbInfraredStatus,
) {
    if previous.map(cgb_ir_trace_status_key) != Some(cgb_ir_trace_status_key(current)) {
        triggers.push(DesktopCgbIrTraceTrigger::StatusChanged {
            slot,
            previous,
            current,
        });
    }
}

fn cgb_ir_trace_status_key(status: CgbInfraredStatus) -> DesktopCgbIrTraceStatusKey {
    DesktopCgbIrTraceStatusKey {
        rp_latch: status.rp_latch,
        emitter_on: status.emitter_on,
        read_enabled: status.read_enabled,
        external_optical_input: status.external_optical_input,
        optical_input_active: status.optical_input_active,
        sensor_warmed: status.sensor_warmed,
        effective_signal_detected: status.effective_signal_detected,
        signal_visible_to_rp: status.signal_visible_to_rp,
        receive_ready: status.receive_ready(),
    }
}

fn collect_cgb_ir_rp_bus_trigger(
    triggers: &mut Vec<DesktopCgbIrTraceTrigger>,
    slot: PlayerSlot,
    activity: Option<CpuBusActivitySnapshot>,
) {
    if let Some(activity) = activity
        && activity.address == CGB_RP_ADDRESS
    {
        triggers.push(DesktopCgbIrTraceTrigger::RpBusActivity { slot, activity });
    }
}

fn collect_cgb_ir_watched_bus_trigger(
    triggers: &mut Vec<DesktopCgbIrTraceTrigger>,
    slot: PlayerSlot,
    activity: Option<CpuBusActivitySnapshot>,
    watched_addresses: &BTreeSet<u16>,
) {
    if let Some(activity) = activity
        && watched_addresses.contains(&activity.address)
    {
        triggers.push(DesktopCgbIrTraceTrigger::WatchedBusActivity { slot, activity });
    }
}

fn collect_cgb_ir_joypad_trigger(
    triggers: &mut Vec<DesktopCgbIrTraceTrigger>,
    slot: PlayerSlot,
    previous: Option<u8>,
    current: u8,
) {
    if previous != Some(current) {
        triggers.push(DesktopCgbIrTraceTrigger::JoypadPressedMaskChanged {
            slot,
            previous,
            current,
        });
    }
}

impl DesktopCh4Nr43TraceCapture {
    fn from_env() -> Result<Self, String> {
        Ok(Self {
            output_path: env::var_os(DESKTOP_CH4_NR43_TRACE_PATH_ENV_VAR).map(PathBuf::from),
            records: Vec::new(),
        })
    }

    fn is_enabled(&self) -> bool {
        self.output_path.is_some()
    }

    fn record_t_cycle(&mut self, machine: &Machine<TraceSummaryBuffer>) {
        if !self.is_enabled() {
            return;
        }

        let Some(apu_write) = machine
            .apu()
            .last_register_write()
            .filter(|observation| observation.address == CH4_NR43_ADDRESS)
            .cloned()
        else {
            return;
        };

        self.records.push(DesktopCh4Nr43TraceRecord {
            t_cycle: machine.next_t_cycle().get().saturating_sub(1),
            cpu: machine.cpu().snapshot(),
            apu_write,
            ch4: machine.apu().channel_4_debug_snapshot(),
        });
    }

    fn write_artifact(&self) -> Result<(), String> {
        let Some(path) = self.output_path.as_ref() else {
            return Ok(());
        };

        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create condensed CH4 NR43 trace artifact directory {parent:?}: {error}"
                )
            })?;
        }

        let mut rendered = String::new();
        for record in &self.records {
            rendered.push_str(&render_desktop_ch4_nr43_trace_record(record));
            rendered.push('\n');
        }
        fs::write(path, rendered).map_err(|error| {
            format!("failed to write condensed CH4 NR43 trace artifact {path:?}: {error}")
        })
    }
}

impl DesktopCh4StartupTraceCapture {
    fn from_env() -> Result<Self, String> {
        Ok(Self {
            output_path: env::var_os(DESKTOP_CH4_STARTUP_TRACE_PATH_ENV_VAR).map(PathBuf::from),
            records: Vec::new(),
            last_ch4: None,
        })
    }

    fn is_enabled(&self) -> bool {
        self.output_path.is_some()
    }

    fn record_t_cycle(&mut self, machine: &Machine<TraceSummaryBuffer>) {
        if !self.is_enabled() {
            return;
        }

        let current_ch4 = machine.apu().channel_4_debug_snapshot();
        let t_cycle = machine.next_t_cycle().get().saturating_sub(1);
        let cpu = machine.cpu().snapshot();

        if let Some(apu_write) = machine.apu().last_register_write().filter(|observation| {
            matches!(
                observation.address,
                MASTER_NR52_ADDRESS | CH4_NR42_ADDRESS | CH4_NR43_ADDRESS | CH4_NR44_ADDRESS
            )
        }) {
            self.records.push(DesktopCh4StartupTraceRecord {
                event: DesktopCh4StartupTraceEventKind::RegisterWrite,
                t_cycle,
                cpu: cpu.clone(),
                apu_write: Some(apu_write.clone()),
                ch4: current_ch4,
            });
        }

        if let Some(previous_ch4) = self.last_ch4
            && previous_ch4.dmg_delayed_start != 0
            && current_ch4.dmg_delayed_start == 0
        {
            self.records.push(DesktopCh4StartupTraceRecord {
                event: DesktopCh4StartupTraceEventKind::DelayedStartFired,
                t_cycle,
                cpu,
                apu_write: None,
                ch4: current_ch4,
            });
        }

        self.last_ch4 = Some(current_ch4);
    }

    fn write_artifact(&self) -> Result<(), String> {
        let Some(path) = self.output_path.as_ref() else {
            return Ok(());
        };

        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(|error| {
                format!("failed to create CH4 startup trace artifact directory {parent:?}: {error}")
            })?;
        }

        let mut rendered = String::new();
        for record in &self.records {
            rendered.push_str(&render_desktop_ch4_startup_trace_record(record));
            rendered.push('\n');
        }
        fs::write(path, rendered).map_err(|error| {
            format!("failed to write CH4 startup trace artifact {path:?}: {error}")
        })
    }
}

impl DesktopCpuWindowTraceCapture {
    fn from_env() -> Self {
        Self {
            output_path: env::var_os(DESKTOP_CPU_WINDOW_TRACE_PATH_ENV_VAR).map(PathBuf::from),
            records: Vec::new(),
            active: false,
            finished: false,
        }
    }

    fn is_enabled(&self) -> bool {
        self.output_path.is_some() && !self.finished
    }

    fn record_t_cycle(&mut self, machine: &Machine<TraceSummaryBuffer>) {
        if !self.is_enabled() {
            return;
        }

        let cpu = machine.cpu().snapshot();
        if !matches!(
            cpu.execution_state,
            CpuExecutionState::FetchOpcode { t_cycle: 0 }
        ) {
            return;
        }

        let pc = cpu.registers.pc;
        if !self.active {
            if pc != CPU_WINDOW_TRACE_START_PC {
                return;
            }
            self.active = true;
        }

        self.records.push(DesktopCpuWindowTraceRecord {
            t_cycle: machine.next_t_cycle().get().saturating_sub(1),
            cpu,
            interrupts: machine.interrupts().snapshot(),
            ppu: machine.ppu().snapshot(),
            ppu_ly_read: machine.ppu().read_register(0xFF44),
            ppu_stat_read: machine.ppu().read_register(0xFF41),
        });

        if pc == CPU_WINDOW_TRACE_END_PC {
            self.finished = true;
        }
    }

    fn write_artifact(&self) -> Result<(), String> {
        let Some(path) = self.output_path.as_ref() else {
            return Ok(());
        };

        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(|error| {
                format!("failed to create CPU window trace artifact directory {parent:?}: {error}")
            })?;
        }

        let mut rendered = String::new();
        for record in &self.records {
            rendered.push_str(&render_desktop_cpu_window_trace_record(record));
            rendered.push('\n');
        }
        fs::write(path, rendered)
            .map_err(|error| format!("failed to write CPU window trace artifact {path:?}: {error}"))
    }
}

impl DesktopTraceServiceFlags {
    fn from_runtime(runtime: &FrontendRuntime) -> Self {
        Self {
            trace_capture: runtime.trace_capture.is_enabled(),
            watch_trace: runtime.watch_trace.is_enabled(),
            pc_watch_trace: runtime.pc_watch_trace.is_enabled(),
            edge_trace: runtime.edge_trace.is_enabled(),
            cgb_ir_trace: runtime.cgb_ir_trace.is_enabled(),
            ch4_nr43_trace: runtime.ch4_nr43_trace.is_enabled(),
            ch4_startup_trace: runtime.ch4_startup_trace.is_enabled(),
            cpu_window_trace: runtime.cpu_window_trace.is_enabled(),
        }
    }

    const fn any(self) -> bool {
        self.trace_capture
            || self.watch_trace
            || self.pc_watch_trace
            || self.edge_trace
            || self.cgb_ir_trace
            || self.ch4_nr43_trace
            || self.ch4_startup_trace
            || self.cpu_window_trace
    }
}

impl DesktopTcycleHostServices {
    fn from_runtime_state(
        session: &DesktopSession,
        machine: &DesktopEmulationSession,
        runtime: &FrontendRuntime,
    ) -> Self {
        Self {
            capture_audio: runtime.audio_output.is_some() || runtime.audio_recorder.is_some(),
            sync_gamepad_rumble: gamepad_rumble_sync_needed(runtime, machine),
            record_rewind: desktop_rewind_recording_active(session, machine, runtime),
            drain_printer: session.external_port_selection == DesktopExternalPortSelection::Printer,
            traces: DesktopTraceServiceFlags::from_runtime(runtime),
        }
    }
}

fn render_desktop_trace_record(record: &DesktopTraceRecord) -> String {
    format!(
        "t_cycle={} cpu.pc={:#06X} cpu.execution_state={:?} cpu.current_opcode={:?} cpu.ime={} cpu.delayed_ime_enable={} cpu.last_bus_activity={} cpu.last_address_event={} apu.powered={} apu.nr50={:#04X} apu.nr51={:#04X} apu.nr52={:#04X} apu.div_apu={} apu.active_mask={:#04X} apu.dac_mask={:#04X} apu.channel_outputs=[{:#04X},{:#04X},{:#04X},{:#04X}] apu.mixer=({}, {}) apu.hpf=({}, {}) irq.if={:#04X} irq.ie={:#04X} joypad.p1={:#04X} joypad.selection_bits={:#04X} joypad.pressed_mask={:#04X} {}{}",
        record.t_cycle,
        record.cpu.registers.pc,
        record.cpu.execution_state,
        record.cpu.current_opcode,
        record.cpu.ime,
        record.cpu.delayed_ime_enable,
        format_cpu_bus_activity(record.cpu.last_bus_activity),
        format_cpu_address_event(record.cpu.last_address_event),
        record.apu.powered,
        record.apu.nr50,
        record.apu.nr51,
        visible_nr52(record.apu.powered, record.apu.channel_active_mask),
        record.apu.div_apu,
        record.apu.channel_active_mask,
        record.apu.channel_dac_mask,
        record.apu.output.channel_digital_outputs[0],
        record.apu.output.channel_digital_outputs[1],
        record.apu.output.channel_digital_outputs[2],
        record.apu.output.channel_digital_outputs[3],
        record.apu.output.mixer_output.left,
        record.apu.output.mixer_output.right,
        record.apu.output.hpf_output.left,
        record.apu.output.hpf_output.right,
        record.interrupts.interrupt_flags,
        record.interrupts.interrupt_enable,
        0xC0 | record.joypad.selection_bits | visible_joypad_low_nibble(&record.joypad),
        record.joypad.selection_bits,
        record.joypad.pressed_mask,
        record.cartridge_trace,
        format_apu_last_register_write(record.apu.last_register_write.as_ref()),
    )
}

fn render_desktop_watch_trace_record(record: &DesktopWatchTraceRecord) -> String {
    format!(
        "t_cycle={} watch.hit_addresses={} cpu.pc={:#06X} cpu.execution_state={:?} cpu.current_opcode={:?} cpu.ime={} cpu.delayed_ime_enable={} cpu.last_bus_activity={} cpu.last_address_event={} ppu.mode={:?} ppu.ly={} ppu.line_dot={} irq.if={:#04X} irq.ie={:#04X} joypad.p1={:#04X} joypad.selection_bits={:#04X} joypad.pressed_mask={:#04X} {}",
        record.t_cycle,
        format_watch_hit_addresses(&record.matched_addresses),
        record.cpu.registers.pc,
        record.cpu.execution_state,
        record.cpu.current_opcode,
        record.cpu.ime,
        record.cpu.delayed_ime_enable,
        format_cpu_bus_activity(record.cpu.last_bus_activity),
        format_cpu_address_event(record.cpu.last_address_event),
        record.ppu_mode,
        record.ppu_ly,
        record.ppu_line_dot,
        record.interrupts.interrupt_flags,
        record.interrupts.interrupt_enable,
        0xC0 | record.joypad.selection_bits | visible_joypad_low_nibble(&record.joypad),
        record.joypad.selection_bits,
        record.joypad.pressed_mask,
        record.cartridge_trace,
    )
}

fn render_desktop_pc_watch_trace_record(record: &DesktopPcWatchTraceRecord) -> String {
    format!(
        "t_cycle={} pc_watch.hit_ranges={} cpu.pc={:#06X} cpu.execution_state={:?} cpu.current_opcode={:?} cpu.ime={} cpu.delayed_ime_enable={} cpu.last_bus_activity={} cpu.last_address_event={} ppu.mode={:?} ppu.ly={} ppu.line_dot={} irq.if={:#04X} irq.ie={:#04X} joypad.p1={:#04X} joypad.selection_bits={:#04X} joypad.pressed_mask={:#04X} {}",
        record.t_cycle,
        format_pc_watch_hit_ranges(&record.matched_ranges),
        record.cpu.registers.pc,
        record.cpu.execution_state,
        record.cpu.current_opcode,
        record.cpu.ime,
        record.cpu.delayed_ime_enable,
        format_cpu_bus_activity(record.cpu.last_bus_activity),
        format_cpu_address_event(record.cpu.last_address_event),
        record.ppu_mode,
        record.ppu_ly,
        record.ppu_line_dot,
        record.interrupts.interrupt_flags,
        record.interrupts.interrupt_enable,
        0xC0 | record.joypad.selection_bits | visible_joypad_low_nibble(&record.joypad),
        record.joypad.selection_bits,
        record.joypad.pressed_mask,
        record.cartridge_trace,
    )
}

fn render_desktop_edge_trace_record(record: &DesktopEdgeTraceRecord) -> String {
    format!(
        "t_cycle={} edge.current_pc_ranges={} edge.triggers={} cpu.pc={:#06X} cpu.execution_state={:?} cpu.current_opcode={:?} cpu.ime={} cpu.delayed_ime_enable={} cpu.last_bus_activity={} cpu.last_address_event={} ppu.mode={:?} ppu.ly={} ppu.line_dot={} irq.if={:#04X} irq.ie={:#04X} joypad.p1={:#04X} joypad.selection_bits={:#04X} joypad.pressed_mask={:#04X} {}",
        record.t_cycle,
        format_pc_watch_hit_ranges(&record.current_pc_ranges),
        format_edge_trace_triggers(&record.triggers),
        record.cpu.registers.pc,
        record.cpu.execution_state,
        record.cpu.current_opcode,
        record.cpu.ime,
        record.cpu.delayed_ime_enable,
        format_cpu_bus_activity(record.cpu.last_bus_activity),
        format_cpu_address_event(record.cpu.last_address_event),
        record.ppu_mode,
        record.ppu_ly,
        record.ppu_line_dot,
        record.interrupts.interrupt_flags,
        record.interrupts.interrupt_enable,
        0xC0 | record.joypad.selection_bits | visible_joypad_low_nibble(&record.joypad),
        record.joypad.selection_bits,
        record.joypad.pressed_mask,
        record.cartridge_trace,
    )
}

fn render_desktop_cgb_ir_trace_record(record: &DesktopCgbIrTraceRecord) -> String {
    format!(
        "t_cycle={} cgb_ir.triggers={} p1={} p2={}",
        record.t_cycle,
        format_cgb_ir_trace_triggers(&record.triggers),
        format_cgb_ir_trace_participant(&record.p1),
        format_cgb_ir_trace_participant(&record.p2),
    )
}

fn render_desktop_ch4_nr43_trace_record(record: &DesktopCh4Nr43TraceRecord) -> String {
    format!(
        "t_cycle={} cpu.pc={:#06X} cpu.execution_state={:?}{} {}",
        record.t_cycle,
        record.cpu.registers.pc,
        record.cpu.execution_state,
        format_apu_last_register_write(Some(&record.apu_write)),
        format_ch4_debug_snapshot(&record.ch4),
    )
}

fn render_desktop_ch4_startup_trace_record(record: &DesktopCh4StartupTraceRecord) -> String {
    format!(
        "event={:?} t_cycle={} cpu.pc={:#06X} cpu.execution_state={:?}{} {}",
        record.event,
        record.t_cycle,
        record.cpu.registers.pc,
        record.cpu.execution_state,
        format_apu_last_register_write(record.apu_write.as_ref()),
        format_ch4_debug_snapshot(&record.ch4),
    )
}

fn render_desktop_cpu_window_trace_record(record: &DesktopCpuWindowTraceRecord) -> String {
    format!(
        "t_cycle={} cpu.pc={:#06X} cpu.execution_state={:?} cpu.current_opcode={:?} cpu.ime={} cpu.delayed_ime_enable={} cpu.last_bus_activity={} cpu.last_address_event={} irq.if={:#04X} irq.ie={:#04X} ppu.ly_internal={:#04X} ppu.ly_read={:#04X} ppu.stat_read={:#04X} ppu.mode={:?} ppu.line_dot={} ppu.mode_dot={} ppu.mode0_start_dot={} ppu.lyc_coincidence={} ppu.stat_irq_line={} ppu.lcd_state={:?} ppu.blank_frame_active={}",
        record.t_cycle,
        record.cpu.registers.pc,
        record.cpu.execution_state,
        record.cpu.current_opcode,
        record.cpu.ime,
        record.cpu.delayed_ime_enable,
        format_cpu_bus_activity(record.cpu.last_bus_activity),
        format_cpu_address_event(record.cpu.last_address_event),
        record.interrupts.interrupt_flags,
        record.interrupts.interrupt_enable,
        record.ppu.ly,
        record.ppu_ly_read,
        record.ppu_stat_read,
        record.ppu.mode,
        record.ppu.line_dot,
        record.ppu.mode_dot,
        record.ppu.mode0_start_dot,
        record.ppu.lyc_coincidence,
        record.ppu.stat_irq_line,
        record.ppu.lcd_state,
        record.ppu.blank_frame_active,
    )
}

fn format_watch_hit_addresses(addresses: &[u16]) -> String {
    let mut rendered = String::from("[");
    for (index, address) in addresses.iter().enumerate() {
        if index > 0 {
            rendered.push(',');
        }
        rendered.push_str(&format!("{address:#06X}"));
    }
    rendered.push(']');
    rendered
}

fn format_pc_watch_hit_ranges(ranges: &[PcWatchRange]) -> String {
    let mut rendered = String::from("[");
    for (index, range) in ranges.iter().enumerate() {
        if index > 0 {
            rendered.push(',');
        }
        rendered.push_str(&format!("{:#06X}..={:#06X}", range.start, range.end));
    }
    rendered.push(']');
    rendered
}

fn format_edge_trace_triggers(triggers: &[DesktopEdgeTraceTrigger]) -> String {
    let mut rendered = String::from("[");
    for (index, trigger) in triggers.iter().enumerate() {
        if index > 0 {
            rendered.push(',');
        }
        match trigger {
            DesktopEdgeTraceTrigger::EnteredPcRange(range) => {
                rendered.push_str(&format!(
                    "enter_pc({:#06X}..={:#06X})",
                    range.start, range.end
                ));
            }
            DesktopEdgeTraceTrigger::AddressValueObserved {
                kind,
                address,
                previous,
                current,
            } => match previous {
                Some(previous) => rendered.push_str(&format!(
                    "change({}@{address:#06X}:{previous:#04X}->{current:#04X})",
                    cpu_bus_access_kind_name(*kind)
                )),
                None => rendered.push_str(&format!(
                    "observe({}@{address:#06X}={current:#04X})",
                    cpu_bus_access_kind_name(*kind)
                )),
            },
        }
    }
    rendered.push(']');
    rendered
}

fn format_cgb_ir_trace_triggers(triggers: &[DesktopCgbIrTraceTrigger]) -> String {
    let mut rendered = String::from("[");
    for (index, trigger) in triggers.iter().enumerate() {
        if index > 0 {
            rendered.push(',');
        }
        match trigger {
            DesktopCgbIrTraceTrigger::StatusChanged {
                slot,
                previous,
                current,
            } => match previous {
                Some(previous) => rendered.push_str(&format!(
                    "{}.status({}->{})",
                    slot.label(),
                    format_cgb_ir_status_short(*previous),
                    format_cgb_ir_status_short(*current)
                )),
                None => rendered.push_str(&format!(
                    "{}.status({})",
                    slot.label(),
                    format_cgb_ir_status_short(*current)
                )),
            },
            DesktopCgbIrTraceTrigger::RpBusActivity { slot, activity } => {
                rendered.push_str(&format!(
                    "{}.rp({}@{:#06X}={:#04X})",
                    slot.label(),
                    cpu_bus_access_kind_name(activity.kind),
                    activity.address,
                    activity.value
                ));
            }
            DesktopCgbIrTraceTrigger::WatchedBusActivity { slot, activity } => {
                rendered.push_str(&format!(
                    "{}.watch({}@{:#06X}={:#04X})",
                    slot.label(),
                    cpu_bus_access_kind_name(activity.kind),
                    activity.address,
                    activity.value
                ));
            }
            DesktopCgbIrTraceTrigger::JoypadPressedMaskChanged {
                slot,
                previous,
                current,
            } => match previous {
                Some(previous) => rendered.push_str(&format!(
                    "{}.joy({previous:#04X}->{current:#04X})",
                    slot.label()
                )),
                None => rendered.push_str(&format!("{}.joy({current:#04X})", slot.label())),
            },
        }
    }
    rendered.push(']');
    rendered
}

fn format_cgb_ir_trace_participant(record: &DesktopCgbIrTraceParticipantRecord) -> String {
    format!(
        "{{pc={:#06X} regs={} rom={} op={:?} bus={} joy={:#04X} watch={} ir={}}}",
        record.cpu.registers.pc,
        format_cpu_register_pairs(record.cpu.registers),
        format_mapped_rom_window(record.rom_window),
        record.cpu.current_opcode,
        format_cpu_bus_activity(record.cpu.last_bus_activity),
        record.joypad.pressed_mask,
        format_cgb_ir_trace_watched_values(&record.watched_values),
        format_cgb_ir_status(record.status),
    )
}

fn format_cpu_register_pairs(registers: CpuRegisters) -> String {
    let af = u16::from_be_bytes([registers.a, registers.f]);
    let bc = u16::from_be_bytes([registers.b, registers.c]);
    let de = u16::from_be_bytes([registers.d, registers.e]);
    let hl = u16::from_be_bytes([registers.h, registers.l]);

    format!(
        "{{af={af:#06X} bc={bc:#06X} de={de:#06X} hl={hl:#06X} sp={:#06X}}}",
        registers.sp
    )
}

fn format_cgb_ir_trace_watched_values(values: &[DesktopCgbIrTraceWatchedValue]) -> String {
    let mut rendered = String::from("[");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            rendered.push(',');
        }
        rendered.push_str(&format_cgb_ir_trace_watched_value(*value));
    }
    rendered.push(']');
    rendered
}

fn format_cgb_ir_trace_watched_value(value: DesktopCgbIrTraceWatchedValue) -> String {
    match value {
        DesktopCgbIrTraceWatchedValue::Wram(sample) => format!(
            "{:#06X}=wram(bank={:#04X},off={:#06X},value={:#04X})",
            sample.address, sample.bank, sample.bank_offset, sample.value
        ),
        DesktopCgbIrTraceWatchedValue::Hram {
            address,
            offset,
            value,
        } => format!("{address:#06X}=hram(off={offset:#04X},value={value:#04X})"),
        DesktopCgbIrTraceWatchedValue::Unsupported { address } => {
            format!("{address:#06X}=unsupported")
        }
    }
}

fn format_mapped_rom_window(window: Option<CartridgeMappedRomWindow>) -> String {
    let Some(window) = window else {
        return "none".to_string();
    };

    format!(
        "{{src={} rom_bank={:#04X} bank_size={:#06X} bank_off={:#06X}}}",
        mapped_rom_source_name(window.source),
        window.bank,
        window.bank_size,
        window.bank_offset,
    )
}

fn mapped_rom_source_name(source: CartridgeMappedRomSource) -> &'static str {
    match source {
        CartridgeMappedRomSource::Rom => "rom",
        CartridgeMappedRomSource::Flash => "flash",
    }
}

fn format_cgb_ir_status(status: CgbInfraredStatus) -> String {
    format!(
        "{{rp={:#04X} emit={} rd={} ext={} opt={} warm={} ctr={} eff={} vis={} ready={}}}",
        status.rp_latch,
        bool_bit(status.emitter_on),
        bool_bit(status.read_enabled),
        bool_bit(status.external_optical_input),
        bool_bit(status.optical_input_active),
        bool_bit(status.sensor_warmed),
        status.sensor_counter,
        bool_bit(status.effective_signal_detected),
        bool_bit(status.signal_visible_to_rp),
        bool_bit(status.receive_ready()),
    )
}

fn format_cgb_ir_status_short(status: CgbInfraredStatus) -> String {
    format!(
        "rp={:#04X}/emit{}/rd{}/ext{}/opt{}/warm{}/eff{}/vis{}/ready{}",
        status.rp_latch,
        bool_bit(status.emitter_on),
        bool_bit(status.read_enabled),
        bool_bit(status.external_optical_input),
        bool_bit(status.optical_input_active),
        bool_bit(status.sensor_warmed),
        bool_bit(status.effective_signal_detected),
        bool_bit(status.signal_visible_to_rp),
        bool_bit(status.receive_ready()),
    )
}

const fn bool_bit(value: bool) -> u8 {
    if value { 1 } else { 0 }
}

fn watched_cpu_addresses(cpu: &CpuSnapshot, watched_addresses: &BTreeSet<u16>) -> Vec<u16> {
    let mut matched_addresses = BTreeSet::new();

    if let Some(activity) = cpu.last_bus_activity
        && watched_addresses.contains(&activity.address)
    {
        matched_addresses.insert(activity.address);
    }

    if let Some(event) = cpu.last_address_event {
        if let Some(address) = event.access_address
            && watched_addresses.contains(&address)
        {
            matched_addresses.insert(address);
        }
        if let Some(address) = event.idu_address
            && watched_addresses.contains(&address)
        {
            matched_addresses.insert(address);
        }
    }

    matched_addresses.into_iter().collect()
}

fn watched_pc_ranges(cpu: &CpuSnapshot, watched_ranges: &[PcWatchRange]) -> Vec<PcWatchRange> {
    watched_ranges
        .iter()
        .copied()
        .filter(|range| range.contains(cpu.registers.pc))
        .collect()
}

fn entered_pc_ranges(
    current_pc_ranges: &[PcWatchRange],
    active_pc_ranges: &BTreeSet<PcWatchRange>,
) -> Vec<PcWatchRange> {
    current_pc_ranges
        .iter()
        .copied()
        .filter(|range| !active_pc_ranges.contains(range))
        .collect()
}

fn watched_bus_value_change(
    activity: Option<CpuBusActivitySnapshot>,
    watched_addresses: &BTreeSet<u16>,
    last_observed_values: &BTreeMap<u16, u8>,
) -> Option<DesktopEdgeTraceTrigger> {
    let activity = activity?;
    if !watched_addresses.contains(&activity.address) {
        return None;
    }

    let previous = last_observed_values.get(&activity.address).copied();
    if previous == Some(activity.value) {
        return None;
    }

    Some(DesktopEdgeTraceTrigger::AddressValueObserved {
        kind: activity.kind,
        address: activity.address,
        previous,
        current: activity.value,
    })
}

fn format_cpu_bus_activity(activity: Option<CpuBusActivitySnapshot>) -> String {
    match activity {
        Some(activity) => format!(
            "{}@{:#06X}={:#04X}",
            cpu_bus_access_kind_name(activity.kind),
            activity.address,
            activity.value,
        ),
        None => "none".to_string(),
    }
}

fn cpu_bus_access_kind_name(kind: CpuBusAccessKind) -> &'static str {
    match kind {
        CpuBusAccessKind::OpcodeFetch => "opcode_fetch",
        CpuBusAccessKind::OperandRead => "operand_read",
        CpuBusAccessKind::DataRead => "data_read",
        CpuBusAccessKind::DataWrite => "data_write",
    }
}

fn format_cpu_address_event(event: Option<CpuAddressEvent>) -> String {
    match event {
        Some(event) => match event.kind {
            CpuAddressEventKind::Read => match event.access_address {
                Some(address) => format!("read@{address:#06X}"),
                None => "read@missing".to_string(),
            },
            CpuAddressEventKind::Write => match event.access_address {
                Some(address) => format!("write@{address:#06X}"),
                None => "write@missing".to_string(),
            },
            CpuAddressEventKind::IncDec => match (event.idu_address, event.update_direction) {
                (Some(address), Some(direction)) => {
                    format!("{}@{address:#06X}", format_update_direction(direction))
                }
                _ => "incdec@missing".to_string(),
            },
            CpuAddressEventKind::ReadWithIncDec | CpuAddressEventKind::WriteWithIncDec => {
                match (
                    event.access_address,
                    event.idu_address,
                    event.update_direction,
                ) {
                    (Some(access), Some(idu), Some(direction)) => format!(
                        "{}+{}@{access:#06X}->{idu:#06X}",
                        match event.kind {
                            CpuAddressEventKind::ReadWithIncDec => "read",
                            CpuAddressEventKind::WriteWithIncDec => "write",
                            _ => unreachable!("combined event already constrained"),
                        },
                        format_update_direction(direction),
                    ),
                    _ => "combined@missing".to_string(),
                }
            }
        },
        None => "none".to_string(),
    }
}

fn format_update_direction(direction: CpuAddressUpdateDirection) -> &'static str {
    match direction {
        CpuAddressUpdateDirection::Increment => "inc",
        CpuAddressUpdateDirection::Decrement => "dec",
    }
}

fn format_apu_last_register_write(observation: Option<&ApuRegisterWriteObservation>) -> String {
    let Some(observation) = observation else {
        return String::new();
    };

    format!(
        " apu.last_write=write@{:#06X}={:#04X} before({}) after({})",
        observation.address,
        observation.value,
        format_apu_register_write_state(&observation.before),
        format_apu_register_write_state(&observation.after),
    )
}

fn format_apu_register_write_state(state: &ApuRegisterWriteState) -> String {
    format!(
        "nr52={:#04X} active={:#04X} dac={:#04X} outputs=[{:#04X},{:#04X},{:#04X},{:#04X}] mixer=({}, {}) hpf=({}, {})",
        state.nr52,
        state.channel_active_mask,
        state.channel_dac_mask,
        state.output.channel_digital_outputs[0],
        state.output.channel_digital_outputs[1],
        state.output.channel_digital_outputs[2],
        state.output.channel_digital_outputs[3],
        state.output.mixer_output.left,
        state.output.mixer_output.right,
        state.output.hpf_output.left,
        state.output.hpf_output.right,
    )
}

fn format_ch4_debug_snapshot(snapshot: &ApuCh4DebugSnapshot) -> String {
    format!(
        "ch4.nr43={:#04X} ch4.shift={} ch4.short_width={} ch4.divider={} ch4.alignment={} ch4.counter_timer={} ch4.noise_counter={:#06X} ch4.countdown_reloaded={} ch4.did_step_counter={} ch4.counter_active={} ch4.background_counting={} ch4.started_with_dac_disabled={} ch4.dmg_delayed_start={} ch4.runtime_active={} ch4.runtime_dac_enabled={} ch4.period_timer={} ch4.lfsr={:#06X} ch4.output={:#04X}{}",
        snapshot.nr43,
        snapshot.clock_shift,
        snapshot.short_width_mode,
        snapshot.clock_divider_code,
        snapshot.alignment,
        snapshot.counter_timer,
        snapshot.noise_counter,
        snapshot.countdown_reloaded,
        snapshot.did_step_counter,
        snapshot.counter_active,
        snapshot.background_counting,
        snapshot.started_with_dac_disabled,
        snapshot.dmg_delayed_start,
        snapshot.runtime_active,
        snapshot.runtime_dac_enabled,
        snapshot.period_timer,
        snapshot.lfsr_state,
        snapshot.current_digital_output,
        format_ch4_live_nr43_trace(snapshot.last_nr43_live_write.as_ref()),
    )
}

fn format_ch4_live_nr43_trace(trace: Option<&ApuCh4Nr43LiveWriteTrace>) -> String {
    let Some(trace) = trace else {
        return " ch4.last_nr43_live_write=none".to_string();
    };

    format!(
        " ch4.last_nr43_live_write=old({:#04X}/shift={}/bit={}) ff({:#04X}/shift={}/bit={}) glitch1({:#04X}/shift={}/bit={}) glitch2({}/shift={}/bit={}) new({:#04X}/shift={}/bit={}) runtime_active={} same_shift_group={} effective_counter={:#06X} countdown_reloaded={} category={:?} action={:?} passes=[reload_seam:{},old_to_ff:{},ff_to_glitch1:{},glitch1_to_glitch2:{},glitch_to_new:{},low_shift_followup:{}] lfsr={:#06X}->{:#06X}",
        trace.old_nr43,
        trace.old_shift,
        trace.old_bit,
        trace.ff_value,
        trace.ff_shift,
        trace.ff_bit,
        trace.glitch_1_value,
        trace.glitch_1_shift,
        trace.glitch_1_bit,
        trace
            .glitch_2_value
            .map(|value| format!("{value:#04X}"))
            .unwrap_or_else(|| "none".to_string()),
        trace
            .glitch_2_shift
            .map(|shift| shift.to_string())
            .unwrap_or_else(|| "none".to_string()),
        trace
            .glitch_2_bit
            .map(|bit| bit.to_string())
            .unwrap_or_else(|| "none".to_string()),
        trace.new_nr43,
        trace.new_shift,
        trace.new_bit,
        trace.runtime_active,
        trace.same_shift_group,
        trace.effective_counter,
        trace.countdown_reloaded,
        trace.decision_category,
        trace.lfsr_action,
        format_ch4_live_nr43_pass(trace.reload_seam.as_ref()),
        format_ch4_live_nr43_pass(trace.old_to_ff.as_ref()),
        format_ch4_live_nr43_pass(trace.ff_to_glitch_1.as_ref()),
        format_ch4_live_nr43_pass(trace.glitch_1_to_glitch_2.as_ref()),
        format_ch4_live_nr43_pass(trace.glitch_to_new.as_ref()),
        format_ch4_live_nr43_pass(trace.low_shift_followup.as_ref()),
        trace.lfsr_before,
        trace.lfsr_after,
    )
}

fn format_ch4_live_nr43_pass(pass: Option<&ApuCh4Nr43PassTrace>) -> String {
    let Some(pass) = pass else {
        return "none".to_string();
    };

    format!(
        "{:?}({:#04X}/{}->{:#04X}/{},bit:{}->{},cat:{:?},act:{:?},lfsr:{:#06X}->{:#06X})",
        pass.kind,
        pass.value_from,
        pass.shift_from,
        pass.value_to,
        pass.shift_to,
        pass.bit_from,
        pass.bit_to,
        pass.category,
        pass.action,
        pass.lfsr_before,
        pass.lfsr_after,
    )
}

fn visible_nr52(powered: bool, active_mask: u8) -> u8 {
    0x70 | if powered {
        0x80 | (active_mask & 0x0F)
    } else {
        0
    }
}

fn visible_joypad_low_nibble(snapshot: &JoypadSnapshot) -> u8 {
    let dpad_selected = snapshot.selection_bits & 0x10 == 0;
    let buttons_selected = snapshot.selection_bits & 0x20 == 0;
    let mut low = 0x0F;
    if dpad_selected {
        if snapshot.pressed_mask & 0x01 != 0 {
            low &= !0x01;
        }
        if snapshot.pressed_mask & 0x02 != 0 {
            low &= !0x02;
        }
        if snapshot.pressed_mask & 0x04 != 0 {
            low &= !0x04;
        }
        if snapshot.pressed_mask & 0x08 != 0 {
            low &= !0x08;
        }
    }
    if buttons_selected {
        if snapshot.pressed_mask & 0x10 != 0 {
            low &= !0x01;
        }
        if snapshot.pressed_mask & 0x20 != 0 {
            low &= !0x02;
        }
        if snapshot.pressed_mask & 0x40 != 0 {
            low &= !0x04;
        }
        if snapshot.pressed_mask & 0x80 != 0 {
            low &= !0x08;
        }
    }
    low
}

fn parse_trace_capture_t_cycles(value: Option<&std::ffi::OsStr>) -> Result<usize, String> {
    let Some(value) = value else {
        return Ok(DEFAULT_TRACE_CAPTURE_T_CYCLES);
    };

    let text = value.to_string_lossy();
    let parsed = text.parse::<usize>().map_err(|error| {
        format!(
            "{DESKTOP_TRACE_T_CYCLES_ENV_VAR} must be a positive integer T-cycle count: {error}"
        )
    })?;
    if parsed == 0 {
        return Err(format!(
            "{DESKTOP_TRACE_T_CYCLES_ENV_VAR} must be greater than zero"
        ));
    }
    Ok(parsed)
}

fn parse_watch_trace_addresses(value: Option<&OsStr>) -> Result<BTreeSet<u16>, String> {
    parse_hex_address_list(
        value,
        DESKTOP_WATCH_TRACE_ADDRESSES_ENV_VAR,
        MissingWatchConfigPolicy::Error,
    )
}

fn parse_edge_trace_addresses(value: Option<&OsStr>) -> Result<BTreeSet<u16>, String> {
    parse_hex_address_list(
        value,
        DESKTOP_EDGE_TRACE_ADDRESSES_ENV_VAR,
        MissingWatchConfigPolicy::AllowEmpty,
    )
}

fn parse_cgb_ir_trace_watch_addresses(value: Option<&OsStr>) -> Result<BTreeSet<u16>, String> {
    parse_hex_address_list(
        value,
        DESKTOP_CGB_IR_TRACE_WATCH_ADDRESSES_ENV_VAR,
        MissingWatchConfigPolicy::AllowEmpty,
    )
}

fn parse_cgb_ir_trace_trigger_addresses(value: Option<&OsStr>) -> Result<BTreeSet<u16>, String> {
    parse_hex_address_list(
        value,
        DESKTOP_CGB_IR_TRACE_TRIGGER_ADDRESSES_ENV_VAR,
        MissingWatchConfigPolicy::AllowEmpty,
    )
}

fn parse_watch_trace_event_count(value: Option<&OsStr>) -> Result<usize, String> {
    parse_positive_event_count(
        value,
        DEFAULT_WATCH_TRACE_EVENTS,
        DESKTOP_WATCH_TRACE_EVENTS_ENV_VAR,
    )
}

fn parse_pc_watch_trace_ranges(value: Option<&OsStr>) -> Result<Vec<PcWatchRange>, String> {
    parse_pc_watch_ranges_for_env(
        value,
        DESKTOP_PC_WATCH_TRACE_RANGES_ENV_VAR,
        MissingWatchConfigPolicy::Error,
    )
}

fn parse_edge_trace_pc_ranges(value: Option<&OsStr>) -> Result<Vec<PcWatchRange>, String> {
    parse_pc_watch_ranges_for_env(
        value,
        DESKTOP_EDGE_TRACE_PC_RANGES_ENV_VAR,
        MissingWatchConfigPolicy::AllowEmpty,
    )
}

fn parse_pc_watch_trace_range_token(
    token: &str,
    env_var_name: &str,
) -> Result<PcWatchRange, String> {
    if let Some((start, end)) = token.split_once("..=") {
        return PcWatchRange::new(
            parse_pc_watch_hex(start.trim(), env_var_name)?,
            parse_pc_watch_hex(end.trim(), env_var_name)?,
        );
    }
    if let Some((start, end)) = token.split_once("..") {
        return PcWatchRange::new(
            parse_pc_watch_hex(start.trim(), env_var_name)?,
            parse_pc_watch_hex(end.trim(), env_var_name)?,
        );
    }
    if let Some((start, end)) = token.split_once('-') {
        return PcWatchRange::new(
            parse_pc_watch_hex(start.trim(), env_var_name)?,
            parse_pc_watch_hex(end.trim(), env_var_name)?,
        );
    }

    let address = parse_pc_watch_hex(token, env_var_name)?;
    PcWatchRange::new(address, address)
}

fn parse_pc_watch_hex(token: &str, env_var_name: &str) -> Result<u16, String> {
    let hex = token
        .strip_prefix("0x")
        .or_else(|| token.strip_prefix("0X"))
        .unwrap_or(token);
    u16::from_str_radix(hex, 16).map_err(|error| {
        format!(
            "{env_var_name} must be a comma-separated list of hex addresses or inclusive ranges: {error}"
        )
    })
}

fn parse_pc_watch_trace_event_count(value: Option<&OsStr>) -> Result<usize, String> {
    parse_positive_event_count(
        value,
        DEFAULT_PC_WATCH_TRACE_EVENTS,
        DESKTOP_PC_WATCH_TRACE_EVENTS_ENV_VAR,
    )
}

fn parse_edge_trace_event_count(value: Option<&OsStr>) -> Result<usize, String> {
    parse_positive_event_count(
        value,
        DEFAULT_EDGE_TRACE_EVENTS,
        DESKTOP_EDGE_TRACE_EVENTS_ENV_VAR,
    )
}

fn parse_cgb_ir_trace_event_count(value: Option<&OsStr>) -> Result<usize, String> {
    parse_positive_event_count(
        value,
        DEFAULT_CGB_IR_TRACE_EVENTS,
        DESKTOP_CGB_IR_TRACE_EVENTS_ENV_VAR,
    )
}

fn cgb_ir_optical_delay_t_cycles_from_env() -> Result<usize, String> {
    parse_cgb_ir_optical_delay_t_cycles(
        env::var_os(DESKTOP_CGB_IR_OPTICAL_DELAY_T_CYCLES_ENV_VAR).as_deref(),
    )
}

fn parse_cgb_ir_optical_delay_t_cycles(value: Option<&OsStr>) -> Result<usize, String> {
    let Some(value) = value else {
        return Ok(DEFAULT_CGB_IR_OPTICAL_PROPAGATION_DELAY_T_CYCLES);
    };

    let text = value.to_string_lossy();
    let parsed = text.parse::<usize>().map_err(|error| {
        format!(
            "{DESKTOP_CGB_IR_OPTICAL_DELAY_T_CYCLES_ENV_VAR} must be an integer T-cycle count: {error}"
        )
    })?;
    if !(MIN_CGB_IR_OPTICAL_PROPAGATION_DELAY_T_CYCLES
        ..=MAX_CGB_IR_OPTICAL_PROPAGATION_DELAY_T_CYCLES)
        .contains(&parsed)
    {
        return Err(format!(
            "{DESKTOP_CGB_IR_OPTICAL_DELAY_T_CYCLES_ENV_VAR} must be between {MIN_CGB_IR_OPTICAL_PROPAGATION_DELAY_T_CYCLES} and {MAX_CGB_IR_OPTICAL_PROPAGATION_DELAY_T_CYCLES} T-cycles"
        ));
    }
    Ok(parsed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MissingWatchConfigPolicy {
    Error,
    AllowEmpty,
}

fn parse_hex_address_list(
    value: Option<&OsStr>,
    env_var_name: &str,
    missing_policy: MissingWatchConfigPolicy,
) -> Result<BTreeSet<u16>, String> {
    let Some(value) = value else {
        return match missing_policy {
            MissingWatchConfigPolicy::Error => Err(format!(
                "{env_var_name} must list one or more watched addresses"
            )),
            MissingWatchConfigPolicy::AllowEmpty => Ok(BTreeSet::new()),
        };
    };

    let mut addresses = BTreeSet::new();
    for raw_token in value
        .to_string_lossy()
        .split(|ch: char| ch == ',' || ch == ';' || ch.is_ascii_whitespace())
    {
        let token = raw_token.trim();
        if token.is_empty() {
            continue;
        }

        let hex = token
            .strip_prefix("0x")
            .or_else(|| token.strip_prefix("0X"))
            .unwrap_or(token);
        let address = u16::from_str_radix(hex, 16).map_err(|error| {
            format!("{env_var_name} must be a comma-separated list of hex addresses: {error}")
        })?;
        addresses.insert(address);
    }

    if addresses.is_empty() {
        return Err(format!(
            "{env_var_name} must list one or more watched addresses"
        ));
    }

    Ok(addresses)
}

fn parse_pc_watch_ranges_for_env(
    value: Option<&OsStr>,
    env_var_name: &str,
    missing_policy: MissingWatchConfigPolicy,
) -> Result<Vec<PcWatchRange>, String> {
    let Some(value) = value else {
        return match missing_policy {
            MissingWatchConfigPolicy::Error => Err(format!(
                "{env_var_name} must list one or more watched PC ranges"
            )),
            MissingWatchConfigPolicy::AllowEmpty => Ok(Vec::new()),
        };
    };

    let mut ranges = Vec::new();
    for raw_token in value.to_string_lossy().split([',', ';']) {
        let token = raw_token.trim();
        if token.is_empty() {
            continue;
        }
        ranges.push(parse_pc_watch_trace_range_token(token, env_var_name)?);
    }

    if ranges.is_empty() {
        return Err(format!(
            "{env_var_name} must list one or more watched PC ranges"
        ));
    }

    ranges.sort_unstable();
    ranges.dedup();
    Ok(ranges)
}

fn parse_positive_event_count(
    value: Option<&OsStr>,
    default: usize,
    env_var_name: &str,
) -> Result<usize, String> {
    let Some(value) = value else {
        return Ok(default);
    };

    let text = value.to_string_lossy();
    let parsed = text.parse::<usize>().map_err(|error| {
        format!("{env_var_name} must be a positive integer event count: {error}")
    })?;
    if parsed == 0 {
        return Err(format!("{env_var_name} must be greater than zero"));
    }
    Ok(parsed)
}

fn write_cartridge_diagnostics(diagnostics: &[CartridgeDiagnostic]) {
    for diagnostic in diagnostics {
        eprintln!(
            "{}: {}",
            diagnostic_severity_name(diagnostic.severity),
            diagnostic.message
        );
    }
}

fn diagnostic_severity_name(severity: CartridgeDiagnosticSeverity) -> &'static str {
    match severity {
        CartridgeDiagnosticSeverity::Warning => "warning",
        CartridgeDiagnosticSeverity::Error => "error",
    }
}
