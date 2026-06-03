use super::*;

#[test]
fn frame_pacer_and_performance_counter_cover_idle_paths() {
    let mut frame_pacer = super::super::FramePacer::new(
        true,
        super::super::frame_duration_for_config(&DesktopConfig::default()),
    );
    frame_pacer.next_frame_start = Instant::now() - Duration::from_secs(1);
    let pacing = frame_pacer.wait_until_next_frame(None);
    assert_eq!(pacing.pacing_duration, Duration::ZERO);
    assert_eq!(pacing.sleep_target_duration, Duration::ZERO);
    assert!(pacing.late_duration > Duration::ZERO);
    assert_eq!(pacing.audio_correction_duration, Duration::ZERO);
    assert_eq!(pacing.oversleep_duration, Duration::ZERO);
    frame_pacer.set_frame_duration(Duration::from_millis(15));
    assert_eq!(frame_pacer.frame_duration, Duration::from_millis(15));
    frame_pacer.set_vsync_enabled(true);
    assert!(frame_pacer.next_frame_start <= Instant::now());

    let mut counter = super::super::PerformanceCounter::new_with_emulation_profile_mode(
        "gb-desktop | no rom".to_string(),
        super::super::EmulationProfileMode::Disabled,
    );
    counter.set_target_frame_rate_hz(60.0);
    let snapshot = counter.snapshot_from_elapsed(Duration::ZERO);
    assert!(snapshot.fps.is_finite());
    assert_eq!(snapshot.audio_queue_ms, None);
}

#[test]
fn desktop_frame_timing_uses_sgb_profile_gb_master_clock() {
    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 0.000_1,
            "expected {expected}, got {actual}"
        );
    }

    let mut config = DesktopConfig::default();
    assert_close(
        super::super::target_frame_rate_hz_for_config(&config),
        59.727_500_6,
    );
    assert_eq!(
        super::super::frame_duration_for_config(&config),
        Duration::from_nanos(16_742_706)
    );

    config.launch.console_model = DesktopConsoleModel::SuperGameBoy;
    config.launch.sgb_video_standard = SgbVideoStandard::Ntsc;
    assert_close(
        super::super::target_frame_rate_hz_for_config(&config),
        61.167_897_0,
    );
    assert_eq!(
        super::super::frame_duration_for_config(&config),
        Duration::from_nanos(16_348_445)
    );

    config.launch.sgb_video_standard = SgbVideoStandard::Pal;
    assert_close(
        super::super::target_frame_rate_hz_for_config(&config),
        60.609_962_4,
    );
    assert_eq!(
        super::super::frame_duration_for_config(&config),
        Duration::from_nanos(16_498_938)
    );

    config.launch.console_model = DesktopConsoleModel::SuperGameBoy2;
    assert_close(
        super::super::target_frame_rate_hz_for_config(&config),
        59.727_500_6,
    );
    assert_eq!(
        super::super::frame_duration_for_config(&config),
        Duration::from_nanos(16_742_706)
    );
}

#[test]
fn host_rtc_sync_flushes_pending_mbc3_clock_ticks_for_lifecycle_sync() {
    let mut machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_test_rom(32 * 1024, 0x0F, 0x00, 0x00))
        .expect("MBC3 RTC cartridge should load");

    let mut rtc_sync = HostRtcSync::new(SystemTime::now());
    rtc_sync.apply_elapsed_to_machine(&mut machine, Duration::from_secs(5));

    assert_eq!(
        machine.cartridge().persistent_state(),
        PersistentCartState::Mbc3Rtc {
            rtc: gb_core::Mbc3RtcPersistentState {
                seconds: 0,
                minutes: 0,
                hours: 0,
                day_counter: 0,
                halt: false,
                carry: false,
            },
        },
    );

    rtc_sync.flush_pending_mbc3_clock_ticks(&mut machine);

    assert_eq!(
        machine.cartridge().persistent_state(),
        PersistentCartState::Mbc3Rtc {
            rtc: gb_core::Mbc3RtcPersistentState {
                seconds: 5,
                minutes: 0,
                hours: 0,
                day_counter: 0,
                halt: false,
                carry: false,
            },
        },
    );
}

#[test]
fn host_rtc_sync_uses_wall_clock_elapsed_for_live_rtc_budget() {
    let mut machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_test_rom(32 * 1024, 0x0F, 0x00, 0x00))
        .expect("MBC3 RTC cartridge should load");

    let mut rtc_sync = HostRtcSync::new(UNIX_EPOCH + Duration::from_secs(10));
    rtc_sync.sync_host_elapsed_to_machine_at(&mut machine, UNIX_EPOCH + Duration::from_secs(12));

    assert_eq!(rtc_sync.pending_mbc3_clock_ticks, 2 * 32_768);

    rtc_sync.flush_pending_mbc3_clock_ticks(&mut machine);

    assert_eq!(
        machine.cartridge().persistent_state(),
        PersistentCartState::Mbc3Rtc {
            rtc: gb_core::Mbc3RtcPersistentState {
                seconds: 2,
                minutes: 0,
                hours: 0,
                day_counter: 0,
                halt: false,
                carry: false,
            },
        },
    );
}

#[test]
fn host_rtc_sync_releases_mbc3_clock_ticks_on_emulated_t_cycle_cadence() {
    let mut machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_test_rom(32 * 1024, 0x0F, 0x00, 0x00))
        .expect("MBC3 RTC cartridge should load");
    machine.advance_mbc3_cartridge_rtc_clock_ticks(32_767);

    let mut rtc_sync = HostRtcSync::new(SystemTime::now());
    rtc_sync.apply_elapsed_to_machine(&mut machine, Duration::from_nanos(30_518));
    assert_eq!(rtc_sync.pending_mbc3_clock_ticks, 1);

    for _ in 0..127 {
        rtc_sync.tick_mbc3_for_emulated_t_cycle(&mut machine);
    }

    assert_eq!(
        machine.cartridge().persistent_state(),
        PersistentCartState::Mbc3Rtc {
            rtc: gb_core::Mbc3RtcPersistentState {
                seconds: 0,
                minutes: 0,
                hours: 0,
                day_counter: 0,
                halt: false,
                carry: false,
            },
        },
    );

    rtc_sync.tick_mbc3_for_emulated_t_cycle(&mut machine);

    assert_eq!(
        machine.cartridge().persistent_state(),
        PersistentCartState::Mbc3Rtc {
            rtc: gb_core::Mbc3RtcPersistentState {
                seconds: 1,
                minutes: 0,
                hours: 0,
                day_counter: 0,
                halt: false,
                carry: false,
            },
        },
    );
    assert_eq!(rtc_sync.pending_mbc3_clock_ticks, 0);
}

#[test]
fn host_rtc_sync_accumulates_fractional_mbc3_clock_ticks() {
    let mut machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_test_rom(32 * 1024, 0x0F, 0x00, 0x00))
        .expect("MBC3 RTC cartridge should load");

    let mut rtc_sync = HostRtcSync::new(SystemTime::now());
    rtc_sync.apply_elapsed_to_machine(&mut machine, Duration::from_nanos(999_999_999));

    assert_eq!(
        machine.cartridge().persistent_state(),
        PersistentCartState::Mbc3Rtc {
            rtc: gb_core::Mbc3RtcPersistentState {
                seconds: 0,
                minutes: 0,
                hours: 0,
                day_counter: 0,
                halt: false,
                carry: false,
            },
        },
    );

    rtc_sync.apply_elapsed_to_machine(&mut machine, Duration::from_nanos(1));
    rtc_sync.flush_pending_mbc3_clock_ticks(&mut machine);

    assert_eq!(
        machine.cartridge().persistent_state(),
        PersistentCartState::Mbc3Rtc {
            rtc: gb_core::Mbc3RtcPersistentState {
                seconds: 1,
                minutes: 0,
                hours: 0,
                day_counter: 0,
                halt: false,
                carry: false,
            },
        },
    );
}

#[test]
fn host_rtc_sync_keeps_huc3_on_elapsed_whole_seconds() {
    let mut machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_test_rom(256 * 1024, 0xFE, 0x03, 0x03))
        .expect("HuC-3 cartridge should load");

    let mut rtc_sync = HostRtcSync::new(SystemTime::now());
    rtc_sync.apply_elapsed_to_machine(&mut machine, Duration::from_millis(1_500));

    let PersistentCartState::Huc3 { rtc, .. } = machine.cartridge().persistent_state() else {
        panic!("expected HuC-3 persistence state");
    };
    assert_eq!(rtc.current_minutes_of_day, 0);
    assert_eq!(rtc.current_days, 0);
    assert_eq!(rtc.current_subminute_seconds, 1);

    rtc_sync.apply_elapsed_to_machine(&mut machine, Duration::from_millis(500));

    let PersistentCartState::Huc3 { rtc, .. } = machine.cartridge().persistent_state() else {
        panic!("expected HuC-3 persistence state");
    };
    assert_eq!(rtc.current_minutes_of_day, 0);
    assert_eq!(rtc.current_days, 0);
    assert_eq!(rtc.current_subminute_seconds, 2);
}
