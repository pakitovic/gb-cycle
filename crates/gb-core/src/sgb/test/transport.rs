use super::support::*;

#[test]
fn multiplayer_edges_keep_invalid_and_noop_controller_slots_inert() {
    let mut state = SgbMultiplayerState::default();
    state.apply_mlt_req_command(&sgb_mlt_req_packet(3));
    assert_eq!(state.player_count, 0);
    state.cycle_selected_player();
    assert_eq!(state.selected_player, 0);
    assert_eq!(state.selected_player_pressed_mask(), 0);

    state.player_count = 4;
    state.selected_player = 9;
    state.player_pressed_masks = [1, 2, 3, 4];
    assert_eq!(state.selected_player_index(), 3);
    assert_eq!(state.selected_player_pressed_mask(), 4);
    assert!(!state.set_player_pressed_mask(0, 0x12));
    assert!(state.set_player_pressed_mask(1, 0x12));
    assert!(!state.set_player_pressed_mask(1, 0x12));
    assert!(state.set_player_pressed_masks([0, 0, 0, 0]));
    assert!(!state.set_player_pressed_masks([0, 0, 0, 0]));
    assert!(!state.set_player_button_pressed(5, JoypadButton::A, true));
    assert!(state.set_player_button_pressed(1, JoypadButton::A, true));
    assert!(!state.set_player_button_pressed(1, JoypadButton::A, true));
    assert!(state.set_player_button_pressed(1, JoypadButton::A, false));
    assert!(!state.set_player_button_pressed(1, JoypadButton::A, false));

    let mut handheld = SgbHost::default();
    assert_eq!(handheld.host_platform(), HostPlatform::Handheld);
    assert!(!handheld.game_link_supported());
    assert!(!handheld.corrected_clock());
    assert!(
        !handheld
            .set_player_palette_override(sgb_screen_palette([0x1111, 0x2222, 0x3333, 0x4444,]))
    );
    assert!(!handheld.clear_player_palette_override());
    handheld.finish_real_boot_handoff();

    let mismatched_profile = SgbHost::new_with_profile(
        HostPlatform::Sgb,
        Some(SgbHostProfile::Sgb2Ntsc),
        StartupMode::SkipBoot,
    );
    assert_eq!(mismatched_profile.profile(), Some(SgbHostProfile::SgbNtsc));

    let mut sgb = SgbHost::new(HostPlatform::Sgb);
    assert!(sgb.set_player_pressed_mask(1, 0x12));
    assert_eq!(sgb.player_pressed_masks()[0], 0x12);
    assert!(sgb.set_player_pressed_masks([1, 2, 3, 4]));
    assert_eq!(sgb.player_pressed_masks(), [1, 2, 3, 4]);
}

#[test]
fn joyp_transport_decodes_single_packet_commands_lsb_first() {
    let mut host = accepted_sgb_host();
    let packet = sgb_command_packet(0x11, 1);
    write_joyp_packet(&mut host, packet);

    let snapshot = host.snapshot();
    assert_eq!(snapshot.packet_transport.reset_pulse_count, 1);
    assert_eq!(snapshot.packet_transport.data_pulse_count, 129);
    assert_eq!(
        snapshot.packet_transport.phase,
        SgbPacketTransportPhase::Idle
    );
    assert_eq!(snapshot.packet_transport.pending_data_bit, None);
    assert_eq!(snapshot.packet_transport.packet_bits_buffered, 128);
    assert_eq!(snapshot.packet_transport.packet_bytes_buffered, 16);
    assert_eq!(
        snapshot.packet_transport.last_trace.status,
        SgbPacketTraceStatus::Complete
    );
    assert_eq!(snapshot.packet_transport.last_trace.command_id, Some(0x11));
    assert_eq!(snapshot.packet_transport.last_trace.packet_count, 1);
    assert_eq!(snapshot.packet_transport.last_trace.packet_index, 1);
    assert_eq!(snapshot.command.last_command_id, Some(0x11));
    assert_eq!(snapshot.command.accepted_command_count, 1);
}

#[test]
fn joyp_transport_matches_cpp_sgb_ext_test_packet_edge_matrix() {
    let mut host = accepted_sgb_host();
    write_sgb_ext_test_packet_basic(&mut host, sgb_mlt_req_packet(1));

    let mut results = Vec::new();
    for (packet, writer) in [
        (
            sgb_mlt_req_packet(3),
            write_sgb_ext_test_packet_basic as fn(&mut SgbHost, [u8; SGB_PACKET_BYTES]),
        ),
        (sgb_mlt_req_packet(0), write_sgb_ext_test_packet_basic),
        (
            sgb_mlt_req_packet(3),
            write_sgb_ext_test_packet_corrupt_stop,
        ),
        (
            sgb_mlt_req_packet(0),
            write_sgb_ext_test_packet_corrupt_stop,
        ),
        (sgb_mlt_req_packet(3), write_sgb_ext_test_packet_avoid_30),
        (sgb_mlt_req_packet(0), write_sgb_ext_test_packet_avoid_30),
    ] {
        writer(&mut host, packet);
        results.push(sgb_ext_test_player_count(&mut host));
    }

    for (transition, controls) in [
        (
            &[
                SgbJoypLineState::Zero,
                SgbJoypLineState::One,
                SgbJoypLineState::Idle,
            ][..],
            [3, 1, 0],
        ),
        (
            &[
                SgbJoypLineState::One,
                SgbJoypLineState::Zero,
                SgbJoypLineState::Idle,
            ][..],
            [3, 1, 0],
        ),
        (
            &[
                SgbJoypLineState::Start,
                SgbJoypLineState::One,
                SgbJoypLineState::Idle,
            ][..],
            [3, 1, 0],
        ),
        (
            &[
                SgbJoypLineState::Start,
                SgbJoypLineState::Zero,
                SgbJoypLineState::Idle,
            ][..],
            [3, 1, 0],
        ),
        (
            &[
                SgbJoypLineState::One,
                SgbJoypLineState::Start,
                SgbJoypLineState::Idle,
            ][..],
            [3, 1, 0],
        ),
        (
            &[
                SgbJoypLineState::Zero,
                SgbJoypLineState::Start,
                SgbJoypLineState::Idle,
            ][..],
            [3, 1, 0],
        ),
    ] {
        for control in controls {
            write_sgb_ext_test_packet_with_second_byte_bit_transition(
                &mut host,
                sgb_mlt_req_packet(control),
                transition,
            );
            results.push(sgb_ext_test_player_count(&mut host));
        }
    }

    for control in [3, 1, 0] {
        write_sgb_ext_test_packet_short_start(&mut host, sgb_mlt_req_packet(control));
        results.push(sgb_ext_test_player_count(&mut host));
    }

    assert_eq!(
        results,
        [
            0x04, 0x01, 0x04, 0x01, 0x01, 0x01, 0x04, 0x02, 0x02, 0x01, 0x01, 0x01, 0x01, 0x01,
            0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01,
        ]
    );
}

#[test]
fn joyp_transport_records_corrupt_stop_without_rejecting_packet() {
    let mut host = accepted_sgb_host();
    write_sgb_ext_test_packet_corrupt_stop(&mut host, sgb_mlt_req_packet(3));

    let snapshot = host.snapshot();
    assert_eq!(snapshot.multiplayer.player_count, 4);
    assert_eq!(snapshot.packet_transport.invalid_stop_bit_count, 1);
    assert_eq!(
        snapshot.packet_transport.last_trace.status,
        SgbPacketTraceStatus::Complete
    );
    assert_eq!(snapshot.command.accepted_command_count, 1);
    assert_eq!(snapshot.command.invalid_packet_count, 1);
}

#[test]
fn joyp_transport_requires_idle_to_confirm_start() {
    let mut host = accepted_sgb_host();
    write_sgb_ext_test_packet_short_start(&mut host, sgb_mlt_req_packet(3));

    let snapshot = host.snapshot();
    assert_eq!(snapshot.multiplayer.player_count, 1);
    assert_eq!(snapshot.command.accepted_command_count, 0);
    assert!(snapshot.command.invalid_packet_count > 0);
    assert_eq!(
        snapshot.packet_transport.last_trace.status,
        SgbPacketTraceStatus::OrphanDataPulse
    );
}

#[test]
fn joyp_transport_uses_last_pending_data_state_before_idle() {
    let mut host = accepted_sgb_host();
    write_sgb_ext_test_packet_with_second_byte_bit_transition(
        &mut host,
        sgb_mlt_req_packet(0),
        &[
            SgbJoypLineState::Zero,
            SgbJoypLineState::One,
            SgbJoypLineState::Idle,
        ],
    );

    let snapshot = host.snapshot();
    assert_eq!(
        snapshot.multiplayer.player_count, 2,
        "$20->$10->$30 substitutes a one for the first MLT_REQ control bit"
    );
    assert_eq!(
        snapshot.packet_transport.last_trace.status,
        SgbPacketTraceStatus::Complete
    );
}

#[test]
fn joyp_transport_treats_start_mid_packet_as_incomplete_reset() {
    let mut host = accepted_sgb_host();
    write_sgb_ext_test_packet_with_second_byte_bit_transition(
        &mut host,
        sgb_mlt_req_packet(3),
        &[
            SgbJoypLineState::Zero,
            SgbJoypLineState::Start,
            SgbJoypLineState::Idle,
        ],
    );

    let snapshot = host.snapshot();
    assert_eq!(snapshot.multiplayer.player_count, 1);
    assert!(snapshot.command.invalid_packet_count > 0);
    assert_ne!(snapshot.command.last_command_id, Some(SGB_COMMAND_MLT_REQ));
}

#[test]
fn mlt_req_selects_one_two_and_four_player_modes() {
    let mut host = accepted_sgb_host();

    write_joyp_packet(&mut host, sgb_mlt_req_packet(1));
    assert_eq!(host.snapshot().multiplayer.player_count, 2);
    assert_eq!(host.snapshot().multiplayer.selected_player, 1);
    assert_eq!(sgb_player_id_value(&host), 0xFF);

    cycle_sgb_player(&mut host);
    assert_eq!(host.snapshot().multiplayer.selected_player, 2);
    assert_eq!(sgb_player_id_value(&host), 0xFE);

    write_joyp_packet(&mut host, sgb_mlt_req_packet(0));
    assert_eq!(host.snapshot().multiplayer.player_count, 1);
    assert_eq!(host.snapshot().multiplayer.selected_player, 1);
    assert_eq!(
        sgb_player_id_value(&host),
        0xFF,
        "one-player mode keeps both P1 rows deselected as ordinary open lines"
    );

    write_joyp_packet(&mut host, sgb_mlt_req_packet(3));
    assert_eq!(host.snapshot().multiplayer.player_count, 4);
    assert_eq!(host.snapshot().multiplayer.selected_player, 1);
    assert_eq!(sgb_player_id_value(&host), 0xFF);
    cycle_sgb_player(&mut host);
    assert_eq!(sgb_player_id_value(&host), 0xFE);
    cycle_sgb_player(&mut host);
    assert_eq!(sgb_player_id_value(&host), 0xFD);
    cycle_sgb_player(&mut host);
    assert_eq!(sgb_player_id_value(&host), 0xFC);
    assert_eq!(
        host.snapshot().multiplayer.player_cycle_count,
        8,
        "SGB player cycling also observes the P15 rises in MLT_REQ packet transport while multiplayer is already enabled"
    );
}

#[test]
fn mlt_req_packet_transport_cycles_player_before_mode_change() {
    let mut host = accepted_sgb_host();
    write_joyp_packet(&mut host, sgb_mlt_req_packet(3));
    assert_eq!(sgb_player_id_value(&host), 0xFF);

    write_joyp_packet(&mut host, sgb_mlt_req_packet(3));
    assert_eq!(
        sgb_player_id_value(&host),
        0xFD,
        "sending MLT_REQ 3 while already in four-player mode cycles the player through the command transport pulses before the command side effect"
    );

    write_joyp_packet(&mut host, sgb_mlt_req_packet(1));
    assert_eq!(
        sgb_player_id_value(&host),
        0xFE,
        "switching from four-player to two-player mode masks the already-cycled player index"
    );
}

#[test]
fn mlt_req_control_2_preserves_hardware_glitched_three_player_state() {
    let mut host = accepted_sgb_host();

    write_joyp_packet(&mut host, sgb_mlt_req_packet(0));
    write_joyp_packet(&mut host, sgb_mlt_req_packet(2));
    assert_eq!(host.snapshot().multiplayer.player_count, 3);
    assert_eq!(sgb_player_id_value(&host), 0xFF);
    cycle_sgb_player(&mut host);
    assert_eq!(
        sgb_player_id_value(&host),
        0xFF,
        "control 2 leaves an odd three-player selector that does not cycle on P15 rises"
    );

    write_joyp_packet(&mut host, sgb_mlt_req_packet(0));
    write_joyp_packet(&mut host, sgb_mlt_req_packet(3));
    write_joyp_packet(&mut host, sgb_mlt_req_packet(2));
    assert_eq!(
        sgb_player_id_value(&host),
        0xFD,
        "control 2 maps the transport-cycled four-player index onto the hardware-observed player 1/player 3 pair"
    );

    write_joyp_packet(&mut host, sgb_mlt_req_packet(0));
    write_joyp_packet(&mut host, sgb_mlt_req_packet(3));
    cycle_sgb_player(&mut host);
    cycle_sgb_player(&mut host);
    write_joyp_packet(&mut host, sgb_mlt_req_packet(2));
    assert_eq!(sgb_player_id_value(&host), 0xFF);
}

#[test]
fn joyp_transport_rejects_complete_packet_until_header_unlocks_sgb() {
    let mut host = SgbHost::new(HostPlatform::Sgb);
    write_joyp_packet(&mut host, sgb_command_packet(0x11, 1));

    let snapshot = host.snapshot();
    assert_eq!(
        snapshot.packet_transport.last_trace.status,
        SgbPacketTraceStatus::RejectedByHeader
    );
    assert_eq!(snapshot.command.rejected_packet_count, 1);
    assert_eq!(snapshot.command.accepted_command_count, 0);
}

#[test]
fn joyp_transport_records_invalid_packet_count_and_stop_bit() {
    let mut invalid_count = accepted_sgb_host();
    write_joyp_packet(&mut invalid_count, sgb_command_packet(0x11, 0));
    assert_eq!(
        invalid_count.snapshot().packet_transport.last_trace.status,
        SgbPacketTraceStatus::InvalidPacketLength
    );
    assert_eq!(invalid_count.snapshot().command.invalid_packet_count, 1);

    let mut invalid_stop = accepted_sgb_host();
    write_joyp_start(&mut invalid_stop);
    for byte in sgb_command_packet(0x11, 1) {
        for bit_index in 0..8 {
            write_joyp_data_bit(&mut invalid_stop, (byte >> bit_index) & 0x01);
        }
    }
    write_joyp_data_bit(&mut invalid_stop, 1);
    assert_eq!(
        invalid_stop.snapshot().packet_transport.last_trace.status,
        SgbPacketTraceStatus::Complete
    );
    assert_eq!(
        invalid_stop
            .snapshot()
            .packet_transport
            .invalid_stop_bit_count,
        1
    );
    assert_eq!(invalid_stop.snapshot().command.accepted_command_count, 1);
    assert_eq!(invalid_stop.snapshot().command.invalid_packet_count, 1);
}

#[test]
fn joyp_transport_records_incomplete_reset_and_orphan_data_pulse() {
    let mut incomplete = accepted_sgb_host();
    write_joyp_start(&mut incomplete);
    write_joyp_data_bit(&mut incomplete, 1);
    write_joyp_start(&mut incomplete);
    assert_eq!(
        incomplete.snapshot().packet_transport.last_trace.status,
        SgbPacketTraceStatus::IncompleteReset
    );
    assert_eq!(incomplete.snapshot().command.invalid_packet_count, 1);

    let mut orphan = accepted_sgb_host();
    write_joyp_data_bit(&mut orphan, 1);
    assert_eq!(
        orphan.snapshot().packet_transport.last_trace.status,
        SgbPacketTraceStatus::OrphanDataPulse
    );
    assert_eq!(orphan.snapshot().command.invalid_packet_count, 1);
}

#[test]
fn joyp_transport_ignores_handheld_hosts() {
    let mut host = SgbHost::new(HostPlatform::Handheld);
    write_joyp_packet(&mut host, sgb_command_packet(0x11, 1));

    let snapshot = host.snapshot();
    assert_eq!(snapshot.packet_transport.reset_pulse_count, 0);
    assert_eq!(snapshot.packet_transport.data_pulse_count, 0);
    assert_eq!(snapshot.command.accepted_command_count, 0);
    assert_eq!(
        snapshot.packet_transport.last_trace.status,
        SgbPacketTraceStatus::None
    );
}

#[test]
fn save_state_restores_partial_packet_transport() {
    let mut host = accepted_sgb_host();
    let packet = sgb_command_packet(0x11, 1);
    write_joyp_start(&mut host);
    for bit_index in 0..32 {
        let byte = packet[bit_index / 8];
        write_joyp_data_bit(&mut host, (byte >> (bit_index % 8)) & 0x01);
    }

    let saved = host.capture_save_state();
    let mut restored = SgbHost::new(HostPlatform::Sgb);
    restored.restore_save_state(&saved);

    for bit_index in 32..128 {
        let byte = packet[bit_index / 8];
        write_joyp_data_bit(&mut restored, (byte >> (bit_index % 8)) & 0x01);
    }
    write_joyp_data_bit(&mut restored, 0);

    let snapshot = restored.snapshot();
    assert_eq!(snapshot.packet_transport.packet_bits_buffered, 128);
    assert_eq!(
        snapshot.packet_transport.last_trace.status,
        SgbPacketTraceStatus::Complete
    );
    assert_eq!(snapshot.command.last_command_id, Some(0x11));
    assert_eq!(snapshot.command.accepted_command_count, 1);
}

#[test]
fn save_state_restores_pending_start_pulse() {
    let mut host = accepted_sgb_host();
    let packet = sgb_command_packet(0x11, 1);
    write_joyp_line(&mut host, SgbJoypLineState::Start);
    assert_eq!(
        host.snapshot().packet_transport.phase,
        SgbPacketTransportPhase::StartPending
    );

    let saved = host.capture_save_state();
    let mut restored = SgbHost::new(HostPlatform::Sgb);
    restored.restore_save_state(&saved);

    write_joyp_line(&mut restored, SgbJoypLineState::Idle);
    for byte in packet {
        for bit_index in 0..8 {
            write_sgb_ext_test_bit(&mut restored, (byte >> bit_index) & 0x01);
        }
    }
    write_sgb_ext_test_bit(&mut restored, 0);

    let snapshot = restored.snapshot();
    assert_eq!(
        snapshot.packet_transport.last_trace.status,
        SgbPacketTraceStatus::Complete
    );
    assert_eq!(snapshot.command.last_command_id, Some(0x11));
    assert_eq!(snapshot.command.accepted_command_count, 1);
}

#[test]
fn save_state_restores_pending_data_bit() {
    let mut host = accepted_sgb_host();
    let packet = sgb_command_packet(0x11, 1);
    write_sgb_ext_test_start(&mut host);
    write_joyp_line(&mut host, SgbJoypLineState::One);
    assert_eq!(
        host.snapshot().packet_transport.phase,
        SgbPacketTransportPhase::DataPending
    );
    assert_eq!(host.snapshot().packet_transport.pending_data_bit, Some(1));

    let saved = host.capture_save_state();
    let mut restored = SgbHost::new(HostPlatform::Sgb);
    restored.restore_save_state(&saved);

    write_joyp_line(&mut restored, SgbJoypLineState::Idle);
    for bit_index in 1..128 {
        let byte = packet[bit_index / 8];
        write_sgb_ext_test_bit(&mut restored, (byte >> (bit_index % 8)) & 0x01);
    }
    write_sgb_ext_test_bit(&mut restored, 0);

    let snapshot = restored.snapshot();
    assert_eq!(snapshot.packet_transport.pending_data_bit, None);
    assert_eq!(
        snapshot.packet_transport.last_trace.status,
        SgbPacketTraceStatus::Complete
    );
    assert_eq!(snapshot.command.last_command_id, Some(0x11));
    assert_eq!(snapshot.command.accepted_command_count, 1);
}

#[test]
fn save_state_restores_sgb_multiplayer_state_and_input_slots() {
    let mut host = accepted_sgb_host();
    write_joyp_packet(&mut host, sgb_mlt_req_packet(3));
    cycle_sgb_player(&mut host);
    assert!(host.set_player_button_pressed(2, JoypadButton::A, true));
    assert!(host.set_player_button_pressed(4, JoypadButton::Start, true));

    let saved = host.capture_save_state();
    let mut restored = SgbHost::new(HostPlatform::Sgb);
    restored.restore_save_state(&saved);

    let snapshot = restored.snapshot();
    assert_eq!(snapshot.multiplayer.player_count, 4);
    assert_eq!(snapshot.multiplayer.selected_player, 2);
    assert_eq!(snapshot.multiplayer.player_pressed_masks[1], 0x10);
    assert_eq!(snapshot.multiplayer.player_pressed_masks[3], 0x80);
    assert_eq!(restored.selected_player_pressed_mask(), 0x10);
    assert_eq!(restored.joyp_read_value(0xFF), 0xFE);
}
