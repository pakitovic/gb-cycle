use std::fs;
use std::path::{Path, PathBuf};

const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;

fn data_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn build_test_rom(program: &[u8]) -> Vec<u8> {
    let mut rom = vec![0xFF; HEADER_MINIMUM_ROM_LEN.max(32 * 1024)];
    for (offset, byte) in program.iter().copied().enumerate() {
        rom[0x0100 + offset] = byte;
    }
    rom[0x0147] = 0x00;
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;
    rom
}

fn assert_fixture_rom_matches_program(relative_path: &str, program: &[u8]) {
    let rom_path = data_path(relative_path);
    let actual = fs::read(&rom_path).unwrap_or_else(|error| {
        panic!("failed to read fixture ROM {}: {error}", rom_path.display())
    });
    let expected = build_test_rom(program);

    assert_eq!(
        actual,
        expected,
        "fixture ROM {} no longer matches its documented synthetic program",
        rom_path.display()
    );
}

fn assert_dmg07_fixture_rom_matches_response_table(relative_path: &str, response_table: &[u8]) {
    let mut program = vec![
        0x21, 0x14, 0x01, 0x7E, 0xE0, 0x01, 0x3E, 0x80, 0xE0, 0x02, 0xF0, 0x02, 0xCB, 0x7F, 0x20,
        0xFA, 0x23, 0xC3, 0x03, 0x01,
    ];
    program.extend_from_slice(response_table);
    assert_fixture_rom_matches_program(relative_path, &program);
}

#[test]
fn basic_left_fixture_matches_its_documented_program() {
    assert_fixture_rom_matches_program(
        "data/fixtures/linked/dmg04/basic-left.gb",
        &[
            0x3E, 0xA5, 0xE0, 0x01, 0x3E, 0x81, 0xE0, 0x02, 0xC3, 0x08, 0x01,
        ],
    );
}

#[test]
fn basic_right_fixture_matches_its_documented_program() {
    assert_fixture_rom_matches_program(
        "data/fixtures/linked/dmg04/basic-right.gb",
        &[
            0x3E, 0x3C, 0xE0, 0x01, 0x3E, 0x80, 0xE0, 0x02, 0xC3, 0x08, 0x01,
        ],
    );
}

#[test]
fn stale_left_fixture_matches_its_documented_program() {
    assert_fixture_rom_matches_program(
        "data/fixtures/linked/dmg04/stale-left.gb",
        &[
            0x3E, 0xA5, 0xE0, 0x01, 0x3E, 0x81, 0xE0, 0x02, 0x06, 0xFF, 0x05, 0x20, 0xFD, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x3E, 0x81, 0xE0, 0x02, 0xC3, 0x16, 0x01,
        ],
    );
}

#[test]
fn stale_right_fixture_matches_its_documented_program() {
    assert_fixture_rom_matches_program(
        "data/fixtures/linked/dmg04/stale-right.gb",
        &[
            0x3E, 0x3C, 0xE0, 0x01, 0x3E, 0x80, 0xE0, 0x02, 0x06, 0xFF, 0x05, 0x20, 0xFD, 0x3E,
            0xF0, 0xE0, 0x01, 0x3E, 0x80, 0xE0, 0x02, 0xC3, 0x15, 0x01,
        ],
    );
}

#[test]
fn double_master_left_fixture_matches_its_documented_program() {
    assert_fixture_rom_matches_program(
        "data/fixtures/linked/dmg04/double-master-left.gb",
        &[
            0x3E, 0xA5, 0xE0, 0x01, 0x3E, 0x81, 0xE0, 0x02, 0xC3, 0x08, 0x01,
        ],
    );
}

#[test]
fn double_master_right_fixture_matches_its_documented_program() {
    assert_fixture_rom_matches_program(
        "data/fixtures/linked/dmg04/double-master-right.gb",
        &[
            0x3E, 0x3C, 0xE0, 0x01, 0x3E, 0x81, 0xE0, 0x02, 0xC3, 0x08, 0x01,
        ],
    );
}

#[test]
fn open_line_right_fixture_matches_its_documented_program() {
    assert_fixture_rom_matches_program(
        "data/fixtures/linked/dmg04/open-line-right.gb",
        &[0xC3, 0x00, 0x01],
    );
}

#[test]
fn dmg04_fixture_readme_exists_next_to_the_binary_fixtures() {
    let readme_path = data_path("data/fixtures/linked/dmg04/README.md");
    assert!(
        Path::new(&readme_path).is_file(),
        "expected {} to document the synthetic linked-session ROM fixtures",
        readme_path.display()
    );
}

#[test]
fn dmg07_p1_fixture_matches_its_documented_program() {
    assert_dmg07_fixture_rom_matches_response_table(
        "data/fixtures/linked/dmg07/p1-basic.gb",
        &[
            0x88, 0x88, 0x00, 0x01, 0xAA, 0xAA, 0xAA, 0x00, 0x00, 0x00, 0x00, 0x00, 0xA1, 0x00,
            0x00, 0x00, 0xA2, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0x00, 0x88, 0x88, 0x00, 0x01,
        ],
    );
}

#[test]
fn dmg07_p2_fixture_matches_its_documented_program() {
    assert_dmg07_fixture_rom_matches_response_table(
        "data/fixtures/linked/dmg07/p2-basic.gb",
        &[
            0x88, 0x88, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xB1, 0x00,
            0x00, 0x00, 0xB2, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x88, 0x88, 0x00, 0x01,
        ],
    );
}

#[test]
fn dmg07_p3_fixture_matches_its_documented_program() {
    assert_dmg07_fixture_rom_matches_response_table(
        "data/fixtures/linked/dmg07/p3-basic.gb",
        &[
            0x88, 0x88, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC1, 0x00,
            0x00, 0x00, 0xC2, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x88, 0x88, 0x00, 0x01,
        ],
    );
}

#[test]
fn dmg07_p4_fixture_matches_its_documented_program() {
    assert_dmg07_fixture_rom_matches_response_table(
        "data/fixtures/linked/dmg07/p4-basic.gb",
        &[
            0x88, 0x88, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xD1, 0x00,
            0x00, 0x00, 0xD2, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x88, 0x88, 0x00, 0x01,
        ],
    );
}

#[test]
fn dmg07_fixture_readme_exists_next_to_the_binary_fixtures() {
    let readme_path = data_path("data/fixtures/linked/dmg07/README.md");
    assert!(
        Path::new(&readme_path).is_file(),
        "expected {} to document the synthetic linked-session ROM fixtures",
        readme_path.display()
    );
}
