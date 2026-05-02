use super::*;
use gb_core::{
    BootRomAssetError, CartridgeClassification, Huc3RtcPersistentState, Mbc3RtcPersistentState,
    SupportedCartridgeFamily,
};
use gb_persistence::{
    CartridgeSaveBackendError, FilesystemCartridgeSaveBackend, decode_machine_save_state_envelope,
};
use std::io;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn unique_temp_dir(label: &str) -> PathBuf {
    env::temp_dir().join(format!(
        "gb-cli-{label}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ))
}

fn build_test_rom_with_header(
    program: &[u8],
    cartridge_type: u8,
    rom_size: u8,
    ram_size: u8,
) -> Vec<u8> {
    let mut rom = vec![0xFF; HEADER_MINIMUM_ROM_LEN.max(32 * 1024)];
    for (offset, byte) in program.iter().copied().enumerate() {
        rom[0x0100 + offset] = byte;
    }
    rom[0x0147] = cartridge_type;
    rom[0x0148] = rom_size;
    rom[0x0149] = ram_size;
    rom
}

fn build_single_byte_serial_rom(byte: u8) -> Vec<u8> {
    build_test_rom_with_header(
        &[
            0x3E, byte, // LD A,d8
            0xE0, 0x01, // LDH (SB),A
            0x3E, 0x81, // LD A,$81
            0xE0, 0x02, // LDH (SC),A
            0xC3, 0x08, 0x01, // JP $0108
        ],
        0x00,
        0x00,
        0x00,
    )
}

fn build_nop_loop_rom() -> Vec<u8> {
    build_test_rom_with_header(
        &[
            0x00, // NOP
            0x00, // NOP
            0xC3, 0x00, 0x01, // JP $0100
        ],
        0x00,
        0x00,
        0x00,
    )
}

fn build_battery_backed_serial_and_ram_rom(byte: u8, ram_value: u8) -> Vec<u8> {
    build_test_rom_with_header(
        &[
            0x3E, ram_value, // LD A,d8
            0xEA, 0x00, 0xA0, // LD ($A000),A
            0x3E, byte, // LD A,d8
            0xE0, 0x01, // LDH (SB),A
            0x3E, 0x81, // LD A,$81
            0xE0, 0x02, // LDH (SC),A
            0xC3, 0x0D, 0x01, // JP $010D
        ],
        0x09,
        0x00,
        0x02,
    )
}

fn build_loaded_machine(rom: Vec<u8>, capture_trace: bool) -> CliMachine {
    let config = MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot);
    let mut machine = CliMachine::new(config, capture_trace);
    machine
        .load_cartridge(rom)
        .expect("test ROM should load into the machine");
    machine
}

fn write_fake_boot_rom(dir: &Path, kind: BootRomKind, fill: u8) {
    fs::create_dir_all(dir).expect("boot ROM directory should be creatable");
    fs::write(dir.join(BootRomAssets::filename(kind)), vec![fill; 0x0100])
        .expect("boot ROM image should be writable");
}

#[derive(Default)]
struct FailOnWrite {
    fail_on_write: Option<usize>,
    fail_on_flush: bool,
    writes: usize,
}

impl Write for FailOnWrite {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.writes += 1;
        if self.fail_on_write == Some(self.writes) {
            Err(io::Error::other("synthetic write failure"))
        } else {
            Ok(buf.len())
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.fail_on_flush {
            Err(io::Error::other("synthetic flush failure"))
        } else {
            Ok(())
        }
    }
}

#[test]
fn parse_run_arguments_keep_the_default_game_boy_model() {
    let action = parse_cli_arguments(["run", "demo.gb"]).expect("run arguments should parse");

    match action {
        CliAction::Run(options) => {
            assert_eq!(options.model, RunModel::GameBoy);
            assert_eq!(options.startup_mode, StartupMode::SkipBoot);
            assert_eq!(options.execution_mode, ExecutionMode::Strict);
            assert_eq!(
                options.default_run_budget,
                Some(DefaultRunBudget::SkipBootFrames {
                    frame_limit: DEFAULT_SKIP_BOOT_FRAME_LIMIT,
                })
            );
            assert_eq!(options.frame_limit, None);
            assert_eq!(options.tcycle_limit, None);
        }
        other => panic!("expected run action, got {other:?}"),
    }
}

#[test]
fn parse_run_arguments_use_the_real_boot_default_budget_profile_when_no_limit_is_provided() {
    let action = parse_cli_arguments(["run", "demo.gb", "--startup", "real-boot"])
        .expect("real-boot arguments should parse");

    match action {
        CliAction::Run(options) => {
            assert_eq!(options.startup_mode, StartupMode::RealBoot);
            assert_eq!(
                options.default_run_budget,
                Some(DefaultRunBudget::RealBootPostHandoff {
                    post_handoff_frame_limit: DEFAULT_REAL_BOOT_POST_HANDOFF_FRAME_LIMIT,
                    safety_frame_limit: DEFAULT_REAL_BOOT_SAFETY_FRAME_LIMIT,
                })
            );
            assert_eq!(options.frame_limit, None);
            assert_eq!(options.tcycle_limit, None);
        }
        other => panic!("expected run action, got {other:?}"),
    }
}

#[test]
fn default_run_limit_profiles_cover_skip_boot_post_handoff_and_safety_cap() {
    assert!(default_run_limit_reached(
        Some(DefaultRunBudget::SkipBootFrames {
            frame_limit: DEFAULT_SKIP_BOOT_FRAME_LIMIT,
        }),
        DEFAULT_SKIP_BOOT_FRAME_LIMIT,
        None,
    ));
    assert!(!default_run_limit_reached(
        Some(DefaultRunBudget::RealBootPostHandoff {
            post_handoff_frame_limit: DEFAULT_REAL_BOOT_POST_HANDOFF_FRAME_LIMIT,
            safety_frame_limit: DEFAULT_REAL_BOOT_SAFETY_FRAME_LIMIT,
        }),
        DEFAULT_REAL_BOOT_POST_HANDOFF_FRAME_LIMIT,
        None,
    ));
    assert!(default_run_limit_reached(
        Some(DefaultRunBudget::RealBootPostHandoff {
            post_handoff_frame_limit: DEFAULT_REAL_BOOT_POST_HANDOFF_FRAME_LIMIT,
            safety_frame_limit: DEFAULT_REAL_BOOT_SAFETY_FRAME_LIMIT,
        }),
        DEFAULT_REAL_BOOT_SAFETY_FRAME_LIMIT,
        None,
    ));
    assert!(!default_run_limit_reached(
        Some(DefaultRunBudget::RealBootPostHandoff {
            post_handoff_frame_limit: DEFAULT_REAL_BOOT_POST_HANDOFF_FRAME_LIMIT,
            safety_frame_limit: DEFAULT_REAL_BOOT_SAFETY_FRAME_LIMIT,
        }),
        121,
        Some(2),
    ));
    assert!(default_run_limit_reached(
        Some(DefaultRunBudget::RealBootPostHandoff {
            post_handoff_frame_limit: DEFAULT_REAL_BOOT_POST_HANDOFF_FRAME_LIMIT,
            safety_frame_limit: DEFAULT_REAL_BOOT_SAFETY_FRAME_LIMIT,
        }),
        122,
        Some(2),
    ));
}

#[test]
fn inspect_rom_reports_the_supported_nomcb_header_shape() {
    let temp_dir = unique_temp_dir("inspect-rom");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path = temp_dir.join("inspect.gb");
    fs::write(&rom_path, build_single_byte_serial_rom(b'O')).expect("test ROM should be writable");

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    run_cli_command(
        [
            "inspect-rom",
            rom_path.to_str().expect("path should be valid UTF-8"),
        ],
        &mut stdout,
        &mut stderr,
    )
    .expect("inspect-rom should succeed");

    let output = String::from_utf8(stdout).expect("inspect output should be UTF-8");
    assert!(output.contains("load_status=ok"));
    assert!(output.contains("mapper_name=ROM ONLY"));
    assert!(output.contains("selection=supported"));
    assert!(stderr.is_empty());

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn run_command_emits_requested_artifacts_and_persists_battery_backed_ram() {
    let temp_dir = unique_temp_dir("run-artifacts");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path =
        temp_dir.join("Legend of Zelda, The - Link's Awakening (USA, Europe) (Rev 2).gb");
    let serial_path = temp_dir.join("artifacts/serial.bin");
    let framebuffer_path = temp_dir.join("artifacts/framebuffer.png");
    let trace_path = temp_dir.join("artifacts/trace.txt");
    let save_root = temp_dir.join("saves");
    fs::write(
        &rom_path,
        build_battery_backed_serial_and_ram_rom(b'R', 0x5A),
    )
    .expect("test ROM should be writable");

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    run_cli_command(
        [
            "run",
            rom_path.to_str().expect("path should be valid UTF-8"),
            "--tcycles",
            "10000",
            "--serial-out",
            serial_path.to_str().expect("path should be valid UTF-8"),
            "--framebuffer-out",
            framebuffer_path
                .to_str()
                .expect("path should be valid UTF-8"),
            "--trace-out",
            trace_path.to_str().expect("path should be valid UTF-8"),
            "--save-dir",
            save_root.to_str().expect("path should be valid UTF-8"),
        ],
        &mut stdout,
        &mut stderr,
    )
    .expect("run command should succeed");

    assert!(
        stdout.is_empty(),
        "serial was written to a file, not stdout"
    );
    assert_eq!(
        fs::read(&serial_path).expect("serial output should exist"),
        b"R"
    );
    let framebuffer = fs::read(&framebuffer_path).expect("framebuffer should exist");
    assert!(framebuffer.starts_with(b"\x89PNG\r\n\x1A\n"));
    let decoder = png::Decoder::new(std::io::Cursor::new(&framebuffer));
    let mut reader = decoder.read_info().expect("PNG should decode");
    let mut buffer = vec![
        0;
        reader
            .output_buffer_size()
            .expect("PNG decoder should expose an output buffer size")
    ];
    let info = reader
        .next_frame(&mut buffer)
        .expect("PNG frame should decode");
    assert_eq!(info.width, FRAMEBUFFER_WIDTH as u32);
    assert_eq!(info.height, FRAMEBUFFER_HEIGHT as u32);
    assert_eq!(info.color_type, png::ColorType::Grayscale);
    let trace = fs::read_to_string(&trace_path).expect("trace should exist");
    assert!(trace.contains("t_cycle="));

    let save_key = derive_save_key(&rom_path).expect("save key should derive");
    let backend = FilesystemCartridgeSaveBackend::new(&save_root);
    let envelope = backend
        .load(&save_key)
        .expect("save should be readable")
        .expect("save should exist");
    match envelope.persistent_state {
        PersistentCartState::NoMbcRam { ram } => assert_eq!(ram[0], 0x5A),
        other => panic!("expected NoMbcRam persistence, got {other:?}"),
    }

    let stderr_output = String::from_utf8(stderr).expect("stderr should be UTF-8");
    assert!(stderr_output.contains("save_writes=1"));
    assert!(stderr_output.contains("serial_bytes=1"));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn run_command_saves_and_restores_machine_save_states() {
    let temp_dir = unique_temp_dir("run-machine-state");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom = build_nop_loop_rom();
    let rom_path = temp_dir.join("state.gb");
    let first_state_path = temp_dir.join("states/first.gbstate");
    let restored_state_path = temp_dir.join("states/restored.gbstate");
    fs::write(&rom_path, &rom).expect("test ROM should be writable");

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    run_cli_command(
        [
            "run",
            rom_path.to_str().expect("path should be valid UTF-8"),
            "--tcycles",
            "64",
            "--state-out",
            first_state_path
                .to_str()
                .expect("path should be valid UTF-8"),
        ],
        &mut stdout,
        &mut stderr,
    )
    .expect("state-out run should succeed");
    assert!(stdout.is_empty());
    let first_state_bytes = fs::read(&first_state_path).expect(".gbstate should be created");
    decode_machine_save_state_envelope(&first_state_bytes).expect(".gbstate should decode");
    let first_stderr = String::from_utf8(stderr).expect("stderr should be UTF-8");
    assert!(first_stderr.contains("state_out="));

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    run_cli_command(
        [
            "run",
            rom_path.to_str().expect("path should be valid UTF-8"),
            "--tcycles",
            "64",
            "--state-in",
            first_state_path
                .to_str()
                .expect("path should be valid UTF-8"),
            "--state-out",
            restored_state_path
                .to_str()
                .expect("path should be valid UTF-8"),
        ],
        &mut stdout,
        &mut stderr,
    )
    .expect("state-in continuation run should succeed");
    assert!(stdout.is_empty());
    let restored = decode_machine_save_state_envelope(
        &fs::read(&restored_state_path).expect("restored .gbstate should exist"),
    )
    .expect("restored .gbstate should decode");
    let stderr_output = String::from_utf8(stderr).expect("stderr should be UTF-8");
    assert!(stderr_output.contains("state_in="));
    assert!(stderr_output.contains("state_out="));

    let mut uninterrupted = build_loaded_machine(rom, false);
    for _ in 0..128 {
        uninterrupted.step_t_cycle();
    }
    assert_eq!(restored.state, uninterrupted.capture_save_state());

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn run_command_rejects_incompatible_machine_save_states() {
    let temp_dir = unique_temp_dir("run-machine-state-mismatch");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path = temp_dir.join("state.gb");
    let state_path = temp_dir.join("state.gbstate");
    fs::write(&rom_path, build_nop_loop_rom()).expect("test ROM should be writable");
    run_cli_command(
        [
            "run",
            rom_path.to_str().expect("path should be valid UTF-8"),
            "--tcycles",
            "16",
            "--state-out",
            state_path.to_str().expect("path should be valid UTF-8"),
        ],
        &mut Vec::new(),
        &mut Vec::new(),
    )
    .expect("state-out seed run should succeed");

    let error = run_cli_command(
        [
            "run",
            rom_path.to_str().expect("path should be valid UTF-8"),
            "--model",
            "mgb",
            "--tcycles",
            "1",
            "--state-in",
            state_path.to_str().expect("path should be valid UTF-8"),
        ],
        &mut Vec::new(),
        &mut Vec::new(),
    )
    .expect_err("model-incompatible state should fail restore");
    assert!(error.contains("failed to restore state"));
    assert!(error.to_ascii_lowercase().contains("model"));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn run_command_state_in_uses_restored_cartridge_state_as_save_baseline() {
    let temp_dir = unique_temp_dir("run-machine-state-save-baseline");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom = build_battery_backed_serial_and_ram_rom(b'B', 0x11);
    let rom_path = temp_dir.join("battery.gb");
    let state_path = temp_dir.join("battery.gbstate");
    let save_root = temp_dir.join("saves");
    fs::write(&rom_path, &rom).expect("battery ROM should be writable");

    run_cli_command(
        [
            "run",
            rom_path.to_str().expect("path should be valid UTF-8"),
            "--tcycles",
            "256",
            "--state-out",
            state_path.to_str().expect("path should be valid UTF-8"),
        ],
        &mut Vec::new(),
        &mut Vec::new(),
    )
    .expect("state-out seed run should succeed");

    let mut seeded_ram = vec![0xEE; 8 * 1024];
    seeded_ram[0] = 0xEE;
    let seed_machine = build_loaded_machine(rom, false);
    let save_key = derive_save_key(&rom_path).expect("save key should derive");
    let mut backend = FilesystemCartridgeSaveBackend::new(&save_root);
    backend
        .save(
            &save_key,
            seed_machine.cartridge().persistence_metadata(),
            &PersistentCartState::NoMbcRam { ram: seeded_ram },
        )
        .expect("pre-existing .gbsav should persist");

    let mut stderr = Vec::new();
    run_cli_command(
        [
            "run",
            rom_path.to_str().expect("path should be valid UTF-8"),
            "--tcycles",
            "1",
            "--state-in",
            state_path.to_str().expect("path should be valid UTF-8"),
            "--save-dir",
            save_root.to_str().expect("path should be valid UTF-8"),
            "--save-policy",
            "on-close",
        ],
        &mut Vec::new(),
        &mut stderr,
    )
    .expect("state-in run should skip pre-existing .gbsav restore");
    let stderr_output = String::from_utf8(stderr).expect("stderr should be UTF-8");
    assert!(
        !stderr_output.contains("save_loaded path="),
        "{stderr_output}"
    );
    assert!(stderr_output.contains("save_loaded_existing=false"));
    assert!(stderr_output.contains("save_writes=0"));
    let envelope = backend
        .load(&save_key)
        .expect("seed .gbsav should remain readable")
        .expect("seed .gbsav should still exist");
    match envelope.persistent_state {
        PersistentCartState::NoMbcRam { ram } => assert_eq!(ram[0], 0xEE),
        other => panic!("expected NoMbcRam persistence, got {other:?}"),
    }

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn saves_commands_export_and_import_external_sav_files() {
    let temp_dir = unique_temp_dir("saves-convert");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path =
        temp_dir.join("Legend of Zelda, The - Link's Awakening (USA, Europe) (Rev 2).gb");
    let save_root = temp_dir.join("saves");
    let external_path = temp_dir.join("exports/battery.sav");
    let rom = build_battery_backed_serial_and_ram_rom(b'S', 0x12);
    fs::write(&rom_path, &rom).expect("test ROM should be writable");

    let report = CartridgeSlot::load(rom, &CompatibilityPolicy::strict())
        .expect("test ROM should load for save seeding");
    let key = derive_save_key(&rom_path).expect("save key should derive");
    let legacy_key =
        legacy_save_key_for_rom_path(None, &rom_path).expect("legacy key should derive");
    let mut backend = FilesystemCartridgeSaveBackend::new(&save_root);
    let mut seeded_ram = vec![0; 8 * 1024];
    seeded_ram[0] = 0x5A;
    seeded_ram[1] = 0xC3;
    backend
        .save(
            &legacy_key,
            report.cartridge().persistence_metadata(),
            &PersistentCartState::NoMbcRam { ram: seeded_ram },
        )
        .expect("legacy seed save should persist");

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    run_cli_command(
        [
            "saves",
            "export",
            rom_path.to_str().expect("path should be valid UTF-8"),
            external_path.to_str().expect("path should be valid UTF-8"),
            "--save-dir",
            save_root.to_str().expect("path should be valid UTF-8"),
        ],
        &mut stdout,
        &mut stderr,
    )
    .expect("save export should succeed");
    assert_eq!(
        &fs::read(&external_path).expect("external save should exist")[..2],
        &[0x5A, 0xC3]
    );
    let output = String::from_utf8(stdout).expect("stdout should be UTF-8");
    assert!(
        output.contains("save_key=Legend of Zelda, The - Link's Awakening (USA, Europe) (Rev 2)"),
        "{output}"
    );
    assert!(output.contains("Legend_of_Zelda_The_-_Link_s_Awakening_USA_Europe_Rev_2.gbsav"));
    assert!(output.contains("external_bytes=8192"));
    let _ = String::from_utf8(stderr).expect("stderr should be UTF-8");

    let mut imported = fs::read(&external_path).expect("external save should be readable");
    imported[0] = 0xA5;
    imported[1] = 0x3C;
    fs::write(&external_path, imported).expect("external save should be writable");

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    run_cli_command(
        [
            "saves",
            "import",
            rom_path.to_str().expect("path should be valid UTF-8"),
            external_path.to_str().expect("path should be valid UTF-8"),
            "--save-dir",
            save_root.to_str().expect("path should be valid UTF-8"),
        ],
        &mut stdout,
        &mut stderr,
    )
    .expect("save import should succeed");

    let envelope = backend
        .load(&key)
        .expect("imported save should be readable")
        .expect("imported save should exist");
    match envelope.persistent_state {
        PersistentCartState::NoMbcRam { ram } => assert_eq!(&ram[..2], &[0xA5, 0x3C]),
        other => panic!("expected NoMbcRam persistence, got {other:?}"),
    }
    let output = String::from_utf8(stdout).expect("stdout should be UTF-8");
    assert!(output.contains("target_gbsav="));
    let _ = String::from_utf8(stderr).expect("stderr should be UTF-8");

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn saves_commands_cover_conversion_error_paths_and_legacy_session_loads() {
    let temp_dir = unique_temp_dir("saves-convert-errors");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let plain_rom_path = temp_dir.join("plain.gb");
    fs::write(&plain_rom_path, build_single_byte_serial_rom(b'N'))
        .expect("plain ROM should be writable");
    let save_root = temp_dir.join("saves");
    let external_path = temp_dir.join("exports/plain.sav");
    let plain_options = SavesOptions {
        direction: SavesDirection::Export,
        rom_path: plain_rom_path.clone(),
        external_save_path: external_path.clone(),
        save_dir: save_root.clone(),
        save_key: None,
    };
    let export_error =
        saves_export_command(plain_options.clone(), &mut Vec::new(), &mut Vec::new())
            .expect_err("plain ROMs should not export saves");
    assert!(export_error.contains("does not expose battery-backed cartridge persistence"));
    let import_error = saves_import_command(
        SavesOptions {
            direction: SavesDirection::Import,
            ..plain_options
        },
        &mut Vec::new(),
        &mut Vec::new(),
    )
    .expect_err("plain ROMs should not import saves");
    assert!(import_error.contains("does not expose battery-backed cartridge persistence"));

    let battery_rom_path =
        temp_dir.join("Legend of Zelda, The - Link's Awakening (USA, Europe) (Rev 2).gb");
    let battery_rom = build_battery_backed_serial_and_ram_rom(b'B', 0x7E);
    fs::write(&battery_rom_path, &battery_rom).expect("battery ROM should be writable");
    let battery_options = SavesOptions {
        direction: SavesDirection::Export,
        rom_path: battery_rom_path.clone(),
        external_save_path: temp_dir.join("exports/battery.sav"),
        save_dir: save_root.clone(),
        save_key: None,
    };
    let no_save_error =
        saves_export_command(battery_options.clone(), &mut Vec::new(), &mut Vec::new())
            .expect_err("missing internal saves should fail export");
    assert!(no_save_error.contains("no gb-cycle save found"));

    let report = CartridgeSlot::load(battery_rom.clone(), &CompatibilityPolicy::strict())
        .expect("battery ROM should load");
    let exact_key = derive_save_key(&battery_rom_path).expect("exact key should derive");
    let legacy_key =
        legacy_save_key_for_rom_path(None, &battery_rom_path).expect("legacy key should derive");
    let mut backend = FilesystemCartridgeSaveBackend::new(&save_root);
    backend
        .save(
            &exact_key,
            report.cartridge().persistence_metadata(),
            &PersistentCartState::Mbc2Ram {
                ram_nibbles: [0; 512],
            },
        )
        .expect("mismatched save should still encode for compatibility checks");
    let mismatch_error =
        saves_export_command(battery_options.clone(), &mut Vec::new(), &mut Vec::new())
            .expect_err("mismatched internal saves should fail restore");
    assert!(mismatch_error.contains("is not compatible with ROM"));

    backend
        .delete(&exact_key)
        .expect("mismatched exact save should be removable");

    let legacy_path = backend.path_for_key(&legacy_key);
    fs::create_dir_all(legacy_path.parent().expect("legacy parent should exist"))
        .expect("legacy parent should be creatable");
    fs::write(&legacy_path, b"not-a-valid-save").expect("broken legacy save should be writable");
    let legacy_load_error =
        saves_export_command(battery_options.clone(), &mut Vec::new(), &mut Vec::new())
            .expect_err("broken legacy saves should surface load errors");
    assert!(legacy_load_error.contains("failed to load save"));
    fs::remove_file(&legacy_path).expect("broken legacy save should be removable");

    let mut legacy_ram = vec![0; 8 * 1024];
    legacy_ram[0] = 0x44;
    backend
        .save(
            &legacy_key,
            report.cartridge().persistence_metadata(),
            &PersistentCartState::NoMbcRam { ram: legacy_ram },
        )
        .expect("legacy save should persist");
    let mut machine = build_loaded_machine(battery_rom, false);
    let session = open_save_session(
        Some(&save_root),
        &RunOptions::default_with_rom(battery_rom_path.clone()),
        &battery_rom_path,
        &mut machine,
        &mut Vec::new(),
        true,
    )
    .expect("legacy save session should open")
    .expect("battery-backed ROMs should create a save session");
    assert!(session.loaded_existing_save);
    assert_eq!(session.key, exact_key);

    let missing_external_error = saves_import_command(
        SavesOptions {
            direction: SavesDirection::Import,
            external_save_path: temp_dir.join("missing.sav"),
            ..battery_options.clone()
        },
        &mut Vec::new(),
        &mut Vec::new(),
    )
    .expect_err("missing external saves should fail import");
    assert!(missing_external_error.contains("failed to read external .sav save"));

    let invalid_external_path = temp_dir.join("imports/invalid.sav");
    fs::create_dir_all(
        invalid_external_path
            .parent()
            .expect("import parent should exist"),
    )
    .expect("import parent should be creatable");
    fs::write(&invalid_external_path, [0xAA]).expect("invalid external save should be writable");
    let invalid_external_error = saves_import_command(
        SavesOptions {
            direction: SavesDirection::Import,
            external_save_path: invalid_external_path,
            save_key: Some("explicit-slot".to_string()),
            ..battery_options
        },
        &mut Vec::new(),
        &mut Vec::new(),
    )
    .expect_err("invalid external save lengths should fail import");
    assert!(invalid_external_error.contains("failed to convert external .sav save"));
    assert!(resolve_saves_key(Some("manual-slot"), &battery_rom_path).is_ok());

    let valid_external_path = temp_dir.join("imports/valid.sav");
    fs::write(&valid_external_path, vec![0x55; 8 * 1024])
        .expect("valid external save should be writable");
    let blocked_save_root = temp_dir.join("blocked-import-save");
    let blocked_backend = FilesystemCartridgeSaveBackend::new(&blocked_save_root);
    let blocked_target_path = blocked_backend.path_for_key(&exact_key);
    let mut blocked_temp_path = blocked_target_path.as_os_str().to_os_string();
    blocked_temp_path.push(".tmp");
    fs::create_dir_all(PathBuf::from(blocked_temp_path))
        .expect("blocked temporary save path should be creatable");
    let save_error = saves_import_command(
        SavesOptions {
            direction: SavesDirection::Import,
            rom_path: battery_rom_path.clone(),
            external_save_path: valid_external_path,
            save_dir: blocked_save_root,
            save_key: None,
        },
        &mut Vec::new(),
        &mut Vec::new(),
    )
    .expect_err("import save backend failures should surface");
    assert!(save_error.contains("failed to save cartridge persistence (saves-import)"));

    let blocking_parent = temp_dir.join("blocking-parent");
    fs::write(&blocking_parent, b"file").expect("blocking parent file should be writable");
    let write_error = write_bytes_with_parent(&blocking_parent.join("child.bin"), b"bytes")
        .expect_err("file parents should block directory creation");
    assert!(write_error.contains("failed to create directory"));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn saves_commands_surface_output_writer_failures() {
    let temp_dir = unique_temp_dir("saves-writer-failures");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path = temp_dir.join("battery.gb");
    let rom = build_battery_backed_serial_and_ram_rom(b'S', 0x22);
    fs::write(&rom_path, &rom).expect("battery ROM should be writable");
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("battery ROM should load");
    let save_root = temp_dir.join("saves");
    let save_key = derive_save_key(&rom_path).expect("save key should derive");
    let mut backend = FilesystemCartridgeSaveBackend::new(&save_root);
    backend
        .save(
            &save_key,
            report.cartridge().persistence_metadata(),
            &PersistentCartState::NoMbcRam {
                ram: vec![0x66; 8 * 1024],
            },
        )
        .expect("internal save should persist");

    for fail_on_write in [5, 7] {
        let options = SavesOptions {
            direction: SavesDirection::Export,
            rom_path: rom_path.clone(),
            external_save_path: temp_dir.join(format!("exports/export-{fail_on_write}.sav")),
            save_dir: save_root.clone(),
            save_key: None,
        };
        let mut output = FailOnWrite {
            fail_on_write: Some(fail_on_write),
            ..FailOnWrite::default()
        };
        let error = saves_export_command(options, &mut output, &mut Vec::new())
            .expect_err("export output write failures should surface");
        assert!(error.contains("failed to write output"));
    }

    let import_path = temp_dir.join("imports/import.sav");
    fs::create_dir_all(import_path.parent().expect("import parent should exist"))
        .expect("import parent should be creatable");
    fs::write(&import_path, vec![0x77; 8 * 1024]).expect("external save should be writable");
    for fail_on_write in [5, 7, 9] {
        let options = SavesOptions {
            direction: SavesDirection::Import,
            rom_path: rom_path.clone(),
            external_save_path: import_path.clone(),
            save_dir: temp_dir.join(format!("import-saves-{fail_on_write}")),
            save_key: None,
        };
        let mut output = FailOnWrite {
            fail_on_write: Some(fail_on_write),
            ..FailOnWrite::default()
        };
        let error = saves_import_command(options, &mut output, &mut Vec::new())
            .expect_err("import output write failures should surface");
        assert!(error.contains("failed to write output"));
    }

    let wrapper_options = SavesOptions {
        direction: SavesDirection::Export,
        rom_path,
        external_save_path: temp_dir.join("exports/wrapper.sav"),
        save_dir: save_root,
        save_key: None,
    };
    saves_command(wrapper_options, &mut Vec::new(), &mut Vec::new())
        .expect("saves command wrapper should dispatch export");

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn framebuffer_artifact_defaults_to_pgm_when_path_is_not_png() {
    let encoded = encode_framebuffer_artifact(Path::new("framebuffer.pgm"), &[0, 1, 2, 3], None)
        .expect("PGM encoding should succeed");

    assert!(encoded.starts_with(b"P5\n160 144\n3\n"));
}

#[test]
fn run_cli_command_routes_help_variants_and_unknown_subcommands() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    run_cli_command(std::iter::empty::<&str>(), &mut stdout, &mut stderr)
        .expect("empty CLI should print help");
    let output = String::from_utf8(stdout.clone()).expect("stdout should be UTF-8");
    assert!(output.contains("Commands:\n"));
    assert!(stderr.is_empty());

    stdout.clear();
    stderr.clear();
    run_cli_command(["run", "--help"], &mut stdout, &mut stderr).expect("run help should succeed");
    assert_eq!(
        String::from_utf8(stdout.clone()).expect("stdout should be UTF-8"),
        RUN_HELP_TEXT
    );
    assert!(stderr.is_empty());

    stdout.clear();
    stderr.clear();
    run_cli_command(["inspect-rom", "--help"], &mut stdout, &mut stderr)
        .expect("inspect help should succeed");
    assert_eq!(
        String::from_utf8(stdout.clone()).expect("stdout should be UTF-8"),
        INSPECT_HELP_TEXT
    );
    assert!(stderr.is_empty());

    stdout.clear();
    stderr.clear();
    run_cli_command(["saves", "--help"], &mut stdout, &mut stderr)
        .expect("saves help should succeed");
    assert_eq!(
        String::from_utf8(stdout).expect("stdout should be UTF-8"),
        SAVES_HELP_TEXT
    );
    assert!(stderr.is_empty());

    assert_eq!(
        run_cli_command(["wat"], &mut Vec::new(), &mut Vec::new())
            .expect_err("unknown subcommands should fail"),
        "unknown subcommand \"wat\"; run `gb-cli --help` for usage"
    );
}

#[test]
fn parse_run_arguments_accepts_the_full_option_matrix() {
    let action = parse_run_arguments([
        "demo.gb",
        "--model",
        "dmg0",
        "--startup",
        "real-boot",
        "--mode",
        "permissive",
        "--boot-rom-dir",
        "boot-assets",
        "--boot-rom-verify",
        "warn",
        "--frames",
        "7",
        "--tcycles",
        "11",
        "--serial-stdout",
        "--serial-out",
        "serial.bin",
        "--framebuffer-out",
        "framebuffer.png",
        "--trace-out",
        "trace.txt",
        "--state-in",
        "input.gbstate",
        "--state-out",
        "output.gbstate",
        "--save-dir",
        "saves",
        "--save-key",
        "demo_save",
        "--save-policy",
        "manual",
    ])
    .expect("run arguments should parse");

    match action {
        CliAction::Run(options) => {
            assert_eq!(options.rom_path, PathBuf::from("demo.gb"));
            assert_eq!(options.model, RunModel::Dmg0);
            assert_eq!(options.startup_mode, StartupMode::RealBoot);
            assert_eq!(options.execution_mode, ExecutionMode::Permissive);
            assert_eq!(options.boot_rom_dir, Some(PathBuf::from("boot-assets")));
            assert_eq!(options.boot_rom_verify, BootRomVerificationMode::Warn);
            assert_eq!(options.frame_limit, Some(7));
            assert_eq!(options.tcycle_limit, Some(11));
            assert!(options.serial_stdout);
            assert_eq!(options.serial_out, Some(PathBuf::from("serial.bin")));
            assert_eq!(
                options.framebuffer_out,
                Some(PathBuf::from("framebuffer.png"))
            );
            assert_eq!(options.trace_out, Some(PathBuf::from("trace.txt")));
            assert_eq!(options.state_in, Some(PathBuf::from("input.gbstate")));
            assert_eq!(options.state_out, Some(PathBuf::from("output.gbstate")));
            assert_eq!(options.save_dir, Some(PathBuf::from("saves")));
            assert_eq!(options.save_key.as_deref(), Some("demo_save"));
            assert_eq!(options.save_policy, SavePolicy::Manual);
            assert_eq!(options.default_run_budget, None);
        }
        other => panic!("expected run action, got {other:?}"),
    }
}

#[test]
fn parse_run_arguments_rejects_invalid_sequences_and_missing_values() {
    let missing_value_cases = [
        (vec!["demo.gb", "--model"], "--model requires a value"),
        (vec!["demo.gb", "--startup"], "--startup requires a value"),
        (vec!["demo.gb", "--mode"], "--mode requires a value"),
        (
            vec!["demo.gb", "--boot-rom-dir"],
            "--boot-rom-dir requires a value",
        ),
        (
            vec!["demo.gb", "--boot-rom-verify"],
            "--boot-rom-verify requires a value",
        ),
        (vec!["demo.gb", "--frames"], "--frames requires a value"),
        (vec!["demo.gb", "--tcycles"], "--tcycles requires a value"),
        (
            vec!["demo.gb", "--serial-out"],
            "--serial-out requires a value",
        ),
        (
            vec!["demo.gb", "--framebuffer-out"],
            "--framebuffer-out requires a value",
        ),
        (
            vec!["demo.gb", "--trace-out"],
            "--trace-out requires a value",
        ),
        (vec!["demo.gb", "--state-in"], "--state-in requires a value"),
        (
            vec!["demo.gb", "--state-out"],
            "--state-out requires a value",
        ),
        (vec!["demo.gb", "--save-dir"], "--save-dir requires a value"),
        (vec!["demo.gb", "--save-key"], "--save-key requires a value"),
        (
            vec!["demo.gb", "--save-policy"],
            "--save-policy requires a value",
        ),
    ];

    for (arguments, expected) in missing_value_cases {
        assert_eq!(
            parse_run_arguments(arguments).expect_err("missing values should fail"),
            expected
        );
    }

    assert_eq!(
        parse_run_arguments(["--model", "dmg"]).expect_err("ROM path must come first"),
        "the ROM path must be the first positional argument to `gb-cli run`"
    );
    assert_eq!(
        parse_run_arguments(std::iter::empty::<&str>()).expect_err("run requires a ROM path"),
        "missing required ROM path; run `gb-cli run --help` for usage"
    );
    assert_eq!(
        parse_run_arguments(["demo.gb", "--save-key", "demo"])
            .expect_err("save key requires save dir"),
        "--save-key requires --save-dir"
    );
    assert_eq!(
        parse_run_arguments(["demo.gb", "--save-policy", "manual"])
            .expect_err("save policy requires save dir"),
        "--save-policy requires --save-dir"
    );
    assert_eq!(
        parse_run_arguments(["demo.gb", "--mystery"]).expect_err("unknown run options should fail"),
        "unknown run option \"--mystery\"; run `gb-cli run --help`"
    );
    assert_eq!(
        parse_run_arguments(["demo.gb", "other.gb"])
            .expect_err("extra positional arguments should fail"),
        "unexpected extra positional argument \"other.gb\"; run `gb-cli run --help`"
    );
}

#[test]
fn parse_inspect_rom_arguments_cover_valid_help_and_error_paths() {
    match parse_inspect_rom_arguments(["demo.gb", "--mode", "experimental"])
        .expect("inspect arguments should parse")
    {
        CliAction::InspectRom(options) => {
            assert_eq!(options.rom_path, PathBuf::from("demo.gb"));
            assert_eq!(options.execution_mode, ExecutionMode::Experimental);
        }
        other => panic!("expected inspect action, got {other:?}"),
    }

    assert_eq!(
        parse_inspect_rom_arguments(["--help"]).expect("help should parse"),
        CliAction::ShowInspectHelp
    );
    assert_eq!(
        parse_inspect_rom_arguments(["demo.gb", "--mode"])
            .expect_err("mode should require a value"),
        "--mode requires a value"
    );
    assert_eq!(
        parse_inspect_rom_arguments(["demo.gb", "--weird"])
            .expect_err("unknown inspect options should fail"),
        "unknown inspect-rom option \"--weird\"; run `gb-cli inspect-rom --help`"
    );
    assert_eq!(
        parse_inspect_rom_arguments(["demo.gb", "other.gb"])
            .expect_err("extra positional arguments should fail"),
        "unexpected extra positional argument \"other.gb\"; run `gb-cli inspect-rom --help`"
    );
    assert_eq!(
        parse_inspect_rom_arguments(std::iter::empty::<&str>())
            .expect_err("inspect requires a ROM path"),
        "missing required ROM path; run `gb-cli inspect-rom --help` for usage"
    );
}

#[test]
fn parse_saves_arguments_cover_valid_help_and_error_paths() {
    match parse_saves_arguments([
        "export",
        "demo.gb",
        "demo.sav",
        "--save-dir",
        "saves",
        "--save-key",
        "slot1",
    ])
    .expect("saves export arguments should parse")
    {
        CliAction::Saves(options) => {
            assert_eq!(options.direction, SavesDirection::Export);
            assert_eq!(options.rom_path, PathBuf::from("demo.gb"));
            assert_eq!(options.external_save_path, PathBuf::from("demo.sav"));
            assert_eq!(options.save_dir, PathBuf::from("saves"));
            assert_eq!(options.save_key.as_deref(), Some("slot1"));
        }
        other => panic!("expected saves action, got {other:?}"),
    }

    match parse_saves_arguments(["import", "demo.gb", "demo.sav", "--save-dir", "saves"])
        .expect("saves import arguments should parse")
    {
        CliAction::Saves(options) => {
            assert_eq!(options.direction, SavesDirection::Import);
            assert_eq!(options.save_key, None);
        }
        other => panic!("expected saves action, got {other:?}"),
    }

    assert_eq!(
        parse_saves_arguments(["--help"]).expect("help should parse"),
        CliAction::ShowSavesHelp
    );
    assert_eq!(
        parse_saves_arguments(std::iter::empty::<&str>()).expect_err("missing action should fail"),
        "missing saves action; run `gb-cli saves --help` for usage"
    );
    assert_eq!(
        parse_saves_arguments(["copy", "demo.gb", "demo.sav", "--save-dir", "saves"])
            .expect_err("unknown action should fail"),
        "unknown saves action \"copy\"; expected export or import"
    );
    assert_eq!(
        parse_saves_arguments(["export", "demo.gb", "demo.sav"])
            .expect_err("save dir should be required"),
        "--save-dir is required"
    );
    assert_eq!(
        parse_saves_arguments(["export", "demo.gb", "demo.sav", "--save-dir"])
            .expect_err("save dir value should be required"),
        "--save-dir requires a value"
    );
    assert_eq!(
        parse_saves_arguments([
            "export",
            "demo.gb",
            "demo.sav",
            "--save-dir",
            "saves",
            "--save-key"
        ])
        .expect_err("save key value should be required"),
        "--save-key requires a value"
    );
    assert_eq!(
        parse_saves_arguments(["export", "demo.gb", "--save-dir", "saves"])
            .expect_err("both positional paths should be required"),
        "missing required ROM path or .sav path; run `gb-cli saves --help` for usage"
    );
    assert_eq!(
        parse_saves_arguments(["export", "demo.gb", "demo.sav", "--weird"])
            .expect_err("unknown option should fail"),
        "unknown saves option \"--weird\"; run `gb-cli saves --help`"
    );
    assert_eq!(
        parse_saves_arguments(["export", "demo.gb", "demo.sav", "extra"])
            .expect_err("extra positional should fail"),
        "unexpected extra positional argument \"extra\"; run `gb-cli saves --help`"
    );
}

#[test]
fn cli_machine_exposes_summary_and_buffered_views() {
    let mut summary = build_loaded_machine(build_single_byte_serial_rom(b'S'), false);
    assert!(summary.at_frame_origin());
    assert!(!summary.is_boot_rom_mapped());
    assert_eq!(
        summary.framebuffer().len(),
        FRAMEBUFFER_WIDTH * FRAMEBUFFER_HEIGHT
    );
    assert_eq!(
        summary.cartridge().persistent_state(),
        PersistentCartState::None
    );
    assert!(
        summary
            .restore_cartridge_persistent_state(&PersistentCartState::None)
            .is_ok()
    );
    assert!(summary.trace_text().is_none());
    summary.step_t_cycle();
    let _ = summary.take_serial_output_bytes();

    let mut buffered = build_loaded_machine(build_single_byte_serial_rom(b'B'), true);
    buffered.step_t_cycle();
    let trace_text = buffered
        .trace_text()
        .expect("buffered machines should expose trace text");
    assert!(trace_text.contains("t_cycle="));
}

#[test]
fn save_session_helpers_cover_skip_restore_and_noop_flush_paths() {
    let temp_dir = unique_temp_dir("save-session");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let mut summary_machine = build_loaded_machine(build_single_byte_serial_rom(b'N'), false);
    let mut stderr = Vec::new();
    let no_battery = open_save_session(
        Some(&temp_dir),
        &RunOptions::default_with_rom(PathBuf::from("nobattery.gb")),
        Path::new("nobattery.gb"),
        &mut summary_machine,
        &mut stderr,
        true,
    )
    .expect("non-battery cartridges should skip save sessions");
    assert!(no_battery.is_none());
    assert!(
        String::from_utf8(stderr)
            .expect("stderr should be UTF-8")
            .contains("save=skipped not_battery_backed=true")
    );

    let save_root = temp_dir.join("battery");
    fs::create_dir_all(&save_root).expect("save root should be creatable");
    let mut battery_machine =
        build_loaded_machine(build_battery_backed_serial_and_ram_rom(b'R', 0), false);
    let save_key = derive_save_key(Path::new("battery.gb")).expect("save key should derive");
    let seeded_state = PersistentCartState::NoMbcRam {
        ram: vec![0x33; 8 * 1024],
    };
    let mut backend = FilesystemCartridgeSaveBackend::new(&save_root);
    backend
        .save(
            &save_key,
            battery_machine.cartridge().persistence_metadata(),
            &seeded_state,
        )
        .expect("seed save should persist");

    let mut stderr = Vec::new();
    let mut session = open_save_session(
        Some(&save_root),
        &RunOptions::default_with_rom(PathBuf::from("battery.gb")),
        Path::new("battery.gb"),
        &mut battery_machine,
        &mut stderr,
        true,
    )
    .expect("save session should open")
    .expect("battery-backed cartridges should open a save session");
    assert!(session.loaded_existing_save);
    assert_eq!(session.last_saved_state, seeded_state);
    assert_eq!(battery_machine.cartridge().persistent_state(), seeded_state);
    assert!(
        !flush_save_if_changed(&mut session, &battery_machine, "no-change")
            .expect("unchanged state should not be re-saved")
    );
    assert!(
        String::from_utf8(stderr)
            .expect("stderr should be UTF-8")
            .contains("save_loaded path=")
    );

    let mut failing_stderr = FailOnWrite {
        fail_on_write: Some(1),
        ..FailOnWrite::default()
    };
    let mut failing_machine =
        build_loaded_machine(build_battery_backed_serial_and_ram_rom(b'R', 0), false);
    let save_loaded_error = open_save_session(
        Some(&save_root),
        &RunOptions::default_with_rom(PathBuf::from("battery.gb")),
        Path::new("battery.gb"),
        &mut failing_machine,
        &mut failing_stderr,
        true,
    )
    .expect_err("save-loaded status write failures should surface");
    assert!(save_loaded_error.contains("failed to write output"));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn save_session_and_flush_error_paths_surface_backend_failures() {
    let mut options = None;
    let rom_path = Some(PathBuf::from("demo.gb"));
    ensure_run_options_initialized(&mut options, &rom_path)
        .expect("existing ROM paths should initialize default options");
    assert_eq!(
        options,
        Some(RunOptions::default_with_rom(PathBuf::from("demo.gb")))
    );

    let temp_dir = unique_temp_dir("save-errors");
    let save_root = temp_dir.join("saves");
    fs::create_dir_all(&save_root).expect("save root should be creatable");

    let mut battery_machine =
        build_loaded_machine(build_battery_backed_serial_and_ram_rom(b'R', 0), false);
    let mut options = RunOptions::default_with_rom(PathBuf::from("battery.gb"));
    options.save_key = Some("battery_manual".to_string());
    let key = CartridgeSaveKey::new("battery_manual").expect("save key should be valid");
    let backend = FilesystemCartridgeSaveBackend::new(&save_root);
    fs::write(backend.path_for_key(&key), b"not-a-valid-save")
        .expect("broken save bytes should be writable");
    let load_error = open_save_session(
        Some(&save_root),
        &options,
        Path::new("battery.gb"),
        &mut battery_machine,
        &mut Vec::new(),
        true,
    )
    .expect_err("broken save files should surface backend load errors");
    assert!(load_error.contains("failed to load save"));

    let blocking_root = temp_dir.join("blocking-root");
    fs::write(&blocking_root, b"file").expect("blocking file should be writable");
    let mut failing_session = SaveSession {
        backend: FilesystemCartridgeSaveBackend::new(&blocking_root),
        key: CartridgeSaveKey::new("battery").expect("save key should be valid"),
        last_saved_state: PersistentCartState::None,
        loaded_existing_save: false,
        save_writes: 0,
    };
    let save_error = flush_save_if_changed(&mut failing_session, &battery_machine, "forced-save")
        .expect_err("broken save roots should surface backend save errors");
    assert!(save_error.contains("failed to save cartridge persistence (forced-save)"));

    let mut mismatch_options = RunOptions::default_with_rom(PathBuf::from("battery.gb"));
    mismatch_options.save_key = Some("battery_mismatch".to_string());
    let mismatch_key = CartridgeSaveKey::new("battery_mismatch").expect("save key should be valid");
    let mut backend = FilesystemCartridgeSaveBackend::new(&save_root);
    backend
        .save(
            &mismatch_key,
            battery_machine.cartridge().persistence_metadata(),
            &PersistentCartState::Mbc2Ram {
                ram_nibbles: [0; 512],
            },
        )
        .expect("mismatched save should persist for restore checks");
    let mut mismatch_machine =
        build_loaded_machine(build_battery_backed_serial_and_ram_rom(b'R', 0), false);
    let restore_error = open_save_session(
        Some(&save_root),
        &mismatch_options,
        Path::new("battery.gb"),
        &mut mismatch_machine,
        &mut Vec::new(),
        true,
    )
    .expect_err("incompatible saved state should surface restore errors");
    assert!(restore_error.contains("failed to restore cartridge persistence"));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn run_command_streams_serial_stdout_in_summary_mode_and_reports_missing_roms() {
    let temp_dir = unique_temp_dir("summary-run");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path = temp_dir.join("serial.gb");
    let framebuffer_path = temp_dir.join("summary/framebuffer.pgm");
    fs::write(&rom_path, build_single_byte_serial_rom(b'Z')).expect("test ROM should be writable");

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut options = RunOptions::default_with_rom(rom_path.clone());
    options.serial_stdout = true;
    options.framebuffer_out = Some(framebuffer_path.clone());
    options.tcycle_limit = Some(10_000);
    run_command(options, &mut stdout, &mut stderr).expect("run command should succeed");

    assert_eq!(stdout, b"Z");
    assert!(
        fs::read(&framebuffer_path)
            .expect("framebuffer should exist")
            .starts_with(b"P5\n160 144\n3\n")
    );
    assert!(
        String::from_utf8(stderr)
            .expect("stderr should be UTF-8")
            .contains("serial_bytes=1")
    );

    let missing = run_command(
        RunOptions::default_with_rom(PathBuf::from("this-rom-does-not-exist.gb")),
        &mut Vec::new(),
        &mut Vec::new(),
    )
    .expect_err("missing ROMs should fail");
    assert!(missing.contains("failed to read ROM"));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn run_command_covers_on_write_frame_flush_and_manual_save_policy() {
    let temp_dir = unique_temp_dir("run-save-policies");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path = temp_dir.join("battery.gb");
    fs::write(
        &rom_path,
        build_battery_backed_serial_and_ram_rom(b'W', 0x5A),
    )
    .expect("battery-backed ROM should be writable");

    let on_write_root = temp_dir.join("on-write");
    let mut on_write = RunOptions::default_with_rom(rom_path.clone());
    on_write.save_dir = Some(on_write_root.clone());
    on_write.save_policy = SavePolicy::OnWrite;
    on_write.frame_limit = Some(1);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    run_command(on_write, &mut stdout, &mut stderr).expect("on-write runs should succeed");
    assert!(stdout.is_empty());
    let stderr_text = String::from_utf8(stderr).expect("stderr should be UTF-8");
    assert!(stderr_text.contains("completed_frames=1"));
    assert!(stderr_text.contains("save_writes=1"));

    let manual_root = temp_dir.join("manual");
    let mut manual = RunOptions::default_with_rom(rom_path);
    manual.save_dir = Some(manual_root);
    manual.save_policy = SavePolicy::Manual;
    manual.frame_limit = Some(1);
    manual.framebuffer_out = Some(temp_dir.join("manual/framebuffer.pgm"));
    manual.trace_out = Some(temp_dir.join("manual/trace.txt"));
    let mut stderr = Vec::new();
    run_command(manual, &mut Vec::new(), &mut stderr).expect("manual runs should succeed");
    let stderr_text = String::from_utf8(stderr).expect("stderr should be UTF-8");
    assert!(stderr_text.contains("save_policy=manual"));
    assert!(stderr_text.contains("save_writes=0"));
    assert!(stderr_text.contains("framebuffer_out="));
    assert!(stderr_text.contains("trace_out="));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn run_command_surfaces_summary_save_and_session_writer_failures() {
    let temp_dir = unique_temp_dir("run-writer-failures");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let plain_rom_path = temp_dir.join("plain.gb");
    fs::write(&plain_rom_path, build_single_byte_serial_rom(b'P'))
        .expect("plain ROM should be writable");

    let mut serial_write_options = RunOptions::default_with_rom(plain_rom_path.clone());
    serial_write_options.serial_stdout = true;
    serial_write_options.tcycle_limit = Some(10_000);
    let mut stdout = FailOnWrite {
        fail_on_write: Some(1),
        ..FailOnWrite::default()
    };
    let serial_write_error = run_command(serial_write_options, &mut stdout, &mut Vec::new())
        .expect_err("serial stdout write failures should surface");
    assert!(serial_write_error.contains("failed to write serial stdout"));

    let mut serial_flush_options = RunOptions::default_with_rom(plain_rom_path.clone());
    serial_flush_options.serial_stdout = true;
    serial_flush_options.tcycle_limit = Some(10_000);
    let mut stdout = FailOnWrite {
        fail_on_flush: true,
        ..FailOnWrite::default()
    };
    let serial_flush_error = run_command(serial_flush_options, &mut stdout, &mut Vec::new())
        .expect_err("serial stdout flush failures should surface");
    assert!(serial_flush_error.contains("failed to flush serial stdout"));

    for fail_on_write in [5, 7] {
        let mut options = RunOptions::default_with_rom(plain_rom_path.clone());
        options.tcycle_limit = Some(0);
        let mut stderr = FailOnWrite {
            fail_on_write: Some(fail_on_write),
            ..FailOnWrite::default()
        };
        let error = run_command(options, &mut Vec::new(), &mut stderr)
            .expect_err("summary write failures should surface");
        assert!(error.contains("failed to write output"));
    }

    let mut framebuffer_options = RunOptions::default_with_rom(plain_rom_path.clone());
    framebuffer_options.tcycle_limit = Some(0);
    framebuffer_options.framebuffer_out = Some(temp_dir.join("framebuffer/frame.pgm"));
    let mut stderr = FailOnWrite {
        fail_on_write: Some(15),
        ..FailOnWrite::default()
    };
    let framebuffer_error = run_command(framebuffer_options, &mut Vec::new(), &mut stderr)
        .expect_err("framebuffer summary write failures should surface");
    assert!(framebuffer_error.contains("failed to write output"));

    let mut skipped_save_options = RunOptions::default_with_rom(plain_rom_path);
    skipped_save_options.tcycle_limit = Some(0);
    skipped_save_options.save_dir = Some(temp_dir.join("plain-saves"));
    let mut stderr = FailOnWrite {
        fail_on_write: Some(1),
        ..FailOnWrite::default()
    };
    let skipped_save_error = run_command(skipped_save_options, &mut Vec::new(), &mut stderr)
        .expect_err("save-session status write failures should surface");
    assert!(skipped_save_error.contains("failed to write output"));

    let battery_rom_path = temp_dir.join("battery.gb");
    fs::write(
        &battery_rom_path,
        build_battery_backed_serial_and_ram_rom(b'B', 0x11),
    )
    .expect("battery ROM should be writable");
    for fail_on_write in [17, 19, 21, 22] {
        let mut options = RunOptions::default_with_rom(battery_rom_path.clone());
        options.tcycle_limit = Some(0);
        options.save_dir = Some(temp_dir.join(format!("battery-saves-{fail_on_write}")));
        let mut stderr = FailOnWrite {
            fail_on_write: Some(fail_on_write),
            ..FailOnWrite::default()
        };
        let error = run_command(options, &mut Vec::new(), &mut stderr)
            .expect_err("save summary write failures should surface");
        assert!(error.contains("failed to write output"));
    }

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn inspect_rom_command_covers_rejected_and_header_error_paths() {
    let temp_dir = unique_temp_dir("inspect-rejected");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path = temp_dir.join("unsupported.gb");
    let mut rom = build_test_rom_with_header(&[0x00], 0x20, 0x55, 0x06);
    rom[0x0143] = 0xAA;
    rom[0x0146] = 0x7F;
    fs::write(&rom_path, rom).expect("unsupported ROM should be writable");

    let mut output = Vec::new();
    inspect_rom_command(
        InspectRomOptions {
            rom_path: rom_path.clone(),
            execution_mode: ExecutionMode::Strict,
        },
        &mut output,
    )
    .expect("unsupported headers should still inspect successfully");
    let text = String::from_utf8(output).expect("inspect output should be UTF-8");
    assert!(text.contains("load_status=rejected"));
    assert!(text.contains("selection=unsupported-documented"));
    assert!(text.contains("rejection_reason="));
    assert!(text.contains("cgb_flag=supported-noncanonical(0xAA)"));
    assert!(text.contains("sgb_flag=unknown(0x7F)"));
    assert!(text.contains("rom_size_bytes=unknown"));
    assert!(text.contains("ram_size_bytes=unknown"));

    let tiny_path = temp_dir.join("tiny.gb");
    fs::write(&tiny_path, [0x00; 4]).expect("tiny ROM should be writable");
    let error = inspect_rom_command(
        InspectRomOptions {
            rom_path: tiny_path,
            execution_mode: ExecutionMode::Strict,
        },
        &mut Vec::new(),
    )
    .expect_err("tiny ROMs should fail header parsing");
    assert!(error.contains("ROM image is too small to contain a cartridge header"));

    let missing_error = inspect_rom_command(
        InspectRomOptions {
            rom_path: temp_dir.join("missing.gb"),
            execution_mode: ExecutionMode::Strict,
        },
        &mut Vec::new(),
    )
    .expect_err("missing ROMs should surface read errors");
    assert!(missing_error.contains("failed to read ROM"));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn inspect_rom_command_surfaces_output_writer_failures() {
    let temp_dir = unique_temp_dir("inspect-writer-failures");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path = temp_dir.join("inspect.gb");
    fs::write(&rom_path, build_single_byte_serial_rom(b'I')).expect("test ROM should be writable");

    for fail_on_write in [5, 9, 11, 13, 15, 17, 19, 21, 23, 25, 27, 29, 31] {
        let mut output = FailOnWrite {
            fail_on_write: Some(fail_on_write),
            ..FailOnWrite::default()
        };
        let error = inspect_rom_command(
            InspectRomOptions {
                rom_path: rom_path.clone(),
                execution_mode: ExecutionMode::Strict,
            },
            &mut output,
        )
        .expect_err("inspect output write failures should surface");
        assert!(error.contains("failed to write output"));
    }

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn boot_rom_path_resolution_and_verification_helpers_cover_host_side_paths() {
    let temp_dir = unique_temp_dir("boot-rom");
    let current_dir = temp_dir.join("cwd");
    let explicit_dir = temp_dir.join("explicit");
    let missing_dir = temp_dir.join("missing");
    let not_dir = temp_dir.join("not-a-dir");
    let short_dir = temp_dir.join("short");
    fs::create_dir_all(&current_dir).expect("cwd should be creatable");
    fs::create_dir_all(&explicit_dir).expect("explicit dir should be creatable");
    fs::create_dir_all(&short_dir).expect("short dir should be creatable");
    fs::write(&not_dir, b"file").expect("blocking file should be writable");
    write_fake_boot_rom(&explicit_dir, BootRomKind::Dmg, 0xAA);
    fs::write(
        short_dir.join(BootRomAssets::filename(BootRomKind::Dmg)),
        vec![0x00; 0x10],
    )
    .expect("short boot ROM image should be writable");

    let mut options = RunOptions::default_with_rom(PathBuf::from("demo.gb"));
    let mut stderr = Vec::new();
    let skip_boot = load_boot_rom_assets(&options, &current_dir, &mut stderr)
        .expect("skip-boot should not require assets");
    assert!(skip_boot.is_empty());
    assert!(stderr.is_empty());

    options.startup_mode = StartupMode::RealBoot;
    options.boot_rom_dir = Some(explicit_dir.clone());
    options.boot_rom_verify = BootRomVerificationMode::Warn;
    let warned_assets = load_boot_rom_assets(&options, &current_dir, &mut stderr)
        .expect("warn mode should still load assets");
    assert!(warned_assets.has_image(BootRomKind::Dmg));
    assert!(
        String::from_utf8(stderr.clone())
            .expect("stderr should be UTF-8")
            .contains("warning: boot ROM asset")
    );
    let mut failing_stderr = FailOnWrite {
        fail_on_write: Some(1),
        ..FailOnWrite::default()
    };
    let warning_write_error = load_boot_rom_assets(&options, &current_dir, &mut failing_stderr)
        .expect_err("warning write failures should surface");
    assert!(warning_write_error.contains("failed to write output"));

    stderr.clear();
    options.boot_rom_verify = BootRomVerificationMode::Off;
    let unchecked_assets = load_boot_rom_assets(&options, &current_dir, &mut stderr)
        .expect("off mode should skip verification");
    assert!(unchecked_assets.has_image(BootRomKind::Dmg));
    assert!(stderr.is_empty());

    options.boot_rom_verify = BootRomVerificationMode::Strict;
    let strict_error = load_boot_rom_assets(&options, &current_dir, &mut Vec::new())
        .expect_err("strict verification should reject mismatched assets");
    assert!(strict_error.contains("unexpected sha256"));

    options.boot_rom_verify = BootRomVerificationMode::Off;
    options.boot_rom_dir = Some(missing_dir.clone());
    let missing_assets = load_boot_rom_assets(&options, &current_dir, &mut Vec::new())
        .expect("missing directories should resolve to no assets");
    assert!(missing_assets.is_empty());

    options.boot_rom_dir = Some(not_dir.clone());
    let directory_error = load_boot_rom_assets(&options, &current_dir, &mut Vec::new())
        .expect_err("file paths should fail directory validation");
    assert!(directory_error.contains("--boot-rom-dir expects a directory path"));

    options.boot_rom_dir = Some(short_dir.clone());
    let short_image_error = load_boot_rom_assets(&options, &current_dir, &mut Vec::new())
        .expect_err("short boot ROM images should fail directory loading");
    assert!(short_image_error.contains("failed to load boot ROM assets from"));
    assert!(short_image_error.contains("is too short"));

    assert_eq!(
        resolve_boot_rom_root(Some(Path::new("custom-assets")), &current_dir),
        Some(current_dir.join("custom-assets"))
    );
    {
        let _guard = ENV_LOCK.lock().expect("env lock should not be poisoned");
        let previous_boot_rom_root = env::var_os(DEFAULT_BOOT_ROM_ROOT_ENV_VAR);
        // SAFETY: tests guard process-wide environment mutations with a mutex.
        unsafe {
            env::set_var(DEFAULT_BOOT_ROM_ROOT_ENV_VAR, &explicit_dir);
        }
        assert_eq!(
            resolve_boot_rom_root(None, &current_dir),
            Some(explicit_dir.clone())
        );
        // SAFETY: tests guard process-wide environment mutations with a mutex.
        unsafe {
            env::remove_var(DEFAULT_BOOT_ROM_ROOT_ENV_VAR);
        }
        assert_eq!(resolve_boot_rom_root(None, &current_dir), None);

        options.boot_rom_dir = None;
        options.boot_rom_verify = BootRomVerificationMode::Strict;
        let unconfigured_error = load_boot_rom_assets(&options, &current_dir, &mut Vec::new())
            .expect_err("strict real-boot should reject missing boot ROM configuration");
        assert!(unconfigured_error.contains("boot ROM root is not configured"));

        options.boot_rom_verify = BootRomVerificationMode::Off;
        let unconfigured_assets = load_boot_rom_assets(&options, &current_dir, &mut Vec::new())
            .expect("verification-off should allow unconfigured boot ROM roots");
        assert!(unconfigured_assets.is_empty());

        // SAFETY: tests guard process-wide environment mutations with a mutex.
        unsafe {
            match previous_boot_rom_root {
                Some(value) => env::set_var(DEFAULT_BOOT_ROM_ROOT_ENV_VAR, value),
                None => env::remove_var(DEFAULT_BOOT_ROM_ROOT_ENV_VAR),
            }
        }
    }

    assert_eq!(
        resolve_path(&current_dir, Path::new("relative/demo.gb")),
        current_dir.join("relative/demo.gb")
    );
    assert_eq!(
        resolve_path(&current_dir, Path::new("/tmp/demo.gb")),
        PathBuf::from("/tmp/demo.gb")
    );
    validate_explicit_directory_input("--boot-rom-dir", None, &explicit_dir)
        .expect("missing explicit paths should be ignored");

    let missing_verify = verify_boot_rom_file(&temp_dir.join("missing.bin"), BootRomKind::Dmg)
        .expect_err("missing boot ROM files should fail");
    assert!(missing_verify.contains("failed to read boot ROM asset"));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn helper_parsers_names_and_formatters_cover_supported_variants() {
    assert_eq!(RunModel::GameBoy.console_model(), ConsoleModel::GameBoy);
    assert_eq!(RunModel::GameBoy.name(), "game-boy");
    assert_eq!(RunModel::Dmg0.console_model(), ConsoleModel::GameBoy);
    assert_eq!(RunModel::Dmg.boot_rom_kind(), BootRomKind::Dmg);
    assert_eq!(RunModel::Mgb.boot_rom_kind(), BootRomKind::Mgb);
    assert_eq!(RunModel::Mgb.name(), "mgb");
    assert_eq!(RunModel::GameBoyLight.boot_rom_kind(), BootRomKind::Mgb);
    assert_eq!(RunModel::GameBoyColor.boot_rom_kind(), BootRomKind::Cgb);
    assert_eq!(SavePolicy::Manual.name(), "manual");
    assert_eq!(SavePolicy::OnClose.name(), "on-close");
    assert_eq!(SavePolicy::OnWrite.name(), "on-write");
    assert_eq!(
        DefaultRunBudget::for_startup_mode(StartupMode::SkipBoot),
        DefaultRunBudget::SkipBootFrames {
            frame_limit: DEFAULT_SKIP_BOOT_FRAME_LIMIT,
        }
    );
    assert_eq!(
        DefaultRunBudget::for_startup_mode(StartupMode::RealBoot),
        DefaultRunBudget::RealBootPostHandoff {
            post_handoff_frame_limit: DEFAULT_REAL_BOOT_POST_HANDOFF_FRAME_LIMIT,
            safety_frame_limit: DEFAULT_REAL_BOOT_SAFETY_FRAME_LIMIT,
        }
    );

    assert_eq!(
        compatibility_for_execution_mode(ExecutionMode::Strict),
        CompatibilityPolicy::strict()
    );
    assert_eq!(
        compatibility_for_execution_mode(ExecutionMode::Permissive),
        CompatibilityPolicy::permissive()
    );
    assert_eq!(
        compatibility_for_execution_mode(ExecutionMode::Experimental),
        CompatibilityPolicy::experimental()
    );

    assert_eq!(parse_run_model("game-boy"), Ok(RunModel::GameBoy));
    assert_eq!(parse_run_model("pocket"), Ok(RunModel::GameBoyPocket));
    assert_eq!(parse_run_model("light"), Ok(RunModel::GameBoyLight));
    assert_eq!(parse_run_model("color"), Ok(RunModel::GameBoyColor));
    assert_eq!(parse_run_model("dmg0"), Ok(RunModel::Dmg0));
    assert_eq!(parse_run_model("dmg"), Ok(RunModel::Dmg));
    assert_eq!(parse_run_model("mgb"), Ok(RunModel::Mgb));
    assert_eq!(parse_run_model("cgb"), Ok(RunModel::Cgb));
    assert!(
        parse_run_model("sgb")
            .expect_err("unsupported models should fail")
            .contains("unsupported --model value")
    );

    assert_eq!(parse_startup_mode("skip-boot"), Ok(StartupMode::SkipBoot));
    assert_eq!(parse_startup_mode("real-boot"), Ok(StartupMode::RealBoot));
    assert!(
        parse_startup_mode("boot")
            .expect_err("unsupported startup modes should fail")
            .contains("unsupported --startup value")
    );

    assert_eq!(parse_execution_mode("strict"), Ok(ExecutionMode::Strict));
    assert_eq!(
        parse_execution_mode("permissive"),
        Ok(ExecutionMode::Permissive)
    );
    assert_eq!(
        parse_execution_mode("experimental"),
        Ok(ExecutionMode::Experimental)
    );
    assert!(
        parse_execution_mode("oracle")
            .expect_err("unsupported execution modes should fail")
            .contains("unsupported --mode value")
    );

    assert_eq!(
        parse_boot_rom_verification_mode("off"),
        Ok(BootRomVerificationMode::Off)
    );
    assert_eq!(
        parse_boot_rom_verification_mode("warn"),
        Ok(BootRomVerificationMode::Warn)
    );
    assert_eq!(
        parse_boot_rom_verification_mode("strict"),
        Ok(BootRomVerificationMode::Strict)
    );
    assert!(
        parse_boot_rom_verification_mode("auto")
            .expect_err("unsupported verification modes should fail")
            .contains("unsupported --boot-rom-verify value")
    );

    assert_eq!(parse_save_policy("manual"), Ok(SavePolicy::Manual));
    assert_eq!(parse_save_policy("on-close"), Ok(SavePolicy::OnClose));
    assert_eq!(parse_save_policy("on-write"), Ok(SavePolicy::OnWrite));
    assert!(
        parse_save_policy("always")
            .expect_err("unsupported save policies should fail")
            .contains("unsupported --save-policy value")
    );

    assert_eq!(parse_positive_u32("--frames", "5"), Ok(5));
    assert_eq!(
        parse_positive_u32("--frames", "0"),
        Err("--frames must be greater than zero".to_string())
    );
    assert!(
        parse_positive_u32("--frames", "abc")
            .expect_err("invalid u32 values should fail")
            .contains("invalid --frames value")
    );

    assert_eq!(parse_positive_u64("--tcycles", "9"), Ok(9));
    assert_eq!(
        parse_positive_u64("--tcycles", "0"),
        Err("--tcycles must be greater than zero".to_string())
    );
    assert!(
        parse_positive_u64("--tcycles", "abc")
            .expect_err("invalid u64 values should fail")
            .contains("invalid --tcycles value")
    );

    assert!(run_limit_reached(Some(2), None, 2, 0));
    assert!(run_limit_reached(None, Some(3), 0, 3));
    assert!(!run_limit_reached(None, None, 0, 0));

    assert_eq!(startup_mode_name(StartupMode::RealBoot), "real-boot");
    assert_eq!(execution_mode_name(ExecutionMode::Strict), "strict");
    assert_eq!(execution_mode_name(ExecutionMode::Permissive), "permissive");
    assert_eq!(
        execution_mode_name(ExecutionMode::Experimental),
        "experimental"
    );
    assert_eq!(
        diagnostic_severity_name(CartridgeDiagnosticSeverity::Warning),
        "warning"
    );
    assert_eq!(
        diagnostic_severity_name(CartridgeDiagnosticSeverity::Error),
        "error"
    );
    assert_eq!(
        selection_name(CartridgeSelection::Supported(
            SupportedCartridgeFamily::NoMbc
        )),
        "supported"
    );
    assert_eq!(
        selection_name(CartridgeSelection::Unsupported(
            UnsupportedCartridgeCategory::PlannedVariant
        )),
        "unsupported-planned-variant"
    );
    assert_eq!(
        selection_name(CartridgeSelection::Unsupported(
            UnsupportedCartridgeCategory::DocumentedButUnsupported
        )),
        "unsupported-documented"
    );
    assert_eq!(
        selection_name(CartridgeSelection::Unsupported(
            UnsupportedCartridgeCategory::ExperimentalHeuristic
        )),
        "unsupported-experimental-heuristic"
    );
    assert_eq!(
        selection_name(CartridgeSelection::Unsupported(
            UnsupportedCartridgeCategory::AccessorySpecialCase
        )),
        "unsupported-accessory"
    );
    assert_eq!(
        selection_name(CartridgeSelection::Unsupported(
            UnsupportedCartridgeCategory::UnknownCode
        )),
        "unsupported-unknown"
    );
    assert_eq!(cgb_flag_name(CgbFlag::None), "none");
    assert_eq!(cgb_flag_name(CgbFlag::Supported), "supported");
    assert_eq!(cgb_flag_name(CgbFlag::Only), "only");
    assert_eq!(
        cgb_flag_name(CgbFlag::SupportedNonCanonical(0xA0)),
        "supported-noncanonical(0xA0)"
    );
    assert_eq!(cgb_flag_name(CgbFlag::Unknown(0x42)), "unknown(0x42)");
    assert_eq!(sgb_flag_name(SgbFlag::None), "none");
    assert_eq!(sgb_flag_name(SgbFlag::Supported), "supported");
    assert_eq!(sgb_flag_name(SgbFlag::Unknown(0x03)), "unknown(0x03)");
    assert_eq!(optional_usize_name(Some(8)), "8");
    assert_eq!(optional_usize_name(None), "unknown");
    assert_eq!(expected_boot_rom_sha256(BootRomKind::Dmg0).len(), 64);
    assert_eq!(expected_boot_rom_sha256(BootRomKind::Dmg).len(), 64);
    assert_eq!(expected_boot_rom_sha256(BootRomKind::Mgb).len(), 64);
    assert_eq!(expected_boot_rom_sha256(BootRomKind::Cgb0).len(), 64);
    assert_eq!(expected_boot_rom_sha256(BootRomKind::Cgb).len(), 64);
    assert_eq!(expected_boot_rom_sha256(BootRomKind::CgbE).len(), 64);
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

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
    assert_eq!(legacy_save_key_for_rom_path(None, Path::new("/")), None);
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
    )
    .expect("PNG encoding should succeed");
    assert!(png_artifact.starts_with(b"\x89PNG\r\n\x1A\n"));
    let rgb555_png_artifact = encode_framebuffer_artifact(
        Path::new("framebuffer.png"),
        &vec![3; FRAMEBUFFER_WIDTH * FRAMEBUFFER_HEIGHT],
        Some(&vec![0x7FFF; FRAMEBUFFER_WIDTH * FRAMEBUFFER_HEIGHT]),
    )
    .expect("CGB RGB555 PNG encoding should succeed");
    assert!(rgb555_png_artifact.starts_with(b"\x89PNG\r\n\x1A\n"));
    let direct_png = encode_framebuffer_png(&vec![0; FRAMEBUFFER_WIDTH * FRAMEBUFFER_HEIGHT])
        .expect("direct PNG encoding should succeed");
    assert!(direct_png.starts_with(b"\x89PNG\r\n\x1A\n"));
    let direct_rgb_png =
        encode_rgb555_framebuffer_png(&vec![0x001F; FRAMEBUFFER_WIDTH * FRAMEBUFFER_HEIGHT])
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
        classification: CartridgeClassification::classify(0x20),
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
    assert!(rejected_message.contains("mapper=MBC6"));
    assert!(rejected_message.contains("selection=unsupported-documented"));
    assert!(rejected_message.contains("diagnostics=[warning warn; error err]"));
    let rejected_without_diagnostics = format_cartridge_load_error(CartridgeLoadError::Rejected {
        classification: CartridgeClassification::classify(0x20),
        execution_mode: ExecutionMode::Strict,
        reason: "no diagnostics".to_string(),
        diagnostics: Vec::new(),
    });
    assert!(rejected_without_diagnostics.contains("cartridge rejected under strict"));
    assert!(!rejected_without_diagnostics.contains("diagnostics=["));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}
