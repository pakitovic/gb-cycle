mod args;
mod cases;
mod command;
mod report;
mod run;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn write_case(path: &Path, id: &str, rom: &Path) {
    fs::write(
        path,
        format!(
            "version = 1\nid = \"{id}\"\nrom = \"{}\"\nmodel = \"DMG\"\nstartup = \"custom-boot\"\nmode = \"permissive\"\n\n[[run]]\nid = \"idle\"\nduration_seconds = 1\n",
            rom.display()
        ),
    )
    .expect("case should write");
}

fn temp_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should be after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "gb-benchmark-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("temp root should create");
    root
}
