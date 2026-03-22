use std::env;
use std::path::{Path, PathBuf};

use gb_core::{BootRomAssets, BootRomKind, ConsoleModel};

pub const BOOT_ROM_STORE_DIR: &str = ".roms/bootrom";
pub const BOOT_ROM_ROOT_ENV_VAR: &str = "GB_CYCLE_BOOT_ROM_ROOT";
pub const ORACLE_STORE_DIR: &str = ".oracles";

pub fn boot_rom_store_root(workspace_root: &Path) -> PathBuf {
    workspace_root.join(BOOT_ROM_STORE_DIR)
}

pub fn discover_boot_rom_store_root(workspace_root: &Path) -> Option<PathBuf> {
    if let Some(root) = env::var_os(BOOT_ROM_ROOT_ENV_VAR) {
        return Some(PathBuf::from(root));
    }

    let default_root = boot_rom_store_root(workspace_root);
    default_root.exists().then_some(default_root)
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

pub fn boot_rom_kind_for_console_model(console_model: ConsoleModel) -> Option<BootRomKind> {
    match console_model {
        ConsoleModel::Dmg0 => Some(BootRomKind::Dmg0),
        ConsoleModel::Dmg => Some(BootRomKind::Dmg),
        ConsoleModel::Mgb => Some(BootRomKind::Mgb),
        ConsoleModel::Cgb => None,
    }
}

pub fn boot_rom_image_path(root: &Path, kind: BootRomKind) -> PathBuf {
    root.join(BootRomAssets::filename(kind))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use gb_core::{BootRomKind, ConsoleModel};

    use super::{
        BOOT_ROM_STORE_DIR, ORACLE_STORE_DIR, boot_rom_image_path, boot_rom_kind_for_console_model,
        boot_rom_store_root, oracle_layout_root, sameboy_tester_oracle_root,
    };

    #[test]
    fn workspace_paths_follow_repo_local_layout() {
        let workspace_root = Path::new("/tmp/gb-cycle");

        assert_eq!(
            boot_rom_store_root(workspace_root),
            workspace_root.join(BOOT_ROM_STORE_DIR)
        );
        assert_eq!(
            oracle_layout_root(workspace_root, "sameboy", "sameboy-tester"),
            workspace_root.join(format!("{ORACLE_STORE_DIR}/sameboy/sameboy-tester"))
        );
        assert_eq!(
            sameboy_tester_oracle_root(workspace_root),
            workspace_root.join(format!("{ORACLE_STORE_DIR}/sameboy/sameboy-tester"))
        );
    }

    #[test]
    fn boot_rom_helpers_map_console_models_and_filenames() {
        let root = Path::new("/tmp/gb-cycle/.roms/bootrom");

        assert_eq!(
            boot_rom_kind_for_console_model(ConsoleModel::Dmg),
            Some(BootRomKind::Dmg)
        );
        assert_eq!(
            boot_rom_kind_for_console_model(ConsoleModel::Mgb),
            Some(BootRomKind::Mgb)
        );
        assert_eq!(boot_rom_kind_for_console_model(ConsoleModel::Cgb), None);
        assert_eq!(
            boot_rom_image_path(root, BootRomKind::Dmg),
            root.join("dmg_boot.bin")
        );
    }
}
