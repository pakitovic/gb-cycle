use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::{env, fs};

static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) fn temp_save_root() -> PathBuf {
    let id = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    let root = env::temp_dir().join(format!(
        "gb-cycle-persistence-tests-{}-{id}",
        std::process::id()
    ));
    if root.exists() {
        fs::remove_dir_all(&root).expect("stale temp save root should be removable");
    }
    fs::create_dir_all(&root).expect("temp save root should be creatable");
    root
}
