use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::git::scrub_inherited_git_repository_context;
use super::manifest::Report;

mod arguments;
mod manifest;
mod materialization;
mod selection;

fn unique_temp_dir(label: &str) -> PathBuf {
    super::git::unique_temp_path(&format!("fetch-{label}"))
}

fn write_reports(workspace_root: &Path, text: &str) {
    let path = workspace_root.join(super::REPORTS_MANIFEST_PATH);
    fs::create_dir_all(path.parent().expect("report manifest should have parent"))
        .expect("report manifest parent should be creatable");
    fs::write(path, text).expect("report manifest should be writable");
}

fn write_source_manifest(workspace_root: &Path, relative_path: &str, text: &str) {
    let path = workspace_root.join(super::DATA_DIR).join(relative_path);
    fs::create_dir_all(path.parent().expect("source manifest should have parent"))
        .expect("source manifest parent should be creatable");
    fs::write(path, text).expect("source manifest should be writable");
}

fn write_basic_reports(workspace_root: &Path, source_path: &str) {
    write_reports(
        workspace_root,
        &format!(
            concat!(
                "status_dir = \".status\"\n",
                "artifact_dir = \".artifacts\"\n",
                "report_file = \"test-report.md\"\n",
                "\n",
                "[[report]]\n",
                "id = \"sample-report\"\n",
                "store_dir = \"sample-report\"\n",
                "sources = \"{}\"\n",
                "family_order = [\"family-a\", \"family-b\"]\n",
            ),
            source_path
        ),
    );
}

fn git(args: &[&str], current_dir: &Path) {
    git_with_env(args, current_dir, &[]);
}

fn git_with_env(args: &[&str], current_dir: &Path, envs: &[(&str, &str)]) {
    let mut command = Command::new("git");
    command.current_dir(current_dir);
    command.envs(envs.iter().copied());
    command.args(args);
    scrub_inherited_git_repository_context(&mut command);
    let output = command.output().expect("git command should spawn");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

fn commit_upstream_repo(root: &Path) -> String {
    git(&["init", "--no-bare"], root);
    git(&["add", "."], root);
    git_with_env(
        &["commit", "-m", "fixture"],
        root,
        &[
            ("GIT_AUTHOR_EMAIL", "gb-cycle@example.invalid"),
            ("GIT_AUTHOR_NAME", "gb-cycle tests"),
            ("GIT_COMMITTER_EMAIL", "gb-cycle@example.invalid"),
            ("GIT_COMMITTER_NAME", "gb-cycle tests"),
        ],
    );

    let mut command = Command::new("git");
    command.current_dir(root);
    command.args(["rev-parse", "HEAD"]);
    scrub_inherited_git_repository_context(&mut command);
    let output = command.output().expect("git rev-parse should spawn");
    assert!(output.status.success(), "git rev-parse should succeed");
    String::from_utf8(output.stdout)
        .expect("git hash should be utf-8")
        .trim()
        .to_string()
}

fn basic_report() -> Report {
    Report {
        id: "sample-report".to_string(),
        store_dir: PathBuf::from("sample-report"),
        sources: PathBuf::from("sources.report.toml"),
        status_dir: PathBuf::from(".status"),
        artifact_dir: PathBuf::from(".artifacts"),
        report_file: PathBuf::from("test-report.md"),
        family_order: Some(vec!["family-a".to_string(), "family-b".to_string()]),
    }
}
