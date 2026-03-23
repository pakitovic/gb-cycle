#![cfg(unix)]

use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use gb_test_runner::{
    GBEMU_SHOOTOUT_ROOT_ENV_VAR, RomRunner, SameBoyTesterExecutionError, SameBoyTesterImageFormat,
    SameBoyTesterRunner, gbdev_dmg_acid2_suite, phase_2_cpu_timing_suite,
};

fn unique_temp_dir(label: &str) -> PathBuf {
    env::temp_dir().join(format!(
        "gb-cycle-sameboy-tester-{}-{}-{}",
        label,
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ))
}

fn write_fake_tester(path: &Path, args_output: &Path) {
    fs::write(
        path,
        format!(
            concat!(
                "#!/bin/sh\n",
                "set -eu\n",
                "args_file=\"{}\"\n",
                ": > \"$args_file\"\n",
                "for arg in \"$@\"; do\n",
                "  printf '%s\\n' \"$arg\" >> \"$args_file\"\n",
                "done\n",
                "ext=bmp\n",
                "for arg in \"$@\"; do\n",
                "  if [ \"$arg\" = \"--tga\" ]; then ext=tga; fi\n",
                "done\n",
                "for last; do rom=\"$last\"; done\n",
                "printf 'fake-image' > \"${{rom%.*}}.$ext\"\n",
                "printf 'fake-log' > \"${{rom%.*}}.log\"\n",
            ),
            args_output.display(),
        ),
    )
    .expect("fake tester should be writable");
    let mut permissions = fs::metadata(path)
        .expect("fake tester metadata should exist")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("fake tester should be executable");
}

#[test]
fn sameboy_tester_runner_stages_dmg_acid2_and_emits_image_and_log_artifacts() {
    let temp_dir = unique_temp_dir("acid2");
    let oracle_root = temp_dir.join("oracle");
    let external_root = temp_dir.join("external");
    let rom_path = external_root.join("testroms/acid/dmg-acid2.gb");
    fs::create_dir_all(
        rom_path
            .parent()
            .expect("test ROM path should have a parent"),
    )
    .expect("external ROM dir should be creatable");
    fs::write(&rom_path, b"fake-rom").expect("external ROM should be writable");

    let args_output = temp_dir.join("tester-args.txt");
    let tester_binary = temp_dir.join("fake-sameboy-tester.sh");
    write_fake_tester(&tester_binary, &args_output);

    let report = SameBoyTesterRunner::new(&oracle_root)
        .with_rom_runner(
            RomRunner::new().with_external_rom_root(GBEMU_SHOOTOUT_ROOT_ENV_VAR, &external_root),
        )
        .with_tester_binary(&tester_binary)
        .with_image_format(SameBoyTesterImageFormat::Tga)
        .run_suite(&gbdev_dmg_acid2_suite())
        .expect("SameBoy tester suite should run");

    assert_eq!(report.cases.len(), 1);
    let case = &report.cases[0];
    assert!(case.staged_rom_path.is_file());
    assert_eq!(
        fs::read(&case.staged_rom_path).expect("staged ROM should be readable"),
        b"fake-rom"
    );
    assert!(case.image_artifact_path.is_file());
    assert!(
        case.image_artifact_path
            .ends_with("testroms/acid/dmg-acid2.tga")
    );
    assert_eq!(
        case.log_artifact_path.as_ref(),
        Some(&oracle_root.join("testroms/acid/dmg-acid2.log"))
    );
    assert!(case.startup_mode_note.is_some());

    let args = fs::read_to_string(args_output).expect("fake tester args should be readable");
    assert!(args.contains("--dmg"));
    assert!(args.contains("--tga"));
    assert!(args.contains("--length\n3\n"));
    assert!(
        args.contains(
            oracle_root
                .join("testroms/acid/dmg-acid2.gb")
                .to_str()
                .expect("staged ROM path should be utf-8")
        )
    );
}

#[test]
fn sameboy_tester_runner_rejects_non_framebuffer_suite() {
    let temp_dir = unique_temp_dir("unsupported");
    let tester_binary = temp_dir.join("fake-sameboy-tester.sh");
    let args_output = temp_dir.join("tester-args.txt");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
    write_fake_tester(&tester_binary, &args_output);

    let error = SameBoyTesterRunner::new(temp_dir.join("oracle"))
        .with_tester_binary(&tester_binary)
        .run_suite(&phase_2_cpu_timing_suite())
        .expect_err("trace suite should be rejected");

    assert!(matches!(
        error,
        SameBoyTesterExecutionError::UnsupportedCapture { .. }
    ));
}
