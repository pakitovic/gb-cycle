use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

pub(super) fn unique_temp_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "gb-cycle-suite-{label}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ))
}

pub(super) fn write_reports(workspace_root: &Path, report_id: &str, source_path: &str) {
    let path = workspace_root.join(super::super::model::REPORTS_MANIFEST_PATH);
    fs::create_dir_all(path.parent().expect("reports path should have parent"))
        .expect("reports parent should be creatable");
    fs::write(
        path,
        format!(
            concat!(
                "status_dir = \".status\"\n",
                "artifact_dir = \".artifacts\"\n",
                "report_file = \"test-report.md\"\n",
                "\n",
                "[[report]]\n",
                "id = \"{}\"\n",
                "store_dir = \"{}\"\n",
                "sources = \"{}\"\n",
            ),
            report_id, report_id, source_path
        ),
    )
    .expect("reports manifest should be writable");
    write_source_manifest(
        workspace_root,
        source_path,
        r#"
[[source]]
id = "test-source"

[[source.family]]
id = "acid"
target_root = "acid"

[[source.family]]
id = "ashiepaws"
target_root = "ashiepaws"

[[source.family]]
id = "ax6"
target_root = "ax6"

[[source.family]]
id = "blargg"
target_root = "blargg"

[[source.family]]
id = "cpp"
target_root = "cpp"

[[source.family]]
id = "daid"
target_root = "daid"

[[source.family]]
id = "docboy-dmg"
target_root = "docboy-dmg"

[[source.family]]
id = "gbmicrotest"
target_root = "gbmicrotest"

[[source.family]]
id = "mealybug-tearoom-tests"
target_root = "mealybug-tearoom-tests"

[[source.family]]
id = "mooneye"
target_root = "mooneye"

[[source.family]]
id = "samesuite"
target_root = "samesuite"
"#,
    );
}

pub(super) fn write_manifest(workspace_root: &Path, relative_path: &str, text: &str) {
    let path = workspace_root
        .join(super::super::model::DATA_DIR)
        .join(relative_path);
    fs::create_dir_all(path.parent().expect("manifest path should have parent"))
        .expect("manifest parent should be creatable");
    fs::write(path, text).expect("suite manifest should be writable");
}

pub(super) fn write_source_manifest(workspace_root: &Path, relative_path: &str, text: &str) {
    write_manifest(workspace_root, relative_path, text);
}

pub(super) fn write_materialized_source_manifest(
    workspace_root: &Path,
    report_id: &str,
    source_path: &str,
    families: &[(&str, &str)],
) {
    let mut text = String::from(
        "[[source]]\n\
         id = \"local-source\"\n\
         git_url = \"file:///unused\"\n\
         git_rev = \"unused\"\n",
    );
    let store_root = workspace_root.join("test").join(report_id);
    for (family_id, target_root) in families {
        let _ = write!(
            &mut text,
            "\n[[source.family]]\n\
             id = {family_id:?}\n\
             target_root = {target_root:?}\n\
             sparse_paths = [{:?}]\n",
            format!("upstream/{family_id}")
        );
        let family_root = store_root.join(target_root);
        for file in materialized_files(&family_root) {
            let target = file
                .strip_prefix(&family_root)
                .expect("materialized file should be below family root");
            let hash = sha256_hex(&fs::read(&file).expect("materialized file should be readable"));
            let source_file = Path::new("upstream").join(family_id).join(target);
            let _ = write!(
                &mut text,
                "\n[[source.family.file]]\n\
                 path = {:?}\n\
                 target = {:?}\n\
                 sha256 = {hash:?}\n",
                source_file.to_string_lossy(),
                target.to_string_lossy()
            );
        }
    }
    write_source_manifest(workspace_root, source_path, &text);
}

fn materialized_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_materialized_files(root, &mut files);
    files.sort();
    files
}

fn collect_materialized_files(root: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries {
        let entry = entry.expect("materialized entry should be readable");
        let path = entry.path();
        let file_name = path.file_name().and_then(|file_name| file_name.to_str());
        if file_name.is_some_and(|file_name| file_name.starts_with('.')) {
            continue;
        }
        if path.is_dir() {
            collect_materialized_files(&path, files);
        } else {
            files.push(path);
        }
    }
}

pub(super) fn commit_upstream_repo(root: &Path) -> String {
    git(&["init", "--no-bare"], root);
    git(&["add", "."], root);
    let mut command = Command::new("git");
    command.current_dir(root);
    command.env("GIT_AUTHOR_EMAIL", "gb-cycle@example.invalid");
    command.env("GIT_AUTHOR_NAME", "gb-cycle tests");
    command.env("GIT_COMMITTER_EMAIL", "gb-cycle@example.invalid");
    command.env("GIT_COMMITTER_NAME", "gb-cycle tests");
    command.args(["commit", "-m", "fixture"]);
    run_git(command, root, "git commit");

    let mut command = Command::new("git");
    command.current_dir(root);
    command.args(["rev-parse", "HEAD"]);
    let output = command.output().expect("git rev-parse should spawn");
    assert!(output.status.success(), "git rev-parse should succeed");
    String::from_utf8(output.stdout)
        .expect("git hash should be utf-8")
        .trim()
        .to_string()
}

fn git(args: &[&str], current_dir: &Path) {
    let mut command = Command::new("git");
    command.current_dir(current_dir);
    command.args(args);
    run_git(command, current_dir, "git command");
}

fn run_git(mut command: Command, current_dir: &Path, label: &str) {
    for key in [
        "GIT_COMMON_DIR",
        "GIT_DIR",
        "GIT_INDEX_FILE",
        "GIT_NAMESPACE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_PREFIX",
        "GIT_WORK_TREE",
    ] {
        command.env_remove(key);
    }
    let output = command.output().expect("git command should spawn");
    assert!(
        output.status.success(),
        "{label} failed in {}: {}",
        current_dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(&mut hex, "{byte:02x}");
    }
    hex
}

fn build_test_rom(program: &[u8]) -> Vec<u8> {
    const PROGRAM_START: usize = 0x0150;
    let mut rom = vec![0xFF; 32 * 1024];
    rom[0x0100..0x0103].copy_from_slice(&[0xC3, 0x50, 0x01]);
    for (offset, byte) in program.iter().copied().enumerate() {
        rom[PROGRAM_START + offset] = byte;
    }
    rom[0x0147] = 0x00;
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;
    rom
}

fn finalize_header_checksums(rom: &mut [u8]) {
    let mut header_checksum = 0_u8;
    for byte in &rom[0x0134..=0x014C] {
        header_checksum = header_checksum.wrapping_sub(*byte).wrapping_sub(1);
    }
    rom[0x014D] = header_checksum;

    let global_checksum = rom
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != 0x014E && *index != 0x014F)
        .fold(0_u16, |checksum, (_, byte)| {
            checksum.wrapping_add(u16::from(*byte))
        });
    let [high, low] = global_checksum.to_be_bytes();
    rom[0x014E] = high;
    rom[0x014F] = low;
}

pub(super) fn build_serial_text_rom(text: &str) -> Vec<u8> {
    let mut program = Vec::new();
    for byte in text.bytes() {
        program.extend_from_slice(&[
            0x3E, byte, // LD A,d8
            0xE0, 0x01, // LDH (SB),A
            0x3E, 0x81, // LD A,$81
            0xE0, 0x02, // LDH (SC),A
            0xF0, 0x02, // LDH A,(SC)
            0xE6, 0x80, // AND $80
            0x20, 0xFA, // JR NZ,-6
        ]);
    }
    program.extend_from_slice(&[0x18, 0xFE]);
    build_test_rom(&program)
}

pub(super) fn build_fibonacci_result_rom(signature: [u8; 6]) -> Vec<u8> {
    build_test_rom(&[
        0x06,
        signature[0], // LD B,d8
        0x0E,
        signature[1], // LD C,d8
        0x16,
        signature[2], // LD D,d8
        0x1E,
        signature[3], // LD E,d8
        0x26,
        signature[4], // LD H,d8
        0x2E,
        signature[5], // LD L,d8
        0x40,         // LD B,B, Mooneye-style magic breakpoint
        0x00,         // NOP
        0x18,
        0xFD, // JR -3, keep the post-breakpoint loop visible near PC
    ])
}

pub(super) fn build_memory_write_rom(address: u16, value: u8) -> Vec<u8> {
    let [low, high] = address.to_le_bytes();
    build_test_rom(&[
        0x3E, value, // LD A,d8
        0xEA, low, high, // LD (a16),A
        0x18, 0xFE, // JR -2
    ])
}

pub(super) fn build_joypad_a_pressed_memory_write_rom(address: u16, value: u8) -> Vec<u8> {
    let [low, high] = address.to_le_bytes();
    build_test_rom(&[
        0x3E, 0x10, // LD A,$10; select button row
        0xE0, 0x00, // LDH ($00),A
        0xF0, 0x00, // LDH A,($00)
        0xE6, 0x01, // AND $01; A button is active-low bit 0
        0x20, 0xFA, // JR NZ,-6
        0x3E, value, // LD A,d8
        0xEA, low, high, // LD (a16),A
        0x18, 0xFE, // JR -2
    ])
}

pub(super) fn build_mbc3_rtc_wait_rom(address: u16, value: u8) -> Vec<u8> {
    let [low, high] = address.to_le_bytes();
    let mut rom = build_test_rom(&[
        0x3E, 0x0A, // LD A,$0A
        0xEA, 0x00, 0x00, // LD ($0000),A; enable RTC
        0x3E, 0x08, // LD A,$08
        0xEA, 0x00, 0x40, // LD ($4000),A; select seconds register
        0x3E, 0x00, // LD A,$00
        0xEA, 0x00, 0x60, // LD ($6000),A; arm latch
        0x3E, 0x01, // LD A,$01
        0xEA, 0x00, 0x60, // LD ($6000),A; latch current RTC state
        0xFA, 0x00, 0xA0, // LD A,($A000)
        0xFE, 0x01, // CP $01
        0x38, 0xEF, // JR C,-17
        0x3E, value, // LD A,d8
        0xEA, low, high, // LD (a16),A
        0x18, 0xFE, // JR -2
    ]);
    rom[0x0147] = 0x0F;
    finalize_header_checksums(&mut rom);
    rom
}

pub(super) fn build_delayed_dmg_handoff_boot_rom() -> Vec<u8> {
    let mut rom = vec![0x00; 0x0100];
    rom[0x0000..0x000D].copy_from_slice(&[
        0x06, 0xFF, // LD B,$FF
        0x0E, 0xFF, // LD C,$FF
        0x0D, // DEC C
        0x20, 0xFD, // JR NZ,$0004
        0x05, // DEC B
        0x20, 0xF8, // JR NZ,$0002
        0xC3, 0xFC, 0x00, // JP $00FC
    ]);
    rom[0x00FC..0x0100].copy_from_slice(&[
        0x3E, 0x01, // LD A,$01
        0xE0, 0x50, // LDH ($FF50),A
    ]);
    rom
}

pub(super) fn build_infinite_loop_rom() -> Vec<u8> {
    build_test_rom(&[0x18, 0xFE])
}

pub(super) fn write_grayscale_png(path: &Path, pixels: &[u8]) {
    fs::create_dir_all(path.parent().expect("PNG path should have parent"))
        .expect("PNG parent should be creatable");
    let file = fs::File::create(path).expect("PNG should be writable");
    let mut encoder = png::Encoder::new(file, 160, 144);
    encoder.set_color(png::ColorType::Grayscale);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .expect("PNG header should be writable");
    writer
        .write_image_data(pixels)
        .expect("PNG data should be writable");
}

pub(super) fn basic_manifest(
    report: &str,
    suite_name: &str,
    family: &str,
    case_id: &str,
    rom: &str,
) -> String {
    format!(
        concat!(
            "family = \"{}\"\n",
            "suite_name = \"{}\"\n",
            "report = \"{}\"\n",
            "console = \"dmg\"\n",
            "timeout_frames = 2\n",
            "oracle = {{ type = \"serial-contains\", expected = \"Passed\" }}\n",
            "\n",
            "[[case]]\n",
            "id = \"{}\"\n",
            "rom = \"{}\"\n",
        ),
        family, suite_name, report, case_id, rom
    )
}
