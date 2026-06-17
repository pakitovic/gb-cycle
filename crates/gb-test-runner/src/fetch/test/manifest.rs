use std::fs;
use std::path::PathBuf;

use gb_core::BootRomAssetKind;

use super::super::boot_rom::{
    load_boot_rom_source_manifest_for_test, supported_boot_rom_asset_filenames_for_test,
};
use super::super::manifest::{
    SourceManifestFile, load_report_manifest, load_source_manifest, report_families,
};
use super::common::{unique_temp_dir, write_basic_reports, write_reports, write_source_manifest};

#[test]
fn built_in_reports_manifest_loads_all_reports() {
    let workspace_root = crate::default_workspace_root();
    let manifest =
        load_report_manifest(&workspace_root).expect("built-in reports manifest should load");
    let ids = manifest
        .reports
        .iter()
        .map(|report| report.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        ids,
        vec![
            "gb-emulator-shootout",
            "docboy",
            "gbmicrotest",
            "blargg",
            "mooneye",
            "little-things-gb",
            "nitro2k01",
            "magen",
            "mealybug-tearoom-tests",
            "samesuite",
            "wilbertpol",
            "rtc3test",
            "linked"
        ]
    );
    let gbmicrotest = manifest
        .reports
        .iter()
        .find(|report| report.id == "gbmicrotest")
        .expect("gbmicrotest report should exist");
    assert!(gbmicrotest.store_dir.ends_with("gbmicrotest"));
    assert_eq!(gbmicrotest.status_dir, PathBuf::from(".status"));
    assert_eq!(gbmicrotest.artifact_dir, PathBuf::from(".artifacts"));
    assert_eq!(gbmicrotest.report_file, PathBuf::from("test-report.md"));
    assert_eq!(gbmicrotest.family_order, None);
    let linked = manifest
        .reports
        .iter()
        .find(|report| report.id == "linked")
        .expect("linked report should exist");
    assert!(linked.local);
    assert_eq!(linked.store_dir, PathBuf::from("linked"));
    assert_eq!(linked.sources, None);
    assert_eq!(linked.status_dir, PathBuf::from(".status"));
    assert_eq!(linked.artifact_dir, PathBuf::from(".artifacts"));
}

#[test]
fn built_in_boot_rom_source_manifest_matches_supported_assets() {
    let workspace_root = crate::default_workspace_root();
    let manifest = load_boot_rom_source_manifest_for_test(&workspace_root)
        .expect("built-in boot ROM source manifest should load");
    let source = manifest
        .sources
        .first()
        .expect("boot ROM manifest should define a source");
    assert_eq!(
        source.file_base_url.as_deref(),
        Some("https://gbdev.gg8.se/files/roms/bootroms/")
    );
    let expected = supported_boot_rom_asset_filenames_for_test();
    let actual = source
        .families
        .iter()
        .flat_map(|family| &family.files)
        .map(|file| file.target.to_string_lossy().to_string())
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    for file in source.families.iter().flat_map(|family| &family.files) {
        assert_eq!(file.path, file.target);
        let asset = boot_rom_asset_for_filename(&file.target.to_string_lossy())
            .expect("boot ROM target should map to an asset");
        assert_eq!(file.size, Some(asset.expected_size() as u64));
        assert_eq!(file.sha256, asset.expected_sha256());
    }
}

fn boot_rom_asset_for_filename(filename: &str) -> Option<BootRomAssetKind> {
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
    .into_iter()
    .find(|asset| asset.filename() == filename)
}

#[test]
fn report_manifest_applies_global_defaults_and_report_overrides() {
    let workspace_root = unique_temp_dir("report-defaults");
    write_reports(
        &workspace_root,
        concat!(
            "status_dir = \".status\"\n",
            "artifact_dir = \".artifacts\"\n",
            "report_file = \"test-report.md\"\n",
            "\n",
            "[[report]]\n",
            "id = \"defaulted\"\n",
            "store_dir = \"defaulted\"\n",
            "sources = \"defaulted/sources.report.toml\"\n",
            "\n",
            "[[report]]\n",
            "id = \"overridden\"\n",
            "store_dir = \"overridden\"\n",
            "sources = \"overridden/sources.report.toml\"\n",
            "status_dir = \".state\"\n",
            "report_file = \"custom-report.md\"\n",
        ),
    );

    let manifest = load_report_manifest(&workspace_root).expect("report manifest should load");
    let defaulted = manifest
        .reports
        .iter()
        .find(|report| report.id == "defaulted")
        .expect("defaulted report should exist");
    assert_eq!(defaulted.status_dir, PathBuf::from(".status"));
    assert_eq!(defaulted.artifact_dir, PathBuf::from(".artifacts"));
    assert_eq!(defaulted.report_file, PathBuf::from("test-report.md"));
    let overridden = manifest
        .reports
        .iter()
        .find(|report| report.id == "overridden")
        .expect("overridden report should exist");
    assert_eq!(overridden.status_dir, PathBuf::from(".state"));
    assert_eq!(overridden.artifact_dir, PathBuf::from(".artifacts"));
    assert_eq!(overridden.report_file, PathBuf::from("custom-report.md"));

    let _ = fs::remove_dir_all(workspace_root);
}

#[test]
fn report_manifest_loads_local_report_without_sources() {
    let workspace_root = unique_temp_dir("local-report");
    write_reports(
        &workspace_root,
        concat!(
            "status_dir = \".status\"\n",
            "artifact_dir = \".artifacts\"\n",
            "report_file = \"test-report.md\"\n",
            "\n",
            "[[report]]\n",
            "id = \"linked\"\n",
            "local = true\n",
            "store_dir = \"linked\"\n",
        ),
    );

    let manifest = load_report_manifest(&workspace_root).expect("local report should load");
    let linked = manifest
        .reports
        .first()
        .expect("linked report should exist");
    assert_eq!(linked.id, "linked");
    assert!(linked.local);
    assert_eq!(linked.sources, None);

    let _ = fs::remove_dir_all(workspace_root);
}

#[test]
fn report_manifest_rejects_non_local_report_without_sources() {
    let workspace_root = unique_temp_dir("missing-sources-report");
    write_reports(
        &workspace_root,
        concat!(
            "status_dir = \".status\"\n",
            "artifact_dir = \".artifacts\"\n",
            "report_file = \"test-report.md\"\n",
            "\n",
            "[[report]]\n",
            "id = \"missing-sources\"\n",
            "store_dir = \"missing-sources\"\n",
        ),
    );

    assert!(
        load_report_manifest(&workspace_root)
            .expect_err("non-local report without sources should fail")
            .contains("must define sources unless local = true")
    );

    let _ = fs::remove_dir_all(workspace_root);
}

#[test]
fn report_manifest_rejects_local_report_with_sources() {
    let workspace_root = unique_temp_dir("local-report-with-sources");
    write_reports(
        &workspace_root,
        concat!(
            "status_dir = \".status\"\n",
            "artifact_dir = \".artifacts\"\n",
            "report_file = \"test-report.md\"\n",
            "\n",
            "[[report]]\n",
            "id = \"linked\"\n",
            "local = true\n",
            "store_dir = \"linked\"\n",
            "sources = \"linked/sources.report.toml\"\n",
        ),
    );

    assert!(
        load_report_manifest(&workspace_root)
            .expect_err("local report with sources should fail")
            .contains("must not define sources")
    );

    let _ = fs::remove_dir_all(workspace_root);
}

#[test]
fn built_in_source_manifests_load_for_each_report() {
    let workspace_root = crate::default_workspace_root();
    let manifest =
        load_report_manifest(&workspace_root).expect("built-in reports manifest should load");
    for report in &manifest.reports {
        if report.local {
            continue;
        }
        let source_manifest = load_source_manifest(&workspace_root, report)
            .unwrap_or_else(|error| panic!("{} should load: {error}", report.id));
        report_families(report, &source_manifest)
            .unwrap_or_else(|error| panic!("{} family list should resolve: {error}", report.id));
        assert!(
            !source_manifest.sources.is_empty(),
            "{} should define at least one source",
            report.id
        );
    }
}

#[test]
fn invalid_report_manifest_rejects_parent_paths() {
    let workspace_root = unique_temp_dir("bad-report");
    write_reports(
        &workspace_root,
        concat!(
            "status_dir = \".status\"\n",
            "artifact_dir = \".artifacts\"\n",
            "report_file = \"test-report.md\"\n",
            "\n",
            "[[report]]\n",
            "id = \"bad\"\n",
            "store_dir = \"../bad\"\n",
            "sources = \"sources.report.toml\"\n",
            "family_order = [\"family\"]\n",
        ),
    );
    assert!(
        load_report_manifest(&workspace_root)
            .expect_err("parent path should fail")
            .contains("parent components")
    );
    let _ = fs::remove_dir_all(workspace_root);
}

#[test]
fn source_manifest_rejects_missing_family_data() {
    let workspace_root = unique_temp_dir("bad-source");
    write_basic_reports(&workspace_root, "sources.report.toml");
    write_source_manifest(
        &workspace_root,
        "sources.report.toml",
        concat!(
            "[[source]]\n",
            "id = \"source\"\n",
            "git_url = \"file:///unused\"\n",
            "git_rev = \"rev\"\n",
            "\n",
            "[[source.family]]\n",
            "id = \"family-a\"\n",
            "target_root = \"family-a\"\n",
            "sparse_paths = []\n",
        ),
    );
    let reports = load_report_manifest(&workspace_root).expect("report should load");
    let report = reports.reports.first().expect("report should exist");
    assert!(
        load_source_manifest(&workspace_root, report)
            .expect_err("empty sparse paths should fail")
            .contains("must define sparse_paths")
    );
    let _ = fs::remove_dir_all(workspace_root);
}

#[test]
fn source_manifest_accepts_zip_archive_without_sparse_paths() {
    let workspace_root = unique_temp_dir("archive-source");
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
            "sources = \"sources.report.toml\"\n",
        ),
    );
    write_source_manifest(
        &workspace_root,
        "sources.report.toml",
        concat!(
            "[[source]]\n",
            "id = \"archive-source\"\n",
            "archive_url = \"https://example.invalid/test-roms.zip\"\n",
            "archive_sha256 = \"1111111111111111111111111111111111111111111111111111111111111111\"\n",
            "archive_format = \"zip\"\n",
            "\n",
            "[[source.family]]\n",
            "id = \"family-a\"\n",
            "target_root = \"family-a\"\n",
            "\n",
            "[[source.family.file]]\n",
            "path = \"roms/family-a/test.gb\"\n",
            "target = \"test.gb\"\n",
            "sha256 = \"0000000000000000000000000000000000000000000000000000000000000000\"\n",
        ),
    );
    let reports = load_report_manifest(&workspace_root).expect("report should load");
    let report = reports.reports.first().expect("report should exist");
    let source_manifest =
        load_source_manifest(&workspace_root, report).expect("archive source should load");
    assert_eq!(
        report_families(report, &source_manifest).expect("families should resolve"),
        vec!["family-a"]
    );
    let _ = fs::remove_dir_all(workspace_root);
}

#[test]
fn source_manifest_accepts_file_base_url_without_sparse_paths() {
    let workspace_root = unique_temp_dir("file-base-source");
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
            "sources = \"sources.report.toml\"\n",
        ),
    );
    write_source_manifest(
        &workspace_root,
        "sources.report.toml",
        concat!(
            "[[source]]\n",
            "id = \"file-base-source\"\n",
            "file_base_url = \"https://example.invalid/test-roms\"\n",
            "\n",
            "[[source.family]]\n",
            "id = \"family-a\"\n",
            "target_root = \"family-a\"\n",
            "\n",
            "[[source.family.file]]\n",
            "path = \"roms/family-a/test.gb\"\n",
            "target = \"test.gb\"\n",
            "sha256 = \"0000000000000000000000000000000000000000000000000000000000000000\"\n",
        ),
    );
    let reports = load_report_manifest(&workspace_root).expect("report should load");
    let report = reports.reports.first().expect("report should exist");
    let source_manifest =
        load_source_manifest(&workspace_root, report).expect("file base source should load");
    assert_eq!(
        report_families(report, &source_manifest).expect("families should resolve"),
        vec!["family-a"]
    );
    let _ = fs::remove_dir_all(workspace_root);
}

#[test]
fn source_manifest_rejects_mixed_git_and_archive_location() {
    let workspace_root = unique_temp_dir("mixed-source");
    write_basic_reports(&workspace_root, "sources.report.toml");
    write_source_manifest(
        &workspace_root,
        "sources.report.toml",
        concat!(
            "[[source]]\n",
            "id = \"source\"\n",
            "git_url = \"file:///unused\"\n",
            "git_rev = \"rev\"\n",
            "archive_url = \"https://example.invalid/test-roms.zip\"\n",
            "archive_sha256 = \"1111111111111111111111111111111111111111111111111111111111111111\"\n",
            "archive_format = \"zip\"\n",
            "\n",
            "[[source.family]]\n",
            "id = \"family-a\"\n",
            "target_root = \"family-a\"\n",
            "sparse_paths = [\"roms/family-a\"]\n",
            "\n",
            "[[source.family.file]]\n",
            "path = \"roms/family-a/test.gb\"\n",
            "target = \"test.gb\"\n",
            "sha256 = \"0000000000000000000000000000000000000000000000000000000000000000\"\n",
        ),
    );
    let reports = load_report_manifest(&workspace_root).expect("report should load");
    let report = reports.reports.first().expect("report should exist");
    assert!(
        load_source_manifest(&workspace_root, report)
            .expect_err("mixed source location should fail")
            .contains("must define exactly one fetch location")
    );
    let _ = fs::remove_dir_all(workspace_root);
}

#[test]
fn source_manifest_rejects_mixed_file_base_and_archive_location() {
    let workspace_root = unique_temp_dir("mixed-file-base-source");
    write_basic_reports(&workspace_root, "sources.report.toml");
    write_source_manifest(
        &workspace_root,
        "sources.report.toml",
        concat!(
            "[[source]]\n",
            "id = \"source\"\n",
            "file_base_url = \"https://example.invalid/test-roms\"\n",
            "archive_url = \"https://example.invalid/test-roms.zip\"\n",
            "archive_sha256 = \"1111111111111111111111111111111111111111111111111111111111111111\"\n",
            "archive_format = \"zip\"\n",
            "\n",
            "[[source.family]]\n",
            "id = \"family-a\"\n",
            "target_root = \"family-a\"\n",
            "\n",
            "[[source.family.file]]\n",
            "path = \"roms/family-a/test.gb\"\n",
            "target = \"test.gb\"\n",
            "sha256 = \"0000000000000000000000000000000000000000000000000000000000000000\"\n",
        ),
    );
    let reports = load_report_manifest(&workspace_root).expect("report should load");
    let report = reports.reports.first().expect("report should exist");
    assert!(
        load_source_manifest(&workspace_root, report)
            .expect_err("mixed source location should fail")
            .contains("must define exactly one fetch location")
    );
    let _ = fs::remove_dir_all(workspace_root);
}

#[test]
fn source_manifest_rejects_bad_archive_sha256() {
    let workspace_root = unique_temp_dir("bad-archive-hash");
    write_basic_reports(&workspace_root, "sources.report.toml");
    write_source_manifest(
        &workspace_root,
        "sources.report.toml",
        concat!(
            "[[source]]\n",
            "id = \"archive-source\"\n",
            "archive_url = \"https://example.invalid/test-roms.zip\"\n",
            "archive_sha256 = \"not-a-sha\"\n",
            "archive_format = \"zip\"\n",
            "\n",
            "[[source.family]]\n",
            "id = \"family-a\"\n",
            "target_root = \"family-a\"\n",
            "\n",
            "[[source.family.file]]\n",
            "path = \"roms/family-a/test.gb\"\n",
            "target = \"test.gb\"\n",
            "sha256 = \"0000000000000000000000000000000000000000000000000000000000000000\"\n",
        ),
    );
    let reports = load_report_manifest(&workspace_root).expect("report should load");
    let report = reports.reports.first().expect("report should exist");
    assert!(
        load_source_manifest(&workspace_root, report)
            .expect_err("invalid archive hash should fail")
            .contains("invalid archive_sha256")
    );
    let _ = fs::remove_dir_all(workspace_root);
}

#[test]
fn can_parse_minimal_nested_source_manifest() {
    let manifest: SourceManifestFile = toml::from_str(concat!(
        "[[source]]\n",
        "id = \"source\"\n",
        "git_url = \"file:///unused\"\n",
        "git_rev = \"rev\"\n",
        "\n",
        "[[source.family]]\n",
        "id = \"family-a\"\n",
        "target_root = \"family-a\"\n",
        "sparse_paths = [\"roms/family-a\"]\n",
        "\n",
        "[[source.family.file]]\n",
        "path = \"roms/family-a/test.gb\"\n",
        "target = \"test.gb\"\n",
        "sha256 = \"0000000000000000000000000000000000000000000000000000000000000000\"\n",
    ))
    .expect("nested source manifest should parse");
    assert_eq!(manifest.sources.len(), 1);
    assert_eq!(manifest.sources[0].families.len(), 1);
    assert_eq!(manifest.sources[0].families[0].files.len(), 1);
}
