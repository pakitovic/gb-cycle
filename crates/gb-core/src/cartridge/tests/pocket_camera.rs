use super::*;

fn load_pocket_camera_slot() -> CartridgeSlot {
    CartridgeSlot::load(build_pocket_camera_rom(), &CompatibilityPolicy::strict())
        .expect("Pocket Camera should load in strict mode")
        .into_parts()
        .0
}

fn load_pocket_camera_device() -> PocketCameraCartridge {
    let slot = load_pocket_camera_slot();
    let Some(CartridgeDevice::PocketCamera(camera)) = slot.device else {
        panic!("expected Pocket Camera device");
    };
    camera
}

fn set_simple_matrix_thresholds(camera: &mut PocketCameraCartridge) {
    camera.write_rom(0x4000, 0x10);
    for cell in 0..16 {
        let base = 0xA006 + cell * 3;
        camera.write_ram(base, 64);
        camera.write_ram(base + 1, 128);
        camera.write_ram(base + 2, 192);
    }
}

fn finish_capture(camera: &mut PocketCameraCartridge) {
    let ready_at = camera
        .capture_ready_at()
        .expect("capture should have a ready cycle");
    camera.write_rom(0x4000, 0x00);
    let _ = camera.read_ram_timed(0xA100, ready_at);
}

#[test]
fn strict_pocket_camera_load_exposes_a_dedicated_supported_family_and_persistence_profile() {
    let report = CartridgeSlot::load(build_pocket_camera_rom(), &CompatibilityPolicy::strict())
        .expect("Pocket Camera should load");
    let (slot, diagnostics) = report.into_parts();

    assert!(diagnostics.is_empty());
    assert_eq!(slot.state(), CartridgeSlotState::PocketCamera);
    assert!(slot.has_pocket_camera());
    assert_eq!(
        slot.classification()
            .map(CartridgeClassification::selection),
        Some(CartridgeSelection::Supported(
            SupportedCartridgeFamily::PocketCamera
        ))
    );
    assert_eq!(
        slot.persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: false,
            profile: CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear {
                    byte_len: POCKET_CAMERA_SUPPORTED_RAM_BYTES,
                },
            },
        }
    );
    assert_eq!(
        slot.snapshot(),
        CartridgeSnapshot {
            state: CartridgeSlotState::PocketCamera,
            rtc_access_ready_at: None,
            camera_capture_ready_at: None,
            camera_registers_selected: false,
        }
    );
    match slot.persistent_state() {
        PersistentCartState::PocketCameraRam { ram } => {
            assert_eq!(ram.len(), POCKET_CAMERA_SUPPORTED_RAM_BYTES);
            assert!(ram.iter().all(|&byte| byte == 0x00));
        }
        other => panic!("expected Pocket Camera RAM state, got {other:?}"),
    }
}

#[test]
fn strict_validation_rejects_malformed_pocket_camera_headers() {
    let wrong_rom_header = build_test_rom(POCKET_CAMERA_SUPPORTED_ROM_BYTES, 0xFC, 0x04, 0x04);
    let wrong_rom_header_error =
        CartridgeSlot::load(wrong_rom_header, &CompatibilityPolicy::strict())
            .expect_err("strict Pocket Camera validation should reject the wrong ROM declaration");
    assert!(matches!(
        wrong_rom_header_error,
        CartridgeLoadError::Rejected { reason, .. }
            if reason.contains("official 1 MiB ROM declaration")
    ));

    let wrong_ram_header = build_test_rom(POCKET_CAMERA_SUPPORTED_ROM_BYTES, 0xFC, 0x05, 0x03);
    let wrong_ram_header_error =
        CartridgeSlot::load(wrong_ram_header, &CompatibilityPolicy::strict())
            .expect_err("strict Pocket Camera validation should reject the wrong RAM declaration");
    assert!(matches!(
        wrong_ram_header_error,
        CartridgeLoadError::Rejected { reason, .. }
            if reason.contains("official 128 KiB RAM declaration")
    ));

    let mismatched_length = build_test_rom(512 * 1024, 0xFC, 0x05, 0x04);
    let mismatched_length_error =
        CartridgeSlot::load(mismatched_length, &CompatibilityPolicy::strict())
            .expect_err("strict Pocket Camera validation should reject length mismatches");
    assert!(matches!(
        mismatched_length_error,
        CartridgeLoadError::Rejected { reason, .. }
            if reason.contains("loaded ROM is 524288 bytes")
    ));
}

#[test]
fn pocket_camera_mapper_keeps_bank_zero_reachable_and_register_window_mirrored() {
    let mut camera = load_pocket_camera_device();

    assert_eq!(camera.read_rom(0x4000), 0x01);
    camera.write_rom(0x2000, 0x00);
    assert_eq!(camera.read_rom(0x4000), 0x00);

    camera.write_rom(0x4000, 0x0F);
    let banked_access = camera.describe_external_access(0xA123);
    assert_eq!(
        banked_access.target(),
        CartridgeExternalTarget::BankedRam { bank: 0x0F }
    );
    assert_eq!(
        banked_access.availability(),
        CartridgeExternalAvailability::Accessible
    );

    camera.write_rom(0x4000, 0x10);
    assert!(camera.registers_selected());
    let register_access = camera.describe_external_access(0xA086);
    assert_eq!(
        register_access.target(),
        CartridgeExternalTarget::PocketCameraRegister { offset: 0x06 }
    );
    assert_eq!(
        register_access.availability(),
        CartridgeExternalAvailability::Accessible
    );

    camera.write_ram(0xA086, 0x5A);
    assert_eq!(camera.registers[0x06], 0x5A);
    camera.write_ram(0xA000, 0x06);
    assert_eq!(camera.read_ram(0xA000), 0x06);
    assert_eq!(camera.read_ram(0xA001), 0x00);
    assert_eq!(camera.read_ram(0xA086), 0x00);
}

#[test]
fn pocket_camera_access_descriptions_cover_ram_busy_reserved_registers_and_writable_ram() {
    let mut camera = load_pocket_camera_device();

    camera.write_rom(0x4000, 0x0E);
    let disabled_write_access = camera.describe_external_access(0xA001);
    assert_eq!(
        disabled_write_access.target(),
        CartridgeExternalTarget::BankedRam { bank: 0x0E }
    );
    assert_eq!(
        disabled_write_access.write_behavior(),
        CartridgeExternalWriteBehavior::Ignored
    );

    camera.write_rom(0x0000, 0x0A);
    let enabled_write_access = camera.describe_external_access(0xA001);
    assert_eq!(
        enabled_write_access.write_behavior(),
        CartridgeExternalWriteBehavior::Storage
    );

    camera.write_rom(0x4000, 0x10);
    camera.write_ram_timed(0xA000, 0x01, TCycle::ZERO);
    camera.write_rom(0x4000, 0x00);
    let busy_access = camera.describe_external_access(0xA001);
    assert_eq!(
        busy_access.availability(),
        CartridgeExternalAvailability::Disabled
    );
    assert_eq!(
        busy_access.read_behavior(),
        CartridgeExternalReadBehavior::FallbackValue(POCKET_CAMERA_WORKING_RAM_READ_VALUE)
    );
    assert_eq!(
        busy_access.write_behavior(),
        CartridgeExternalWriteBehavior::Ignored
    );

    camera.write_rom(0x4000, 0x10);
    let reserved_register_access = camera.describe_external_access(0xA036);
    assert_eq!(
        reserved_register_access.target(),
        CartridgeExternalTarget::PocketCameraRegister { offset: 0x36 }
    );
    assert_eq!(
        reserved_register_access.availability(),
        CartridgeExternalAvailability::Reserved
    );
    assert_eq!(
        reserved_register_access.read_behavior(),
        CartridgeExternalReadBehavior::FallbackValue(0)
    );
    assert_eq!(
        reserved_register_access.write_behavior(),
        CartridgeExternalWriteBehavior::Ignored
    );
    assert_eq!(camera.read_ram(0xA036), 0x00);
}

#[test]
fn pocket_camera_busy_timing_and_pause_resume_follow_a000_bit0_semantics() {
    let mut camera = load_pocket_camera_device();
    camera.write_rom(0x0000, 0x0A);
    camera.write_ram(0xA000, 0x5A);

    camera.write_rom(0x4000, 0x10);
    camera.write_ram_timed(0xA002, 0x03, TCycle::ZERO);
    camera.write_ram_timed(0xA003, 0x00, TCycle::ZERO);
    camera.write_ram_timed(0xA000, 0x01, TCycle::new(100));

    let started_ready_at = TCycle::new(100 + 4 * (32_446 + 512 + 16 * 0x0300));
    assert_eq!(camera.capture_ready_at(), Some(started_ready_at));

    camera.write_rom(0x4000, 0x00);
    assert_eq!(camera.read_ram_timed(0xA000, TCycle::new(101)), 0x00);
    camera.write_ram_timed(0xA000, 0x11, TCycle::new(102));

    camera.write_rom(0x4000, 0x10);
    camera.write_ram_timed(0xA000, 0x00, TCycle::new(1_000));
    assert_eq!(camera.capture_ready_at(), None);
    match &camera.capture_state {
        PocketCameraCaptureState::Paused {
            remaining_t_cycles, ..
        } => assert_eq!(*remaining_t_cycles, started_ready_at.get() - 1_000),
        other => panic!("expected a paused capture, got {other:?}"),
    }

    camera.write_rom(0x4000, 0x00);
    assert_eq!(camera.read_ram_timed(0xA000, TCycle::new(1_001)), 0x5A);

    camera.write_rom(0x4000, 0x10);
    camera.write_ram_timed(0xA002, 0x00, TCycle::new(1_002));
    camera.write_ram_timed(0xA003, 0x30, TCycle::new(1_002));
    camera.write_ram_timed(0xA000, 0x01, TCycle::new(1_500));
    assert_eq!(
        camera.capture_ready_at(),
        Some(TCycle::new(1_500 + started_ready_at.get() - 1_000))
    );

    finish_capture(&mut camera);
    assert_eq!(camera.capture_ready_at(), None);
}

#[test]
fn pocket_camera_persistence_restore_rejects_wrong_kind_and_ram_length() {
    let mut camera = load_pocket_camera_device();

    assert_eq!(
        camera.restore_persistent_state(&PersistentCartState::PocketCameraRam { ram: vec![0] }),
        Err(CartridgePersistentStateError::RamLengthMismatch {
            expected: POCKET_CAMERA_SUPPORTED_RAM_BYTES,
            actual: 1,
        })
    );
    assert_eq!(
        camera.restore_persistent_state(&PersistentCartState::None),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "PocketCameraRam",
            actual: "None",
        })
    );
}

#[test]
fn pocket_camera_frame_api_normalizes_inputs_and_clear_restores_the_placeholder() {
    let mut slot = load_pocket_camera_slot();
    let single_pixel = PocketCameraFrame {
        width: 1,
        height: 1,
        grayscale_pixels: vec![0x33],
    };
    slot.set_pocket_camera_frame(single_pixel)
        .expect("camera slots should accept host frames");

    let Some(CartridgeDevice::PocketCamera(camera)) = slot.device.as_ref() else {
        panic!("expected Pocket Camera device");
    };
    assert_eq!(camera.host_frame.len(), POCKET_CAMERA_CAPTURE_PIXEL_COUNT);
    assert!(camera.host_frame.iter().all(|&value| value == 0x33));

    assert_eq!(
        slot.set_pocket_camera_frame(PocketCameraFrame {
            width: 2,
            height: 2,
            grayscale_pixels: vec![0x10, 0x20, 0x30],
        }),
        Err(PocketCameraFrameError::InvalidDimensions {
            width: 2,
            height: 2,
            pixel_len: 3,
        })
    );

    slot.clear_pocket_camera_frame()
        .expect("camera slots should restore the placeholder");
    let Some(CartridgeDevice::PocketCamera(camera)) = slot.device.as_ref() else {
        panic!("expected Pocket Camera device");
    };
    assert_eq!(
        camera.host_frame,
        PocketCameraCartridge::placeholder_frame()
    );

    let mut no_mbc = CartridgeSlot::load(
        build_test_rom(NO_MBC_SUPPORTED_ROM_BYTES, 0x09, 0x00, 0x02),
        &CompatibilityPolicy::strict(),
    )
    .expect("NoMBC+BATTERY should load")
    .into_parts()
    .0;
    assert_eq!(
        no_mbc.set_pocket_camera_frame(PocketCameraFrame {
            width: 1,
            height: 1,
            grayscale_pixels: vec![0x00],
        }),
        Err(PocketCameraFrameError::UnsupportedCartridge)
    );
    assert_eq!(
        no_mbc.clear_pocket_camera_frame(),
        Err(PocketCameraFrameError::UnsupportedCartridge)
    );
}

#[test]
fn pocket_camera_uniform_capture_writes_exact_tile_planes_and_invert_flips_them() {
    let mut normal = load_pocket_camera_device();
    normal
        .set_host_frame(PocketCameraFrame {
            width: 1,
            height: 1,
            grayscale_pixels: vec![0x00],
        })
        .expect("uniform black frame should normalize");
    set_simple_matrix_thresholds(&mut normal);
    normal.write_rom(0x4000, 0x10);
    normal.write_ram(0xA001, 0x80);
    normal.write_ram(0xA002, 0x03);
    normal.write_ram(0xA003, 0x00);
    normal.write_ram(0xA004, 0x00);
    normal.write_ram_timed(0xA000, 0x01, TCycle::ZERO);
    finish_capture(&mut normal);
    let normal_tiles = &normal.ram[POCKET_CAMERA_CAPTURE_TILE_BASE_OFFSET
        ..POCKET_CAMERA_CAPTURE_TILE_BASE_OFFSET + POCKET_CAMERA_CAPTURE_TILE_BYTES];
    assert!(
        normal_tiles
            .chunks_exact(2)
            .all(|pair| pair[0] == 0x00 && pair[1] == 0xFF)
    );

    let mut inverted = load_pocket_camera_device();
    inverted
        .set_host_frame(PocketCameraFrame {
            width: 1,
            height: 1,
            grayscale_pixels: vec![0x00],
        })
        .expect("uniform black frame should normalize");
    set_simple_matrix_thresholds(&mut inverted);
    inverted.write_rom(0x4000, 0x10);
    inverted.write_ram(0xA001, 0x80);
    inverted.write_ram(0xA002, 0x03);
    inverted.write_ram(0xA003, 0x00);
    inverted.write_ram(0xA004, 0x08);
    inverted.write_ram_timed(0xA000, 0x01, TCycle::ZERO);
    finish_capture(&mut inverted);
    let inverted_tiles = &inverted.ram[POCKET_CAMERA_CAPTURE_TILE_BASE_OFFSET
        ..POCKET_CAMERA_CAPTURE_TILE_BASE_OFFSET + POCKET_CAMERA_CAPTURE_TILE_BYTES];
    assert!(
        inverted_tiles
            .chunks_exact(2)
            .all(|pair| pair[0] == 0xFF && pair[1] == 0x00)
    );
}

#[test]
fn pocket_camera_edge_mode_changes_the_captured_tiles_for_at_least_one_supported_threshold_set() {
    let step_frame = PocketCameraFrame {
        width: POCKET_CAMERA_CAPTURE_WIDTH as u16,
        height: POCKET_CAMERA_CAPTURE_HEIGHT as u16,
        grayscale_pixels: (0..POCKET_CAMERA_CAPTURE_PIXEL_COUNT)
            .map(|index| {
                let x = index % POCKET_CAMERA_CAPTURE_WIDTH;
                if x < POCKET_CAMERA_CAPTURE_WIDTH / 2 {
                    0x00
                } else {
                    0xFF
                }
            })
            .collect(),
    };
    let candidate_thresholds = [
        [64, 128, 192],
        [80, 112, 144],
        [96, 128, 160],
        [110, 130, 150],
        [120, 136, 200],
    ];

    let mut found_difference = false;
    for [r0, r1, r2] in candidate_thresholds {
        let mut baseline = load_pocket_camera_device();
        baseline
            .set_host_frame(step_frame.clone())
            .expect("step frame should normalize");
        baseline.write_rom(0x4000, 0x10);
        for cell in 0..16 {
            let base = 0xA006 + cell * 3;
            baseline.write_ram(base, r0);
            baseline.write_ram(base + 1, r1);
            baseline.write_ram(base + 2, r2);
        }
        baseline.write_ram(0xA001, 0x80);
        baseline.write_ram(0xA002, 0x03);
        baseline.write_ram(0xA003, 0x00);
        baseline.write_ram(0xA004, 0x00);
        baseline.write_ram_timed(0xA000, 0x01, TCycle::ZERO);
        finish_capture(&mut baseline);

        let mut edge = load_pocket_camera_device();
        edge.set_host_frame(step_frame.clone())
            .expect("step frame should normalize");
        edge.write_rom(0x4000, 0x10);
        for cell in 0..16 {
            let base = 0xA006 + cell * 3;
            edge.write_ram(base, r0);
            edge.write_ram(base + 1, r1);
            edge.write_ram(base + 2, r2);
        }
        edge.write_ram(0xA001, 0xE0);
        edge.write_ram(0xA002, 0x03);
        edge.write_ram(0xA003, 0x00);
        edge.write_ram(0xA004, 0x20);
        edge.write_ram_timed(0xA000, 0x01, TCycle::ZERO);
        finish_capture(&mut edge);

        let baseline_tiles = &baseline.ram[POCKET_CAMERA_CAPTURE_TILE_BASE_OFFSET
            ..POCKET_CAMERA_CAPTURE_TILE_BASE_OFFSET + POCKET_CAMERA_CAPTURE_TILE_BYTES];
        let edge_tiles = &edge.ram[POCKET_CAMERA_CAPTURE_TILE_BASE_OFFSET
            ..POCKET_CAMERA_CAPTURE_TILE_BASE_OFFSET + POCKET_CAMERA_CAPTURE_TILE_BYTES];
        if baseline_tiles != edge_tiles {
            found_difference = true;
            break;
        }
    }

    assert!(found_difference);
}

#[test]
fn pocket_camera_horizontal_edge_mode_uses_the_mode_two_filter_path() {
    let mut camera = load_pocket_camera_device();
    camera
        .set_host_frame(PocketCameraFrame {
            width: POCKET_CAMERA_CAPTURE_WIDTH as u16,
            height: POCKET_CAMERA_CAPTURE_HEIGHT as u16,
            grayscale_pixels: (0..POCKET_CAMERA_CAPTURE_PIXEL_COUNT)
                .map(|index| {
                    let x = index % POCKET_CAMERA_CAPTURE_WIDTH;
                    ((x * 255) / (POCKET_CAMERA_CAPTURE_WIDTH - 1)) as u8
                })
                .collect(),
        })
        .expect("gradient frame should normalize");
    set_simple_matrix_thresholds(&mut camera);

    camera.write_rom(0x4000, 0x10);
    camera.write_ram(0xA001, 0x20);
    camera.write_ram(0xA002, 0x03);
    camera.write_ram(0xA003, 0x00);
    camera.write_ram(0xA004, 0x10);
    camera.write_ram_timed(0xA000, 0x03, TCycle::ZERO);
    finish_capture(&mut camera);

    let tiles = &camera.ram[POCKET_CAMERA_CAPTURE_TILE_BASE_OFFSET
        ..POCKET_CAMERA_CAPTURE_TILE_BASE_OFFSET + POCKET_CAMERA_CAPTURE_TILE_BYTES];
    assert!(tiles.iter().any(|&byte| byte != 0x00));
}
