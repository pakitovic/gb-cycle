#![allow(dead_code)]

pub mod synthetic_cartridge;

use std::path::{Path, PathBuf};
use std::{env, fs, io};

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

pub fn fixture_accept_writes_enabled(env_var: &str) -> bool {
    env::var_os(env_var).is_some()
}

pub fn ensure_text_fixture(path: &Path, expected: &str, accept_env_var: &str) -> String {
    if fixture_accept_writes_enabled(accept_env_var) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("fixture directory should be creatable");
        }
        fs::write(path, expected).expect("text fixture should be writable");
    }

    let fixture = read_text_fixture(path).expect("text fixture should be readable");
    assert_eq!(fixture, expected);
    fixture
}

#[track_caller]
pub fn assert_directory_exists(path: &Path) {
    assert!(
        path.is_dir(),
        "expected test directory to exist: {}",
        path.display()
    );
}
