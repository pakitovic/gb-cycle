#![allow(dead_code)]

use std::path::Path;
use std::{env, fs, io};

use super::paths;

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

pub fn ensure_binary_fixture(path: &Path, expected: &[u8], accept_env_var: &str) -> Vec<u8> {
    if fixture_accept_writes_enabled(accept_env_var) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("fixture directory should be creatable");
        }
        fs::write(path, expected).expect("binary fixture should be writable");
    }

    let fixture = read_binary_fixture(path).expect("binary fixture should be readable");
    assert_eq!(fixture, expected);
    fixture
}

pub fn ensure_suite_text_fixture(
    suite: &str,
    file_name: &str,
    expected: &str,
    accept_env_var: &str,
) -> String {
    let path = paths::suite_trace_fixture_path(suite, file_name);
    ensure_text_fixture(&path, expected, accept_env_var)
}

pub fn ensure_suite_binary_fixture(
    suite: &str,
    file_name: &str,
    expected: &[u8],
    accept_env_var: &str,
) -> Vec<u8> {
    let path = paths::rom_fixture_path(suite, file_name);
    ensure_binary_fixture(&path, expected, accept_env_var)
}
