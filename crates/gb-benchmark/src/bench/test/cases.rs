use std::fs;

use super::super::cases::{generate_benchmark_cases, normalize_cases, rewrite_rom_dir};
use super::temp_root;

#[test]
fn sample_is_written_under_test_bench() {
    let root = temp_root("sample");
    let mut output = Vec::new();
    super::super::cases::write_sample_case(&root, &mut output).expect("sample should write");

    assert!(root.join("test/bench/game.toml").is_file());
    let output = String::from_utf8(output).expect("output should be utf-8");
    assert!(output.contains("test/bench/game.toml"));
}

#[test]
fn generate_cases_uses_rom_suffix_for_model_and_safe_id() {
    let root = temp_root("generate");
    let case_dir = root.join("cases");
    let rom_dir = root.join("roms");
    fs::create_dir_all(&case_dir).expect("case dir should create");
    fs::create_dir_all(&rom_dir).expect("rom dir should create");
    fs::write(rom_dir.join("Dr Mario.gb"), [0_u8]).expect("rom should write");
    fs::write(rom_dir.join("Zelda.gbc"), [0_u8]).expect("rom should write");

    let mut output = Vec::new();
    generate_benchmark_cases(&case_dir, &rom_dir, None, &mut output)
        .expect("cases should generate");

    let dmg = fs::read_to_string(case_dir.join("Dr Mario.toml")).expect("DMG case exists");
    assert!(dmg.contains("id = \"dr-mario\""));
    assert!(dmg.contains("model = \"DMG\""));
    let cgb = fs::read_to_string(case_dir.join("Zelda.toml")).expect("CGB case exists");
    assert!(cgb.contains("model = \"CGB\""));
}

#[test]
fn normalize_and_rewrite_cases_use_top_level_rom() {
    let root = temp_root("normalize");
    let case_dir = root.join("cases");
    let rom_dir = root.join("next-roms");
    fs::create_dir_all(&case_dir).expect("case dir should create");
    fs::create_dir_all(&rom_dir).expect("rom dir should create");
    fs::write(
        case_dir.join("old.toml"),
        "version = 1\nid = \"old\"\nrom = \"/old/Dr Mario.gb\" # keep\nmodel = \"DMG\"\n\n[[run]]\nid = \"idle\"\nduration_seconds = 1\n",
    )
    .expect("case should write");

    let mut output = Vec::new();
    normalize_cases(&case_dir, &mut output).expect("case should normalize");
    assert!(case_dir.join("Dr Mario.toml").is_file());

    rewrite_rom_dir(&case_dir, &rom_dir, &mut output).expect("rom dir should rewrite");
    let text = fs::read_to_string(case_dir.join("Dr Mario.toml")).expect("case should read");
    assert!(text.contains(&format!(
        "rom = \"{}\"# keep",
        rom_dir.join("Dr Mario.gb").display()
    )));
}
