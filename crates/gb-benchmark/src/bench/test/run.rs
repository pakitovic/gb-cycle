use std::fs;

use super::super::run::filter_valid_cases;
use super::{temp_root, write_case};

#[test]
fn filter_valid_cases_skips_missing_and_empty_roms() {
    let root = temp_root("filter");
    let case_dir = root.join("cases");
    fs::create_dir_all(&case_dir).expect("case dir should create");
    let good_rom = root.join("good.gb");
    let empty_rom = root.join("empty.gb");
    fs::write(&good_rom, [0_u8]).expect("good rom should write");
    fs::write(&empty_rom, []).expect("empty rom should write");
    let good_case = case_dir.join("good.toml");
    let empty_case = case_dir.join("empty.toml");
    write_case(&good_case, "good", &good_rom);
    write_case(&empty_case, "empty", &empty_rom);

    let mut output = Vec::new();
    let valid = filter_valid_cases(&[good_case.clone(), empty_case], &mut output)
        .expect("filtering should succeed");

    assert_eq!(valid, vec![good_case]);
    let output = String::from_utf8(output).expect("output should be utf-8");
    assert!(output.contains("validated 1/2 benchmark case ROM(s); skipped 1"));
    assert!(output.contains("ROM is empty"));
}
