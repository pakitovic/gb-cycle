use gb_core::{
    CartridgeLoadError, CartridgePersistentStateError, CartridgeSlot, Machine, PersistentCartState,
    TraceSummaryBuffer,
};
use gb_desktop::{DEFAULT_SAVE_FLUSH_DEBOUNCE, DesktopSaveFlushPolicy};
use gb_persistence::{
    CartridgeSaveBackend, CartridgeSaveKey, FilesystemCartridgeSaveBackend,
    uses_battery_backed_hardware_persistence,
};
use std::path::{Path, PathBuf};
use std::time::Instant;

pub struct DesktopSaveSession {
    backend: FilesystemCartridgeSaveBackend,
    key: CartridgeSaveKey,
    flush_policy: DesktopSaveFlushPolicy,
    last_saved_state: PersistentCartState,
    pending_debounced_flush_deadline: Option<Instant>,
}

impl DesktopSaveSession {
    pub fn open(
        save_root: Option<&Path>,
        flush_policy: DesktopSaveFlushPolicy,
        key: Option<CartridgeSaveKey>,
        machine: &mut Machine<TraceSummaryBuffer>,
    ) -> Result<Option<Self>, String> {
        Self::open_with_legacy_fallback(save_root, flush_policy, key, None, machine)
    }

    pub fn open_with_legacy_fallback(
        save_root: Option<&Path>,
        flush_policy: DesktopSaveFlushPolicy,
        key: Option<CartridgeSaveKey>,
        legacy_key: Option<CartridgeSaveKey>,
        machine: &mut Machine<TraceSummaryBuffer>,
    ) -> Result<Option<Self>, String> {
        let Some(save_root) = save_root else {
            return Ok(None);
        };

        let metadata = machine.cartridge().persistence_metadata();
        if !uses_battery_backed_hardware_persistence(metadata) {
            return Ok(None);
        }

        let Some(key) = key else {
            return Ok(None);
        };

        let backend = FilesystemCartridgeSaveBackend::new(save_root);
        let load_result = backend.load(&key).map_err(|error| {
            format!(
                "failed to load save {}: {error}",
                backend.path_for_key(&key).display()
            )
        })?;
        let legacy_load_result = if load_result.is_none() {
            legacy_key
                .as_ref()
                .filter(|legacy_key| *legacy_key != &key)
                .map(|legacy_key| {
                    backend.load(legacy_key).map_err(|error| {
                        format!(
                            "failed to load save {}: {error}",
                            backend.path_for_key(legacy_key).display()
                        )
                    })
                })
                .transpose()?
                .flatten()
        } else {
            None
        };

        if let Some(envelope) = load_result.or(legacy_load_result) {
            let elapsed_seconds = backend
                .current_unix_seconds()
                .saturating_sub(envelope.backend_metadata.saved_at_unix_seconds);
            let mut restored_state = envelope.persistent_state;
            apply_elapsed_off_session_seconds(&mut restored_state, elapsed_seconds);
            machine
                .restore_cartridge_persistent_state(&restored_state)
                .map_err(format_restore_error)?;
        }

        let last_saved_state = machine.cartridge().persistent_state();
        Ok(Some(Self {
            backend,
            key,
            flush_policy,
            last_saved_state,
            pending_debounced_flush_deadline: None,
        }))
    }

    pub fn save_path(&self) -> PathBuf {
        self.backend.path_for_key(&self.key)
    }

    pub fn flush_policy(&self) -> DesktopSaveFlushPolicy {
        self.flush_policy
    }

    pub fn maybe_flush_at_frame_boundary(
        &mut self,
        machine: &Machine<TraceSummaryBuffer>,
        now: Instant,
    ) -> Result<bool, String> {
        match self.flush_policy {
            DesktopSaveFlushPolicy::Manual | DesktopSaveFlushPolicy::OnClose => Ok(false),
            DesktopSaveFlushPolicy::OnWrite => self.flush_if_changed(machine, "frame-boundary"),
            DesktopSaveFlushPolicy::Debounced => self.flush_if_debounced(machine, now),
        }
    }

    pub fn flush_if_changed(
        &mut self,
        machine: &Machine<TraceSummaryBuffer>,
        reason: &str,
    ) -> Result<bool, String> {
        let current_state = machine.cartridge().persistent_state();
        self.flush_current_state_if_changed(machine, current_state, reason)
    }

    pub fn close(&mut self, machine: &Machine<TraceSummaryBuffer>) -> Result<(), String> {
        if self.flush_policy.flush_on_close() {
            self.flush_if_changed(machine, "close").map(|_| ())
        } else {
            Ok(())
        }
    }

    fn flush_if_debounced(
        &mut self,
        machine: &Machine<TraceSummaryBuffer>,
        now: Instant,
    ) -> Result<bool, String> {
        let current_state = machine.cartridge().persistent_state();
        if current_state == self.last_saved_state {
            self.pending_debounced_flush_deadline = None;
            return Ok(false);
        }

        let deadline = self
            .pending_debounced_flush_deadline
            .get_or_insert(now + DEFAULT_SAVE_FLUSH_DEBOUNCE);
        if now < *deadline {
            return Ok(false);
        }

        self.flush_current_state_if_changed(machine, current_state, "debounced-frame-boundary")
    }

    fn flush_current_state_if_changed(
        &mut self,
        machine: &Machine<TraceSummaryBuffer>,
        current_state: PersistentCartState,
        reason: &str,
    ) -> Result<bool, String> {
        if current_state == self.last_saved_state {
            self.pending_debounced_flush_deadline = None;
            return Ok(false);
        }

        self.backend
            .save(
                &self.key,
                machine.cartridge().persistence_metadata(),
                &current_state,
            )
            .map_err(|error| {
                format!(
                    "failed to save cartridge persistence ({reason}) to {}: {error}",
                    self.save_path().display()
                )
            })?;
        self.last_saved_state = current_state;
        self.pending_debounced_flush_deadline = None;
        Ok(true)
    }
}

fn apply_elapsed_off_session_seconds(state: &mut PersistentCartState, elapsed_seconds: u64) {
    match state {
        PersistentCartState::Mbc3Rtc { rtc } | PersistentCartState::Mbc3RamRtc { rtc, .. } => {
            rtc.apply_elapsed_seconds(elapsed_seconds);
        }
        PersistentCartState::Huc3 { rtc, .. } => rtc.apply_elapsed_seconds(elapsed_seconds),
        _ => {}
    }
}

fn format_restore_error(error: CartridgePersistentStateError) -> String {
    format!("failed to restore cartridge persistence: {error:?}")
}

#[allow(dead_code)]
fn format_load_error(error: CartridgeLoadError) -> String {
    format!("{error:?}")
}

#[allow(dead_code)]
fn _cartridge(_slot: &CartridgeSlot) {}

#[cfg(test)]
mod tests {
    use super::*;
    use gb_core::{ConsoleModel, MachineConfig};
    use std::env;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;
    const ENTRY_POINT_START: usize = 0x0100;
    const LOGO_START: usize = 0x0104;
    const TITLE_START: usize = 0x0134;
    const CGB_FLAG_ADDRESS: usize = 0x0143;
    const SGB_FLAG_ADDRESS: usize = 0x0146;
    const CARTRIDGE_TYPE_ADDRESS: usize = 0x0147;
    const ROM_SIZE_ADDRESS: usize = 0x0148;
    const RAM_SIZE_ADDRESS: usize = 0x0149;
    const HEADER_CHECKSUM_ADDRESS: usize = 0x014D;
    static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn debounced_policy_waits_for_the_configured_interval_before_flushing() {
        let root = temp_save_root();
        let mut machine = load_machine(build_banked_mbc2_rom(0x06, 0x03, 0x00));
        let mut session = DesktopSaveSession::open(
            Some(&root),
            DesktopSaveFlushPolicy::Debounced,
            Some(CartridgeSaveKey::new("debounced").expect("key should be valid")),
            &mut machine,
        )
        .expect("debounced save session should open")
        .expect("battery-backed cartridge should create a session");
        mutate_mbc2_persistent_state(&mut machine, 0x07);

        let start = Instant::now();
        assert!(
            !session
                .maybe_flush_at_frame_boundary(&machine, start)
                .expect("first frame-boundary check should succeed")
        );
        assert!(!session.save_path().exists());

        let before_deadline = (start + DEFAULT_SAVE_FLUSH_DEBOUNCE)
            .checked_sub(Duration::from_millis(1))
            .expect("deadline should exceed the pre-flush probe");
        assert!(
            !session
                .maybe_flush_at_frame_boundary(&machine, before_deadline)
                .expect("pre-deadline debounce probe should succeed")
        );
        assert!(!session.save_path().exists());

        assert!(
            session
                .maybe_flush_at_frame_boundary(&machine, start + DEFAULT_SAVE_FLUSH_DEBOUNCE)
                .expect("deadline debounce probe should succeed")
        );
        assert!(session.save_path().is_file());

        fs::remove_dir_all(root).expect("temp save root should be removable");
    }

    #[test]
    fn debounced_policy_still_flushes_on_close_before_the_interval_elapses() {
        let root = temp_save_root();
        let mut machine = load_machine(build_banked_mbc2_rom(0x06, 0x03, 0x00));
        let mut session = DesktopSaveSession::open(
            Some(&root),
            DesktopSaveFlushPolicy::Debounced,
            Some(CartridgeSaveKey::new("debounced-close").expect("key should be valid")),
            &mut machine,
        )
        .expect("debounced save session should open")
        .expect("battery-backed cartridge should create a session");
        mutate_mbc2_persistent_state(&mut machine, 0x03);

        assert!(
            !session
                .maybe_flush_at_frame_boundary(&machine, Instant::now())
                .expect("initial debounce probe should succeed")
        );
        assert!(!session.save_path().exists());

        session
            .close(&machine)
            .expect("close should flush even when debounce is still pending");
        assert!(session.save_path().is_file());

        fs::remove_dir_all(root).expect("temp save root should be removable");
    }

    #[test]
    fn on_write_policy_flushes_at_the_next_frame_boundary_without_waiting() {
        let root = temp_save_root();
        let mut machine = load_machine(build_banked_mbc2_rom(0x06, 0x03, 0x00));
        let mut session = DesktopSaveSession::open(
            Some(&root),
            DesktopSaveFlushPolicy::OnWrite,
            Some(CartridgeSaveKey::new("on-write").expect("key should be valid")),
            &mut machine,
        )
        .expect("on-write save session should open")
        .expect("battery-backed cartridge should create a session");
        mutate_mbc2_persistent_state(&mut machine, 0x0E);

        assert!(
            session
                .maybe_flush_at_frame_boundary(&machine, Instant::now())
                .expect("on-write frame-boundary check should succeed")
        );
        assert!(session.save_path().is_file());

        fs::remove_dir_all(root).expect("temp save root should be removable");
    }

    #[test]
    fn open_restores_existing_battery_backed_save_from_disk() {
        let root = temp_save_root();
        let key = CartridgeSaveKey::new("restore".to_string()).expect("key should be valid");
        let mut saved_machine = load_machine(build_banked_mbc2_rom(0x06, 0x03, 0x00));
        mutate_mbc2_persistent_state(&mut saved_machine, 0x0A);
        let expected_state = saved_machine.cartridge().persistent_state();

        let mut backend = FilesystemCartridgeSaveBackend::new(&root);
        backend
            .save(
                &key,
                saved_machine.cartridge().persistence_metadata(),
                &expected_state,
            )
            .expect("pre-existing save should write");

        let mut restored_machine = load_machine(build_banked_mbc2_rom(0x06, 0x03, 0x00));
        let session = DesktopSaveSession::open(
            Some(&root),
            DesktopSaveFlushPolicy::Manual,
            Some(key.clone()),
            &mut restored_machine,
        )
        .expect("save session should load an existing save")
        .expect("battery-backed cartridge should create a session");

        assert_eq!(
            session.save_path(),
            root.join(format!("{}.gbsav", key.as_str()))
        );
        assert_eq!(
            restored_machine.cartridge().persistent_state(),
            expected_state
        );

        fs::remove_dir_all(root).expect("temp save root should be removable");
    }

    #[test]
    fn open_restores_legacy_sanitized_save_but_writes_exact_key() {
        let root = temp_save_root();
        let exact_key =
            CartridgeSaveKey::new("Legend of Zelda, The - Link's Awakening (USA, Europe) (Rev 2)")
                .expect("exact ROM stem should be valid");
        let legacy_key =
            CartridgeSaveKey::new("Legend_of_Zelda_The_-_Link_s_Awakening_USA_Europe_Rev_2")
                .expect("legacy sanitized key should be valid");
        let mut saved_machine = load_machine(build_banked_mbc2_rom(0x06, 0x03, 0x00));
        mutate_mbc2_persistent_state(&mut saved_machine, 0x0A);
        let expected_state = saved_machine.cartridge().persistent_state();

        let mut backend = FilesystemCartridgeSaveBackend::new(&root);
        backend
            .save(
                &legacy_key,
                saved_machine.cartridge().persistence_metadata(),
                &expected_state,
            )
            .expect("legacy save should write");

        let mut restored_machine = load_machine(build_banked_mbc2_rom(0x06, 0x03, 0x00));
        let mut session = DesktopSaveSession::open_with_legacy_fallback(
            Some(&root),
            DesktopSaveFlushPolicy::OnClose,
            Some(exact_key.clone()),
            Some(legacy_key.clone()),
            &mut restored_machine,
        )
        .expect("save session should load a legacy save")
        .expect("battery-backed cartridge should create a session");

        assert_eq!(
            restored_machine.cartridge().persistent_state(),
            expected_state
        );
        assert_eq!(
            session.save_path(),
            root.join(format!("{}.gbsav", exact_key.as_str()))
        );
        assert!(backend.path_for_key(&legacy_key).is_file());
        mutate_mbc2_persistent_state(&mut restored_machine, 0x0B);
        session
            .close(&restored_machine)
            .expect("closing should rewrite through the exact key");
        assert!(backend.path_for_key(&exact_key).is_file());

        fs::remove_dir_all(root).expect("temp save root should be removable");
    }

    #[test]
    fn open_surfaces_corrupt_existing_save_files() {
        let root = temp_save_root();
        let key = CartridgeSaveKey::new("corrupt".to_string()).expect("key should be valid");
        let backend = FilesystemCartridgeSaveBackend::new(&root);
        fs::write(backend.path_for_key(&key), b"not-a-valid-save")
            .expect("corrupt save payload should write");
        let mut machine = load_machine(build_banked_mbc2_rom(0x06, 0x03, 0x00));

        let error = DesktopSaveSession::open(
            Some(&root),
            DesktopSaveFlushPolicy::Manual,
            Some(key),
            &mut machine,
        )
        .err()
        .expect("corrupt save payloads should surface as load errors");
        assert!(error.contains("failed to load save"));
        assert!(error.contains(".gbsav"));

        fs::remove_dir_all(root).expect("temp save root should be removable");
    }

    #[test]
    fn on_close_policy_defers_frame_boundary_flushes_but_flushes_when_closed() {
        let root = temp_save_root();
        let mut machine = load_machine(build_banked_mbc2_rom(0x06, 0x03, 0x00));
        let mut session = DesktopSaveSession::open(
            Some(&root),
            DesktopSaveFlushPolicy::OnClose,
            Some(CartridgeSaveKey::new("on-close".to_string()).expect("key should be valid")),
            &mut machine,
        )
        .expect("on-close save session should open")
        .expect("battery-backed cartridge should create a session");
        mutate_mbc2_persistent_state(&mut machine, 0x05);

        assert!(
            !session
                .maybe_flush_at_frame_boundary(&machine, Instant::now())
                .expect("frame-boundary checks should be skipped for on-close sessions")
        );
        assert!(!session.save_path().exists());

        session
            .close(&machine)
            .expect("on-close sessions should flush when the session closes");
        assert!(session.save_path().is_file());

        fs::remove_dir_all(root).expect("temp save root should be removable");
    }

    #[test]
    fn debounced_policy_clears_pending_deadline_when_state_returns_to_saved() {
        let root = temp_save_root();
        let mut machine = load_machine(build_banked_mbc2_rom(0x06, 0x03, 0x00));
        let mut session = DesktopSaveSession::open(
            Some(&root),
            DesktopSaveFlushPolicy::Debounced,
            Some(CartridgeSaveKey::new("debounce-reset".to_string()).expect("key should be valid")),
            &mut machine,
        )
        .expect("debounced save session should open")
        .expect("battery-backed cartridge should create a session");
        let original_state = machine.cartridge().persistent_state();
        mutate_mbc2_persistent_state(&mut machine, 0x09);

        let start = Instant::now();
        assert!(
            !session
                .maybe_flush_at_frame_boundary(&machine, start)
                .expect("initial debounce probe should succeed")
        );
        assert!(session.pending_debounced_flush_deadline.is_some());

        machine
            .restore_cartridge_persistent_state(&original_state)
            .expect("restoring the saved state should succeed");
        assert!(
            !session
                .maybe_flush_at_frame_boundary(&machine, start + Duration::from_millis(1))
                .expect("unchanged debounce probe should succeed")
        );
        assert!(session.pending_debounced_flush_deadline.is_none());

        fs::remove_dir_all(root).expect("temp save root should be removable");
    }

    #[test]
    fn flush_if_changed_surfaces_backend_save_errors() {
        let root = temp_save_root();
        let mut machine = load_machine(build_banked_mbc2_rom(0x06, 0x03, 0x00));
        let mut session = DesktopSaveSession::open(
            Some(&root),
            DesktopSaveFlushPolicy::OnWrite,
            Some(CartridgeSaveKey::new("save-error".to_string()).expect("key should be valid")),
            &mut machine,
        )
        .expect("on-write save session should open")
        .expect("battery-backed cartridge should create a session");
        let blocking_root = root.join("not-a-directory");
        fs::write(&blocking_root, b"occupied").expect("blocking file should exist");
        session.backend = FilesystemCartridgeSaveBackend::new(&blocking_root);
        mutate_mbc2_persistent_state(&mut machine, 0x0B);

        let error = session
            .flush_if_changed(&machine, "test-save")
            .expect_err("save failures should surface through the desktop session");
        assert!(error.contains("failed to save cartridge persistence (test-save)"));
        assert!(error.contains(".gbsav"));

        fs::remove_dir_all(root).expect("temp save root should be removable");
    }

    #[test]
    fn temp_save_root_reuses_stale_directory_ids_cleanly() {
        let saved_counter = TEMP_DIR_COUNTER.load(Ordering::Relaxed);
        TEMP_DIR_COUNTER.store(42, Ordering::Relaxed);
        let root = temp_save_root();
        fs::write(root.join("stale.bin"), b"stale").expect("stale marker should write");

        TEMP_DIR_COUNTER.store(42, Ordering::Relaxed);
        let reused_root = temp_save_root();
        assert_eq!(reused_root, root);
        assert!(!reused_root.join("stale.bin").exists());

        TEMP_DIR_COUNTER.store(saved_counter, Ordering::Relaxed);
        fs::remove_dir_all(reused_root).expect("temp save root should be removable");
    }

    #[test]
    fn build_banked_mbc2_rom_maps_supported_size_codes_to_expected_lengths() {
        let cases = [
            (0x00, 32 * 1024usize),
            (0x01, 64 * 1024usize),
            (0x02, 128 * 1024usize),
            (0x03, 256 * 1024usize),
            (0x04, 512 * 1024usize),
        ];
        for (rom_size_code, expected_len) in cases {
            let rom = build_banked_mbc2_rom(0x06, rom_size_code, 0x00);
            assert_eq!(rom.len(), expected_len);
        }
    }

    #[test]
    #[should_panic(expected = "unsupported MBC2 ROM size code for test")]
    fn build_banked_mbc2_rom_rejects_unsupported_size_codes() {
        let _ = build_banked_mbc2_rom(0x06, 0x05, 0x00);
    }

    #[test]
    fn open_returns_none_without_a_root_key_or_battery_backed_cartridge() {
        let root = temp_save_root();
        let mut battery_machine = load_machine(build_banked_mbc2_rom(0x06, 0x03, 0x00));
        assert!(
            DesktopSaveSession::open(
                None,
                DesktopSaveFlushPolicy::Manual,
                Some(CartridgeSaveKey::new("unused").expect("key should be valid")),
                &mut battery_machine,
            )
            .expect("omitting the save root should not fail")
            .is_none()
        );
        assert!(
            DesktopSaveSession::open(
                Some(&root),
                DesktopSaveFlushPolicy::Manual,
                None,
                &mut battery_machine,
            )
            .expect("omitting the save key should not fail")
            .is_none()
        );

        let mut no_battery_machine = load_machine(build_test_rom(32 * 1024, 0x00, 0x00, 0x00));
        assert!(
            DesktopSaveSession::open(
                Some(&root),
                DesktopSaveFlushPolicy::Manual,
                Some(CartridgeSaveKey::new("nobattery").expect("key should be valid")),
                &mut no_battery_machine,
            )
            .expect("non-battery cartridges should not error")
            .is_none()
        );

        fs::remove_dir_all(root).expect("temp save root should be removable");
    }

    #[test]
    fn manual_sessions_expose_their_policy_and_do_not_flush_on_close_without_changes() {
        let root = temp_save_root();
        let mut machine = load_machine(build_banked_mbc2_rom(0x06, 0x03, 0x00));
        let key = CartridgeSaveKey::new("manual".to_string()).expect("key should be valid");
        let mut session = DesktopSaveSession::open(
            Some(&root),
            DesktopSaveFlushPolicy::Manual,
            Some(key.clone()),
            &mut machine,
        )
        .expect("manual save session should open")
        .expect("battery-backed cartridge should create a session");

        assert_eq!(session.flush_policy(), DesktopSaveFlushPolicy::Manual);
        assert_eq!(
            session.save_path(),
            root.join(format!("{}.gbsav", key.as_str()))
        );
        assert!(
            !session
                .flush_if_changed(&machine, "no-op")
                .expect("unchanged state should be a no-op")
        );
        session
            .close(&machine)
            .expect("manual close without changes should not fail");
        assert!(!session.save_path().exists());

        fs::remove_dir_all(root).expect("temp save root should be removable");
    }

    #[test]
    fn persistence_helpers_cover_rtc_advancement_and_error_formatting() {
        let mut rtc_state = PersistentCartState::Mbc3Rtc {
            rtc: gb_core::Mbc3RtcPersistentState {
                seconds: 58,
                minutes: 59,
                hours: 23,
                day_counter: 0,
                halt: false,
                carry: false,
            },
        };
        apply_elapsed_off_session_seconds(&mut rtc_state, 2);
        assert!(matches!(rtc_state, PersistentCartState::Mbc3Rtc { .. }));
        if let PersistentCartState::Mbc3Rtc { rtc } = rtc_state {
            assert_eq!(rtc.seconds, 0);
            assert_eq!(rtc.minutes, 0);
            assert_eq!(rtc.hours, 0);
            assert_eq!(rtc.day_counter, 1);
        }

        let mut huc3_state = PersistentCartState::Huc3 {
            ram: vec![0x11; 8],
            mcu_ram: [0; 256],
            rtc: gb_core::Huc3RtcPersistentState {
                current_minutes_of_day: 1,
                current_days: 0,
                current_subminute_seconds: 59,
                event_minutes_of_day: 5,
                event_days: 0,
            },
            rom_bank: 0,
            ram_bank: 0,
            select_mode: 0x0D,
            access_address: 0,
            mailbox_command: 0,
            mailbox_argument: 0,
            last_response_nybble: 0,
            semaphore_ready: true,
            ir_emitter_on: false,
            ir_light_detected: false,
            last_control_write: None,
            last_unsupported_command: None,
            last_unsupported_argument: None,
        };
        apply_elapsed_off_session_seconds(&mut huc3_state, 2);
        if let PersistentCartState::Huc3 { rtc, .. } = huc3_state {
            assert_eq!(rtc.current_minutes_of_day, 2);
            assert_eq!(rtc.current_subminute_seconds, 1);
        } else {
            panic!("expected Huc3 state");
        }

        let mut plain_ram = PersistentCartState::Mbc5Ram { ram: vec![1, 2, 3] };
        let before = plain_ram.clone();
        apply_elapsed_off_session_seconds(&mut plain_ram, 120);
        assert_eq!(plain_ram, before);

        assert!(
            format_restore_error(CartridgePersistentStateError::KindMismatch {
                expected: "MBC2",
                actual: "MBC3",
            })
            .contains("KindMismatch")
        );
        assert!(
            format_load_error(CartridgeLoadError::HeaderParse(
                gb_core::CartridgeHeaderParseError::ImageTooSmall {
                    actual_size: 4,
                    minimum_size: 0x150,
                },
            ))
            .contains("HeaderParse")
        );

        let machine = load_machine(build_banked_mbc2_rom(0x06, 0x03, 0x00));
        _cartridge(machine.cartridge());
    }

    fn load_machine(rom: Vec<u8>) -> Machine<TraceSummaryBuffer> {
        let mut machine = Machine::new_summary(MachineConfig::new(ConsoleModel::Dmg));
        machine
            .load_cartridge(rom)
            .expect("test cartridge should load");
        machine
    }

    fn mutate_mbc2_persistent_state(machine: &mut Machine<TraceSummaryBuffer>, value: u8) {
        let mut state = machine.cartridge().persistent_state();
        assert!(matches!(state, PersistentCartState::Mbc2Ram { .. }));
        if let PersistentCartState::Mbc2Ram { ram_nibbles } = &mut state {
            ram_nibbles[0] = value & 0x0F;
        }
        machine
            .restore_cartridge_persistent_state(&state)
            .expect("restoring test persistent state should succeed");
    }

    fn temp_save_root() -> PathBuf {
        let id = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = env::temp_dir().join(format!(
            "gb-cycle-desktop-save-session-tests-{}-{id}",
            std::process::id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).expect("stale temp save root should be removable");
        }
        fs::create_dir_all(&root).expect("temp save root should be creatable");
        root
    }

    fn build_test_rom(
        len: usize,
        cartridge_type: u8,
        rom_size_code: u8,
        ram_size_code: u8,
    ) -> Vec<u8> {
        let mut rom = vec![0xFF; len.max(HEADER_MINIMUM_ROM_LEN)];
        rom[0x0000] = 0x12;
        rom[ENTRY_POINT_START..ENTRY_POINT_START + 4].copy_from_slice(&[0x31, 0xFE, 0xFF, 0xAF]);
        rom[LOGO_START..LOGO_START + 48].copy_from_slice(&[0xCE; 48]);
        rom[TITLE_START..TITLE_START + 8].copy_from_slice(b"DESKTOP!");
        rom[CGB_FLAG_ADDRESS] = 0x80;
        rom[SGB_FLAG_ADDRESS] = 0x03;
        rom[CARTRIDGE_TYPE_ADDRESS] = cartridge_type;
        rom[ROM_SIZE_ADDRESS] = rom_size_code;
        rom[RAM_SIZE_ADDRESS] = ram_size_code;
        rom[HEADER_CHECKSUM_ADDRESS] = 0x7F;
        rom
    }

    fn build_banked_mbc2_rom(cartridge_type: u8, rom_size_code: u8, ram_size_code: u8) -> Vec<u8> {
        let rom_size = match rom_size_code {
            0x00 => 32 * 1024,
            0x01 => 64 * 1024,
            0x02 => 128 * 1024,
            0x03 => 256 * 1024,
            0x04 => 512 * 1024,
            _ => panic!("unsupported MBC2 ROM size code for test"),
        };
        let bank_count = rom_size / 0x4000;
        let mut rom = build_test_rom(rom_size, cartridge_type, rom_size_code, ram_size_code);

        for bank in 0..bank_count {
            let start = bank * 0x4000;
            rom[start] = bank as u8;
            rom[start + 0x0100] = bank as u8;
        }

        rom
    }
}
