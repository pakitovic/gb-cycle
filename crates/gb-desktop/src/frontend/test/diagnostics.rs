use super::*;

#[test]
fn trace_capture_t_cycles_parser_uses_default_and_rejects_zero() {
    assert_eq!(parse_trace_capture_t_cycles(None), Ok(8_192));
    assert_eq!(
        parse_trace_capture_t_cycles(Some(OsStr::new("4096"))),
        Ok(4_096)
    );
    assert!(
        parse_trace_capture_t_cycles(Some(OsStr::new("0")))
            .expect_err("zero trace window should be rejected")
            .contains("must be greater than zero")
    );
}

#[test]
fn watch_trace_parsers_accept_hex_addresses_and_reject_empty_or_zero_counts() {
    assert_eq!(
        parse_watch_trace_addresses(Some(OsStr::new("FF00, 0xFF82;C400"))),
        Ok(BTreeSet::from([0xFF00, 0xFF82, 0xC400]))
    );
    assert!(
        parse_watch_trace_addresses(Some(OsStr::new(" , ; ")))
            .expect_err("empty watch-address list should be rejected")
            .contains("must list one or more watched addresses")
    );
    assert_eq!(parse_watch_trace_event_count(None), Ok(4_096));
    assert_eq!(
        parse_watch_trace_event_count(Some(OsStr::new("1024"))),
        Ok(1_024)
    );
    assert!(
        parse_watch_trace_event_count(Some(OsStr::new("0")))
            .expect_err("zero watch trace event count should be rejected")
            .contains("must be greater than zero")
    );
}

#[test]
fn pc_watch_trace_parsers_accept_ranges_and_reject_empty_or_zero_counts() {
    assert_eq!(
        parse_pc_watch_trace_ranges(Some(OsStr::new("03C0-03EF, 0x05FD..=0x062F, 4C00"))),
        Ok(vec![
            super::super::PcWatchRange {
                start: 0x03C0,
                end: 0x03EF,
            },
            super::super::PcWatchRange {
                start: 0x05FD,
                end: 0x062F,
            },
            super::super::PcWatchRange {
                start: 0x4C00,
                end: 0x4C00,
            },
        ])
    );
    assert!(
        parse_pc_watch_trace_ranges(Some(OsStr::new(" , ; ")))
            .expect_err("empty PC watch-range list should be rejected")
            .contains("must list one or more watched PC ranges")
    );
    assert!(
        parse_pc_watch_trace_ranges(Some(OsStr::new("062F-05FD")))
            .expect_err("descending PC watch range should be rejected")
            .contains("range end")
    );
    assert_eq!(parse_pc_watch_trace_event_count(None), Ok(4_096));
    assert_eq!(
        parse_pc_watch_trace_event_count(Some(OsStr::new("2048"))),
        Ok(2_048)
    );
    assert!(
        parse_pc_watch_trace_event_count(Some(OsStr::new("0")))
            .expect_err("zero PC watch trace event count should be rejected")
            .contains("must be greater than zero")
    );
}

#[test]
fn edge_trace_parsers_allow_optional_targets_and_reject_zero_counts() {
    assert_eq!(parse_edge_trace_addresses(None), Ok(BTreeSet::new()));
    assert_eq!(
        parse_edge_trace_addresses(Some(OsStr::new("FF82,C409"))),
        Ok(BTreeSet::from([0xC409, 0xFF82]))
    );
    assert_eq!(parse_edge_trace_pc_ranges(None), Ok(Vec::new()));
    assert_eq!(
        parse_edge_trace_pc_ranges(Some(OsStr::new("4C00-4C41, 05FD"))),
        Ok(vec![
            super::super::PcWatchRange {
                start: 0x05FD,
                end: 0x05FD,
            },
            super::super::PcWatchRange {
                start: 0x4C00,
                end: 0x4C41,
            },
        ])
    );
    assert_eq!(parse_edge_trace_event_count(None), Ok(4_096));
    assert_eq!(
        parse_edge_trace_event_count(Some(OsStr::new("512"))),
        Ok(512)
    );
    assert!(
        parse_edge_trace_event_count(Some(OsStr::new("0")))
            .expect_err("zero edge trace event count should be rejected")
            .contains("must be greater than zero")
    );
}

#[test]
fn cgb_ir_trace_parser_and_renderer_include_status_and_rp_bus_context() {
    assert_eq!(parse_cgb_ir_trace_event_count(None), Ok(16_384));
    assert_eq!(
        parse_cgb_ir_trace_event_count(Some(OsStr::new("512"))),
        Ok(512)
    );
    assert_eq!(
        parse_cgb_ir_trace_watch_addresses(None),
        Ok(BTreeSet::new())
    );
    assert_eq!(
        parse_cgb_ir_trace_watch_addresses(Some(OsStr::new("D8AF,D8B0,FF8C"))),
        Ok(BTreeSet::from([0xD8AF, 0xD8B0, 0xFF8C]))
    );
    assert_eq!(
        parse_cgb_ir_trace_trigger_addresses(Some(OsStr::new("D8AF,D8B0"))),
        Ok(BTreeSet::from([0xD8AF, 0xD8B0]))
    );
    assert!(
        parse_cgb_ir_trace_event_count(Some(OsStr::new("0")))
            .expect_err("zero CGB IR trace event count should be rejected")
            .contains("must be greater than zero")
    );
    assert_eq!(
        parse_cgb_ir_optical_delay_t_cycles(None),
        Ok(super::super::DEFAULT_CGB_IR_OPTICAL_PROPAGATION_DELAY_T_CYCLES)
    );
    assert_eq!(
        parse_cgb_ir_optical_delay_t_cycles(Some(OsStr::new("80"))),
        Ok(80)
    );
    assert!(
        parse_cgb_ir_optical_delay_t_cycles(Some(OsStr::new("0")))
            .expect_err("zero CGB IR optical delay should be rejected")
            .contains("must be between")
    );

    let machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoyColor).with_startup_mode(StartupMode::SkipBoot),
    );
    let mut cpu = machine.cpu().snapshot();
    cpu.registers.a = 0x12;
    cpu.registers.f = 0x30;
    cpu.registers.b = 0x45;
    cpu.registers.c = 0x67;
    cpu.registers.d = 0x89;
    cpu.registers.e = 0xAB;
    cpu.registers.h = 0xCD;
    cpu.registers.l = 0xEF;
    cpu.registers.sp = 0xFFF0;
    cpu.last_bus_activity = Some(CpuBusActivitySnapshot {
        kind: CpuBusAccessKind::DataWrite,
        address: 0xFF56,
        value: 0xC0,
    });
    let rp_activity = cpu.last_bus_activity.expect("test CPU has RP bus activity");
    let status = CgbInfraredStatus {
        rp_latch: 0xC0,
        emitter_on: false,
        read_enabled: true,
        external_optical_input: false,
        optical_input_active: false,
        sensor_counter: 19_900,
        sensor_warmed: true,
        effective_signal_detected: false,
        signal_visible_to_rp: false,
    };
    let participant = super::super::DesktopCgbIrTraceParticipantRecord {
        status,
        cpu,
        joypad: machine.joypad().snapshot(),
        rom_window: Some(CartridgeMappedRomWindow {
            source: CartridgeMappedRomSource::Rom,
            bank: 0x12,
            bank_size: 0x4000,
            bank_offset: 0x1385,
        }),
        watched_values: vec![
            super::super::DesktopCgbIrTraceWatchedValue::Wram(DebugWramAddressSample {
                address: 0xD8B0,
                bank: 0x01,
                bank_offset: 0x08B0,
                value: 0xFF,
            }),
            super::super::DesktopCgbIrTraceWatchedValue::Hram {
                address: 0xFF8C,
                offset: 0x0C,
                value: 0x10,
            },
        ],
    };
    let rendered = render_desktop_cgb_ir_trace_record(&super::super::DesktopCgbIrTraceRecord {
        t_cycle: 456,
        triggers: vec![
            super::super::DesktopCgbIrTraceTrigger::StatusChanged {
                slot: super::super::PlayerSlot::P1,
                previous: None,
                current: status,
            },
            super::super::DesktopCgbIrTraceTrigger::RpBusActivity {
                slot: super::super::PlayerSlot::P1,
                activity: rp_activity,
            },
            super::super::DesktopCgbIrTraceTrigger::WatchedBusActivity {
                slot: super::super::PlayerSlot::P2,
                activity: CpuBusActivitySnapshot {
                    kind: CpuBusAccessKind::DataWrite,
                    address: 0xD8B0,
                    value: 0xFF,
                },
            },
        ],
        p1: participant.clone(),
        p2: participant,
    });

    assert!(rendered.contains("t_cycle=456"));
    assert!(rendered.contains("P1.status("));
    assert!(rendered.contains("P1.rp(data_write@0xFF56=0xC0)"));
    assert!(rendered.contains("P2.watch(data_write@0xD8B0=0xFF)"));
    assert!(rendered.contains("regs={af=0x1230 bc=0x4567 de=0x89AB hl=0xCDEF sp=0xFFF0}"));
    assert!(rendered.contains("rom={src=rom rom_bank=0x12 bank_size=0x4000 bank_off=0x1385}"));
    assert!(rendered.contains(
        "watch=[0xD8B0=wram(bank=0x01,off=0x08B0,value=0xFF),0xFF8C=hram(off=0x0C,value=0x10)]"
    ));
    assert!(rendered.contains("ir={rp=0xC0 emit=0 rd=1"));
    assert!(rendered.contains("ready=1"));
}

#[test]
fn cgb_ir_trace_status_trigger_ignores_counter_only_changes() {
    let previous = CgbInfraredStatus {
        rp_latch: 0xC0,
        emitter_on: false,
        read_enabled: true,
        external_optical_input: false,
        optical_input_active: false,
        sensor_counter: 19_900,
        sensor_warmed: true,
        effective_signal_detected: false,
        signal_visible_to_rp: false,
    };
    let mut current = previous;
    current.sensor_counter = 19_901;
    let mut triggers = Vec::new();

    super::super::collect_cgb_ir_status_trigger(
        &mut triggers,
        super::super::PlayerSlot::P1,
        Some(previous),
        current,
    );

    assert!(
        triggers.is_empty(),
        "counter-only sensor warmup/decay changes should not saturate the IR trace"
    );

    current.effective_signal_detected = true;
    super::super::collect_cgb_ir_status_trigger(
        &mut triggers,
        super::super::PlayerSlot::P1,
        Some(previous),
        current,
    );

    assert_eq!(triggers.len(), 1);
}

#[test]
fn cgb_ir_trace_capture_records_linked_session_context_and_writes_artifact() {
    let root = temp_test_root("cgb-ir-trace-capture");
    let artifact_path = root.join("nested").join("trace.txt");
    let mut capture = super::super::DesktopCgbIrTraceCapture {
        output_path: Some(artifact_path.clone()),
        watched_addresses: BTreeSet::from([0xA000, 0xC000, 0xFF80]),
        watched_trigger_addresses: BTreeSet::from([0xC000, 0xFF56]),
        max_records: 1,
        records: VecDeque::new(),
        last_p1_status: None,
        last_p2_status: None,
        last_p1_pressed_mask: None,
        last_p2_pressed_mask: None,
    };
    let mut session =
        super::super::linked_session::DesktopEmulationSession::new_linked_cgb_infrared_two_player(
            cgb_skip_boot_summary_machine(),
            cgb_skip_boot_summary_machine(),
        )
        .expect("linked CGB IR session should build");

    {
        let p1 = session
            .machine_for_player_slot_mut(super::super::PlayerSlot::P1)
            .expect("P1 should exist in CGB IR session");
        p1.write_bus(0xC000, 0xAB);
        p1.write_bus(0xFF80, 0x42);
        p1.write_bus(0xFF56, 0xC0);
    }
    capture.record_t_cycle(&session);

    assert_eq!(capture.records.len(), 1);
    let first = capture.records.back().expect("first trace record");
    assert!(first.triggers.iter().any(|trigger| {
        matches!(
            trigger,
            super::super::DesktopCgbIrTraceTrigger::StatusChanged {
                slot: super::super::PlayerSlot::P1,
                previous: None,
                ..
            }
        )
    }));
    assert!(first.triggers.iter().any(|trigger| {
        matches!(
            trigger,
            super::super::DesktopCgbIrTraceTrigger::JoypadPressedMaskChanged {
                slot: super::super::PlayerSlot::P1,
                previous: None,
                current: 0
            }
        )
    }));
    assert!(first.p1.watched_values.iter().any(|value| {
        matches!(
            value,
            super::super::DesktopCgbIrTraceWatchedValue::Wram(sample)
                if sample.address == 0xC000 && sample.value == 0xAB
        )
    }));
    assert!(first.p1.watched_values.iter().any(|value| {
        matches!(
            value,
            super::super::DesktopCgbIrTraceWatchedValue::Hram {
                address: 0xFF80,
                offset: 0,
                value: 0x42
            }
        )
    }));
    assert!(first.p1.watched_values.iter().any(|value| {
        matches!(
            value,
            super::super::DesktopCgbIrTraceWatchedValue::Unsupported { address: 0xA000 }
        )
    }));

    let mut manual_triggers = Vec::new();
    let manual_activity = CpuBusActivitySnapshot {
        kind: CpuBusAccessKind::DataWrite,
        address: 0xFF56,
        value: 0xC1,
    };
    super::super::collect_cgb_ir_rp_bus_trigger(
        &mut manual_triggers,
        super::super::PlayerSlot::P1,
        Some(manual_activity),
    );
    super::super::collect_cgb_ir_watched_bus_trigger(
        &mut manual_triggers,
        super::super::PlayerSlot::P1,
        Some(manual_activity),
        &BTreeSet::from([0xFF56]),
    );
    super::super::collect_cgb_ir_joypad_trigger(
        &mut manual_triggers,
        super::super::PlayerSlot::P2,
        Some(0x01),
        0x05,
    );
    assert!(manual_triggers.iter().any(|trigger| {
        matches!(
            trigger,
            super::super::DesktopCgbIrTraceTrigger::RpBusActivity {
                slot: super::super::PlayerSlot::P1,
                activity
            } if activity.address == 0xFF56
        )
    }));
    assert!(manual_triggers.iter().any(|trigger| {
        matches!(
            trigger,
            super::super::DesktopCgbIrTraceTrigger::WatchedBusActivity {
                slot: super::super::PlayerSlot::P1,
                activity
            } if activity.address == 0xFF56
        )
    }));
    assert!(manual_triggers.iter().any(|trigger| {
        matches!(
            trigger,
            super::super::DesktopCgbIrTraceTrigger::JoypadPressedMaskChanged {
                slot: super::super::PlayerSlot::P2,
                previous: Some(0x01),
                current: 0x05
            }
        )
    }));

    let p1 = session
        .machine_for_player_slot_mut(super::super::PlayerSlot::P1)
        .expect("P1 should still exist in CGB IR session");
    p1.write_bus(0xC000, 0xCD);
    p1.write_bus(0xFF56, 0xC1);
    capture.record_t_cycle(&session);

    assert_eq!(capture.records.len(), 1);
    let second = capture.records.back().expect("second trace record");
    assert!(second.triggers.iter().any(|trigger| {
        matches!(
            trigger,
            super::super::DesktopCgbIrTraceTrigger::StatusChanged {
                slot: super::super::PlayerSlot::P1,
                previous: Some(_),
                ..
            }
        )
    }));
    assert!(second.p1.watched_values.iter().any(|value| {
        matches!(
            value,
            super::super::DesktopCgbIrTraceWatchedValue::Wram(sample)
                if sample.address == 0xC000 && sample.value == 0xCD
        )
    }));

    let single = super::super::linked_session::DesktopEmulationSession::new_single(
        cgb_skip_boot_summary_machine(),
    );
    capture.record_t_cycle(&single);
    assert!(capture.last_p1_status.is_none());
    assert!(capture.last_p2_status.is_none());
    assert!(capture.last_p1_pressed_mask.is_none());
    assert!(capture.last_p2_pressed_mask.is_none());

    capture
        .write_artifact()
        .expect("CGB IR trace artifact should be writable");
    let rendered =
        fs::read_to_string(&artifact_path).expect("CGB IR trace artifact should be readable");
    assert!(rendered.contains("cgb_ir.triggers="));
    assert!(rendered.contains("0xC000=wram"));
    assert!(rendered.contains("0xFF80=hram"));
    assert!(rendered.contains("0xA000=unsupported"));
}

#[test]
fn current_cgb_ir_hud_snapshot_tracks_linked_status_and_ignores_single_sessions() {
    let single = super::super::linked_session::DesktopEmulationSession::new_single(
        cgb_skip_boot_summary_machine(),
    );
    assert_eq!(super::super::current_cgb_ir_hud_snapshot(&single), None);

    let mut linked =
        super::super::linked_session::DesktopEmulationSession::new_linked_cgb_infrared_two_player(
            cgb_skip_boot_summary_machine(),
            cgb_skip_boot_summary_machine(),
        )
        .expect("linked CGB IR session should build");
    linked
        .machine_for_player_slot_mut(super::super::PlayerSlot::P1)
        .expect("P1 should exist in CGB IR session")
        .write_bus(0xFF56, 0xC1);

    let snapshot = super::super::current_cgb_ir_hud_snapshot(&linked)
        .expect("linked CGB IR session should expose helper HUD state");
    assert!(snapshot.p1.emitter_on);
    assert!(snapshot.p1.read_enabled);
    assert!(snapshot.p1.optical_input_active);
    assert!(!snapshot.p2.emitter_on);
    assert!(!snapshot.p2.read_enabled);
    assert!(!snapshot.p2.optical_input_active);

    let mut accessory =
        super::super::linked_session::DesktopEmulationSession::new_pokemon_pikachu_color(
            cgb_skip_boot_summary_machine(),
            PokemonPikachuColorGift::Watts1,
            gb_core::PokemonPikachuColorRegion::Auto,
        );
    accessory
        .machine_for_player_slot_mut(super::super::PlayerSlot::P1)
        .expect("P1 should exist in Pokemon Pikachu Color session")
        .write_bus(0xFF56, 0xC1);
    accessory.step_t_cycle();

    let snapshot = super::super::current_cgb_ir_hud_snapshot(&accessory)
        .expect("Pokemon Pikachu Color session should expose helper HUD state");
    assert!(snapshot.p1.emitter_on);
    assert!(snapshot.p1.read_enabled);
    assert!(snapshot.p2.read_enabled);
    assert!(snapshot.p2.optical_input_active);
}

#[test]
fn watched_cpu_addresses_consider_bus_activity_and_address_events() {
    let machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    let mut cpu = machine.cpu().snapshot();
    cpu.last_bus_activity = Some(CpuBusActivitySnapshot {
        kind: CpuBusAccessKind::DataRead,
        address: 0xFF82,
        value: 0x30,
    });
    cpu.last_address_event = Some(CpuAddressEvent {
        kind: CpuAddressEventKind::ReadWithIncDec,
        access_address: Some(0xC409),
        idu_address: Some(0xC41F),
        update_direction: Some(CpuAddressUpdateDirection::Increment),
    });

    assert_eq!(
        watched_cpu_addresses(&cpu, &BTreeSet::from([0xFF82, 0xC409, 0xC41F])),
        vec![0xC409, 0xC41F, 0xFF82]
    );
}

#[test]
fn watched_pc_ranges_match_current_program_counter() {
    let machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    let mut cpu = machine.cpu().snapshot();
    cpu.registers.pc = 0x0604;

    assert_eq!(
        watched_pc_ranges(
            &cpu,
            &[
                super::super::PcWatchRange {
                    start: 0x03C0,
                    end: 0x03EF,
                },
                super::super::PcWatchRange {
                    start: 0x05FD,
                    end: 0x062F,
                },
                super::super::PcWatchRange {
                    start: 0x0600,
                    end: 0x0608,
                },
            ]
        ),
        vec![
            super::super::PcWatchRange {
                start: 0x05FD,
                end: 0x062F,
            },
            super::super::PcWatchRange {
                start: 0x0600,
                end: 0x0608,
            },
        ]
    );
}

#[test]
fn entered_pc_ranges_only_reports_new_membership() {
    let current = vec![
        super::super::PcWatchRange {
            start: 0x03C0,
            end: 0x03EF,
        },
        super::super::PcWatchRange {
            start: 0x4C00,
            end: 0x4C41,
        },
    ];
    let active = BTreeSet::from([super::super::PcWatchRange {
        start: 0x03C0,
        end: 0x03EF,
    }]);

    assert_eq!(
        entered_pc_ranges(&current, &active),
        vec![super::super::PcWatchRange {
            start: 0x4C00,
            end: 0x4C41,
        }]
    );
}

#[test]
fn watched_bus_value_change_reports_first_observation_and_changes() {
    let watched_addresses = BTreeSet::from([0xFF82]);
    let activity = Some(CpuBusActivitySnapshot {
        kind: CpuBusAccessKind::DataRead,
        address: 0xFF82,
        value: 0x31,
    });

    assert_eq!(
        watched_bus_value_change(activity, &watched_addresses, &BTreeMap::new()),
        Some(
            super::super::DesktopEdgeTraceTrigger::AddressValueObserved {
                kind: CpuBusAccessKind::DataRead,
                address: 0xFF82,
                previous: None,
                current: 0x31,
            }
        )
    );

    assert_eq!(
        watched_bus_value_change(
            activity,
            &watched_addresses,
            &BTreeMap::from([(0xFF82, 0x31)]),
        ),
        None
    );

    assert_eq!(
        watched_bus_value_change(
            activity,
            &watched_addresses,
            &BTreeMap::from([(0xFF82, 0x30)]),
        ),
        Some(
            super::super::DesktopEdgeTraceTrigger::AddressValueObserved {
                kind: CpuBusAccessKind::DataRead,
                address: 0xFF82,
                previous: Some(0x30),
                current: 0x31,
            }
        )
    );
}

#[test]
fn desktop_trace_renderer_includes_apu_last_write_when_present() {
    let machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF1A, 0x80);
    apu.write_register(0xFF1E, 0x80);
    apu.write_register(0xFF1A, 0x00);

    let rendered = render_desktop_trace_record(&super::super::DesktopTraceRecord {
        t_cycle: 123,
        cpu: machine.cpu().snapshot(),
        apu: apu.snapshot(),
        interrupts: machine.interrupts().snapshot(),
        joypad: machine.joypad().snapshot(),
        cartridge_trace: "state=NoMbc".to_string(),
    });

    assert!(rendered.contains("apu.last_write=write@0xFF1A=0x00"));
    assert!(rendered.contains("state=NoMbc"));
    assert!(rendered.contains("before("));
    assert!(rendered.contains("after("));
}

#[test]
fn desktop_watch_trace_renderer_and_capture_from_env_include_match_context() {
    let _guard = crate::lock_sdl_test();
    let root = temp_test_root("watch-trace-capture");
    let output_path = root.join("artifacts").join("desktop-watch-trace.txt");
    unsafe {
        std::env::set_var(super::super::DESKTOP_WATCH_TRACE_PATH_ENV_VAR, &output_path);
        std::env::set_var(
            super::super::DESKTOP_WATCH_TRACE_ADDRESSES_ENV_VAR,
            "FF82,C400,C409",
        );
        std::env::set_var(super::super::DESKTOP_WATCH_TRACE_EVENTS_ENV_VAR, "2");
    }
    let mut capture =
        super::super::DesktopWatchTraceCapture::from_env().expect("watch trace capture from env");
    unsafe {
        std::env::remove_var(super::super::DESKTOP_WATCH_TRACE_PATH_ENV_VAR);
        std::env::remove_var(super::super::DESKTOP_WATCH_TRACE_ADDRESSES_ENV_VAR);
        std::env::remove_var(super::super::DESKTOP_WATCH_TRACE_EVENTS_ENV_VAR);
    }

    assert_eq!(capture.output_path.as_deref(), Some(output_path.as_path()));
    assert_eq!(
        capture.watched_addresses,
        BTreeSet::from([0xC400, 0xC409, 0xFF82])
    );
    assert_eq!(capture.max_records, 2);

    let machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    let mut cpu = machine.cpu().snapshot();
    cpu.last_bus_activity = Some(CpuBusActivitySnapshot {
        kind: CpuBusAccessKind::DataRead,
        address: 0xFF82,
        value: 0x30,
    });
    cpu.last_address_event = Some(CpuAddressEvent {
        kind: CpuAddressEventKind::Write,
        access_address: Some(0xC400),
        idu_address: None,
        update_direction: None,
    });

    let rendered = render_desktop_watch_trace_record(&super::super::DesktopWatchTraceRecord {
        t_cycle: 123,
        matched_addresses: vec![0xC400, 0xFF82],
        cpu,
        interrupts: machine.interrupts().snapshot(),
        joypad: machine.joypad().snapshot(),
        ppu_mode: machine.ppu().access_mode(),
        ppu_ly: machine.ppu().ly(),
        ppu_line_dot: machine.ppu().line_dot(),
        cartridge_trace: "state=Huc1 io_mode=Ram".to_string(),
    });
    assert!(rendered.contains("watch.hit_addresses=[0xC400,0xFF82]"));
    assert!(rendered.contains("ppu.mode="));
    assert!(rendered.contains("state=Huc1"));

    capture
        .records
        .push_back(super::super::DesktopWatchTraceRecord {
            t_cycle: 1,
            matched_addresses: vec![0xFF82],
            cpu: machine.cpu().snapshot(),
            interrupts: machine.interrupts().snapshot(),
            joypad: machine.joypad().snapshot(),
            ppu_mode: machine.ppu().access_mode(),
            ppu_ly: machine.ppu().ly(),
            ppu_line_dot: machine.ppu().line_dot(),
            cartridge_trace: "state=Empty".to_string(),
        });
    capture
        .records
        .push_back(super::super::DesktopWatchTraceRecord {
            t_cycle: 2,
            matched_addresses: vec![0xC400],
            cpu: machine.cpu().snapshot(),
            interrupts: machine.interrupts().snapshot(),
            joypad: machine.joypad().snapshot(),
            ppu_mode: machine.ppu().access_mode(),
            ppu_ly: machine.ppu().ly(),
            ppu_line_dot: machine.ppu().line_dot(),
            cartridge_trace: "state=Empty".to_string(),
        });
    capture
        .write_artifact()
        .expect("watch trace artifact should be writable");
    let artifact = fs::read_to_string(&output_path).expect("watch trace artifact should exist");
    assert_eq!(artifact.lines().count(), 2);
    assert!(artifact.contains("watch.hit_addresses=[0xFF82]"));
    assert!(artifact.contains("watch.hit_addresses=[0xC400]"));
}

#[test]
fn desktop_watch_trace_record_t_cycle_captures_matching_bus_activity_and_noops_when_disabled() {
    let root = temp_test_root("watch-trace-record");
    let mut machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_test_rom(32 * 1024, 0x00, 0x00, 0x00))
        .expect("plain ROM should load");

    super::super::DesktopWatchTraceCapture {
        output_path: None,
        watched_addresses: BTreeSet::from([0x0100, 0x0101, 0x0102, 0x0103]),
        max_records: 1,
        records: VecDeque::new(),
    }
    .write_artifact()
    .expect("disabled watch trace capture should be a no-op");

    let mut disabled_capture = super::super::DesktopWatchTraceCapture {
        output_path: None,
        watched_addresses: BTreeSet::from([0x0100, 0x0101, 0x0102, 0x0103]),
        max_records: 1,
        records: VecDeque::new(),
    };
    for _ in 0..4 {
        machine.step_t_cycle();
        disabled_capture.record_t_cycle(&machine);
    }
    assert!(disabled_capture.records.is_empty());

    let mut capture = super::super::DesktopWatchTraceCapture {
        output_path: Some(root.join("watch-trace.txt")),
        watched_addresses: BTreeSet::from([0x0100, 0x0101, 0x0102, 0x0103]),
        max_records: 1,
        records: VecDeque::new(),
    };
    for _ in 0..8 {
        machine.step_t_cycle();
        capture.record_t_cycle(&machine);
        if !capture.records.is_empty() {
            break;
        }
    }

    let record = capture
        .records
        .front()
        .expect("watch trace should retain at least one matching record");
    assert!(!record.matched_addresses.is_empty());
    assert!(
        record
            .matched_addresses
            .iter()
            .all(|address| [0x0100, 0x0101, 0x0102, 0x0103].contains(address))
    );
    assert!(record.cartridge_trace.contains("state=NoMbc"));
}

#[test]
fn desktop_pc_watch_trace_renderer_and_capture_from_env_include_match_context() {
    let _guard = crate::lock_sdl_test();
    let root = temp_test_root("pc-watch-trace-capture");
    let output_path = root.join("artifacts").join("desktop-pc-watch-trace.txt");
    unsafe {
        std::env::set_var(
            super::super::DESKTOP_PC_WATCH_TRACE_PATH_ENV_VAR,
            &output_path,
        );
        std::env::set_var(
            super::super::DESKTOP_PC_WATCH_TRACE_RANGES_ENV_VAR,
            "03C0-03EF,05FD..=062F",
        );
        std::env::set_var(super::super::DESKTOP_PC_WATCH_TRACE_EVENTS_ENV_VAR, "2");
    }
    let mut capture = super::super::DesktopPcWatchTraceCapture::from_env()
        .expect("PC watch trace capture from env");
    unsafe {
        std::env::remove_var(super::super::DESKTOP_PC_WATCH_TRACE_PATH_ENV_VAR);
        std::env::remove_var(super::super::DESKTOP_PC_WATCH_TRACE_RANGES_ENV_VAR);
        std::env::remove_var(super::super::DESKTOP_PC_WATCH_TRACE_EVENTS_ENV_VAR);
    }

    assert_eq!(capture.output_path.as_deref(), Some(output_path.as_path()));
    assert_eq!(
        capture.watched_ranges,
        vec![
            super::super::PcWatchRange {
                start: 0x03C0,
                end: 0x03EF,
            },
            super::super::PcWatchRange {
                start: 0x05FD,
                end: 0x062F,
            },
        ]
    );
    assert_eq!(capture.max_records, 2);

    let machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    let mut cpu = machine.cpu().snapshot();
    cpu.registers.pc = 0x0604;
    cpu.last_bus_activity = Some(CpuBusActivitySnapshot {
        kind: CpuBusAccessKind::DataRead,
        address: 0xFF82,
        value: 0x30,
    });

    let rendered = render_desktop_pc_watch_trace_record(&super::super::DesktopPcWatchTraceRecord {
        t_cycle: 123,
        matched_ranges: vec![
            super::super::PcWatchRange {
                start: 0x05FD,
                end: 0x062F,
            },
            super::super::PcWatchRange {
                start: 0x0600,
                end: 0x0608,
            },
        ],
        cpu,
        interrupts: machine.interrupts().snapshot(),
        joypad: machine.joypad().snapshot(),
        ppu_mode: machine.ppu().access_mode(),
        ppu_ly: machine.ppu().ly(),
        ppu_line_dot: machine.ppu().line_dot(),
        cartridge_trace: "state=Huc1 io_mode=Ram".to_string(),
    });
    assert!(rendered.contains("pc_watch.hit_ranges=[0x05FD..=0x062F,0x0600..=0x0608]"));
    assert!(rendered.contains("cpu.pc=0x0604"));
    assert!(rendered.contains("state=Huc1"));

    capture
        .records
        .push_back(super::super::DesktopPcWatchTraceRecord {
            t_cycle: 1,
            matched_ranges: vec![super::super::PcWatchRange {
                start: 0x03C0,
                end: 0x03EF,
            }],
            cpu: machine.cpu().snapshot(),
            interrupts: machine.interrupts().snapshot(),
            joypad: machine.joypad().snapshot(),
            ppu_mode: machine.ppu().access_mode(),
            ppu_ly: machine.ppu().ly(),
            ppu_line_dot: machine.ppu().line_dot(),
            cartridge_trace: "state=Empty".to_string(),
        });
    capture
        .records
        .push_back(super::super::DesktopPcWatchTraceRecord {
            t_cycle: 2,
            matched_ranges: vec![super::super::PcWatchRange {
                start: 0x05FD,
                end: 0x062F,
            }],
            cpu: machine.cpu().snapshot(),
            interrupts: machine.interrupts().snapshot(),
            joypad: machine.joypad().snapshot(),
            ppu_mode: machine.ppu().access_mode(),
            ppu_ly: machine.ppu().ly(),
            ppu_line_dot: machine.ppu().line_dot(),
            cartridge_trace: "state=Empty".to_string(),
        });
    capture
        .write_artifact()
        .expect("PC watch trace artifact should be writable");
    let artifact = fs::read_to_string(&output_path).expect("PC watch trace artifact should exist");
    assert_eq!(artifact.lines().count(), 2);
    assert!(artifact.contains("pc_watch.hit_ranges=[0x03C0..=0x03EF]"));
    assert!(artifact.contains("pc_watch.hit_ranges=[0x05FD..=0x062F]"));
}

#[test]
fn desktop_pc_watch_trace_record_t_cycle_captures_matching_program_counters_and_noops_when_disabled()
 {
    let root = temp_test_root("pc-watch-trace-record");
    let mut machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_test_rom(32 * 1024, 0x00, 0x00, 0x00))
        .expect("plain ROM should load");

    super::super::DesktopPcWatchTraceCapture {
        output_path: None,
        watched_ranges: vec![super::super::PcWatchRange {
            start: 0x0100,
            end: 0x0103,
        }],
        max_records: 1,
        records: VecDeque::new(),
    }
    .write_artifact()
    .expect("disabled PC watch trace capture should be a no-op");

    let mut disabled_capture = super::super::DesktopPcWatchTraceCapture {
        output_path: None,
        watched_ranges: vec![super::super::PcWatchRange {
            start: 0x0100,
            end: 0x0103,
        }],
        max_records: 1,
        records: VecDeque::new(),
    };
    for _ in 0..4 {
        machine.step_t_cycle();
        disabled_capture.record_t_cycle(&machine);
    }
    assert!(disabled_capture.records.is_empty());

    let mut capture = super::super::DesktopPcWatchTraceCapture {
        output_path: Some(root.join("pc-watch-trace.txt")),
        watched_ranges: vec![super::super::PcWatchRange {
            start: 0x0100,
            end: 0x0103,
        }],
        max_records: 1,
        records: VecDeque::new(),
    };
    for _ in 0..8 {
        machine.step_t_cycle();
        capture.record_t_cycle(&machine);
        if !capture.records.is_empty() {
            break;
        }
    }

    let record = capture
        .records
        .front()
        .expect("PC watch trace should retain at least one matching record");
    assert_eq!(
        record.matched_ranges,
        vec![super::super::PcWatchRange {
            start: 0x0100,
            end: 0x0103,
        }]
    );
    assert!(record.cpu.registers.pc >= 0x0100 && record.cpu.registers.pc <= 0x0103);
    assert!(record.cartridge_trace.contains("state=NoMbc"));
}

#[test]
fn desktop_edge_trace_renderer_and_capture_from_env_include_entry_and_change_context() {
    let _guard = crate::lock_sdl_test();
    let root = temp_test_root("edge-trace-capture");
    let output_path = root.join("artifacts").join("desktop-edge-trace.txt");
    unsafe {
        std::env::set_var(super::super::DESKTOP_EDGE_TRACE_PATH_ENV_VAR, &output_path);
        std::env::set_var(
            super::super::DESKTOP_EDGE_TRACE_ADDRESSES_ENV_VAR,
            "FF82,C409",
        );
        std::env::set_var(
            super::super::DESKTOP_EDGE_TRACE_PC_RANGES_ENV_VAR,
            "4C00-4C41",
        );
        std::env::set_var(super::super::DESKTOP_EDGE_TRACE_EVENTS_ENV_VAR, "2");
    }
    let mut capture =
        super::super::DesktopEdgeTraceCapture::from_env().expect("edge trace capture from env");
    unsafe {
        std::env::remove_var(super::super::DESKTOP_EDGE_TRACE_PATH_ENV_VAR);
        std::env::remove_var(super::super::DESKTOP_EDGE_TRACE_ADDRESSES_ENV_VAR);
        std::env::remove_var(super::super::DESKTOP_EDGE_TRACE_PC_RANGES_ENV_VAR);
        std::env::remove_var(super::super::DESKTOP_EDGE_TRACE_EVENTS_ENV_VAR);
    }

    assert_eq!(capture.output_path.as_deref(), Some(output_path.as_path()));
    assert_eq!(capture.watched_addresses, BTreeSet::from([0xC409, 0xFF82]));
    assert_eq!(
        capture.watched_pc_ranges,
        vec![super::super::PcWatchRange {
            start: 0x4C00,
            end: 0x4C41,
        }]
    );
    assert_eq!(capture.max_records, 2);

    let machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    let mut cpu = machine.cpu().snapshot();
    cpu.registers.pc = 0x4C0E;
    cpu.last_bus_activity = Some(CpuBusActivitySnapshot {
        kind: CpuBusAccessKind::DataRead,
        address: 0xFF82,
        value: 0x31,
    });

    let rendered = render_desktop_edge_trace_record(&super::super::DesktopEdgeTraceRecord {
        t_cycle: 123,
        current_pc_ranges: vec![super::super::PcWatchRange {
            start: 0x4C00,
            end: 0x4C41,
        }],
        triggers: vec![
            super::super::DesktopEdgeTraceTrigger::EnteredPcRange(super::super::PcWatchRange {
                start: 0x4C00,
                end: 0x4C41,
            }),
            super::super::DesktopEdgeTraceTrigger::AddressValueObserved {
                kind: CpuBusAccessKind::DataRead,
                address: 0xFF82,
                previous: Some(0x30),
                current: 0x31,
            },
        ],
        cpu,
        interrupts: machine.interrupts().snapshot(),
        joypad: machine.joypad().snapshot(),
        ppu_mode: machine.ppu().access_mode(),
        ppu_ly: machine.ppu().ly(),
        ppu_line_dot: machine.ppu().line_dot(),
        cartridge_trace: "state=Huc1 io_mode=Ram".to_string(),
    });
    assert!(rendered.contains("edge.current_pc_ranges=[0x4C00..=0x4C41]"));
    assert!(
        rendered.contains(
            "edge.triggers=[enter_pc(0x4C00..=0x4C41),change(data_read@0xFF82:0x30->0x31)]"
        )
    );
    assert!(rendered.contains("cpu.pc=0x4C0E"));
    assert!(rendered.contains("state=Huc1"));

    capture
        .records
        .push_back(super::super::DesktopEdgeTraceRecord {
            t_cycle: 1,
            current_pc_ranges: vec![super::super::PcWatchRange {
                start: 0x4C00,
                end: 0x4C41,
            }],
            triggers: vec![super::super::DesktopEdgeTraceTrigger::EnteredPcRange(
                super::super::PcWatchRange {
                    start: 0x4C00,
                    end: 0x4C41,
                },
            )],
            cpu: machine.cpu().snapshot(),
            interrupts: machine.interrupts().snapshot(),
            joypad: machine.joypad().snapshot(),
            ppu_mode: machine.ppu().access_mode(),
            ppu_ly: machine.ppu().ly(),
            ppu_line_dot: machine.ppu().line_dot(),
            cartridge_trace: "state=Empty".to_string(),
        });
    capture
        .records
        .push_back(super::super::DesktopEdgeTraceRecord {
            t_cycle: 2,
            current_pc_ranges: vec![super::super::PcWatchRange {
                start: 0x4C00,
                end: 0x4C41,
            }],
            triggers: vec![
                super::super::DesktopEdgeTraceTrigger::AddressValueObserved {
                    kind: CpuBusAccessKind::DataRead,
                    address: 0xFF82,
                    previous: Some(0x30),
                    current: 0x31,
                },
            ],
            cpu: machine.cpu().snapshot(),
            interrupts: machine.interrupts().snapshot(),
            joypad: machine.joypad().snapshot(),
            ppu_mode: machine.ppu().access_mode(),
            ppu_ly: machine.ppu().ly(),
            ppu_line_dot: machine.ppu().line_dot(),
            cartridge_trace: "state=Empty".to_string(),
        });
    capture
        .write_artifact()
        .expect("edge trace artifact should be writable");
    let artifact = fs::read_to_string(&output_path).expect("edge trace artifact should exist");
    assert_eq!(artifact.lines().count(), 2);
    assert!(artifact.contains("enter_pc(0x4C00..=0x4C41)"));
    assert!(artifact.contains("change(data_read@0xFF82:0x30->0x31)"));
}

#[test]
fn desktop_edge_trace_record_t_cycle_tracks_pc_entry_and_bus_changes() {
    let root = temp_test_root("edge-trace-record");
    let mut machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_test_rom(32 * 1024, 0x00, 0x00, 0x00))
        .expect("plain ROM should load");

    super::super::DesktopEdgeTraceCapture {
        output_path: None,
        watched_addresses: BTreeSet::from([
            0x0100, 0x0101, 0x0102, 0x0103, 0x0104, 0x0105, 0x0106, 0x0107,
        ]),
        watched_pc_ranges: vec![super::super::PcWatchRange {
            start: 0x0100,
            end: 0x0107,
        }],
        active_pc_ranges: BTreeSet::new(),
        last_observed_values: BTreeMap::new(),
        max_records: 1,
        records: VecDeque::new(),
    }
    .write_artifact()
    .expect("disabled edge trace capture should be a no-op");

    let mut disabled_capture = super::super::DesktopEdgeTraceCapture {
        output_path: None,
        watched_addresses: BTreeSet::from([
            0x0100, 0x0101, 0x0102, 0x0103, 0x0104, 0x0105, 0x0106, 0x0107,
        ]),
        watched_pc_ranges: vec![super::super::PcWatchRange {
            start: 0x0100,
            end: 0x0107,
        }],
        active_pc_ranges: BTreeSet::new(),
        last_observed_values: BTreeMap::new(),
        max_records: 1,
        records: VecDeque::new(),
    };
    for _ in 0..4 {
        machine.step_t_cycle();
        disabled_capture.record_t_cycle(&machine);
    }
    assert!(disabled_capture.records.is_empty());

    let mut capture = super::super::DesktopEdgeTraceCapture {
        output_path: Some(root.join("edge-trace.txt")),
        watched_addresses: BTreeSet::from([
            0x0100, 0x0101, 0x0102, 0x0103, 0x0104, 0x0105, 0x0106, 0x0107,
        ]),
        watched_pc_ranges: vec![super::super::PcWatchRange {
            start: 0x0100,
            end: 0x0107,
        }],
        active_pc_ranges: BTreeSet::new(),
        last_observed_values: BTreeMap::new(),
        max_records: 1,
        records: VecDeque::new(),
    };
    for _ in 0..32 {
        machine.step_t_cycle();
        capture.record_t_cycle(&machine);
        if !capture.records.is_empty() && !capture.last_observed_values.is_empty() {
            break;
        }
    }

    let record = capture
        .records
        .front()
        .expect("edge trace should retain at least one matching record");
    assert_eq!(
        record.current_pc_ranges,
        vec![super::super::PcWatchRange {
            start: 0x0100,
            end: 0x0107,
        }]
    );
    assert!(!record.triggers.is_empty());
    assert!(!capture.last_observed_values.is_empty());
    assert!(
        capture
            .active_pc_ranges
            .contains(&super::super::PcWatchRange {
                start: 0x0100,
                end: 0x0107,
            })
    );
}

#[test]
fn desktop_ch4_nr43_trace_capture_and_formatters_cover_env_filter_and_artifact() {
    let _guard = crate::lock_sdl_test();
    let root = temp_test_root("ch4-nr43-trace-capture");
    let output_path = root.join("artifacts").join("desktop-ch4-nr43-trace.txt");
    unsafe {
        std::env::set_var(
            super::super::DESKTOP_CH4_NR43_TRACE_PATH_ENV_VAR,
            &output_path,
        );
    }
    let mut capture =
        super::super::DesktopCh4Nr43TraceCapture::from_env().expect("CH4 trace capture from env");
    unsafe {
        std::env::remove_var(super::super::DESKTOP_CH4_NR43_TRACE_PATH_ENV_VAR);
    }

    assert_eq!(capture.output_path.as_deref(), Some(output_path.as_path()));
    assert!(capture.records.is_empty());

    let mut machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.write_bus(0xFF26, 0x80);
    capture.record_t_cycle(&machine);
    assert!(capture.records.is_empty());

    machine.write_bus(super::super::CH4_NR43_ADDRESS, 0x35);
    capture.record_t_cycle(&machine);
    assert_eq!(capture.records.len(), 1);
    let record = &capture.records[0];
    assert_eq!(record.apu_write.address, super::super::CH4_NR43_ADDRESS);
    let rendered = super::super::render_desktop_ch4_nr43_trace_record(record);
    assert!(rendered.contains("apu.last_write=write@0xFF22=0x35"));
    assert!(rendered.contains("ch4.nr43=0x35"));
    assert!(rendered.contains("ch4.last_nr43_live_write="));

    let rich_rendered = super::super::format_ch4_debug_snapshot(&ApuCh4DebugSnapshot {
        nr43: 0x3F,
        clock_shift: 7,
        short_width_mode: true,
        clock_divider_code: 3,
        alignment: 2,
        counter_timer: 96,
        noise_counter: 0x1234,
        countdown_reloaded: true,
        did_step_counter: true,
        counter_active: true,
        background_counting: false,
        started_with_dac_disabled: false,
        dmg_delayed_start: 0,
        runtime_active: true,
        runtime_dac_enabled: true,
        period_timer: 48,
        lfsr_state: 0x4567,
        current_digital_output: 0x0F,
        last_nr43_live_write: Some(ApuCh4Nr43LiveWriteTrace {
            runtime_active: true,
            same_shift_group: false,
            old_nr43: 0x15,
            ff_value: 0xFF,
            new_nr43: 0x3F,
            old_shift: 1,
            ff_shift: 15,
            glitch_1_value: 0xFF,
            glitch_2_value: Some(0x3F),
            glitch_1_shift: 14,
            glitch_2_shift: Some(7),
            new_shift: 7,
            effective_counter: 0x2345,
            countdown_reloaded: true,
            old_bit: false,
            ff_bit: true,
            glitch_1_bit: true,
            glitch_2_bit: Some(false),
            new_bit: true,
            decision_category: ApuCh4Nr43LiveWriteCategory::LowShiftFollowup,
            lfsr_action: ApuCh4Nr43LfsrAction::ForcedShortStepThenLowShiftCorruption,
            reload_seam: Some(ApuCh4Nr43PassTrace {
                kind: ApuCh4Nr43PassKind::ReloadSeam,
                value_from: 0x15,
                value_to: 0x15,
                shift_from: 1,
                shift_to: 1,
                bit_from: false,
                bit_to: false,
                category: ApuCh4Nr43LiveWriteCategory::None,
                action: ApuCh4Nr43LfsrAction::PlainStep,
                lfsr_before: 0x7FFF,
                lfsr_after: 0x3FFF,
            }),
            old_to_ff: Some(ApuCh4Nr43PassTrace {
                kind: ApuCh4Nr43PassKind::OldToFf,
                value_from: 0x15,
                value_to: 0xFF,
                shift_from: 1,
                shift_to: 15,
                bit_from: false,
                bit_to: true,
                category: ApuCh4Nr43LiveWriteCategory::Category1,
                action: ApuCh4Nr43LfsrAction::ForcedShortStep,
                lfsr_before: 0x3FFF,
                lfsr_after: 0x1FFF,
            }),
            ff_to_glitch_1: Some(ApuCh4Nr43PassTrace {
                kind: ApuCh4Nr43PassKind::FfToGlitch1,
                value_from: 0xFF,
                value_to: 0xFF,
                shift_from: 15,
                shift_to: 14,
                bit_from: true,
                bit_to: true,
                category: ApuCh4Nr43LiveWriteCategory::Category2,
                action: ApuCh4Nr43LfsrAction::ForcedShortStep,
                lfsr_before: 0x1FFF,
                lfsr_after: 0x0FFF,
            }),
            glitch_1_to_glitch_2: Some(ApuCh4Nr43PassTrace {
                kind: ApuCh4Nr43PassKind::Glitch1ToGlitch2,
                value_from: 0xFF,
                value_to: 0x3F,
                shift_from: 14,
                shift_to: 7,
                bit_from: true,
                bit_to: false,
                category: ApuCh4Nr43LiveWriteCategory::RisingEdgeForcedShort,
                action: ApuCh4Nr43LfsrAction::ForcedShortStep,
                lfsr_before: 0x0FFF,
                lfsr_after: 0x07FF,
            }),
            glitch_to_new: Some(ApuCh4Nr43PassTrace {
                kind: ApuCh4Nr43PassKind::GlitchToNew,
                value_from: 0x3F,
                value_to: 0x3F,
                shift_from: 7,
                shift_to: 7,
                bit_from: false,
                bit_to: true,
                category: ApuCh4Nr43LiveWriteCategory::None,
                action: ApuCh4Nr43LfsrAction::None,
                lfsr_before: 0x07FF,
                lfsr_after: 0x07FF,
            }),
            low_shift_followup: Some(ApuCh4Nr43PassTrace {
                kind: ApuCh4Nr43PassKind::LowShiftFollowup,
                value_from: 0x3F,
                value_to: 0x3F,
                shift_from: 7,
                shift_to: 7,
                bit_from: true,
                bit_to: true,
                category: ApuCh4Nr43LiveWriteCategory::LowShiftFollowup,
                action: ApuCh4Nr43LfsrAction::ForcedShortStepThenLowShiftCorruption,
                lfsr_before: 0x07FF,
                lfsr_after: 0x3F7F,
            }),
            lfsr_before: 0x7FFF,
            lfsr_after: 0x3F7F,
        }),
    });
    assert!(rich_rendered.contains("ch4.nr43=0x3F"));
    assert!(rich_rendered.contains("category=LowShiftFollowup"));
    assert!(rich_rendered.contains("action=ForcedShortStepThenLowShiftCorruption"));
    assert!(rich_rendered.contains("low_shift_followup:LowShiftFollowup"));
    assert!(rich_rendered.contains("lfsr=0x7FFF->0x3F7F"));

    capture
        .write_artifact()
        .expect("CH4 trace artifact should be writable");
    let artifact = fs::read_to_string(&output_path).expect("CH4 trace artifact should exist");
    assert_eq!(artifact.lines().count(), 1);
    assert!(artifact.contains("apu.last_write=write@0xFF22=0x35"));
    assert!(artifact.contains("ch4.nr43=0x35"));

    assert_eq!(
        super::super::format_ch4_live_nr43_trace(None),
        " ch4.last_nr43_live_write=none"
    );
    super::super::DesktopCh4Nr43TraceCapture::from_env()
        .expect("capture without env should still construct")
        .write_artifact()
        .expect("trace capture without a path should be a no-op");
}

#[test]
fn desktop_trace_artifact_errors_surface_target_paths_for_edge_and_ch4_traces() {
    let _guard = crate::lock_sdl_test();
    let root = temp_test_root("trace-artifact-errors");

    let edge_dir_path = root.join("edge-artifact-dir");
    fs::create_dir_all(&edge_dir_path).expect("edge artifact directory should be creatable");
    let edge_error = super::super::DesktopEdgeTraceCapture {
        output_path: Some(edge_dir_path.clone()),
        watched_addresses: BTreeSet::new(),
        watched_pc_ranges: Vec::new(),
        active_pc_ranges: BTreeSet::new(),
        last_observed_values: BTreeMap::new(),
        max_records: 1,
        records: VecDeque::new(),
    }
    .write_artifact()
    .expect_err("writing an edge trace to a directory should fail");
    assert!(edge_error.contains("failed to write desktop edge trace artifact"));
    assert!(edge_error.contains(&format!("{edge_dir_path:?}")));

    let blocked_parent = root.join("blocked-parent");
    fs::write(&blocked_parent, b"not a directory")
        .expect("blocked parent file should be creatable");
    let ch4_parent_error = super::super::DesktopCh4Nr43TraceCapture {
        output_path: Some(blocked_parent.join("artifact.txt")),
        records: Vec::new(),
    }
    .write_artifact()
    .expect_err("creating a CH4 trace directory under a file should fail");
    assert!(
        ch4_parent_error.contains("failed to create condensed CH4 NR43 trace artifact directory")
    );

    let ch4_dir_path = root.join("ch4-artifact-dir");
    fs::create_dir_all(&ch4_dir_path).expect("CH4 artifact directory should be creatable");
    let ch4_write_error = super::super::DesktopCh4Nr43TraceCapture {
        output_path: Some(ch4_dir_path.clone()),
        records: Vec::new(),
    }
    .write_artifact()
    .expect_err("writing a CH4 trace to a directory should fail");
    assert!(ch4_write_error.contains("failed to write condensed CH4 NR43 trace artifact"));
    assert!(ch4_write_error.contains(&format!("{ch4_dir_path:?}")));
}

#[test]
fn desktop_trace_capture_from_env_keeps_a_ring_buffer_and_writes_the_artifact() {
    let _guard = crate::lock_sdl_test();
    let root = temp_test_root("trace-capture");
    let output_path = root.join("artifacts").join("desktop-trace.txt");
    unsafe {
        std::env::set_var(super::super::DESKTOP_TRACE_PATH_ENV_VAR, &output_path);
        std::env::set_var(super::super::DESKTOP_TRACE_T_CYCLES_ENV_VAR, "2");
    }
    let mut capture =
        super::super::DesktopTraceCapture::from_env().expect("trace capture from env");
    unsafe {
        std::env::remove_var(super::super::DESKTOP_TRACE_PATH_ENV_VAR);
        std::env::remove_var(super::super::DESKTOP_TRACE_T_CYCLES_ENV_VAR);
    }

    assert_eq!(capture.output_path.as_deref(), Some(output_path.as_path()));
    assert!(capture.is_enabled());
    assert_eq!(capture.max_t_cycles, 2);

    let mut machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    for _ in 0..3 {
        machine.step_t_cycle();
        capture.record_t_cycle(&machine);
    }

    assert_eq!(capture.records.len(), 2);
    capture
        .write_artifact()
        .expect("trace artifact should be writable");
    let rendered = fs::read_to_string(&output_path).expect("trace artifact should exist");
    assert_eq!(rendered.lines().count(), 2);
    assert!(rendered.contains("cpu.pc=0x0100"));
    assert!(rendered.contains("apu.nr50=0x77"));
    assert!(rendered.contains("state=Empty"));

    super::super::DesktopTraceCapture {
        enabled: false,
        output_path: None,
        max_t_cycles: 2,
        records: std::collections::VecDeque::new(),
    }
    .write_artifact()
    .expect("disabled trace capture should be a no-op");
}

#[test]
fn desktop_ch4_nr43_trace_capture_records_live_writes_and_writes_the_artifact() {
    let _guard = crate::lock_sdl_test();
    let root = temp_test_root("ch4-nr43-trace-capture");
    let output_path = root.join("artifacts").join("ch4-nr43-trace.txt");
    unsafe {
        std::env::set_var(
            super::super::DESKTOP_CH4_NR43_TRACE_PATH_ENV_VAR,
            &output_path,
        );
    }
    let mut capture =
        super::super::DesktopCh4Nr43TraceCapture::from_env().expect("trace capture from env");
    unsafe {
        std::env::remove_var(super::super::DESKTOP_CH4_NR43_TRACE_PATH_ENV_VAR);
    }

    let mut machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.write_bus(0xFF26, 0x80);
    machine.write_bus(super::super::CH4_NR42_ADDRESS, 0xF0);
    machine.write_bus(super::super::CH4_NR43_ADDRESS, 0x00);

    capture.record_t_cycle(&machine);

    assert_eq!(capture.records.len(), 1);
    assert_eq!(
        capture.records[0].apu_write.address,
        super::super::CH4_NR43_ADDRESS
    );
    capture
        .write_artifact()
        .expect("CH4 NR43 trace artifact should be writable");
    let rendered = fs::read_to_string(&output_path).expect("CH4 NR43 trace artifact should exist");
    assert!(rendered.contains("apu.last_write=write@0xFF22=0x00"));

    super::super::DesktopCh4Nr43TraceCapture {
        output_path: None,
        records: Vec::new(),
    }
    .write_artifact()
    .expect("disabled CH4 NR43 trace capture should be a no-op");
}

#[test]
fn desktop_ch4_startup_trace_capture_records_register_writes_and_delayed_start_events() {
    let _guard = crate::lock_sdl_test();
    let root = temp_test_root("ch4-startup-trace-capture");
    let output_path = root.join("artifacts").join("ch4-startup-trace.txt");
    unsafe {
        std::env::set_var(
            super::super::DESKTOP_CH4_STARTUP_TRACE_PATH_ENV_VAR,
            &output_path,
        );
    }
    let mut capture =
        super::super::DesktopCh4StartupTraceCapture::from_env().expect("trace capture from env");
    unsafe {
        std::env::remove_var(super::super::DESKTOP_CH4_STARTUP_TRACE_PATH_ENV_VAR);
    }

    let mut machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.write_bus(super::super::MASTER_NR52_ADDRESS, 0x80);
    capture.record_t_cycle(&machine);

    assert_eq!(capture.records.len(), 1);
    assert_eq!(
        capture.records[0].event,
        super::super::DesktopCh4StartupTraceEventKind::RegisterWrite
    );
    assert_eq!(
        capture.records[0]
            .apu_write
            .as_ref()
            .expect("register-write event should keep the write observation")
            .address,
        super::super::MASTER_NR52_ADDRESS
    );

    let idle_machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    let mut previous_ch4 = idle_machine.apu().channel_4_debug_snapshot();
    previous_ch4.dmg_delayed_start = 1;
    capture.last_ch4 = Some(previous_ch4);
    capture.record_t_cycle(&idle_machine);

    assert_eq!(capture.records.len(), 2);
    assert_eq!(
        capture.records[1].event,
        super::super::DesktopCh4StartupTraceEventKind::DelayedStartFired
    );
    assert!(capture.records[1].apu_write.is_none());

    capture
        .write_artifact()
        .expect("CH4 startup trace artifact should be writable");
    let rendered =
        fs::read_to_string(&output_path).expect("CH4 startup trace artifact should exist");
    assert!(rendered.contains("event=RegisterWrite"));
    assert!(rendered.contains("event=DelayedStartFired"));

    super::super::DesktopCh4StartupTraceCapture {
        output_path: None,
        records: Vec::new(),
        last_ch4: None,
    }
    .write_artifact()
    .expect("disabled CH4 startup trace capture should be a no-op");
}

#[test]
fn desktop_cpu_window_trace_capture_from_env_writes_a_prebuilt_artifact() {
    let _guard = crate::lock_sdl_test();
    let root = temp_test_root("cpu-window-trace-capture");
    let output_path = root.join("artifacts").join("cpu-window-trace.txt");
    unsafe {
        std::env::set_var(
            super::super::DESKTOP_CPU_WINDOW_TRACE_PATH_ENV_VAR,
            &output_path,
        );
    }
    let mut capture = super::super::DesktopCpuWindowTraceCapture::from_env();
    unsafe {
        std::env::remove_var(super::super::DESKTOP_CPU_WINDOW_TRACE_PATH_ENV_VAR);
    }

    assert_eq!(capture.output_path.as_deref(), Some(output_path.as_path()));
    assert!(!capture.active);
    assert!(!capture.finished);

    let mut machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.step_t_cycle();
    capture
        .records
        .push(super::super::DesktopCpuWindowTraceRecord {
            t_cycle: machine.next_t_cycle().get().saturating_sub(1),
            cpu: machine.cpu().snapshot(),
            interrupts: machine.interrupts().snapshot(),
            ppu: machine.ppu().snapshot(),
            ppu_ly_read: machine.ppu().read_register(0xFF44),
            ppu_stat_read: machine.ppu().read_register(0xFF41),
        });

    capture
        .write_artifact()
        .expect("CPU window trace artifact should be writable");
    let rendered =
        fs::read_to_string(&output_path).expect("CPU window trace artifact should exist");
    assert!(rendered.contains("ppu.ly_read="));
    assert!(rendered.contains("ppu.stat_read="));

    super::super::DesktopCpuWindowTraceCapture {
        output_path: None,
        records: Vec::new(),
        active: false,
        finished: false,
    }
    .write_artifact()
    .expect("disabled CPU window trace capture should be a no-op");
}

#[test]
fn desktop_trace_helpers_cover_bus_address_joypad_and_apu_formatting() {
    assert_eq!(super::super::format_cpu_bus_activity(None), "none");
    assert_eq!(
        super::super::format_cpu_bus_activity(Some(CpuBusActivitySnapshot {
            kind: CpuBusAccessKind::OpcodeFetch,
            address: 0x0100,
            value: 0x31,
        })),
        "opcode_fetch@0x0100=0x31"
    );
    assert_eq!(
        super::super::format_cpu_bus_activity(Some(CpuBusActivitySnapshot {
            kind: CpuBusAccessKind::OperandRead,
            address: 0x0101,
            value: 0xFE,
        })),
        "operand_read@0x0101=0xFE"
    );
    assert_eq!(
        super::super::format_cpu_bus_activity(Some(CpuBusActivitySnapshot {
            kind: CpuBusAccessKind::DataRead,
            address: 0xC123,
            value: 0x45,
        })),
        "data_read@0xC123=0x45"
    );
    assert_eq!(
        super::super::format_cpu_bus_activity(Some(CpuBusActivitySnapshot {
            kind: CpuBusAccessKind::DataWrite,
            address: 0xFF40,
            value: 0x91,
        })),
        "data_write@0xFF40=0x91"
    );

    assert_eq!(
        super::super::format_cpu_address_event(Some(CpuAddressEvent {
            kind: CpuAddressEventKind::Read,
            access_address: Some(0xC000),
            idu_address: None,
            update_direction: None,
        })),
        "read@0xC000"
    );
    assert_eq!(
        super::super::format_cpu_address_event(Some(CpuAddressEvent {
            kind: CpuAddressEventKind::Read,
            access_address: None,
            idu_address: None,
            update_direction: None,
        })),
        "read@missing"
    );
    assert_eq!(
        super::super::format_cpu_address_event(Some(CpuAddressEvent {
            kind: CpuAddressEventKind::Write,
            access_address: Some(0xC001),
            idu_address: None,
            update_direction: None,
        })),
        "write@0xC001"
    );
    assert_eq!(
        super::super::format_cpu_address_event(Some(CpuAddressEvent {
            kind: CpuAddressEventKind::Write,
            access_address: None,
            idu_address: None,
            update_direction: None,
        })),
        "write@missing"
    );
    assert_eq!(
        super::super::format_cpu_address_event(Some(CpuAddressEvent {
            kind: CpuAddressEventKind::IncDec,
            access_address: None,
            idu_address: Some(0xC002),
            update_direction: Some(CpuAddressUpdateDirection::Increment),
        })),
        "inc@0xC002"
    );
    assert_eq!(
        super::super::format_cpu_address_event(Some(CpuAddressEvent {
            kind: CpuAddressEventKind::IncDec,
            access_address: None,
            idu_address: None,
            update_direction: None,
        })),
        "incdec@missing"
    );
    assert_eq!(
        super::super::format_cpu_address_event(Some(CpuAddressEvent {
            kind: CpuAddressEventKind::ReadWithIncDec,
            access_address: Some(0xC003),
            idu_address: Some(0xC004),
            update_direction: Some(CpuAddressUpdateDirection::Decrement),
        })),
        "read+dec@0xC003->0xC004"
    );
    assert_eq!(
        super::super::format_cpu_address_event(Some(CpuAddressEvent {
            kind: CpuAddressEventKind::ReadWithIncDec,
            access_address: None,
            idu_address: None,
            update_direction: None,
        })),
        "combined@missing"
    );
    assert_eq!(
        super::super::format_cpu_address_event(Some(CpuAddressEvent {
            kind: CpuAddressEventKind::WriteWithIncDec,
            access_address: Some(0xC005),
            idu_address: Some(0xC006),
            update_direction: Some(CpuAddressUpdateDirection::Increment),
        })),
        "write+inc@0xC005->0xC006"
    );
    assert_eq!(
        super::super::format_cpu_address_event(Some(CpuAddressEvent {
            kind: CpuAddressEventKind::WriteWithIncDec,
            access_address: None,
            idu_address: None,
            update_direction: None,
        })),
        "combined@missing"
    );
    assert_eq!(
        super::super::format_update_direction(CpuAddressUpdateDirection::Increment),
        "inc"
    );
    assert_eq!(
        super::super::format_update_direction(CpuAddressUpdateDirection::Decrement),
        "dec"
    );
    assert_eq!(super::super::visible_nr52(true, 0x0B), 0xFB);
    assert_eq!(super::super::visible_nr52(false, 0x0B), 0x70);
    assert_eq!(
        super::super::visible_joypad_low_nibble(&JoypadSnapshot {
            console_model: ConsoleModel::GameBoy,
            status: JoypadStatus::Ready,
            selection_bits: 0x00,
            pressed_mask: 0xFF,
        }),
        0x00
    );
    assert_eq!(
        super::super::visible_joypad_low_nibble(&JoypadSnapshot {
            console_model: ConsoleModel::GameBoy,
            status: JoypadStatus::Ready,
            selection_bits: 0x30,
            pressed_mask: 0xFF,
        }),
        0x0F
    );
    assert_eq!(
        super::super::format_ch4_live_nr43_trace(None),
        " ch4.last_nr43_live_write=none"
    );
    let ch4_trace = ApuCh4Nr43LiveWriteTrace {
        runtime_active: true,
        same_shift_group: false,
        old_nr43: 0x4C,
        ff_value: 0xFF,
        glitch_1_value: 0x4C,
        glitch_2_value: Some(0x7C),
        old_shift: 4,
        ff_shift: 15,
        glitch_1_shift: 4,
        glitch_2_shift: Some(7),
        new_shift: 3,
        new_nr43: 0x3C,
        effective_counter: 0x0123,
        countdown_reloaded: true,
        old_bit: false,
        ff_bit: true,
        glitch_1_bit: false,
        glitch_2_bit: Some(true),
        new_bit: true,
        decision_category: ApuCh4Nr43LiveWriteCategory::RisingEdgeForcedShort,
        lfsr_action: ApuCh4Nr43LfsrAction::ForcedShortStep,
        reload_seam: Some(ApuCh4Nr43PassTrace {
            kind: ApuCh4Nr43PassKind::ReloadSeam,
            value_from: 0x4C,
            value_to: 0x4C,
            shift_from: 4,
            shift_to: 4,
            bit_from: false,
            bit_to: false,
            category: ApuCh4Nr43LiveWriteCategory::None,
            action: ApuCh4Nr43LfsrAction::PlainStep,
            lfsr_before: 0x7FFF,
            lfsr_after: 0x3FFF,
        }),
        old_to_ff: Some(ApuCh4Nr43PassTrace {
            kind: ApuCh4Nr43PassKind::OldToFf,
            value_from: 0x4C,
            value_to: 0xFF,
            shift_from: 4,
            shift_to: 15,
            bit_from: false,
            bit_to: true,
            category: ApuCh4Nr43LiveWriteCategory::Category2,
            action: ApuCh4Nr43LfsrAction::PlainStep,
            lfsr_before: 0x3FFF,
            lfsr_after: 0x1FFF,
        }),
        ff_to_glitch_1: Some(ApuCh4Nr43PassTrace {
            kind: ApuCh4Nr43PassKind::FfToGlitch1,
            value_from: 0xFF,
            value_to: 0x4C,
            shift_from: 15,
            shift_to: 4,
            bit_from: true,
            bit_to: false,
            category: ApuCh4Nr43LiveWriteCategory::Category1,
            action: ApuCh4Nr43LfsrAction::SetFeedbackBits,
            lfsr_before: 0x1FFF,
            lfsr_after: 0x5FFF,
        }),
        glitch_1_to_glitch_2: Some(ApuCh4Nr43PassTrace {
            kind: ApuCh4Nr43PassKind::Glitch1ToGlitch2,
            value_from: 0x4C,
            value_to: 0x7C,
            shift_from: 4,
            shift_to: 7,
            bit_from: false,
            bit_to: true,
            category: ApuCh4Nr43LiveWriteCategory::RisingEdgeForcedShort,
            action: ApuCh4Nr43LfsrAction::ForcedShortStep,
            lfsr_before: 0x5FFF,
            lfsr_after: 0x2FFF,
        }),
        glitch_to_new: Some(ApuCh4Nr43PassTrace {
            kind: ApuCh4Nr43PassKind::GlitchToNew,
            value_from: 0x7C,
            value_to: 0x3C,
            shift_from: 7,
            shift_to: 3,
            bit_from: true,
            bit_to: true,
            category: ApuCh4Nr43LiveWriteCategory::None,
            action: ApuCh4Nr43LfsrAction::None,
            lfsr_before: 0x2FFF,
            lfsr_after: 0x2FFF,
        }),
        low_shift_followup: Some(ApuCh4Nr43PassTrace {
            kind: ApuCh4Nr43PassKind::LowShiftFollowup,
            value_from: 0x3C,
            value_to: 0x3C,
            shift_from: 3,
            shift_to: 3,
            bit_from: true,
            bit_to: true,
            category: ApuCh4Nr43LiveWriteCategory::LowShiftFollowup,
            action: ApuCh4Nr43LfsrAction::PlainStep,
            lfsr_before: 0x2FFF,
            lfsr_after: 0x17FF,
        }),
        lfsr_before: 0x7FFF,
        lfsr_after: 0x17FF,
    };
    let ch4_trace_text = super::super::format_ch4_live_nr43_trace(Some(&ch4_trace));
    assert!(ch4_trace_text.contains("ff(0xFF/shift=15/bit=true)"));
    assert!(ch4_trace_text.contains("glitch2(0x7C/shift=7/bit=true)"));
    assert!(ch4_trace_text.contains("old_to_ff:OldToFf"));
    assert!(ch4_trace_text.contains("low_shift_followup:LowShiftFollowup"));

    let base_state = ApuRegisterWriteState {
        powered: true,
        nr50: 0x77,
        nr51: 0xFF,
        nr52: 0xFB,
        channel_active_mask: 0x0B,
        channel_dac_mask: 0x0F,
        output: ApuOutputSnapshot {
            channel_digital_outputs: [0x01, 0x02, 0x03, 0x04],
            channel_dac_outputs: [0; 4],
            vin_analog_output: ApuStereoOutputSnapshot::default(),
            mixer_output: ApuStereoOutputSnapshot { left: 5, right: 6 },
            master_output: ApuStereoOutputSnapshot::default(),
            hpf_output: ApuStereoOutputSnapshot { left: 7, right: 8 },
            hpf_capacitor: Default::default(),
        },
    };
    assert_eq!(super::super::format_apu_last_register_write(None), "");
    let rendered =
        super::super::format_apu_last_register_write(Some(&ApuRegisterWriteObservation {
            address: 0xFF1A,
            value: 0x00,
            before: base_state,
            after: ApuRegisterWriteState {
                nr52: 0xF7,
                channel_active_mask: 0x07,
                ..base_state
            },
        }));
    assert!(rendered.contains("apu.last_write=write@0xFF1A=0x00"));
    assert!(rendered.contains("before("));
    assert!(rendered.contains("after("));
}
