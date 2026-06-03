use std::fs;

use askama::Template;

use crate::{GB_CLI_FRONTEND, GB_DESKTOP_FRONTEND};

use super::super::report::build_index_report;
use super::{temp_root, write_case};

#[test]
fn askama_index_escapes_case_data_and_supports_cli_columns() {
    let root = temp_root("index");
    let benchmark_dir = root.join("test/bench");
    let case_dir = root.join("cases");
    fs::create_dir_all(benchmark_dir.join("gb-cli")).expect("cli dir should create");
    fs::create_dir_all(benchmark_dir.join("gb-desktop")).expect("desktop dir should create");
    fs::create_dir_all(&case_dir).expect("case dir should create");
    let rom = root.join("evil<&>.gb");
    fs::write(&rom, [0_u8]).expect("rom should write");
    let case = case_dir.join("case.toml");
    write_case(&case, "evil", &rom);
    for frontend in [GB_CLI_FRONTEND, GB_DESKTOP_FRONTEND] {
        fs::write(
            benchmark_dir.join(frontend).join("evil-idle-stats.toml"),
            format!(
                "version = 1\nfrontend = \"{frontend}\"\nid = \"evil\"\nartifact_id = \"evil-idle\"\nrom = \"{}\"\nmodel = \"DMG\"\nstartup = \"custom-boot\"\nmode = \"permissive\"\ntest_runner = true\nduration_seconds = 1\ntarget_frames = 60\ncompleted_frames = 60\nelapsed_seconds = 1.0\nfps = 60.0\nspeed_percent = 100.0\n",
                rom.display()
            ),
        )
        .expect("stats should write");
        fs::write(benchmark_dir.join(frontend).join("evil-idle.png"), [0_u8])
            .expect("image should write");
    }

    let report = build_index_report(&benchmark_dir, &case_dir, true)
        .expect("report should build")
        .render()
        .expect("report should render");

    assert!(report.contains("evil&#60;&#38;&#62;.gb"));
    assert!(!report.contains("evil<&>.gb"));
    assert!(report.contains("<th>gb-cli</th>"));
    assert!(report.contains("<th>gb-desktop</th>"));
    assert!(report.contains("60.00 FPS<br>100.0%"));
}
