use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use gb_core::{BootRomAssets, BootRomKind, PersistentCartState};
use gb_persistence::{
    CartridgeSaveBackend, FilesystemCartridgeSaveBackend, decode_machine_save_state_envelope,
};

const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;

fn unique_temp_dir(label: &str) -> PathBuf {
    env::temp_dir().join(format!(
        "gb-cli-integration-{label}-{}-{}",
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

fn set_header_flags(rom: &mut [u8], cgb_flag: u8, sgb_flag: u8) {
    rom[0x0143] = cgb_flag;
    rom[0x0146] = sgb_flag;
}

fn write_fake_boot_rom(dir: &PathBuf, kind: BootRomKind, fill: u8) {
    fs::create_dir_all(dir).expect("boot ROM dir should be creatable");
    fs::write(dir.join(BootRomAssets::filename(kind)), vec![fill; 0x0100])
        .expect("boot ROM file should be writable");
}

#[test]
fn binary_help_prints_usage_and_exits_successfully() {
    let output = Command::new(env!("CARGO_BIN_EXE_gb-cli"))
        .arg("--help")
        .output()
        .expect("gb-cli binary should run");

    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .expect("stdout should be UTF-8")
            .contains("Usage:\n  gb-cli run <rom> [options]")
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn binary_unknown_subcommands_fail_with_a_formatted_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_gb-cli"))
        .arg("unknown")
        .output()
        .expect("gb-cli binary should run");

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");
    assert!(
        String::from_utf8(output.stderr)
            .expect("stderr should be UTF-8")
            .contains("error: unknown subcommand \"unknown\"; run `gb-cli --help` for usage")
    );
}

#[test]
fn binary_run_executes_a_rom_and_streams_serial_stdout() {
    let temp_dir = unique_temp_dir("run");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path = temp_dir.join("serial.gb");
    fs::write(&rom_path, build_single_byte_serial_rom(b'Z')).expect("test ROM should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_gb-cli"))
        .args([
            "run",
            rom_path.to_str().expect("path should be valid UTF-8"),
            "--tcycles",
            "10000",
            "--serial-stdout",
        ])
        .output()
        .expect("gb-cli binary should run");

    assert!(output.status.success());
    assert_eq!(output.stdout, b"Z");
    assert!(
        String::from_utf8(output.stderr)
            .expect("stderr should be UTF-8")
            .contains("serial_bytes=1")
    );

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn binary_inspect_rom_surfaces_header_parse_failures() {
    let temp_dir = unique_temp_dir("inspect");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path = temp_dir.join("small.gb");
    fs::write(&rom_path, [0x00; 4]).expect("tiny ROM should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_gb-cli"))
        .args([
            "inspect-rom",
            rom_path.to_str().expect("path should be valid UTF-8"),
        ])
        .output()
        .expect("gb-cli binary should run");

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");
    assert!(
        String::from_utf8(output.stderr)
            .expect("stderr should be UTF-8")
            .contains("error: ROM image is too small to contain a cartridge header")
    );

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn binary_run_with_artifacts_and_persistence_covers_headless_paths() {
    let temp_dir = unique_temp_dir("run-artifacts");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path = temp_dir.join("battery.gb");
    let serial_path = temp_dir.join("artifacts/serial.bin");
    let framebuffer_path = temp_dir.join("artifacts/framebuffer.pgm");
    let trace_path = temp_dir.join("artifacts/trace.txt");
    let save_root = temp_dir.join("saves");
    fs::write(
        &rom_path,
        build_battery_backed_serial_and_ram_rom(b'Q', 0x5A),
    )
    .expect("battery-backed ROM should be writable");

    fs::create_dir_all(&save_root).expect("save root should be creatable");
    let save_key =
        gb_persistence::CartridgeSaveKey::new("battery").expect("save key should be valid");
    let mut backend = FilesystemCartridgeSaveBackend::new(&save_root);
    backend
        .save(
            &save_key,
            gb_core::CartridgePersistenceMetadata {
                has_battery: true,
                has_rtc: false,
                profile: gb_core::CartridgePersistenceProfile::PersistentRam {
                    ram: gb_core::CartridgeRamPayloadKind::Linear { byte_len: 8 * 1024 },
                },
            },
            &PersistentCartState::NoMbcRam {
                ram: vec![0x22; 8 * 1024],
            },
        )
        .expect("seed save should persist");

    let output = Command::new(env!("CARGO_BIN_EXE_gb-cli"))
        .args([
            "run",
            rom_path.to_str().expect("path should be valid UTF-8"),
            "--model",
            "pocket",
            "--startup",
            "skip-boot",
            "--mode",
            "experimental",
            "--frames",
            "1",
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
            "--save-policy",
            "on-close",
        ])
        .output()
        .expect("gb-cli binary should run");

    assert!(output.status.success());
    assert_eq!(
        fs::read(&serial_path).expect("serial artifact should exist"),
        b"Q"
    );
    assert!(
        fs::read(&framebuffer_path)
            .expect("framebuffer artifact should exist")
            .starts_with(b"P5\n160 144\n3\n")
    );
    assert!(
        fs::read_to_string(&trace_path)
            .expect("trace artifact should exist")
            .contains("t_cycle=")
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("model=pocket"));
    assert!(stderr.contains("startup=skip-boot"));
    assert!(stderr.contains("mode=experimental"));
    assert!(stderr.contains("save_loaded path="));
    assert!(stderr.contains("save_writes=1"));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn binary_run_saves_and_loads_machine_save_state_artifacts() {
    let temp_dir = unique_temp_dir("state-artifacts");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path = temp_dir.join("state.gb");
    let state_path = temp_dir.join("states/slot1.gbstate");
    let continued_state_path = temp_dir.join("states/slot2.gbstate");
    fs::write(&rom_path, build_nop_loop_rom()).expect("test ROM should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_gb-cli"))
        .args([
            "run",
            rom_path.to_str().expect("path should be valid UTF-8"),
            "--tcycles",
            "64",
            "--state-out",
            state_path.to_str().expect("path should be valid UTF-8"),
        ])
        .output()
        .expect("gb-cli binary should run");

    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .expect("stderr should be UTF-8")
            .contains("state_out=")
    );
    let first_state_bytes = fs::read(&state_path).expect(".gbstate should be written");
    let first_state =
        decode_machine_save_state_envelope(&first_state_bytes).expect(".gbstate should decode");

    let output = Command::new(env!("CARGO_BIN_EXE_gb-cli"))
        .args([
            "run",
            rom_path.to_str().expect("path should be valid UTF-8"),
            "--tcycles",
            "64",
            "--state-in",
            state_path.to_str().expect("path should be valid UTF-8"),
            "--state-out",
            continued_state_path
                .to_str()
                .expect("path should be valid UTF-8"),
        ])
        .output()
        .expect("gb-cli binary should run");

    assert!(output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("state_in="));
    assert!(stderr.contains("state_out="));
    let continued_state_bytes =
        fs::read(&continued_state_path).expect("continued .gbstate should be written");
    let continued_state = decode_machine_save_state_envelope(&continued_state_bytes)
        .expect("continued .gbstate should decode");
    assert!(
        continued_state.state.metadata().next_t_cycle > first_state.state.metadata().next_t_cycle,
        "state-in run should continue from the restored machine state"
    );

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn binary_run_rejects_corrupt_machine_save_state_artifacts() {
    let temp_dir = unique_temp_dir("state-corrupt");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path = temp_dir.join("state.gb");
    let corrupt_state_path = temp_dir.join("corrupt.gbstate");
    fs::write(&rom_path, build_nop_loop_rom()).expect("test ROM should be writable");
    fs::write(&corrupt_state_path, b"not-a-gbstate").expect("corrupt state should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_gb-cli"))
        .args([
            "run",
            rom_path.to_str().expect("path should be valid UTF-8"),
            "--tcycles",
            "1",
            "--state-in",
            corrupt_state_path
                .to_str()
                .expect("path should be valid UTF-8"),
        ])
        .output()
        .expect("gb-cli binary should run");

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");
    assert!(
        String::from_utf8(output.stderr)
            .expect("stderr should be UTF-8")
            .contains("failed to decode .gbstate state")
    );

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn binary_real_boot_warns_for_mismatched_boot_rom_assets() {
    let temp_dir = unique_temp_dir("real-boot");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path = temp_dir.join("serial.gb");
    let boot_dir = temp_dir.join("bootroms");
    fs::write(&rom_path, build_single_byte_serial_rom(b'B')).expect("plain ROM should be writable");
    write_fake_boot_rom(&boot_dir, BootRomKind::Dmg, 0x00);

    let output = Command::new(env!("CARGO_BIN_EXE_gb-cli"))
        .args([
            "run",
            rom_path.to_str().expect("path should be valid UTF-8"),
            "--startup",
            "real-boot",
            "--boot-rom-dir",
            boot_dir.to_str().expect("path should be valid UTF-8"),
            "--boot-rom-verify",
            "warn",
            "--tcycles",
            "1",
        ])
        .output()
        .expect("gb-cli binary should run");

    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .expect("stderr should be UTF-8")
            .contains("warning: boot ROM asset Dmg")
    );

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn binary_inspect_rom_reports_supported_header_details() {
    let temp_dir = unique_temp_dir("inspect-supported");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path = temp_dir.join("warn.gb");
    let mut rom = build_test_rom_with_header(&[0x00], 0x08, 0x00, 0x02);
    set_header_flags(&mut rom, 0x80, 0x03);
    fs::write(&rom_path, rom).expect("supported ROM should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_gb-cli"))
        .args([
            "inspect-rom",
            rom_path.to_str().expect("path should be valid UTF-8"),
            "--mode",
            "experimental",
        ])
        .output()
        .expect("gb-cli binary should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.contains("selection=supported"));
    assert!(stdout.contains("cgb_flag=supported"));
    assert!(stdout.contains("sgb_flag=supported"));
    assert!(stdout.contains("diagnostic_count=1"));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}
