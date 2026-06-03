use std::path::PathBuf;

use super::super::args::{BenchAction, BenchRunOptions, parse_bench_arguments};

#[test]
fn parse_arguments_accepts_run_and_maintenance_modes() {
    assert_eq!(parse_bench_arguments(["--sample"]), Ok(BenchAction::Sample));
    assert_eq!(
        parse_bench_arguments(["cases", "--gb-cli", "--test", "one.toml"]),
        Ok(BenchAction::Run(BenchRunOptions {
            case_dir: Some(PathBuf::from("cases")),
            single_test: Some(PathBuf::from("one.toml")),
            include_cli: true,
        }))
    );
    assert_eq!(
        parse_bench_arguments(["cases", "--rom-dir", "roms", "--generate-cases"]),
        Ok(BenchAction::GenerateCases {
            case_dir: PathBuf::from("cases"),
            rom_dir: PathBuf::from("roms"),
            template_path: None,
        })
    );
}

#[test]
fn parse_arguments_rejects_invalid_combinations() {
    assert_eq!(
        parse_bench_arguments(["--sample", "cases"]).expect_err("sample combinations fail"),
        "--sample cannot be combined with benchmark run options"
    );
    assert_eq!(
        parse_bench_arguments(["cases", "--template", "case.toml"])
            .expect_err("template without generation fails"),
        "--template requires --generate-cases"
    );
    assert_eq!(
        parse_bench_arguments(["cases", "--rom-dir", "roms", "--gb-cli"])
            .expect_err("rom dir cannot combine with runs"),
        "--rom-dir cannot be combined with benchmark run options"
    );
}
