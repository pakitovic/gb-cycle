use std::fs;
use std::io::Write;
use std::path::PathBuf;

use gb_core::BootRomAssetKind;

use super::super::git::sha256_hex;
use super::super::manifest::{Source, SourceFamily, SourceFile};
use super::super::run_fetch_command;
use super::super::validate::validate_materialization_targets;
use super::common::{
    basic_report, commit_upstream_repo, unique_temp_dir, write_basic_reports, write_reports,
    write_source_manifest,
};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

#[test]
fn duplicate_targets_are_rejected() {
    let report = basic_report();
    let source = Source {
        id: "source".to_string(),
        git_url: Some("file:///unused".to_string()),
        git_rev: Some("rev".to_string()),
        file_base_url: None,
        archive_url: None,
        archive_sha256: None,
        archive_format: None,
        families: vec![SourceFamily {
            id: "family-a".to_string(),
            target_root: PathBuf::from("family-a"),
            sparse_paths: vec![PathBuf::from("roms")],
            files: vec![
                SourceFile {
                    path: PathBuf::from("roms/a.gb"),
                    target: PathBuf::from("same.gb"),
                    size: None,
                    sha256: "0".repeat(64),
                },
                SourceFile {
                    path: PathBuf::from("roms/b.gb"),
                    target: PathBuf::from("same.gb"),
                    size: None,
                    sha256: "1".repeat(64),
                },
            ],
        }],
    };
    assert!(
        validate_materialization_targets(&report, &[source])
            .expect_err("duplicate targets should fail")
            .contains("duplicate materialization target")
    );
}

#[test]
fn materializes_selected_family_from_local_git_source() {
    let workspace_root = unique_temp_dir("workspace");
    let upstream_root = unique_temp_dir("upstream");
    fs::create_dir_all(upstream_root.join("roms/family-a"))
        .expect("upstream family directory should be creatable");
    fs::write(upstream_root.join("roms/family-a/test.gb"), b"rom bytes")
        .expect("upstream ROM should be writable");
    fs::write(upstream_root.join("roms/family-a/other.gb"), b"other bytes")
        .expect("upstream ROM should be writable");
    let commit = commit_upstream_repo(&upstream_root);
    let hash = sha256_hex(b"rom bytes");

    write_basic_reports(&workspace_root, "report/sources.report.toml");
    write_source_manifest(
        &workspace_root,
        "report/sources.report.toml",
        &format!(
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
                "\n",
                "[[source.family.file]]\n",
                "path = \"roms/family-a/test.gb\"\n",
                "target = \"test.gb\"\n",
                "sha256 = {:?}\n",
                "\n",
                "[[source.family]]\n",
                "id = \"family-b\"\n",
                "target_root = \"family-b\"\n",
                "sparse_paths = [\"roms/family-b\"]\n",
                "\n",
                "[[source.family.file]]\n",
                "path = \"roms/family-b/test.gb\"\n",
                "target = \"test.gb\"\n",
                "sha256 = \"0000000000000000000000000000000000000000000000000000000000000000\"\n",
            ),
            upstream_root.display().to_string(),
            commit,
            hash,
        ),
    );
    let store_root = workspace_root.join("test/sample-report");
    fs::create_dir_all(store_root.join(".status")).expect("status dir should be creatable");
    fs::write(store_root.join(".status/family-a.toml"), b"status")
        .expect("status should be writable");
    fs::create_dir_all(store_root.join("family-a")).expect("family root should be creatable");
    fs::write(store_root.join("family-a/stale.gb"), b"stale")
        .expect("stale ROM should be writable");

    let mut output = Vec::new();
    run_fetch_command(["sample-report", "family-a"], &workspace_root, &mut output)
        .expect("fetch should materialize selected family");

    assert_eq!(
        fs::read(store_root.join("family-a/test.gb")).expect("materialized ROM should exist"),
        b"rom bytes"
    );
    assert!(
        !store_root.join("family-a/stale.gb").exists(),
        "selected family root should be replaced"
    );
    assert_eq!(
        fs::read(store_root.join(".status/family-a.toml")).expect("status should be preserved"),
        b"status"
    );
    let output = String::from_utf8(output).expect("output should be utf-8");
    assert!(output.contains("materialized test ROM families family-a"));

    let _ = fs::remove_dir_all(workspace_root);
    let _ = fs::remove_dir_all(upstream_root);
}

#[test]
fn materializes_selected_family_from_local_zip_archive_source() {
    let workspace_root = unique_temp_dir("archive-workspace");
    let archive_root = unique_temp_dir("archive-source");
    let archive_path = archive_root.join("test-roms.zip");
    fs::create_dir_all(&archive_root).expect("archive root should be creatable");
    write_test_zip(
        &archive_path,
        &[
            ("roms/family-a/test.gb", b"rom bytes".as_slice()),
            ("roms/family-a/not-declared.gb", b"not declared".as_slice()),
            ("roms/family-b/test.gb", b"other family".as_slice()),
        ],
    );
    let archive_hash =
        sha256_hex(&fs::read(&archive_path).expect("archive should be readable for hash"))
            .to_uppercase();
    let rom_hash = sha256_hex(b"rom bytes").to_uppercase();

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
            "sources = \"report/sources.report.toml\"\n",
        ),
    );
    write_source_manifest(
        &workspace_root,
        "report/sources.report.toml",
        &format!(
            concat!(
                "[[source]]\n",
                "id = \"archive-source\"\n",
                "archive_url = {:?}\n",
                "archive_sha256 = {:?}\n",
                "archive_format = \"zip\"\n",
                "\n",
                "[[source.family]]\n",
                "id = \"family-a\"\n",
                "target_root = \"family-a\"\n",
                "\n",
                "[[source.family.file]]\n",
                "path = \"roms/family-a/test.gb\"\n",
                "target = \"test.gb\"\n",
                "sha256 = {:?}\n",
                "\n",
                "[[source.family]]\n",
                "id = \"family-b\"\n",
                "target_root = \"family-b\"\n",
                "\n",
                "[[source.family.file]]\n",
                "path = \"roms/family-b/test.gb\"\n",
                "target = \"test.gb\"\n",
                "sha256 = \"0000000000000000000000000000000000000000000000000000000000000000\"\n",
            ),
            format!("file://{}", archive_path.display()),
            archive_hash,
            rom_hash,
        ),
    );

    let store_root = workspace_root.join("test/sample-report");
    fs::create_dir_all(store_root.join("family-a")).expect("family root should be creatable");
    fs::write(store_root.join("family-a/stale.gb"), b"stale")
        .expect("stale ROM should be writable");

    let mut output = Vec::new();
    run_fetch_command(["sample-report", "family-a"], &workspace_root, &mut output)
        .expect("archive fetch should materialize selected family");

    assert_eq!(
        fs::read(store_root.join("family-a/test.gb")).expect("materialized ROM should exist"),
        b"rom bytes"
    );
    assert!(
        !store_root.join("family-a/not-declared.gb").exists(),
        "archive extraction should only materialize declared files"
    );
    assert!(
        !store_root.join("family-b/test.gb").exists(),
        "unselected archive families should not be materialized"
    );
    assert!(
        !store_root.join("family-a/stale.gb").exists(),
        "selected family root should be replaced"
    );

    let _ = fs::remove_dir_all(workspace_root);
    let _ = fs::remove_dir_all(archive_root);
}

#[test]
fn materializes_selected_family_from_local_file_base_source() {
    let workspace_root = unique_temp_dir("file-base-workspace");
    let upstream_root = unique_temp_dir("file-base-source");
    fs::create_dir_all(upstream_root.join("release/family-a"))
        .expect("upstream family directory should be creatable");
    fs::create_dir_all(upstream_root.join("release/family-b"))
        .expect("upstream family directory should be creatable");
    fs::write(upstream_root.join("release/family-a/test.gb"), b"rom bytes")
        .expect("upstream ROM should be writable");
    fs::write(
        upstream_root.join("release/family-b/test.gb"),
        b"other family",
    )
    .expect("upstream ROM should be writable");
    let rom_hash = sha256_hex(b"rom bytes");

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
            "sources = \"report/sources.report.toml\"\n",
        ),
    );
    write_source_manifest(
        &workspace_root,
        "report/sources.report.toml",
        &format!(
            concat!(
                "[[source]]\n",
                "id = \"file-base-source\"\n",
                "file_base_url = {:?}\n",
                "\n",
                "[[source.family]]\n",
                "id = \"family-a\"\n",
                "target_root = \"family-a\"\n",
                "\n",
                "[[source.family.file]]\n",
                "path = \"family-a/test.gb\"\n",
                "target = \"test.gb\"\n",
                "sha256 = {:?}\n",
                "\n",
                "[[source.family]]\n",
                "id = \"family-b\"\n",
                "target_root = \"family-b\"\n",
                "\n",
                "[[source.family.file]]\n",
                "path = \"family-b/test.gb\"\n",
                "target = \"test.gb\"\n",
                "sha256 = \"0000000000000000000000000000000000000000000000000000000000000000\"\n",
            ),
            format!("file://{}", upstream_root.join("release").display()),
            rom_hash,
        ),
    );

    let store_root = workspace_root.join("test/sample-report");
    fs::create_dir_all(store_root.join("family-a")).expect("family root should be creatable");
    fs::write(store_root.join("family-a/stale.gb"), b"stale")
        .expect("stale ROM should be writable");

    let mut output = Vec::new();
    run_fetch_command(["sample-report", "family-a"], &workspace_root, &mut output)
        .expect("file base fetch should materialize selected family");

    assert_eq!(
        fs::read(store_root.join("family-a/test.gb")).expect("materialized ROM should exist"),
        b"rom bytes"
    );
    assert!(
        !store_root.join("family-b/test.gb").exists(),
        "unselected file base families should not be materialized"
    );
    assert!(
        !store_root.join("family-a/stale.gb").exists(),
        "selected family root should be replaced"
    );

    let _ = fs::remove_dir_all(workspace_root);
    let _ = fs::remove_dir_all(upstream_root);
}

#[test]
fn archive_source_fetch_rejects_archive_hash_mismatch() {
    let workspace_root = unique_temp_dir("archive-mismatch-workspace");
    let archive_root = unique_temp_dir("archive-mismatch-source");
    let archive_path = archive_root.join("test-roms.zip");
    fs::create_dir_all(&archive_root).expect("archive root should be creatable");
    write_test_zip(&archive_path, &[("roms/family-a/test.gb", b"rom bytes")]);
    let rom_hash = sha256_hex(b"rom bytes");

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
            "sources = \"report/sources.report.toml\"\n",
        ),
    );
    write_source_manifest(
        &workspace_root,
        "report/sources.report.toml",
        &format!(
            concat!(
                "[[source]]\n",
                "id = \"archive-source\"\n",
                "archive_url = {:?}\n",
                "archive_sha256 = \"0000000000000000000000000000000000000000000000000000000000000000\"\n",
                "archive_format = \"zip\"\n",
                "\n",
                "[[source.family]]\n",
                "id = \"family-a\"\n",
                "target_root = \"family-a\"\n",
                "\n",
                "[[source.family.file]]\n",
                "path = \"roms/family-a/test.gb\"\n",
                "target = \"test.gb\"\n",
                "sha256 = {:?}\n",
            ),
            format!("file://{}", archive_path.display()),
            rom_hash,
        ),
    );

    let mut output = Vec::new();
    let error = run_fetch_command(["sample-report"], &workspace_root, &mut output)
        .expect_err("archive hash mismatch should fail");
    assert!(error.contains("archive hash mismatch"));
    assert!(
        !workspace_root
            .join("test/sample-report/family-a/test.gb")
            .exists()
    );

    let _ = fs::remove_dir_all(workspace_root);
    let _ = fs::remove_dir_all(archive_root);
}

#[test]
fn materializes_all_report_families_when_selection_is_omitted() {
    let workspace_root = unique_temp_dir("workspace-all");
    let upstream_root = unique_temp_dir("upstream-all");
    fs::create_dir_all(upstream_root.join("roms/family-a"))
        .expect("upstream family directory should be creatable");
    fs::create_dir_all(upstream_root.join("roms/family-b"))
        .expect("upstream family directory should be creatable");
    fs::write(upstream_root.join("roms/family-a/test.gb"), b"rom a")
        .expect("upstream ROM should be writable");
    fs::write(upstream_root.join("roms/family-b/test.gb"), b"rom b")
        .expect("upstream ROM should be writable");
    let commit = commit_upstream_repo(&upstream_root);
    let hash_a = sha256_hex(b"rom a");
    let hash_b = sha256_hex(b"rom b");

    write_basic_reports(&workspace_root, "report/sources.report.toml");
    write_source_manifest(
        &workspace_root,
        "report/sources.report.toml",
        &format!(
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
                "\n",
                "[[source.family.file]]\n",
                "path = \"roms/family-a/test.gb\"\n",
                "target = \"test.gb\"\n",
                "sha256 = {:?}\n",
                "\n",
                "[[source.family]]\n",
                "id = \"family-b\"\n",
                "target_root = \"family-b\"\n",
                "sparse_paths = [\"roms/family-b\"]\n",
                "\n",
                "[[source.family.file]]\n",
                "path = \"roms/family-b/test.gb\"\n",
                "target = \"test.gb\"\n",
                "sha256 = {:?}\n",
            ),
            upstream_root.display().to_string(),
            commit,
            hash_a,
            hash_b,
        ),
    );

    let mut output = Vec::new();
    run_fetch_command(["sample-report"], &workspace_root, &mut output)
        .expect("fetch should materialize all report families");

    let store_root = workspace_root.join("test/sample-report");
    assert_eq!(
        fs::read(store_root.join("family-a/test.gb")).expect("family-a ROM should exist"),
        b"rom a"
    );
    assert_eq!(
        fs::read(store_root.join("family-b/test.gb")).expect("family-b ROM should exist"),
        b"rom b"
    );
    let output = String::from_utf8(output).expect("output should be utf-8");
    assert!(output.contains("materialized test ROM families family-a, family-b"));

    let _ = fs::remove_dir_all(workspace_root);
    let _ = fs::remove_dir_all(upstream_root);
}

#[test]
fn flat_target_root_replaces_selected_first_components_only() {
    let workspace_root = unique_temp_dir("flat-workspace");
    let upstream_root = unique_temp_dir("flat-upstream");
    fs::create_dir_all(upstream_root.join("roms")).expect("upstream roms should be creatable");
    fs::write(upstream_root.join("roms/boot.gb"), b"boot")
        .expect("upstream ROM should be writable");
    let commit = commit_upstream_repo(&upstream_root);
    let hash = sha256_hex(b"boot");

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
            "sources = \"flat/sources.report.toml\"\n",
        ),
    );
    write_source_manifest(
        &workspace_root,
        "flat/sources.report.toml",
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
                "path = \"roms/boot.gb\"\n",
                "target = \"boot/boot.gb\"\n",
                "sha256 = {:?}\n",
            ),
            upstream_root.display().to_string(),
            commit,
            hash,
        ),
    );
    let store_root = workspace_root.join("test/flat-report");
    fs::create_dir_all(store_root.join(".artifacts")).expect("artifacts should be creatable");
    fs::write(store_root.join("test-report.md"), b"report").expect("report should be writable");
    fs::create_dir_all(store_root.join("boot")).expect("boot dir should be creatable");
    fs::write(store_root.join("boot/stale.gb"), b"stale").expect("stale ROM should be writable");

    let mut output = Vec::new();
    run_fetch_command(["flat-report", "flat-family"], &workspace_root, &mut output)
        .expect("flat fetch should materialize");

    assert_eq!(
        fs::read(store_root.join("boot/boot.gb")).expect("materialized ROM should exist"),
        b"boot"
    );
    assert!(!store_root.join("boot/stale.gb").exists());
    assert!(store_root.join(".artifacts").is_dir());
    assert_eq!(
        fs::read(store_root.join("test-report.md")).expect("report should be preserved"),
        b"report"
    );

    let _ = fs::remove_dir_all(workspace_root);
    let _ = fs::remove_dir_all(upstream_root);
}

#[test]
fn boot_rom_fetch_materializes_manifest_assets_and_preserves_unrelated_files() {
    let workspace_root = unique_temp_dir("boot-rom-fetch-workspace");
    let upstream_root = unique_temp_dir("boot-rom-fetch-upstream");
    let output_root = unique_temp_dir("boot-rom-fetch-output");
    fs::create_dir_all(&upstream_root).expect("upstream boot ROM dir should be creatable");
    fs::create_dir_all(&output_root).expect("output boot ROM dir should be creatable");
    fs::write(output_root.join("unrelated.txt"), b"keep")
        .expect("unrelated output file should be writable");
    fs::write(output_root.join("dmg_boot.bin"), b"stale")
        .expect("stale boot ROM should be writable");

    let manifest = boot_rom_source_manifest(&upstream_root, |asset, index| {
        vec![index as u8; asset.expected_size()]
    });
    write_source_manifest(&workspace_root, "sources.boot-rom.toml", &manifest);

    let mut output = Vec::new();
    run_fetch_command(
        [
            "--boot-rom",
            output_root.to_str().expect("output path should be UTF-8"),
        ],
        &workspace_root,
        &mut output,
    )
    .expect("boot ROM fetch should materialize assets");

    assert_eq!(
        fs::read(output_root.join("unrelated.txt")).expect("unrelated file should remain"),
        b"keep"
    );
    for (index, asset) in boot_rom_assets().into_iter().enumerate() {
        assert_eq!(
            fs::read(output_root.join(asset.filename()))
                .expect("boot ROM asset should be materialized"),
            vec![index as u8; asset.expected_size()]
        );
    }
    let output = String::from_utf8(output).expect("output should be UTF-8");
    assert!(output.contains("materialized 10 boot ROM assets"));

    let _ = fs::remove_dir_all(workspace_root);
    let _ = fs::remove_dir_all(upstream_root);
    let _ = fs::remove_dir_all(output_root);
}

#[test]
fn boot_rom_fetch_rejects_downloaded_size_mismatch_before_writing_output() {
    let workspace_root = unique_temp_dir("boot-rom-fetch-size-workspace");
    let upstream_root = unique_temp_dir("boot-rom-fetch-size-upstream");
    let output_root = unique_temp_dir("boot-rom-fetch-size-output");
    fs::create_dir_all(&upstream_root).expect("upstream boot ROM dir should be creatable");

    let manifest = boot_rom_source_manifest(&upstream_root, |asset, index| {
        if asset == BootRomAssetKind::Dmg {
            vec![index as u8; asset.expected_size() - 1]
        } else {
            vec![index as u8; asset.expected_size()]
        }
    });
    write_source_manifest(&workspace_root, "sources.boot-rom.toml", &manifest);

    let mut output = Vec::new();
    let error = run_fetch_command(
        [
            "--boot-rom",
            output_root.to_str().expect("output path should be UTF-8"),
        ],
        &workspace_root,
        &mut output,
    )
    .expect_err("boot ROM fetch should reject size mismatch");

    assert!(error.contains("size mismatch"));
    assert!(!output_root.exists());

    let _ = fs::remove_dir_all(workspace_root);
    let _ = fs::remove_dir_all(upstream_root);
}

fn boot_rom_source_manifest<F>(upstream_root: &std::path::Path, mut bytes_for: F) -> String
where
    F: FnMut(BootRomAssetKind, usize) -> Vec<u8>,
{
    let mut manifest = format!(
        concat!(
            "[[source]]\n",
            "id = \"local-boot-roms\"\n",
            "file_base_url = \"file://{}\"\n",
            "\n",
            "[[source.family]]\n",
            "id = \"boot-roms\"\n",
            "target_root = \"\"\n",
        ),
        upstream_root.display()
    );
    for (index, asset) in boot_rom_assets().into_iter().enumerate() {
        let bytes = bytes_for(asset, index);
        fs::write(upstream_root.join(asset.filename()), &bytes)
            .expect("upstream boot ROM asset should be writable");
        manifest.push_str(&format!(
            concat!(
                "\n",
                "[[source.family.file]]\n",
                "path = {:?}\n",
                "target = {:?}\n",
                "size = {}\n",
                "sha256 = {:?}\n",
            ),
            asset.filename(),
            asset.filename(),
            asset.expected_size(),
            sha256_hex(&bytes),
        ));
    }
    manifest
}

fn boot_rom_assets() -> [BootRomAssetKind; 10] {
    [
        BootRomAssetKind::Dmg0,
        BootRomAssetKind::Dmg,
        BootRomAssetKind::Mgb,
        BootRomAssetKind::Sgb,
        BootRomAssetKind::Sgb2,
        BootRomAssetKind::Cgb0,
        BootRomAssetKind::Cgb,
        BootRomAssetKind::CgbE,
        BootRomAssetKind::CgbAgb0,
        BootRomAssetKind::CgbAgb,
    ]
}

fn write_test_zip(path: &std::path::Path, entries: &[(&str, &[u8])]) {
    let file = fs::File::create(path).expect("zip file should be creatable");
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for (name, bytes) in entries {
        zip.start_file(*name, options)
            .expect("zip entry should start");
        zip.write_all(bytes).expect("zip entry should be writable");
    }
    zip.finish().expect("zip should finish");
}
