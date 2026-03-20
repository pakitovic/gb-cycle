#![allow(dead_code)]

pub mod synthetic_cartridge;

use std::path::{Path, PathBuf};
use std::{fs, io};

pub fn tests_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests")
}

pub fn fixtures_dir() -> PathBuf {
    tests_dir().join("fixtures")
}

pub fn rom_fixtures_dir() -> PathBuf {
    fixtures_dir().join("roms")
}

pub fn trace_fixtures_dir() -> PathBuf {
    fixtures_dir().join("traces")
}

pub fn read_text_fixture(path: &Path) -> io::Result<String> {
    fs::read_to_string(path)
}

pub fn read_binary_fixture(path: &Path) -> io::Result<Vec<u8>> {
    fs::read(path)
}

#[track_caller]
pub fn assert_directory_exists(path: &Path) {
    assert!(
        path.is_dir(),
        "expected test directory to exist: {}",
        path.display()
    );
}
