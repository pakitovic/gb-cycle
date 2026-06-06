use super::*;

#[test]
fn save_key_framebuffer_io_and_formatting_helpers_cover_remaining_host_utilities() {
    assert_eq!(
        derive_save_key(Path::new(
            "Legend of Zelda, The - Link's Awakening (USA, Europe) (Rev 2).gb"
        ))
        .expect("save key should derive"),
        CartridgeSaveKey::new("Legend of Zelda, The - Link's Awakening (USA, Europe) (Rev 2)")
            .expect("expected save key should be valid")
    );
    assert!(
        derive_save_key(Path::new("/"))
            .expect_err("root paths do not provide a file name")
            .contains("could not derive a save key")
    );
    assert!(
        derive_save_key(Path::new("bad*name.gb"))
            .expect_err("unsafe save key characters should fail")
            .contains("invalid character `*`")
    );
    assert_eq!(
        parse_save_key("bad/key").expect_err("invalid save keys should fail"),
        "save key contains invalid character `/` at index 3"
    );
    assert_eq!(
        format_save_key_error(CartridgeSaveKeyError::Empty),
        "save key must not be empty"
    );
    assert_eq!(
        format_save_key_error(CartridgeSaveKeyError::InvalidCharacter {
            index: 3,
            character: '/',
        }),
        "save key contains invalid character `/` at index 3"
    );

    let mut rtc_only = PersistentCartState::Mbc3Rtc {
        rtc: Mbc3RtcPersistentState {
            seconds: 58,
            minutes: 59,
            hours: 23,
            day_counter: 0,
            halt: false,
            carry: false,
        },
    };
    apply_elapsed_off_session_seconds(&mut rtc_only, 5);
    assert_ne!(
        rtc_only,
        PersistentCartState::Mbc3Rtc {
            rtc: Mbc3RtcPersistentState {
                seconds: 58,
                minutes: 59,
                hours: 23,
                day_counter: 0,
                halt: false,
                carry: false,
            },
        }
    );

    let original_rtc = Mbc3RtcPersistentState {
        seconds: 1,
        minutes: 2,
        hours: 3,
        day_counter: 4,
        halt: false,
        carry: false,
    };
    let mut rtc_and_ram = PersistentCartState::Mbc3RamRtc {
        ram: vec![0x55; 8],
        rtc: original_rtc,
    };
    apply_elapsed_off_session_seconds(&mut rtc_and_ram, 61);
    match rtc_and_ram {
        PersistentCartState::Mbc3RamRtc { ram, rtc } => {
            assert_eq!(ram, vec![0x55; 8]);
            assert_ne!(rtc, original_rtc);
        }
        other => panic!("expected Mbc3RamRtc, got {other:?}"),
    }

    let original_huc3_rtc = Huc3RtcPersistentState {
        current_minutes_of_day: 10,
        current_days: 2,
        current_subminute_seconds: 58,
        event_minutes_of_day: 30,
        event_days: 2,
    };
    let mut huc3 = PersistentCartState::Huc3 {
        ram: vec![0xAA; 8],
        mcu_ram: [0; 256],
        rtc: original_huc3_rtc,
        rom_bank: 0,
        ram_bank: 0,
        select_mode: 0x0D,
        access_address: 0,
        mailbox_command: 0,
        mailbox_argument: 0,
        last_response_nybble: 0,
        semaphore_ready: true,
        ir_emitter_on: false,
        ir_light_detected: false,
        last_control_write: None,
        last_unsupported_command: None,
        last_unsupported_argument: None,
    };
    apply_elapsed_off_session_seconds(&mut huc3, 5);
    match huc3 {
        PersistentCartState::Huc3 { ram, rtc, .. } => {
            assert_eq!(ram, vec![0xAA; 8]);
            assert_eq!(rtc.current_minutes_of_day, 11);
            assert_eq!(rtc.current_days, 2);
            assert_eq!(rtc.current_subminute_seconds, 3);
        }
        other => panic!("expected Huc3, got {other:?}"),
    }

    let mut untouched = PersistentCartState::None;
    apply_elapsed_off_session_seconds(&mut untouched, 999);
    assert_eq!(untouched, PersistentCartState::None);

    assert_eq!(
        framebuffer_output_format(Path::new("framebuffer.PNG")),
        FramebufferOutputFormat::Png
    );
    assert_eq!(
        framebuffer_output_format(Path::new("framebuffer.raw")),
        FramebufferOutputFormat::Pgm
    );
    assert_eq!(framebuffer_pixel_to_grayscale(0), 255);
    assert_eq!(framebuffer_pixel_to_grayscale(1), 170);
    assert_eq!(framebuffer_pixel_to_grayscale(2), 85);
    assert_eq!(framebuffer_pixel_to_grayscale(3), 0);
    assert_eq!(framebuffer_pixel_to_grayscale(9), 0);

    let png_artifact = encode_framebuffer_artifact(
        Path::new("framebuffer.png"),
        &vec![0; FRAMEBUFFER_WIDTH * FRAMEBUFFER_HEIGHT],
        None,
        None,
        None,
    )
    .expect("PNG encoding should succeed");
    assert!(png_artifact.starts_with(b"\x89PNG\r\n\x1A\n"));
    let palette_pgm_artifact = encode_framebuffer_artifact(
        Path::new("framebuffer.pgm"),
        &[0, 1, 2, 3, 9],
        None,
        None,
        Some(DMG_GREY_DISPLAY_PALETTE),
    )
    .expect("palette PGM encoding should succeed");
    assert_eq!(
        palette_pgm_artifact,
        b"P5\n160 144\n255\n\xff\xaa\x55\x00\x00".to_vec()
    );
    let palette_png_artifact = encode_framebuffer_artifact(
        Path::new("framebuffer.png"),
        &vec![0; FRAMEBUFFER_WIDTH * FRAMEBUFFER_HEIGHT],
        None,
        None,
        Some(DMG_GREY_DISPLAY_PALETTE),
    )
    .expect("palette PNG encoding should succeed");
    assert!(palette_png_artifact.starts_with(b"\x89PNG\r\n\x1A\n"));
    let rgb555_png_artifact = encode_framebuffer_artifact(
        Path::new("framebuffer.png"),
        &vec![3; FRAMEBUFFER_WIDTH * FRAMEBUFFER_HEIGHT],
        None,
        Some(&vec![0x7FFF; FRAMEBUFFER_WIDTH * FRAMEBUFFER_HEIGHT]),
        Some(DMG_GREY_DISPLAY_PALETTE),
    )
    .expect("CGB RGB555 PNG encoding should succeed");
    assert!(rgb555_png_artifact.starts_with(b"\x89PNG\r\n\x1A\n"));
    let sgb_rgb555_png_artifact = encode_framebuffer_artifact(
        Path::new("framebuffer.png"),
        &vec![3; FRAMEBUFFER_WIDTH * FRAMEBUFFER_HEIGHT],
        Some((
            SGB_HOST_FRAMEBUFFER_WIDTH,
            SGB_HOST_FRAMEBUFFER_HEIGHT,
            &vec![0x7FFF; SGB_HOST_FRAMEBUFFER_WIDTH * SGB_HOST_FRAMEBUFFER_HEIGHT],
        )),
        Some(&vec![0x001F; FRAMEBUFFER_WIDTH * FRAMEBUFFER_HEIGHT]),
        Some(DMG_GREY_DISPLAY_PALETTE),
    )
    .expect("SGB RGB555 PNG encoding should succeed");
    assert!(sgb_rgb555_png_artifact.starts_with(b"\x89PNG\r\n\x1A\n"));
    let sgb_info = decode_png_info(&sgb_rgb555_png_artifact);
    assert_eq!(sgb_info.width, SGB_HOST_FRAMEBUFFER_WIDTH as u32);
    assert_eq!(sgb_info.height, SGB_HOST_FRAMEBUFFER_HEIGHT as u32);
    let sgb_lcd_rgb555_png_artifact = encode_framebuffer_artifact(
        Path::new("framebuffer.png"),
        &vec![3; FRAMEBUFFER_WIDTH * FRAMEBUFFER_HEIGHT],
        Some((
            FRAMEBUFFER_WIDTH,
            FRAMEBUFFER_HEIGHT,
            &vec![0x7FFF; FRAMEBUFFER_WIDTH * FRAMEBUFFER_HEIGHT],
        )),
        Some(&vec![0x001F; FRAMEBUFFER_WIDTH * FRAMEBUFFER_HEIGHT]),
        Some(DMG_GREY_DISPLAY_PALETTE),
    )
    .expect("SGB LCD RGB555 PNG encoding should succeed");
    assert!(sgb_lcd_rgb555_png_artifact.starts_with(b"\x89PNG\r\n\x1A\n"));
    let sgb_lcd_info = decode_png_info(&sgb_lcd_rgb555_png_artifact);
    assert_eq!(sgb_lcd_info.width, FRAMEBUFFER_WIDTH as u32);
    assert_eq!(sgb_lcd_info.height, FRAMEBUFFER_HEIGHT as u32);
    let borrowed_sgb_border = SgbBorrowedBorder::new(SgbBorderState::default());
    let borrowed_sgb_png_artifact = encode_framebuffer_artifact_with_borrowed_sgb_border(
        Path::new("framebuffer.png"),
        &vec![3; FRAMEBUFFER_WIDTH * FRAMEBUFFER_HEIGHT],
        None,
        Some(&borrowed_sgb_border),
        Some(&vec![0x001F; FRAMEBUFFER_WIDTH * FRAMEBUFFER_HEIGHT]),
        Some(DMG_GREY_DISPLAY_PALETTE),
    )
    .expect("borrowed SGB border PNG encoding should succeed");
    let borrowed_sgb_info = decode_png_info(&borrowed_sgb_png_artifact);
    assert_eq!(borrowed_sgb_info.width, SGB_HOST_FRAMEBUFFER_WIDTH as u32);
    assert_eq!(borrowed_sgb_info.height, SGB_HOST_FRAMEBUFFER_HEIGHT as u32);
    let direct_png = encode_framebuffer_png(&vec![0; FRAMEBUFFER_WIDTH * FRAMEBUFFER_HEIGHT])
        .expect("direct PNG encoding should succeed");
    assert!(direct_png.starts_with(b"\x89PNG\r\n\x1A\n"));
    let direct_palette_png = encode_framebuffer_palette_png(
        &vec![0; FRAMEBUFFER_WIDTH * FRAMEBUFFER_HEIGHT],
        DMG_GREY_DISPLAY_PALETTE,
    )
    .expect("direct palette PNG encoding should succeed");
    assert!(direct_palette_png.starts_with(b"\x89PNG\r\n\x1A\n"));
    let direct_rgb_png = encode_rgb555_framebuffer_png(
        FRAMEBUFFER_WIDTH,
        FRAMEBUFFER_HEIGHT,
        &vec![0x001F; FRAMEBUFFER_WIDTH * FRAMEBUFFER_HEIGHT],
    )
    .expect("direct CGB RGB555 PNG encoding should succeed");
    assert!(direct_rgb_png.starts_with(b"\x89PNG\r\n\x1A\n"));
    let grayscale_png =
        encode_grayscale_png(2, 2, &[0, 170, 85, 255]).expect("small grayscale PNG should encode");
    let rgb_png = encode_rgb_png(1, 1, &[[255, 0, 0]]).expect("small RGB PNG should encode");
    assert!(rgb_png.starts_with(b"\x89PNG\r\n\x1A\n"));
    assert!(grayscale_png.starts_with(b"\x89PNG\r\n\x1A\n"));
    let png_error = png_encoding_io_error(png::EncodingError::IoError(io::Error::other("bad png")));
    assert_eq!(png_error.kind(), io::ErrorKind::Other);
    assert!(png_error.to_string().contains("bad png"));
    assert_eq!(
        format_framebuffer_artifact_error(
            Path::new("framebuffer.png"),
            io::Error::other("bad png")
        ),
        "failed to encode framebuffer artifact framebuffer.png: bad png"
    );
    assert_eq!(
        format_save_load_error(
            Path::new("demo.gbsav"),
            CartridgeSaveBackendError::UnexpectedEof {
                offset: 1,
                needed: 2,
                remaining: 0,
            }
        ),
        "failed to load save demo.gbsav: unexpected end of save payload at offset 1: needed 2 bytes but only 0 remain"
    );
    assert_eq!(
        format_save_flush_error(
            Path::new("demo.gbsav"),
            "frame-boundary",
            CartridgeSaveBackendError::UnexpectedEof {
                offset: 1,
                needed: 2,
                remaining: 0,
            }
        ),
        "failed to save cartridge persistence (frame-boundary) to demo.gbsav: unexpected end of save payload at offset 1: needed 2 bytes but only 0 remain"
    );
    assert_eq!(
        format_boot_rom_asset_load_error(
            Path::new("bootrom"),
            BootRomAssetError::DirectoryNotFound {
                path: PathBuf::from("bootrom"),
            }
        ),
        "failed to load boot ROM assets from bootrom: boot ROM asset directory does not exist: bootrom"
    );

    let temp_dir = unique_temp_dir("helpers");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
    let nested_text = temp_dir.join("nested/path/report.txt");
    write_text_file_with_parent(&nested_text, "trace=ok")
        .expect("text output should create parent directories");
    assert_eq!(
        fs::read_to_string(&nested_text).expect("text output should be readable"),
        "trace=ok"
    );

    let nested_bytes = temp_dir.join("nested/path/frame.bin");
    write_bytes_with_parent(&nested_bytes, b"\x01\x02").expect("byte output should be writable");
    assert_eq!(
        fs::read(&nested_bytes).expect("byte output should be readable"),
        b"\x01\x02"
    );

    let blocking_parent = temp_dir.join("blocking");
    fs::write(&blocking_parent, b"file").expect("blocking file should be writable");
    let write_error = write_bytes_with_parent(&blocking_parent.join("child.bin"), b"\x00")
        .expect_err("non-directory parents should fail");
    assert!(write_error.contains("failed to create directory"));
    let blocking_target = temp_dir.join("target-dir.bin");
    fs::create_dir_all(&blocking_target).expect("blocking target directory should be creatable");
    let write_file_error = write_bytes_with_parent(&blocking_target, b"\x00")
        .expect_err("directory targets should fail file writes");
    assert!(write_file_error.contains("failed to write"));

    let missing_conversion_rom = temp_dir.join("missing-conversion.gb");
    let conversion_read_error =
        load_cartridge_for_save_conversion(&missing_conversion_rom, &mut Vec::new())
            .expect_err("missing conversion ROMs should surface read errors");
    assert!(conversion_read_error.contains("failed to read ROM"));

    let mut write_error_writer = FailOnWrite {
        fail_on_write: Some(1),
        ..FailOnWrite::default()
    };
    assert!(
        write_text(&mut write_error_writer, "hello")
            .expect_err("write failures should be surfaced")
            .contains("failed to write output")
    );
    let mut newline_error_writer = FailOnWrite {
        fail_on_write: Some(2),
        ..FailOnWrite::default()
    };
    assert!(
        writeln_checked(&mut newline_error_writer, "line")
            .expect_err("newline writes should be checked")
            .contains("failed to write output")
    );
    let mut first_write_error_writer = FailOnWrite {
        fail_on_write: Some(1),
        ..FailOnWrite::default()
    };
    assert!(
        writeln_checked(&mut first_write_error_writer, "line")
            .expect_err("line writes should fail fast on the first write")
            .contains("failed to write output")
    );
    let mut ok_writer = Vec::new();
    write_text(&mut ok_writer, "plain").expect("plain writes should succeed");
    writeln_checked(&mut ok_writer, "line").expect("line writes should succeed");
    assert_eq!(
        String::from_utf8(ok_writer).expect("output should be UTF-8"),
        "plainline\n"
    );

    let diagnostics = vec![
        CartridgeDiagnostic {
            severity: CartridgeDiagnosticSeverity::Warning,
            message: "warn".to_string(),
        },
        CartridgeDiagnostic {
            severity: CartridgeDiagnosticSeverity::Error,
            message: "err".to_string(),
        },
    ];
    let mut diagnostic_output = Vec::new();
    write_cartridge_diagnostics(&mut diagnostic_output, &diagnostics)
        .expect("diagnostics should be writable");
    let diagnostic_text =
        String::from_utf8(diagnostic_output).expect("diagnostic output should be UTF-8");
    assert!(diagnostic_text.contains("warning: warn"));
    assert!(diagnostic_text.contains("error: err"));
    let mut failing_diagnostic_output = FailOnWrite {
        fail_on_write: Some(1),
        ..FailOnWrite::default()
    };
    let diagnostic_write_error =
        write_cartridge_diagnostics(&mut failing_diagnostic_output, &diagnostics)
            .expect_err("diagnostic write failures should surface");
    assert!(diagnostic_write_error.contains("failed to write output"));

    assert_eq!(
        format_header_parse_error(CartridgeHeaderParseError::ImageTooSmall {
            actual_size: 4,
            minimum_size: HEADER_MINIMUM_ROM_LEN,
        }),
        format!(
            "ROM image is too small to contain a cartridge header: expected at least {} bytes, got 4",
            HEADER_MINIMUM_ROM_LEN
        )
    );
    assert_eq!(
        format_cartridge_load_error(CartridgeLoadError::HeaderParse(
            CartridgeHeaderParseError::ImageTooSmall {
                actual_size: 4,
                minimum_size: HEADER_MINIMUM_ROM_LEN,
            }
        )),
        format!(
            "ROM image is too small to contain a cartridge header: expected at least {} bytes, got 4",
            HEADER_MINIMUM_ROM_LEN
        )
    );
    let rejected_message = format_cartridge_load_error(CartridgeLoadError::Rejected {
        classification: CartridgeClassification::classify(0xFD),
        execution_mode: ExecutionMode::Experimental,
        reason: "requires dedicated hardware".to_string(),
        diagnostics: vec![
            CartridgeDiagnostic {
                severity: CartridgeDiagnosticSeverity::Warning,
                message: "warn".to_string(),
            },
            CartridgeDiagnostic {
                severity: CartridgeDiagnosticSeverity::Error,
                message: "err".to_string(),
            },
        ],
    });
    assert!(rejected_message.contains("cartridge rejected under experimental"));
    assert!(rejected_message.contains("mapper=BANDAI TAMA5"));
    assert!(rejected_message.contains("selection=unsupported-accessory"));
    assert!(rejected_message.contains("diagnostics=[warning warn; error err]"));
    let rejected_without_diagnostics = format_cartridge_load_error(CartridgeLoadError::Rejected {
        classification: CartridgeClassification::classify(0xFD),
        execution_mode: ExecutionMode::Strict,
        reason: "no diagnostics".to_string(),
        diagnostics: Vec::new(),
    });
    assert!(rejected_without_diagnostics.contains("cartridge rejected under strict"));
    assert!(!rejected_without_diagnostics.contains("diagnostics=["));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}
