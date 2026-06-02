use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::super::model::Report;

pub(super) fn unique_temp_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "gb-cycle-suite-link-{label}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ))
}

pub(super) fn write_reports(workspace: &Path) {
    let data_dir = workspace.join("crates/gb-test-runner/data");
    fs::create_dir_all(&data_dir).expect("data dir should be created");
    fs::write(
        data_dir.join("reports.toml"),
        r#"status_dir = ".status"
artifact_dir = ".artifacts"
report_file = "test-report.md"

[[report]]
id = "linked"
local = true
store_dir = "linked"
"#,
    )
    .expect("reports.toml should be written");
}

pub(super) fn linked_report() -> Report {
    Report {
        id: "linked".to_string(),
        local: true,
        store_dir: PathBuf::from("linked"),
        sources: None,
        status_dir: PathBuf::from(".status"),
        artifact_dir: PathBuf::from(".artifacts"),
    }
}

pub(super) fn copy_dmg04_basic_fixtures(workspace: &Path) {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/linked/fixtures/dmg04");
    let target_root = workspace.join("crates/gb-test-runner/data/linked/fixtures/dmg04");
    fs::create_dir_all(&target_root).expect("fixture dir should be created");
    for file in ["basic-left.gb", "basic-right.gb", "basic-exchange.snapshot"] {
        fs::copy(source_root.join(file), target_root.join(file)).expect("fixture should be copied");
    }
}

pub(super) fn write_dmg04_manifest(workspace: &Path, expected: &str) {
    let linked_dir = workspace.join("crates/gb-test-runner/data/linked");
    fs::create_dir_all(&linked_dir).expect("linked dir should be created");
    fs::write(
        linked_dir.join("dmg04.link.suite.toml"),
        format!(
            r#"report = "linked"
suite_name = "dmg04"
family = "linked"
topology = "dmg04"
timeout_tcycles = 5000

[[case]]
id = "dmg04-basic-exchange"
oracle = {{ type = "serial-hex-exact", target_participant = "left", expected = {expected:?} }}

  [[case.participant]]
  id = "left"
  rom = "fixtures/dmg04/basic-left.gb"
  console = "dmg"

  [[case.participant]]
  id = "right"
  rom = "fixtures/dmg04/basic-right.gb"
  console = "dmg"
"#
        ),
    )
    .expect("manifest should be written");
}
