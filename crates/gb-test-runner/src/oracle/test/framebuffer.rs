use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;

use super::super::{
    FramebufferObservation, Oracle, OracleConfig, OracleObservations, OracleOutcome, OracleStep,
};

const PIXELS: usize = 160 * 144;

#[derive(Debug, Deserialize)]
struct OracleWrapper {
    oracle: OracleConfig,
}

fn parse_oracle_config(text: &str) -> OracleConfig {
    toml::from_str::<OracleWrapper>(text)
        .expect("oracle config should parse")
        .oracle
}

fn temp_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "gb-cycle-framebuffer-oracle-{label}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ))
}

fn write_pgm(path: &Path, pixels: &[u8]) {
    let mut bytes = b"P5\n160 144\n255\n".to_vec();
    bytes.extend_from_slice(pixels);
    fs::write(path, bytes).expect("PGM fixture should be writable");
}

fn observations<'a>(
    executed_tcycles: u64,
    dmg: Option<&'a [u8]>,
    cgb_rgb555: Option<&'a [u16]>,
    in_vblank: bool,
) -> OracleObservations<'a> {
    OracleObservations {
        serial: b"",
        cpu: None,
        executed_tcycles,
        framebuffer: FramebufferObservation {
            dmg,
            cgb_rgb555,
            in_vblank,
        },
        participants: &[],
    }
}

fn pass_oracle(path: &Path) -> Oracle {
    Oracle::from_manifest(&parse_oracle_config(&format!(
        "oracle = {{ type = \"framebuffer\", fixture = {:?} }}",
        path.to_string_lossy()
    )))
    .expect("framebuffer oracle should parse")
}

#[test]
fn framebuffer_defaults_compare_dmg_palette_rank_fixture() {
    let temp_dir = temp_dir("dmg-default");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
    let fixture_path = temp_dir.join("fixture.pgm");
    let mut fixture_pixels = vec![255; PIXELS];
    fixture_pixels[0..4].copy_from_slice(&[255, 170, 85, 0]);
    write_pgm(&fixture_path, &fixture_pixels);

    let mut actual = vec![0; PIXELS];
    actual[0..4].copy_from_slice(&[0, 1, 2, 3]);
    let mut oracle = pass_oracle(&fixture_path);

    assert_eq!(
        oracle
            .finish(observations(1, Some(&actual), None, true))
            .expect("oracle should finish"),
        OracleOutcome::Passed
    );

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn framebuffer_fixture_array_accepts_any_matching_fixture() {
    let temp_dir = temp_dir("fixture-array");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
    let mismatch_path = temp_dir.join("mismatch.pgm");
    let match_path = temp_dir.join("match.pgm");
    let mut mismatch_pixels = vec![255; PIXELS];
    mismatch_pixels[0] = 0;
    write_pgm(&mismatch_path, &mismatch_pixels);
    write_pgm(&match_path, &vec![255; PIXELS]);

    let mut actual = vec![0; PIXELS];
    actual.fill(0);
    let mut oracle = Oracle::from_manifest(&parse_oracle_config(&format!(
        "oracle = {{ type = \"framebuffer\", fixture = [{:?}, {:?}] }}",
        mismatch_path.to_string_lossy(),
        match_path.to_string_lossy()
    )))
    .expect("framebuffer oracle should parse");

    assert_eq!(
        oracle
            .finish(observations(1, Some(&actual), None, true))
            .expect("oracle should finish"),
        OracleOutcome::Passed
    );

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn framebuffer_cgb_grayscale_tolerance_compares_absolute_luma() {
    let temp_dir = temp_dir("cgb-tolerance");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
    let fixture_path = temp_dir.join("fixture.pgm");
    write_pgm(&fixture_path, &vec![250; PIXELS]);

    let rgb555 = vec![0x7FFF; PIXELS];
    let mut oracle = Oracle::from_manifest(&parse_oracle_config(&format!(
        concat!(
            "oracle = {{ type = \"framebuffer\", source = \"cgb\", projection = \"grayscale\", ",
            "compare = \"grayscale-tolerance\", fixture = {:?} }}"
        ),
        fixture_path.to_string_lossy()
    )))
    .expect("framebuffer oracle should parse");

    assert_eq!(
        oracle
            .finish(observations(1, None, Some(&rgb555), true))
            .expect("oracle should finish"),
        OracleOutcome::Passed
    );

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn framebuffer_reports_mismatch_as_failed_outcome() {
    let temp_dir = temp_dir("mismatch");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
    let fixture_path = temp_dir.join("fixture.pgm");
    write_pgm(&fixture_path, &vec![255; PIXELS]);

    let mut actual = vec![0; PIXELS];
    actual[0] = 3;
    let mut oracle = pass_oracle(&fixture_path);
    let outcome = oracle
        .finish(observations(1, Some(&actual), None, true))
        .expect("oracle should finish");

    assert!(matches!(outcome, OracleOutcome::Failed(message) if message.contains("did not match")));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn framebuffer_until_match_check_at_stops_on_exact_tcycle() {
    let temp_dir = temp_dir("check-at");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
    let fixture_path = temp_dir.join("fixture.pgm");
    write_pgm(&fixture_path, &vec![255; PIXELS]);

    let actual = vec![0; PIXELS];
    let mut oracle = Oracle::from_manifest(&parse_oracle_config(&format!(
        concat!(
            "oracle = {{ type = \"framebuffer\", mode = \"until-match\", ",
            "check_at_tcycles = 3, fixture = {:?} }}"
        ),
        fixture_path.to_string_lossy()
    )))
    .expect("framebuffer oracle should parse");

    assert_eq!(
        oracle
            .observe(observations(2, Some(&actual), None, true))
            .expect("oracle should observe"),
        OracleStep::Continue
    );
    assert_eq!(
        oracle
            .observe(observations(3, Some(&actual), None, true))
            .expect("oracle should observe"),
        OracleStep::Stop
    );
    assert_eq!(
        oracle
            .finish(observations(3, Some(&actual), None, true))
            .expect("oracle should finish"),
        OracleOutcome::Passed
    );

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn framebuffer_until_match_interval_waits_for_vblank() {
    let temp_dir = temp_dir("interval");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
    let fixture_path = temp_dir.join("fixture.pgm");
    write_pgm(&fixture_path, &vec![255; PIXELS]);

    let actual = vec![0; PIXELS];
    let mut oracle = Oracle::from_manifest(&parse_oracle_config(&format!(
        concat!(
            "oracle = {{ type = \"framebuffer\", mode = \"until-match\", ",
            "check_interval_tcycles = 2, fixture = {:?} }}"
        ),
        fixture_path.to_string_lossy()
    )))
    .expect("framebuffer oracle should parse");

    assert_eq!(
        oracle
            .observe(observations(2, Some(&actual), None, false))
            .expect("oracle should observe"),
        OracleStep::Continue
    );
    assert_eq!(
        oracle
            .observe(observations(3, Some(&actual), None, true))
            .expect("oracle should observe"),
        OracleStep::Stop
    );

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn framebuffer_info_mode_is_successful_without_fixture() {
    let mut oracle = Oracle::from_manifest(&parse_oracle_config(
        "oracle = { type = \"framebuffer\", mode = \"info\" }",
    ))
    .expect("framebuffer info oracle should parse");

    assert_eq!(
        oracle
            .observe(observations(1, None, None, false))
            .expect("oracle should observe"),
        OracleStep::Continue
    );
    assert_eq!(
        oracle
            .finish(observations(1, None, None, false))
            .expect("oracle should finish"),
        OracleOutcome::Passed
    );
}

#[test]
fn framebuffer_rejects_invalid_parameter_combinations() {
    assert!(
        Oracle::from_manifest(&parse_oracle_config("oracle = { type = \"framebuffer\" }"))
            .expect_err("fixture should be required")
            .contains("requires fixture")
    );
    assert!(
        Oracle::from_manifest(&parse_oracle_config(
            "oracle = { type = \"framebuffer\", mode = \"info\", fixture = \"unused.png\" }"
        ))
        .expect_err("info fixture should fail")
        .contains("does not use fixture")
    );
    assert!(
        Oracle::from_manifest(&parse_oracle_config(
            "oracle = { type = \"framebuffer\", compare = \"grayscale-tolerance\", fixture = \"missing.pgm\" }"
        ))
        .expect_err("tolerance compare needs grayscale projection")
        .contains("requires projection")
    );
    assert!(
        Oracle::from_manifest(&parse_oracle_config(
            "oracle = { type = \"framebuffer\", mode = \"final\", check_interval_tcycles = 1, fixture = \"missing.pgm\" }"
        ))
        .expect_err("check interval should require until-match")
        .contains("require mode")
    );
}
