use super::*;
use std::env;
use std::error::Error as _;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_root(label: &str) -> PathBuf {
    let id = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    let root = env::temp_dir().join(format!(
        "gb-cycle-config-tests-{label}-{}-{id}",
        std::process::id()
    ));
    if root.exists() {
        fs::remove_dir_all(&root).expect("stale config temp root should be removable");
    }
    fs::create_dir_all(&root).expect("config temp root should be creatable");
    root
}

#[path = "test/boot.rs"]
mod boot;
#[path = "test/defaults.rs"]
mod defaults;
#[path = "test/errors.rs"]
mod errors;
#[path = "test/input.rs"]
mod input;
#[path = "test/machine.rs"]
mod machine;
#[path = "test/models.rs"]
mod models;
#[path = "test/persistence.rs"]
mod persistence;
#[path = "test/save.rs"]
mod save;
