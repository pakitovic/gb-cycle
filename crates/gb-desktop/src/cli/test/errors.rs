use super::*;

#[test]
fn cli_reports_missing_values_unknown_options_and_extra_positional_arguments() {
    assert_eq!(
        parse_cli_arguments(["--model"]).expect_err("missing values should fail"),
        "--model requires a value"
    );
    assert_eq!(
        parse_cli_arguments(["--revision"]).expect_err("missing revision values should fail"),
        "--revision requires a value"
    );
    assert_eq!(
        parse_cli_arguments(["--mode"]).expect_err("missing mode values should fail"),
        "--mode requires a value"
    );
    assert_eq!(
        parse_cli_arguments(["--boot-rom-dir"])
            .expect_err("missing boot ROM directory values should fail"),
        "--boot-rom-dir requires a value"
    );
    assert_eq!(
        parse_cli_arguments(["--save-key"]).expect_err("missing save-key values should fail"),
        "--save-key requires a value"
    );
    assert!(parse_cli_arguments(["--mystery"]).is_err());
    assert!(parse_cli_arguments(["first.gb", "second.gb"]).is_err());
}

#[test]
fn cli_reports_remaining_missing_values_and_invalid_flag_inputs() {
    assert_eq!(
        parse_cli_arguments(["--startup"]).expect_err("missing startup values should fail"),
        "--startup requires a value"
    );
    assert_eq!(
        parse_cli_arguments(["--boot-rom-verify"])
            .expect_err("missing boot ROM verification values should fail"),
        "--boot-rom-verify requires a value"
    );
    assert_eq!(
        parse_cli_arguments(["--save-dir"]).expect_err("missing save-dir values should fail"),
        "--save-dir requires a value"
    );
    assert_eq!(
        parse_cli_arguments(["--save-policy"]).expect_err("missing save-policy values should fail"),
        "--save-policy requires a value"
    );
    assert_eq!(
        parse_cli_arguments(["--scale"]).expect_err("missing scale values should fail"),
        "--scale requires a value"
    );
    assert_eq!(
        parse_cli_arguments(["--palette"]).expect_err("missing palette values should fail"),
        "--palette requires a value"
    );
    assert_eq!(
        parse_cli_arguments(["--frame-blend"]).expect_err("missing frame-blend values should fail"),
        "--frame-blend requires a value"
    );
    assert_eq!(
        parse_cli_arguments(["--audio-record"])
            .expect_err("missing audio-record values should fail"),
        "--audio-record requires a value"
    );
    assert_eq!(
        parse_cli_arguments(["--audio-record-rate"])
            .expect_err("missing audio-record-rate values should fail"),
        "--audio-record-rate requires a value"
    );
    assert_eq!(
        parse_cli_arguments(["--audio-record-stems"])
            .expect_err("missing audio-record-stems values should fail"),
        "--audio-record-stems requires a value"
    );
    assert_eq!(
        parse_cli_arguments(["--link-rom"]).expect_err("missing linked peer values should fail"),
        "--link-rom requires a value"
    );
    assert_eq!(
        parse_cli_arguments(["--exit-after-frames"])
            .expect_err("missing exit-after-frames values should fail"),
        "--exit-after-frames requires a value"
    );
    assert_eq!(
        parse_cli_arguments(["--gamepad-direction"])
            .expect_err("missing gamepad direction values should fail"),
        "--gamepad-direction requires a value"
    );
    assert_eq!(
        parse_cli_arguments(["--gamepad-face-layout"])
            .expect_err("missing face-layout values should fail"),
        "--gamepad-face-layout requires a value"
    );
    assert_eq!(
        parse_cli_arguments(["--gamepad-preferred-name"])
            .expect_err("missing preferred-name values should fail"),
        "--gamepad-preferred-name requires a value"
    );
    assert_eq!(
        parse_cli_arguments(["--gamepad-preferred-path"])
            .expect_err("missing preferred-path values should fail"),
        "--gamepad-preferred-path requires a value"
    );
    assert_eq!(
        parse_cli_arguments(["--gamepad-bind-a"])
            .expect_err("missing gamepad binding values should fail"),
        "--gamepad-bind-a requires a value"
    );

    assert!(parse_cli_arguments(["--model", "gba"]).is_err());
    assert!(parse_cli_arguments(["--cgb-revision", "cgb-e"]).is_err());
    assert!(parse_cli_arguments(["--model", "DMG", "--revision", "cpu-cgb-e"]).is_err());
    assert!(parse_cli_arguments(["--model", "DMG", "--boot-rom", "cgbE"]).is_err());
    assert!(parse_cli_arguments(["--startup", "warm-boot"]).is_err());
    assert!(parse_cli_arguments(["--mode", "fast"]).is_err());
    assert!(parse_cli_arguments(["--boot-rom", "sgb"]).is_err());
    assert!(parse_cli_arguments(["--boot-rom-verify", "lenient"]).is_err());
    assert!(parse_cli_arguments(["--save-key", "bad/key"]).is_err());
    assert!(parse_cli_arguments(["--save-policy", "later"]).is_err());
    assert!(parse_cli_arguments(["--palette", "green"]).is_err());
    assert!(parse_cli_arguments(["--frame-blend", "smart"]).is_err());
    assert!(parse_cli_arguments(["--scale", "0"]).is_err());
    assert!(parse_cli_arguments(["--audio-record-rate", "0"]).is_err());
    assert!(parse_cli_arguments(["--audio-record-rate", "wide"]).is_err());
    assert!(
        parse_cli_arguments(["--audio-record-rate", "96000"]).is_err(),
        "recording rate alone should not silently enable a capture sink"
    );
    assert!(
        parse_cli_arguments(["--audio-record-stems", "ch4"]).is_err(),
        "recording stems alone should not silently enable a capture sink"
    );
    assert!(parse_cli_arguments(["--exit-after-frames", "0"]).is_err());
    assert!(parse_cli_arguments(["--gamepad-direction", "stick-only"]).is_err());
    assert!(parse_cli_arguments(["--gamepad-face-layout", "north-a"]).is_err());
    assert!(parse_cli_arguments(["--gamepad-preferred-name", "   "]).is_err());
    assert!(parse_cli_arguments(["--gamepad-preferred-path", "   "]).is_err());
    assert!(parse_cli_arguments(["--gamepad-bind-a", "touchpad"]).is_err());
    assert!(
        parse_cli_arguments([
            "demo.gb",
            "--audio-record",
            "out.wav",
            "--audio-record-stems",
            "noise"
        ])
        .is_err()
    );
}
