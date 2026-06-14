mod boot_rom;
mod fetch;
mod oracle;
mod report;
mod report_label;
mod rtc;
mod runtime;
mod suite;
mod suite_link;
use std::path::{Path, PathBuf};

pub use fetch::{fetch_help_text, run_fetch_command};
pub use report::{report_help_text, run_report_command};
pub use suite::{run_suite_command, suite_help_text};
pub use suite_link::{run_suite_link_command, suite_link_help_text};
pub fn default_workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("workspace root should be two levels above gb-test-runner")
        .to_path_buf()
}
