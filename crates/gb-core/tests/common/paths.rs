#![allow(dead_code)]

use std::path::{Path, PathBuf};

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

pub fn rom_fixture_path(suite: &str, file_name: &str) -> PathBuf {
    rom_fixtures_dir().join(suite).join(file_name)
}

pub fn trace_fixture_path(file_name: &str) -> PathBuf {
    trace_fixtures_dir().join(file_name)
}

pub fn suite_trace_fixture_path(suite: &str, file_name: &str) -> PathBuf {
    trace_fixtures_dir().join(suite).join(file_name)
}

#[track_caller]
pub fn assert_directory_exists(path: &Path) {
    assert!(
        path.is_dir(),
        "expected test directory to exist: {}",
        path.display()
    );
}
