use std::fs;

use super::super::ensure_report_families_materialized;
use super::super::git::sha256_hex;
use super::common::{commit_upstream_repo, unique_temp_dir, write_reports, write_source_manifest};

#[test]
fn ensure_keeps_materialized_family_when_hashes_match() {
    let workspace_root = unique_temp_dir("ensure-matches");
    write_reports(
        &workspace_root,
        concat!(
            "status_dir = \".status\"\n",
            "artifact_dir = \".artifacts\"\n",
            "report_file = \"test-report.md\"\n",
            "\n",
            "[[report]]\n",
            "id = \"sample-report\"\n",
            "store_dir = \"sample-report\"\n",
            "sources = \"sample-report/sources.report.toml\"\n",
        ),
    );
    let bytes = b"materialized rom";
    let hash = sha256_hex(bytes);
    write_source_manifest(
        &workspace_root,
        "sample-report/sources.report.toml",
        &format!(
            concat!(
                "[[source]]\n",
                "id = \"local-source\"\n",
                "git_url = \"file:///unused\"\n",
                "git_rev = \"unused\"\n",
                "\n",
                "[[source.family]]\n",
                "id = \"family-a\"\n",
                "target_root = \"family-a\"\n",
                "sparse_paths = [\"roms/family-a\"]\n",
                "\n",
                "[[source.family.file]]\n",
                "path = \"roms/family-a/test.gb\"\n",
                "target = \"test.gb\"\n",
                "sha256 = {:?}\n",
            ),
            hash
        ),
    );
    let target = workspace_root.join("test/sample-report/family-a/test.gb");
    fs::create_dir_all(target.parent().expect("target should have parent"))
        .expect("target parent should be creatable");
    fs::write(&target, bytes).expect("target should be writable");

    let mut output = Vec::new();
    ensure_report_families_materialized(
        &workspace_root,
        "sample-report",
        &["family-a".to_string()],
        &mut output,
    )
    .expect("matching materialization should not fetch");

    assert!(output.is_empty());
    assert_eq!(
        fs::read(&target).expect("target should still exist"),
        bytes,
        "matching materialized file should be preserved"
    );

    let _ = fs::remove_dir_all(workspace_root);
}

#[test]
fn ensure_fetches_family_when_rom_is_missing() {
    let workspace_root = unique_temp_dir("ensure-missing-rom-workspace");
    let upstream_root = unique_temp_dir("ensure-missing-rom-upstream");
    fs::create_dir_all(upstream_root.join("roms/family-a"))
        .expect("upstream family should be creatable");
    fs::write(upstream_root.join("roms/family-a/test.gb"), b"rom bytes")
        .expect("upstream ROM should be writable");
    let commit = commit_upstream_repo(&upstream_root);
    let hash = sha256_hex(b"rom bytes");
    write_sample_source(
        &workspace_root,
        &upstream_root,
        &commit,
        &[("test.gb", hash.as_str())],
    );

    let mut output = Vec::new();
    ensure_report_families_materialized(
        &workspace_root,
        "sample-report",
        &["family-a".to_string()],
        &mut output,
    )
    .expect("missing ROM should be fetched");

    assert_eq!(
        fs::read(workspace_root.join("test/sample-report/family-a/test.gb"))
            .expect("ROM should be materialized"),
        b"rom bytes"
    );
    let output = String::from_utf8(output).expect("output should be utf-8");
    assert!(output.contains("requires materialization: missing"));
    assert!(output.contains("materialized test ROM families family-a"));

    let _ = fs::remove_dir_all(workspace_root);
    let _ = fs::remove_dir_all(upstream_root);
}

#[test]
fn ensure_fetches_family_when_fixture_is_missing() {
    let workspace_root = unique_temp_dir("ensure-missing-fixture-workspace");
    let upstream_root = unique_temp_dir("ensure-missing-fixture-upstream");
    fs::create_dir_all(upstream_root.join("roms/family-a"))
        .expect("upstream family should be creatable");
    fs::write(upstream_root.join("roms/family-a/test.gb"), b"rom bytes")
        .expect("upstream ROM should be writable");
    fs::write(
        upstream_root.join("roms/family-a/test.png"),
        b"fixture bytes",
    )
    .expect("upstream fixture should be writable");
    let commit = commit_upstream_repo(&upstream_root);
    let rom_hash = sha256_hex(b"rom bytes");
    let fixture_hash = sha256_hex(b"fixture bytes");
    write_sample_source(
        &workspace_root,
        &upstream_root,
        &commit,
        &[
            ("test.gb", rom_hash.as_str()),
            ("test.png", fixture_hash.as_str()),
        ],
    );
    let rom_target = workspace_root.join("test/sample-report/family-a/test.gb");
    fs::create_dir_all(rom_target.parent().expect("target should have parent"))
        .expect("target parent should be creatable");
    fs::write(&rom_target, b"rom bytes").expect("materialized ROM should be writable");

    let mut output = Vec::new();
    ensure_report_families_materialized(
        &workspace_root,
        "sample-report",
        &["family-a".to_string()],
        &mut output,
    )
    .expect("missing fixture should fetch the family");

    assert_eq!(
        fs::read(workspace_root.join("test/sample-report/family-a/test.png"))
            .expect("fixture should be materialized"),
        b"fixture bytes"
    );
    let output = String::from_utf8(output).expect("output should be utf-8");
    assert!(output.contains("test.png"));

    let _ = fs::remove_dir_all(workspace_root);
    let _ = fs::remove_dir_all(upstream_root);
}

#[test]
fn ensure_fetches_family_when_hash_mismatches() {
    let workspace_root = unique_temp_dir("ensure-hash-workspace");
    let upstream_root = unique_temp_dir("ensure-hash-upstream");
    fs::create_dir_all(upstream_root.join("roms/family-a"))
        .expect("upstream family should be creatable");
    fs::write(upstream_root.join("roms/family-a/test.gb"), b"new bytes")
        .expect("upstream ROM should be writable");
    let commit = commit_upstream_repo(&upstream_root);
    let hash = sha256_hex(b"new bytes");
    write_sample_source(
        &workspace_root,
        &upstream_root,
        &commit,
        &[("test.gb", hash.as_str())],
    );
    let target = workspace_root.join("test/sample-report/family-a/test.gb");
    fs::create_dir_all(target.parent().expect("target should have parent"))
        .expect("target parent should be creatable");
    fs::write(&target, b"old bytes").expect("stale ROM should be writable");

    let mut output = Vec::new();
    ensure_report_families_materialized(
        &workspace_root,
        "sample-report",
        &["family-a".to_string()],
        &mut output,
    )
    .expect("hash mismatch should fetch the family");

    assert_eq!(
        fs::read(&target).expect("target should be rematerialized"),
        b"new bytes"
    );
    let output = String::from_utf8(output).expect("output should be utf-8");
    assert!(output.contains("hash mismatch"));

    let _ = fs::remove_dir_all(workspace_root);
    let _ = fs::remove_dir_all(upstream_root);
}

#[test]
fn ensure_respects_flat_target_root() {
    let workspace_root = unique_temp_dir("ensure-flat-workspace");
    let upstream_root = unique_temp_dir("ensure-flat-upstream");
    fs::create_dir_all(upstream_root.join("roms/boot")).expect("upstream should be creatable");
    fs::write(upstream_root.join("roms/boot/poweron.gb"), b"flat rom")
        .expect("upstream ROM should be writable");
    let commit = commit_upstream_repo(&upstream_root);
    let hash = sha256_hex(b"flat rom");
    write_reports(
        &workspace_root,
        concat!(
            "status_dir = \".status\"\n",
            "artifact_dir = \".artifacts\"\n",
            "report_file = \"test-report.md\"\n",
            "\n",
            "[[report]]\n",
            "id = \"flat-report\"\n",
            "store_dir = \"flat-report\"\n",
            "sources = \"flat-report/sources.report.toml\"\n",
        ),
    );
    write_source_manifest(
        &workspace_root,
        "flat-report/sources.report.toml",
        &format!(
            concat!(
                "[[source]]\n",
                "id = \"local-source\"\n",
                "git_url = {:?}\n",
                "git_rev = {:?}\n",
                "\n",
                "[[source.family]]\n",
                "id = \"flat-family\"\n",
                "target_root = \"\"\n",
                "sparse_paths = [\"roms\"]\n",
                "\n",
                "[[source.family.file]]\n",
                "path = \"roms/boot/poweron.gb\"\n",
                "target = \"boot/poweron.gb\"\n",
                "sha256 = {:?}\n",
            ),
            upstream_root.display().to_string(),
            commit,
            hash,
        ),
    );

    let mut output = Vec::new();
    ensure_report_families_materialized(
        &workspace_root,
        "flat-report",
        &["flat-family".to_string()],
        &mut output,
    )
    .expect("flat family should materialize");

    assert_eq!(
        fs::read(workspace_root.join("test/flat-report/boot/poweron.gb"))
            .expect("flat ROM should be materialized"),
        b"flat rom"
    );

    let _ = fs::remove_dir_all(workspace_root);
    let _ = fs::remove_dir_all(upstream_root);
}

fn write_sample_source(
    workspace_root: &std::path::Path,
    upstream_root: &std::path::Path,
    commit: &str,
    files: &[(&str, &str)],
) {
    write_reports(
        workspace_root,
        concat!(
            "status_dir = \".status\"\n",
            "artifact_dir = \".artifacts\"\n",
            "report_file = \"test-report.md\"\n",
            "\n",
            "[[report]]\n",
            "id = \"sample-report\"\n",
            "store_dir = \"sample-report\"\n",
            "sources = \"sample-report/sources.report.toml\"\n",
        ),
    );
    let mut source = format!(
        concat!(
            "[[source]]\n",
            "id = \"local-source\"\n",
            "git_url = {:?}\n",
            "git_rev = {:?}\n",
            "\n",
            "[[source.family]]\n",
            "id = \"family-a\"\n",
            "target_root = \"family-a\"\n",
            "sparse_paths = [\"roms/family-a\"]\n",
        ),
        upstream_root.display().to_string(),
        commit
    );
    for (target, hash) in files {
        source.push_str(&format!(
            concat!(
                "\n",
                "[[source.family.file]]\n",
                "path = \"roms/family-a/{}\"\n",
                "target = \"{}\"\n",
                "sha256 = {:?}\n",
            ),
            target, target, hash
        ));
    }
    write_source_manifest(workspace_root, "sample-report/sources.report.toml", &source);
}
