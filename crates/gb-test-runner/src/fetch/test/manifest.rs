use std::fs;
use std::path::PathBuf;

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
    assert_eq!(ids, vec!["gb-emulator-shootout", "docboy", "gbmicrotest"]);
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
fn built_in_source_manifests_load_for_each_report() {
    let workspace_root = crate::default_workspace_root();
    let manifest =
        load_report_manifest(&workspace_root).expect("built-in reports manifest should load");
    for report in &manifest.reports {
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
