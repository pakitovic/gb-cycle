use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;

use super::super::{
    FramebufferObservation, LinkedParticipantObservation, LinkedSessionObservation, Oracle,
    OracleConfig, OracleFixtureRoots, OracleObservations, OracleOutcome, OracleStep,
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

fn write_rgb_png(path: &Path, pixels: &[[u8; 3]]) {
    let mut bytes = Vec::with_capacity(pixels.len() * 3);
    for pixel in pixels {
        bytes.extend_from_slice(pixel);
    }
    let file = fs::File::create(path).expect("RGB PNG fixture should be writable");
    let mut encoder = png::Encoder::new(file, 160, 144);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .expect("RGB PNG header should be writable");
    writer
        .write_image_data(&bytes)
        .expect("RGB PNG data should be writable");
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
        memory: &[],
        executed_tcycles,
        framebuffer: FramebufferObservation {
            dmg,
            cgb_rgb555,
            in_vblank,
        },
        participants: &[],
        linked: None,
    }
}

fn pass_oracle(path: &Path) -> Oracle {
    Oracle::from_manifest(&parse_oracle_config(&format!(
        "oracle = {{ type = \"framebuffer\", fixture = {:?} }}",
        path.to_string_lossy()
    )))
    .expect("framebuffer oracle should parse")
}

fn linked_observations<'a>(
    participants: &'a [LinkedParticipantObservation<'a>],
) -> OracleObservations<'a> {
    OracleObservations {
        serial: b"",
        cpu: None,
        memory: &[],
        executed_tcycles: 1,
        framebuffer: FramebufferObservation::empty(),
        participants: &[],
        linked: Some(LinkedSessionObservation {
            snapshot: None,
            trace: None,
            topology_trace: None,
            participants,
        }),
    }
}

fn local_oracle(
    config: &OracleConfig,
    store_root: &Path,
    local_root: &Path,
) -> Result<Oracle, String> {
    Oracle::from_manifest_with_fixture_roots(
        config,
        OracleFixtureRoots {
            store: store_root,
            local: local_root,
        },
    )
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
fn framebuffer_target_participant_can_read_linked_participant_framebuffer() {
    let temp_dir = temp_dir("linked-participant");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
    let fixture_path = temp_dir.join("fixture.pgm");
    write_pgm(&fixture_path, &vec![255; PIXELS]);
    let mut actual = vec![0; PIXELS];
    actual.fill(0);
    let participant = LinkedParticipantObservation {
        id: "master",
        serial: b"",
        serial_hex: "",
        snapshot: None,
        trace: None,
        framebuffer: FramebufferObservation {
            dmg: Some(&actual),
            cgb_rgb555: None,
            in_vblank: true,
        },
    };
    let mut oracle = Oracle::from_manifest(&parse_oracle_config(&format!(
        "oracle = {{ type = \"framebuffer\", target_participant = \"master\", fixture = {:?} }}",
        fixture_path.to_string_lossy()
    )))
    .expect("linked participant framebuffer oracle should parse");

    assert_eq!(
        oracle
            .finish(linked_observations(std::slice::from_ref(&participant)))
            .expect("linked participant framebuffer oracle should finish"),
        OracleOutcome::Passed
    );

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn framebuffer_local_fixture_resolves_against_report_data_root() {
    let temp_dir = temp_dir("local-fixture");
    let store_root = temp_dir.join("store");
    let local_root = temp_dir.join("data");
    let fixture_path = local_root.join("fixtures/pass.pgm");
    fs::create_dir_all(fixture_path.parent().expect("fixture should have parent"))
        .expect("fixture parent should be creatable");
    write_pgm(&fixture_path, &vec![255; PIXELS]);

    let mut oracle = local_oracle(
        &parse_oracle_config(
            "oracle = { type = \"framebuffer\", local = true, fixture = \"fixtures/pass.pgm\" }",
        ),
        &store_root,
        &local_root,
    )
    .expect("local framebuffer oracle should parse");
    let actual = vec![0; PIXELS];

    assert_eq!(
        oracle
            .finish(observations(1, Some(&actual), None, true))
            .expect("oracle should finish"),
        OracleOutcome::Passed
    );

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn framebuffer_local_fixture_array_accepts_any_matching_fixture() {
    let temp_dir = temp_dir("local-fixture-array");
    let store_root = temp_dir.join("store");
    let local_root = temp_dir.join("data");
    let mismatch_path = local_root.join("fixtures/mismatch.pgm");
    let match_path = local_root.join("fixtures/match.pgm");
    fs::create_dir_all(mismatch_path.parent().expect("fixture should have parent"))
        .expect("fixture parent should be creatable");
    let mut mismatch_pixels = vec![255; PIXELS];
    mismatch_pixels[0] = 0;
    write_pgm(&mismatch_path, &mismatch_pixels);
    write_pgm(&match_path, &vec![255; PIXELS]);

    let mut oracle = local_oracle(
        &parse_oracle_config(
            "oracle = { type = \"framebuffer\", local = true, fixture = [\"fixtures/mismatch.pgm\", \"fixtures/match.pgm\"] }",
        ),
        &store_root,
        &local_root,
    )
    .expect("local framebuffer oracle should parse");
    let actual = vec![0; PIXELS];

    assert_eq!(
        oracle
            .finish(observations(1, Some(&actual), None, true))
            .expect("oracle should finish"),
        OracleOutcome::Passed
    );

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn framebuffer_local_fixture_rejects_paths_outside_report_data_root() {
    let temp_dir = temp_dir("local-fixture-confined");
    let store_root = temp_dir.join("store");
    let local_root = temp_dir.join("data");
    fs::create_dir_all(&local_root).expect("local root should be creatable");

    assert!(
        local_oracle(
            &parse_oracle_config(
                "oracle = { type = \"framebuffer\", local = true, fixture = \"../escape.pgm\" }",
            ),
            &store_root,
            &local_root,
        )
        .expect_err("local fixture with parent component should fail")
        .contains("must not contain '..'")
    );

    let absolute_fixture = temp_dir.join("absolute.pgm");
    assert!(
        local_oracle(
            &parse_oracle_config(&format!(
                "oracle = {{ type = \"framebuffer\", local = true, fixture = {:?} }}",
                absolute_fixture.to_string_lossy()
            )),
            &store_root,
            &local_root,
        )
        .expect_err("absolute local fixture should fail")
        .contains("must be relative")
    );

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn framebuffer_local_parameter_must_be_boolean_and_is_unused_by_info_mode() {
    assert!(
        Oracle::from_manifest(&parse_oracle_config(
            "oracle = { type = \"framebuffer\", local = \"true\", fixture = \"fixtures/pass.pgm\" }",
        ))
        .expect_err("string local parameter should fail")
        .contains("field local must be a boolean")
    );
    assert!(
        Oracle::from_manifest(&parse_oracle_config(
            "oracle = { type = \"framebuffer\", mode = \"info\", local = true }",
        ))
        .expect_err("info mode with local fixture flag should fail")
        .contains("does not use local fixtures")
    );
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
fn framebuffer_cgb_rgb_projection_compares_exact_color() {
    let temp_dir = temp_dir("cgb-rgb-exact");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
    let red_fixture = temp_dir.join("red.png");
    let green_fixture = temp_dir.join("green.png");
    write_rgb_png(&red_fixture, &vec![[255, 0, 0]; PIXELS]);
    write_rgb_png(&green_fixture, &vec![[0, 255, 0]; PIXELS]);

    let rgb555 = vec![0x001F; PIXELS];
    let mut matching_oracle = Oracle::from_manifest(&parse_oracle_config(&format!(
        "oracle = {{ type = \"framebuffer\", source = \"cgb\", projection = \"rgb\", fixture = {:?} }}",
        red_fixture.to_string_lossy()
    )))
    .expect("RGB framebuffer oracle should parse");
    assert_eq!(
        matching_oracle
            .framebuffer_artifact_descriptor()
            .expect("RGB framebuffer oracle should expose artifacts")
            .projection,
        "rgb"
    );
    assert_eq!(
        matching_oracle
            .finish(observations(1, None, Some(&rgb555), true))
            .expect("oracle should finish"),
        OracleOutcome::Passed
    );

    let mut mismatching_oracle = Oracle::from_manifest(&parse_oracle_config(&format!(
        "oracle = {{ type = \"framebuffer\", source = \"cgb\", projection = \"rgb\", fixture = {:?} }}",
        green_fixture.to_string_lossy()
    )))
    .expect("RGB framebuffer oracle should parse");
    let outcome = mismatching_oracle
        .finish(observations(1, None, Some(&rgb555), true))
        .expect("oracle should finish");
    assert!(matches!(
        outcome,
        OracleOutcome::Failed(message) if message.contains("exact RGB")
    ));

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
