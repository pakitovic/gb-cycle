use std::path::PathBuf;

use super::super::manifest::{
    Report, Source, SourceFamily, SourceFile, SourceManifestFile, report_families, select_families,
};
use super::common::basic_report;

#[test]
fn empty_selection_selects_all_report_families() {
    let report = basic_report();
    let available_families = vec!["family-a".to_string(), "family-b".to_string()];
    assert_eq!(
        select_families(&report, &available_families, &[]).expect("families should select"),
        vec!["family-a", "family-b"]
    );
}

#[test]
fn selects_requested_families_in_report_order() {
    let report = basic_report();
    let available_families = vec!["family-a".to_string(), "family-b".to_string()];
    assert_eq!(
        select_families(
            &report,
            &available_families,
            &["family-b".to_string(), "family-a".to_string()]
        )
        .expect("families should select"),
        vec!["family-a", "family-b"]
    );
}

#[test]
fn derives_report_families_alphabetically_when_family_order_is_omitted() {
    let report = Report {
        family_order: None,
        ..basic_report()
    };
    let source_manifest = SourceManifestFile {
        sources: vec![Source {
            id: "source".to_string(),
            git_url: "file:///unused".to_string(),
            git_rev: "rev".to_string(),
            families: vec![
                SourceFamily {
                    id: "family-b".to_string(),
                    target_root: PathBuf::from("family-b"),
                    sparse_paths: vec![PathBuf::from("roms/family-b")],
                    files: vec![SourceFile {
                        path: PathBuf::from("roms/family-b/test.gb"),
                        target: PathBuf::from("test.gb"),
                        sha256: "0".repeat(64),
                    }],
                },
                SourceFamily {
                    id: "family-a".to_string(),
                    target_root: PathBuf::from("family-a"),
                    sparse_paths: vec![PathBuf::from("roms/family-a")],
                    files: vec![SourceFile {
                        path: PathBuf::from("roms/family-a/test.gb"),
                        target: PathBuf::from("test.gb"),
                        sha256: "1".repeat(64),
                    }],
                },
            ],
        }],
    };

    assert_eq!(
        report_families(&report, &source_manifest).expect("families should derive"),
        vec!["family-a", "family-b"]
    );
}
