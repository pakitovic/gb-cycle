use std::env;
use std::path::{Path, PathBuf};

use gb_core::{BootRomAssets, ConsoleModel, HardwareRevision};

pub const BOOT_ROM_ROOT_ENV_VAR: &str = "GB_CYCLE_BOOT_ROM_ROOT";
pub const ORACLE_STORE_DIR: &str = ".oracles";

pub fn discover_boot_rom_root() -> Option<PathBuf> {
    if let Some(root) = env::var_os(BOOT_ROM_ROOT_ENV_VAR) {
        return Some(PathBuf::from(root));
    }
    None
}

pub fn oracle_store_root(workspace_root: &Path) -> PathBuf {
    workspace_root.join(ORACLE_STORE_DIR)
}

pub fn oracle_layout_root(workspace_root: &Path, oracle: &str, layout: &str) -> PathBuf {
    oracle_store_root(workspace_root).join(oracle).join(layout)
}

pub fn sameboy_tester_oracle_root(workspace_root: &Path) -> PathBuf {
    oracle_layout_root(workspace_root, "sameboy", "sameboy-tester")
}

pub fn sameboy_case_bundle_oracle_root(workspace_root: &Path) -> PathBuf {
    oracle_layout_root(workspace_root, "sameboy", "case-bundle")
}

pub fn boot_rom_revision_for_console_model(console_model: ConsoleModel) -> HardwareRevision {
    console_model.default_revision()
}

pub fn boot_rom_image_path(root: &Path, revision: HardwareRevision) -> PathBuf {
    root.join(BootRomAssets::filename(revision))
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::path::Path;

    use gb_core::{ConsoleModel, HardwareRevision};

    use super::{
        BOOT_ROM_ROOT_ENV_VAR, ORACLE_STORE_DIR, boot_rom_image_path,
        boot_rom_revision_for_console_model, discover_boot_rom_root, oracle_layout_root,
        sameboy_case_bundle_oracle_root, sameboy_tester_oracle_root,
    };

    fn set_env_var(key: &str, value: impl AsRef<std::ffi::OsStr>) {
        // SAFETY: this test serializes environment mutation through `env_lock()`
        // and restores the touched variable before dropping the guard.
        unsafe {
            env::set_var(key, value);
        }
    }

    fn remove_env_var(key: &str) {
        // SAFETY: this test serializes environment mutation through `env_lock()`
        // and restores the touched variable before dropping the guard.
        unsafe {
            env::remove_var(key);
        }
    }

    #[test]
    fn workspace_paths_follow_repo_local_oracle_layout() {
        let workspace_root = Path::new("/tmp/gb-cycle");

        assert_eq!(
            oracle_layout_root(workspace_root, "sameboy", "sameboy-tester"),
            workspace_root.join(format!("{ORACLE_STORE_DIR}/sameboy/sameboy-tester"))
        );
        assert_eq!(
            sameboy_tester_oracle_root(workspace_root),
            workspace_root.join(format!("{ORACLE_STORE_DIR}/sameboy/sameboy-tester"))
        );
        assert_eq!(
            sameboy_case_bundle_oracle_root(workspace_root),
            workspace_root.join(format!("{ORACLE_STORE_DIR}/sameboy/case-bundle"))
        );
    }

    #[test]
    fn boot_rom_root_discovery_prefers_environment_override() {
        let _guard = crate::test_support::lock_env();
        let previous = env::var_os(BOOT_ROM_ROOT_ENV_VAR);
        let root = Path::new("/tmp/gb-cycle/bootroms");

        set_env_var(BOOT_ROM_ROOT_ENV_VAR, root);

        assert_eq!(discover_boot_rom_root(), Some(root.to_path_buf()));

        match previous {
            Some(value) => set_env_var(BOOT_ROM_ROOT_ENV_VAR, value),
            None => remove_env_var(BOOT_ROM_ROOT_ENV_VAR),
        }
    }

    #[test]
    fn boot_rom_helpers_map_console_models_and_filenames() {
        let root = Path::new("/tmp/gb-cycle/bootroms");

        assert_eq!(
            boot_rom_revision_for_console_model(ConsoleModel::GameBoy),
            HardwareRevision::DmgCpuC
        );
        assert_eq!(
            boot_rom_revision_for_console_model(ConsoleModel::GameBoyPocket),
            HardwareRevision::CpuMgb
        );
        assert_eq!(
            boot_rom_revision_for_console_model(ConsoleModel::GameBoyLight),
            HardwareRevision::CpuMgb
        );
        assert_eq!(
            boot_rom_revision_for_console_model(ConsoleModel::GameBoyColor),
            HardwareRevision::CpuCgbC
        );
        assert_eq!(
            boot_rom_image_path(root, HardwareRevision::DmgCpuC),
            root.join("dmg_boot.bin")
        );
    }
}
