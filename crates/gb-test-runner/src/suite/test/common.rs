use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

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
id = "blargg"
target_root = "blargg"

[[source.family]]
id = "cpp"
target_root = "cpp"

[[source.family]]
id = "docboy-dmg"
target_root = "docboy-dmg"

[[source.family]]
id = "gbmicrotest"
target_root = "gbmicrotest"

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

pub(super) fn build_infinite_loop_rom() -> Vec<u8> {
    build_test_rom(&[0x18, 0xFE])
}

pub(super) fn basic_manifest(suite_name: &str, family: &str, case_id: &str, rom: &str) -> String {
    format!(
        concat!(
            "family = \"{}\"\n",
            "suite_name = \"{}\"\n",
            "console = \"dmg\"\n",
            "timeout_frames = 2\n",
            "oracle = {{ type = \"serial-contains\", expected = \"Passed\" }}\n",
            "\n",
            "[[case]]\n",
            "id = \"{}\"\n",
            "rom = \"{}\"\n",
        ),
        family, suite_name, case_id, rom
    )
}
