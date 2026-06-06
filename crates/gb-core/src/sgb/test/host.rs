use super::support::*;

#[test]
fn profile_descriptors_capture_sgb_and_sgb2_capabilities() {
    assert_eq!(SgbHostProfile::ALL.len(), 3);
    assert_eq!(
        SgbHostProfile::default_for_host_platform(HostPlatform::Sgb),
        Some(SgbHostProfile::SgbNtsc)
    );
    assert_eq!(
        SgbHostProfile::default_for_host_platform(HostPlatform::Sgb2),
        Some(SgbHostProfile::Sgb2Ntsc)
    );
    assert_eq!(
        SgbHostProfile::default_for_host_platform(HostPlatform::Handheld),
        None
    );
    assert_eq!(
        SgbHostProfile::SgbPal.video_standard(),
        SgbVideoStandard::Pal
    );
    assert_eq!(SgbHostProfile::SgbNtsc.real_boot_filename(), "sgb_boot.bin");
    assert_eq!(
        SgbHostProfile::Sgb2Ntsc.real_boot_filename(),
        "sgb2_boot.bin"
    );
    assert!(!SgbHostProfile::SgbNtsc.game_link_supported());
    assert!(SgbHostProfile::Sgb2Ntsc.game_link_supported());
    assert!(!SgbHostProfile::SgbNtsc.corrected_clock());
    assert!(SgbHostProfile::Sgb2Ntsc.corrected_clock());
    assert_eq!(
        SgbHostProfile::SgbNtsc
            .timing()
            .gb_master_clock_hz
            .rounded_hz(),
        4_295_454
    );
    assert_eq!(
        SgbHostProfile::SgbPal
            .timing()
            .gb_master_clock_hz
            .rounded_hz(),
        4_256_274
    );
    assert_eq!(
        SgbHostProfile::Sgb2Ntsc
            .timing()
            .gb_master_clock_hz
            .rounded_hz(),
        4_194_304
    );
}

#[test]
fn host_state_is_inert_for_handheld_and_ready_for_sgb_profiles() {
    let handheld = SgbHost::new(HostPlatform::Handheld);
    assert_eq!(handheld.status(), SgbHostStatus::Disabled);
    assert_eq!(handheld.profile(), None);
    assert_eq!(
        handheld.command_acceptance(),
        SgbCommandAcceptance::Disabled
    );
    assert_eq!(handheld.snapshot().multiplayer.player_count, 0);

    let sgb = SgbHost::new(HostPlatform::Sgb);
    assert_eq!(sgb.status(), SgbHostStatus::Ready);
    assert_eq!(sgb.profile(), Some(SgbHostProfile::SgbNtsc));
    assert_eq!(sgb.backend_kind(), SgbHostBackendKind::DeterministicHle);
    assert_eq!(
        sgb.command_acceptance(),
        SgbCommandAcceptance::AwaitingCartridgeHeader
    );
    assert_eq!(sgb.snapshot().multiplayer.player_count, 1);
    assert!(!sgb.game_link_supported());

    let sgb2 = SgbHost::new(HostPlatform::Sgb2);
    assert_eq!(sgb2.profile(), Some(SgbHostProfile::Sgb2Ntsc));
    assert!(sgb2.game_link_supported());
    assert!(sgb2.corrected_clock());
}

#[test]
fn sgb_direct_boot_profiles_publish_sgb_cpu_fingerprint() {
    for startup_mode in [StartupMode::SkipBoot, StartupMode::CustomBoot] {
        let dmg = crate::Machine::new(
            crate::MachineConfig::new(crate::ConsoleModel::GameBoy).with_startup_mode(startup_mode),
        );
        assert_eq!(dmg.cpu().startup_state().a, 0x01);
        assert_eq!(dmg.cpu().startup_state().c, 0x13);

        let sgb = crate::Machine::new(
            crate::MachineConfig::new(crate::ConsoleModel::GameBoy)
                .with_sgb_profile(SgbHostProfile::SgbNtsc)
                .with_startup_mode(startup_mode),
        );
        assert_eq!(sgb.cpu().startup_state().a, 0x01);
        assert_eq!(sgb.cpu().startup_state().f, 0x00);
        assert_eq!(sgb.cpu().startup_state().c, 0x14);
        assert_eq!(sgb.cpu().startup_state().e, 0x00);
        assert_eq!(sgb.cpu().startup_state().h, 0xC0);
        assert_eq!(sgb.cpu().startup_state().l, 0x60);

        let sgb2 = crate::Machine::new(
            crate::MachineConfig::new(crate::ConsoleModel::GameBoy)
                .with_sgb_profile(SgbHostProfile::Sgb2Ntsc)
                .with_startup_mode(startup_mode),
        );
        assert_eq!(sgb2.cpu().startup_state().a, 0xFF);
        assert_eq!(sgb2.cpu().startup_state().f, 0x00);
        assert_eq!(sgb2.cpu().startup_state().c, 0x14);
        assert_eq!(sgb2.cpu().startup_state().e, 0x00);
        assert_eq!(sgb2.cpu().startup_state().h, 0xC0);
        assert_eq!(sgb2.cpu().startup_state().l, 0x60);
    }
}

#[test]
fn system_control_commands_are_persisted_and_icon_can_suppress_packets() {
    let mut host = accepted_sgb_host();
    write_joyp_packet(&mut host, sgb_atrc_en_packet(true));
    assert!(host.snapshot().system.attraction_disabled);
    assert_eq!(host.snapshot().system.atrc_en_count, 1);

    write_joyp_packet(&mut host, sgb_test_en_packet(true));
    assert!(host.snapshot().system.test_mode_enabled);
    assert_eq!(host.snapshot().system.test_en_count, 1);

    write_joyp_packet(&mut host, sgb_icon_en_packet(0x03));
    assert_eq!(host.snapshot().system.icon_disable_bits, 0x03);
    assert_eq!(host.snapshot().system.icon_en_count, 1);
    write_joyp_packet(&mut host, sgb_pal01_packet());
    assert_eq!(
        host.snapshot().command.last_command_id,
        Some(SGB_COMMAND_PAL01)
    );
    assert_eq!(host.snapshot().video.palette_command_count, 1);

    write_joyp_packet(&mut host, sgb_icon_en_packet(0x04));
    assert_eq!(host.snapshot().system.icon_disable_bits, 0x04);
    let palette_command_count = host.snapshot().video.palette_command_count;
    let accepted_command_count = host.snapshot().command.accepted_command_count;
    write_joyp_packet(&mut host, sgb_pal01_packet());
    let snapshot = host.snapshot();
    assert_eq!(
        snapshot.packet_transport.last_trace.status,
        SgbPacketTraceStatus::SuppressedByIcon
    );
    assert_eq!(snapshot.packet_gate.icon_suppressed_packet_count, 1);
    assert_eq!(
        snapshot.packet_gate.last_suppressed_command_id,
        Some(SGB_COMMAND_PAL01)
    );
    assert_eq!(snapshot.video.palette_command_count, palette_command_count);
    assert_eq!(
        snapshot.command.accepted_command_count,
        accepted_command_count
    );

    let saved = host.capture_save_state();
    let mut restored = SgbHost::new(HostPlatform::Sgb);
    restored.restore_save_state(&saved);
    assert_eq!(restored.snapshot().system, snapshot.system);
    assert_eq!(restored.snapshot().packet_gate, snapshot.packet_gate);
}

#[test]
fn vram_transfer_command_captures_over_five_frame_window_and_survives_save_state() {
    let mut host = accepted_sgb_host();
    write_joyp_packet(&mut host, sgb_pal_trn_packet());
    let snapshot = host.snapshot();
    assert_eq!(
        snapshot.video.vram_transfer.pending,
        Some(SgbPendingVramTransfer {
            command_id: SGB_COMMAND_PAL_TRN,
            target: SgbVramTransferTarget::Pal,
            frame_starts_until_capture: 1,
            phase: SgbVramTransferPhase::WaitingForNextFrame,
            frames_captured: 0,
            total_frames: SGB_VRAM_TRANSFER_TOTAL_FRAMES,
            source_mode: SgbVramTransferSourceMode::Unresolved,
        })
    );
    assert!(snapshot.video.vram_transfer.last_completed.is_none());
    assert_eq!(
        snapshot.packet_gate.busy_frames_remaining,
        SGB_VRAM_TRANSFER_TOTAL_FRAMES
    );

    let mut expected = [0; SGB_VRAM_TRANSFER_BYTES];
    for frame_index in 0..2 {
        let payload = frame_payload(frame_index);
        let (chunk_start, chunk_end) =
            vram_transfer_chunk_range(frame_index, SGB_VRAM_TRANSFER_TOTAL_FRAMES);
        expected[chunk_start..chunk_end].copy_from_slice(&payload[chunk_start..chunk_end]);
        assert_eq!(
            host.advance_frame_start(
                &transfer_vram_from_payload(&payload),
                fallback_transfer_display(),
            ),
            Ok(None)
        );
        let snapshot = host.snapshot();
        let pending = snapshot
            .video
            .vram_transfer
            .pending
            .expect("PAL_TRN should remain pending until the fifth frame");
        assert_eq!(pending.phase, SgbVramTransferPhase::Capturing);
        assert_eq!(pending.frames_captured, frame_index + 1);
        assert_eq!(
            snapshot.packet_gate.busy_frames_remaining,
            SGB_VRAM_TRANSFER_TOTAL_FRAMES - frame_index - 1
        );
        assert_eq!(
            snapshot
                .video
                .vram_transfer
                .partial_payload
                .as_ref()
                .expect("partial _TRN payload should be save-state-visible")
                .bytes[chunk_start..chunk_end],
            expected[chunk_start..chunk_end]
        );
    }

    let mid_save = host.capture_save_state();
    let mid_snapshot = host.snapshot();
    let mut restored = SgbHost::new(HostPlatform::Sgb);
    restored.restore_save_state(&mid_save);
    assert_eq!(
        restored.snapshot().video.vram_transfer,
        mid_snapshot.video.vram_transfer
    );
    assert_eq!(restored.snapshot().packet_gate, mid_snapshot.packet_gate);

    for frame_index in 2..SGB_VRAM_TRANSFER_TOTAL_FRAMES {
        let payload = frame_payload(frame_index);
        let (chunk_start, chunk_end) =
            vram_transfer_chunk_range(frame_index, SGB_VRAM_TRANSFER_TOTAL_FRAMES);
        expected[chunk_start..chunk_end].copy_from_slice(&payload[chunk_start..chunk_end]);
        let result = restored
            .advance_frame_start(
                &transfer_vram_from_payload(&payload),
                fallback_transfer_display(),
            )
            .expect("restored PAL_TRN frame capture should continue exactly");
        if frame_index + 1 < SGB_VRAM_TRANSFER_TOTAL_FRAMES {
            assert_eq!(result, None);
            assert_eq!(
                restored
                    .snapshot()
                    .video
                    .vram_transfer
                    .pending
                    .expect("transfer should still be pending")
                    .frames_captured,
                frame_index + 1
            );
        } else {
            assert_eq!(result, Some(SgbVramTransferTarget::Pal));
        }
    }

    let snapshot = restored.snapshot();
    assert_eq!(snapshot.packet_gate.busy_frames_remaining, 0);
    assert!(snapshot.video.vram_transfer.pending.is_none());
    assert!(snapshot.video.vram_transfer.partial_payload.is_none());
    assert_eq!(snapshot.video.vram_transfer.completed_transfer_count, 1);
    assert_eq!(
        snapshot
            .video
            .vram_transfer
            .last_completed
            .as_ref()
            .expect("completed PAL_TRN should retain the final payload")
            .payload
            .bytes,
        expected.to_vec()
    );
    assert_eq!(
        snapshot.video.system_palettes.palette_wrapping(0).colors[0].raw(),
        u16::from_le_bytes([expected[0], expected[1]]) & SGB_RGB555_MASK
    );
}

#[test]
fn packet_gate_rejects_packets_while_busy_and_accepts_after_transfer_finishes() {
    let mut host = accepted_sgb_host();
    write_joyp_packet(&mut host, sgb_pal_trn_packet());
    assert_eq!(host.snapshot().command.accepted_command_count, 1);
    assert_eq!(
        host.snapshot().packet_gate.busy_frames_remaining,
        SGB_VRAM_TRANSFER_TOTAL_FRAMES
    );

    write_joyp_packet(&mut host, sgb_pal01_packet());
    let rejected = host.snapshot();
    assert_eq!(
        rejected.packet_transport.last_trace.status,
        SgbPacketTraceStatus::RejectedWhileBusy
    );
    assert_eq!(rejected.packet_gate.busy_rejected_packet_count, 1);
    assert_eq!(
        rejected.packet_gate.last_busy_command_id,
        Some(SGB_COMMAND_PAL01)
    );
    assert_eq!(rejected.command.accepted_command_count, 1);
    assert_eq!(rejected.video.palette_command_count, 0);

    for frame_index in 0..SGB_VRAM_TRANSFER_TOTAL_FRAMES {
        let payload = frame_payload(frame_index);
        let result = host
            .advance_frame_start(
                &transfer_vram_from_payload(&payload),
                fallback_transfer_display(),
            )
            .expect("busy PAL_TRN should advance from deterministic host frames");
        if frame_index + 1 < SGB_VRAM_TRANSFER_TOTAL_FRAMES {
            assert_eq!(result, None);
        } else {
            assert_eq!(result, Some(SgbVramTransferTarget::Pal));
        }
    }
    assert_eq!(host.snapshot().packet_gate.busy_frames_remaining, 0);

    write_joyp_packet(&mut host, sgb_pal01_packet());
    let accepted = host.snapshot();
    assert_eq!(
        accepted.packet_transport.last_trace.status,
        SgbPacketTraceStatus::Complete
    );
    assert_eq!(accepted.packet_gate.busy_rejected_packet_count, 1);
    assert_eq!(accepted.command.last_command_id, Some(SGB_COMMAND_PAL01));
    assert_eq!(accepted.command.accepted_command_count, 2);
    assert_eq!(accepted.video.palette_command_count, 1);
}

#[test]
fn host_backend_contract_records_sound_data_and_jump_requests() {
    let mut host = accepted_sgb_host();

    write_joyp_packet(&mut host, sgb_sound_packet());
    let sound = SgbSoundRequest::from_packet(&sgb_sound_packet());
    assert_eq!(
        host.snapshot().audio.last_request,
        Some(SgbHostAudioRequest::Sound(sound))
    );
    assert_eq!(host.snapshot().audio.sound_command_count, 1);
    assert_eq!(host.snapshot().audio.pending_host_audio_events, 1);
    assert_eq!(sound.effect_a.code, 0x17);
    assert_eq!(sound.effect_a.pitch, 0);
    assert_eq!(sound.effect_a.volume, 3);
    assert_eq!(sound.effect_b.code, 0x24);
    assert_eq!(sound.effect_b.pitch, 1);
    assert_eq!(sound.effect_b.volume, 2);
    assert_eq!(sound.music_score, 0x05);

    write_joyp_packet(&mut host, sgb_data_snd_packet());
    let data_snd = SgbDataSendRequest::from_packet(&sgb_data_snd_packet());
    assert_eq!(data_snd.destination, SgbSnesAddress::new(0x7E, 0x2100));
    assert_eq!(data_snd.payload(), &[0xAA, 0xBB, 0xCC]);
    assert_eq!(
        host.snapshot().snes_host.last_request,
        Some(SgbSnesHostRequest::DataSend(data_snd))
    );
    assert_eq!(host.snapshot().snes_host.data_snd_count, 1);
    assert_eq!(host.snapshot().snes_host.uploaded_payload_bytes, 3);

    write_joyp_packet(&mut host, sgb_data_trn_packet());
    assert_eq!(
        host.snapshot().video.vram_transfer.pending,
        Some(SgbPendingVramTransfer {
            command_id: SGB_COMMAND_DATA_TRN,
            target: SgbVramTransferTarget::SnesData(SgbSnesAddress::new(0x7E, 0x2200)),
            frame_starts_until_capture: 1,
            phase: SgbVramTransferPhase::WaitingForNextFrame,
            frames_captured: 0,
            total_frames: SGB_VRAM_TRANSFER_TOTAL_FRAMES,
            source_mode: SgbVramTransferSourceMode::Unresolved,
        })
    );
    host.capture_pending_vram_transfer(&[0x42; SGB_VRAM_TRANSFER_BYTES])
        .expect("DATA_TRN should capture through the shared 4 KiB transfer seam");
    assert_eq!(
        host.snapshot().snes_host.last_request,
        Some(SgbSnesHostRequest::DataTransfer(SgbDataTransferRequest {
            destination: SgbSnesAddress::new(0x7E, 0x2200),
            payload_bytes: SGB_SNES_DATA_TRN_BYTES,
        }))
    );
    assert_eq!(host.snapshot().snes_host.data_trn_count, 1);
    assert_eq!(
        host.snapshot().snes_host.uploaded_payload_bytes,
        3 + SGB_SNES_DATA_TRN_BYTES
    );

    write_joyp_packet(&mut host, sgb_jump_packet());
    assert_eq!(
        host.snapshot().snes_host.last_request,
        Some(SgbSnesHostRequest::Jump(SgbJumpRequest {
            program_counter: SgbSnesAddress::new(0x7E, 0x1234),
            nmi_handler: SgbSnesAddress::new(0x7E, 0x5678),
        }))
    );
    assert!(host.snapshot().snes_host.execution_enabled);
    assert_eq!(
        host.snapshot().snes_host.program_counter,
        Some(SgbSnesAddress::new(0x7E, 0x1234))
    );
    assert_eq!(host.snapshot().snes_host.jump_count, 1);
}

#[test]
fn sound_transfer_uses_the_shared_vram_transfer_backend_seam_and_survives_save_state() {
    let mut host = accepted_sgb_host();
    write_joyp_packet(&mut host, sgb_sou_trn_packet());
    assert_eq!(
        host.snapshot().video.vram_transfer.pending,
        Some(SgbPendingVramTransfer {
            command_id: SGB_COMMAND_SOU_TRN,
            target: SgbVramTransferTarget::Sound,
            frame_starts_until_capture: 1,
            phase: SgbVramTransferPhase::WaitingForNextFrame,
            frames_captured: 0,
            total_frames: SGB_VRAM_TRANSFER_TOTAL_FRAMES,
            source_mode: SgbVramTransferSourceMode::Unresolved,
        })
    );

    let mut payload = [0; SGB_VRAM_TRANSFER_BYTES];
    payload[0] = 0x04;
    payload[1] = 0x00;
    payload[2] = 0x80;
    payload[3] = 0x21;
    host.capture_pending_vram_transfer(&payload)
        .expect("SOU_TRN should capture through the shared 4 KiB transfer seam");

    let expected = SgbHostAudioRequest::SoundTransfer(SgbSoundTransferRequest {
        first_packet: SgbSoundTransferPacket::Data {
            size: 4,
            destination: SgbApuRamAddress::new(0x2180),
        },
        payload_bytes: SGB_VRAM_TRANSFER_BYTES as u32,
    });
    assert_eq!(host.snapshot().audio.last_request, Some(expected));
    assert_eq!(host.snapshot().audio.sound_transfer_count, 1);
    assert_eq!(
        host.snapshot().audio.transferred_payload_bytes,
        SGB_VRAM_TRANSFER_BYTES as u32
    );

    let saved = host.capture_save_state();
    let mut restored = SgbHost::new(HostPlatform::Sgb);
    restored.restore_save_state(&saved);
    assert_eq!(restored.snapshot().audio.last_request, Some(expected));
    assert_eq!(
        restored.snapshot().video.vram_transfer.last_completed,
        host.snapshot().video.vram_transfer.last_completed
    );
}

#[test]
fn real_boot_startup_selects_profile_specific_boot_asset() {
    let sgb = SgbHost::new_with_startup(HostPlatform::Sgb, StartupMode::RealBoot);
    assert_eq!(sgb.startup().startup_mode, StartupMode::RealBoot);
    assert_eq!(
        sgb.startup().real_boot_asset,
        Some(SgbRealBootAsset::SgbBoot)
    );
    assert_eq!(
        sgb.startup()
            .real_boot_asset
            .map(SgbRealBootAsset::filename),
        Some("sgb_boot.bin")
    );

    let sgb2 = SgbHost::new_with_startup(HostPlatform::Sgb2, StartupMode::RealBoot);
    assert_eq!(
        sgb2.startup().real_boot_asset,
        Some(SgbRealBootAsset::Sgb2Boot)
    );
    assert_eq!(
        sgb2.startup()
            .real_boot_asset
            .map(SgbRealBootAsset::filename),
        Some("sgb2_boot.bin")
    );

    let handheld = SgbHost::new_with_startup(HostPlatform::Handheld, StartupMode::RealBoot);
    assert_eq!(handheld.startup().real_boot_asset, None);
}

#[test]
fn cartridge_header_controls_sgb_command_acceptance() {
    let supported = test_header(SgbFlag::Supported, SGB_HEADER_OLD_LICENSEE_CODE_REQUIRED);
    let unsupported = test_header(SgbFlag::None, SGB_HEADER_OLD_LICENSEE_CODE_REQUIRED);
    let wrong_licensee = test_header(SgbFlag::Supported, 0x01);

    let mut sgb = SgbHost::new(HostPlatform::Sgb);
    assert_eq!(
        sgb.command_acceptance(),
        SgbCommandAcceptance::AwaitingCartridgeHeader
    );
    sgb.apply_cartridge_header(Some(&supported));
    assert_eq!(sgb.command_acceptance(), SgbCommandAcceptance::Accepted);
    assert_eq!(sgb.startup().cartridge_sgb_flag, Some(SgbFlag::Supported));
    assert_eq!(sgb.startup().old_licensee_code, Some(0x33));

    sgb.apply_cartridge_header(Some(&unsupported));
    assert_eq!(
        sgb.command_acceptance(),
        SgbCommandAcceptance::RejectedByHeader
    );

    sgb.apply_cartridge_header(Some(&wrong_licensee));
    assert_eq!(
        sgb.command_acceptance(),
        SgbCommandAcceptance::RejectedByHeader
    );

    sgb.apply_cartridge_header(None);
    assert_eq!(
        sgb.command_acceptance(),
        SgbCommandAcceptance::AwaitingCartridgeHeader
    );

    let mut handheld = SgbHost::new(HostPlatform::Handheld);
    handheld.apply_cartridge_header(Some(&supported));
    assert_eq!(
        handheld.command_acceptance(),
        SgbCommandAcceptance::Disabled
    );
}

#[test]
fn configured_sgb_machines_construct_and_restore_with_host_state() {
    for (host_platform, profile) in [
        (HostPlatform::Sgb, SgbHostProfile::SgbNtsc),
        (HostPlatform::Sgb2, SgbHostProfile::Sgb2Ntsc),
    ] {
        let config = crate::MachineConfig::new(crate::ConsoleModel::GameBoy)
            .with_host_platform(host_platform)
            .with_startup_mode(crate::StartupMode::SkipBoot);
        let mut machine = crate::Machine::new(config.clone());
        assert_eq!(machine.config().operating_mode, crate::OperatingMode::Dmg);
        assert_eq!(machine.sgb_host().profile(), Some(profile));

        let saved = machine.capture_save_state();
        machine.step_t_cycle();
        machine
            .restore_save_state(&saved)
            .expect("matching SGB host save state should restore");
        assert_eq!(machine.capture_save_state(), saved);

        let fresh = crate::Machine::new(config);
        assert_eq!(fresh.sgb_host().profile(), Some(profile));
    }
}
