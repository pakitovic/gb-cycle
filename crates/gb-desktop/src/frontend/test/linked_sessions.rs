use super::*;

#[test]
fn desktop_emulation_session_can_wrap_a_two_console_dmg04_runtime() {
    let primary = dmg_skip_boot_summary_machine();
    let secondary = dmg_skip_boot_summary_machine();

    let linked =
        super::super::linked_session::DesktopEmulationSession::new_linked_dmg04_two_player(
            primary, secondary,
        )
        .expect("desktop linked session should build from two aligned machines");

    assert_eq!(
        linked.kind(),
        super::super::linked_session::DesktopEmulationSessionKind::LinkedDmg04TwoPlayer
    );
    assert_eq!(linked.linked_topology_kind(), LinkedTopologyKind::Dmg04);
    assert_eq!(
        linked.external_port().attachment_kind(),
        ExternalPortAttachmentKind::GameLinkDmg04
    );
    assert_eq!(
        linked
            .secondary_machine()
            .expect("secondary machine should exist")
            .external_port()
            .attachment_kind(),
        ExternalPortAttachmentKind::GameLinkDmg04
    );
}

#[test]
fn desktop_emulation_session_can_wrap_a_two_console_cgb_ir_runtime() {
    let primary = cgb_skip_boot_summary_machine();
    let secondary = cgb_skip_boot_summary_machine();

    let linked =
        super::super::linked_session::DesktopEmulationSession::new_linked_cgb_infrared_two_player(
            primary, secondary,
        )
        .expect("desktop CGB IR session should build from two aligned machines");

    assert_eq!(
        linked.kind(),
        super::super::linked_session::DesktopEmulationSessionKind::LinkedCgbInfraredTwoPlayer
    );
    assert_eq!(
        linked.linked_topology_kind(),
        LinkedTopologyKind::CgbInfrared
    );
    assert_eq!(
        linked.external_port().attachment_kind(),
        ExternalPortAttachmentKind::None
    );
    assert_eq!(
        linked
            .secondary_machine()
            .expect("secondary CGB IR machine should exist")
            .external_port()
            .attachment_kind(),
        ExternalPortAttachmentKind::None
    );
}

#[test]
fn desktop_emulation_session_can_wrap_contiguous_dmg07_slots() {
    let linked = super::super::linked_session::DesktopEmulationSession::new_linked_dmg07(
        vec![
            dmg_skip_boot_summary_machine(),
            dmg_skip_boot_summary_machine(),
            dmg_skip_boot_summary_machine(),
            dmg_skip_boot_summary_machine(),
        ],
        super::super::DesktopDmg07PlayerCount::Four,
    )
    .expect("desktop DMG-07 session should build from four aligned machines");

    assert_eq!(
        linked.kind(),
        super::super::linked_session::DesktopEmulationSessionKind::LinkedDmg07 {
            player_count: super::super::DesktopDmg07PlayerCount::Four,
        }
    );
    assert_eq!(linked.linked_topology_kind(), LinkedTopologyKind::Dmg07);
    assert_eq!(
        linked.dmg07_player_count(),
        Some(super::super::DesktopDmg07PlayerCount::Four)
    );
    assert_dmg07_slot_port(&linked, super::super::PlayerSlot::P1, Dmg07Port::P1);
    assert_dmg07_slot_port(&linked, super::super::PlayerSlot::P2, Dmg07Port::P2);
    assert_dmg07_slot_port(&linked, super::super::PlayerSlot::P3, Dmg07Port::P3);
    assert_dmg07_slot_port(&linked, super::super::PlayerSlot::P4, Dmg07Port::P4);

    let error = super::super::linked_session::DesktopEmulationSession::new_linked_dmg07(
        vec![
            dmg_skip_boot_summary_machine(),
            dmg_skip_boot_summary_machine(),
        ],
        super::super::DesktopDmg07PlayerCount::Four,
    )
    .expect_err("wrong desktop DMG-07 machine count should be rejected");
    assert!(error.contains("requires 4 machines, found 2"));
}

#[test]
fn desktop_emulation_session_can_return_to_a_single_primary_machine() {
    let primary = dmg_skip_boot_summary_machine();
    let secondary = dmg_skip_boot_summary_machine();

    let mut linked =
        super::super::linked_session::DesktopEmulationSession::new_linked_dmg04_two_player(
            primary, secondary,
        )
        .expect("desktop linked session should build from two aligned machines");

    linked.step_t_cycle();
    let primary_wram_before = linked.read_bus(0xC000);
    linked
        .secondary_machine_mut()
        .expect("secondary machine should exist")
        .write_bus(0xC000, 0x3C);

    let mut primary = linked.into_primary_machine();

    assert_eq!(primary.next_t_cycle(), gb_core::TCycle::new(1));
    assert_eq!(
        primary.external_port().attachment_kind(),
        ExternalPortAttachmentKind::None
    );
    assert_eq!(primary.read_bus(0xC000), primary_wram_before);
}

#[test]
fn game_link_menu_action_loads_a_secondary_rom_into_a_linked_runtime() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("game-link-activate", true, false, false);
    let secondary_rom_path = harness.root.join("linked-secondary.gb");
    fs::write(
        &secondary_rom_path,
        build_test_rom(32 * 1024, 0x00, 0x00, 0x00),
    )
    .expect("secondary link ROM should be writable");

    harness.runtime.open_rom_dialog_mode = super::super::OpenRomDialogMode::LinkedSecondary;

    harness
        .runtime
        .open_rom_dialog
        .sender
        .send(PathDialogResult::Selected(secondary_rom_path.clone()))
        .expect("secondary ROM selection should send");
    harness
        .process_pending_open_rom_dialog()
        .expect("secondary ROM selection should activate GAME LINK");

    assert_eq!(
        harness.session.external_port_selection,
        DesktopExternalPortSelection::GameLink
    );
    assert_eq!(
        harness.session.linked_secondary_rom_path(),
        Some(secondary_rom_path.as_path())
    );
    assert_eq!(
        harness.machine.kind(),
        super::super::linked_session::DesktopEmulationSessionKind::LinkedDmg04TwoPlayer
    );
    assert_eq!(
        harness.machine.linked_topology_kind(),
        LinkedTopologyKind::Dmg04
    );
    assert_eq!(
        harness.machine.external_port().attachment_kind(),
        ExternalPortAttachmentKind::GameLinkDmg04
    );
    assert_eq!(
        harness
            .machine
            .secondary_machine()
            .expect("secondary linked machine should exist")
            .external_port()
            .attachment_kind(),
        ExternalPortAttachmentKind::GameLinkDmg04
    );
}

#[test]
fn game_link_same_game_action_clones_the_primary_rom_without_opening_a_dialog() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("game-link-same-game", true, false, false);
    let primary_rom_path = harness
        .session
        .rom_path()
        .expect("primary ROM should be loaded")
        .to_path_buf();
    for _ in 0..256 {
        harness.machine.step_t_cycle();
    }
    assert_ne!(harness.machine.next_t_cycle(), gb_core::TCycle::ZERO);

    assert!(
        harness
            .execute_action(super::super::MenuAction::SetGameLinkSameGame)
            .expect("SAME GAME should build a fresh GAME LINK session")
            .is_none()
    );

    assert_eq!(
        harness.session.external_port_selection,
        DesktopExternalPortSelection::GameLink
    );
    assert_eq!(
        harness.session.linked_secondary_rom_path(),
        Some(primary_rom_path.as_path())
    );
    assert_eq!(harness.session.dmg07_player_count, None);
    assert!(!harness.runtime.open_rom_dialog.is_pending());
    assert_eq!(
        harness.runtime.open_rom_dialog_mode,
        super::super::OpenRomDialogMode::Primary
    );
    assert_eq!(
        harness.machine.kind(),
        super::super::linked_session::DesktopEmulationSessionKind::LinkedDmg04TwoPlayer
    );
    assert_eq!(
        harness.machine.linked_topology_kind(),
        LinkedTopologyKind::Dmg04
    );
    assert_eq!(
        harness.machine.primary_machine().next_t_cycle(),
        gb_core::TCycle::ZERO
    );
    assert_eq!(
        harness.machine.external_port().attachment_kind(),
        ExternalPortAttachmentKind::GameLinkDmg04
    );
    assert_eq!(
        harness
            .machine
            .secondary_machine()
            .expect("secondary linked machine should exist")
            .external_port()
            .attachment_kind(),
        ExternalPortAttachmentKind::GameLinkDmg04
    );
}

#[test]
fn selecting_none_after_game_link_returns_to_a_single_primary_runtime() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("game-link-detach", true, false, false);
    let secondary_rom_path = harness.root.join("linked-secondary.gb");
    fs::write(
        &secondary_rom_path,
        build_test_rom(32 * 1024, 0x00, 0x00, 0x00),
    )
    .expect("secondary link ROM should be writable");

    harness.runtime.open_rom_dialog_mode = super::super::OpenRomDialogMode::LinkedSecondary;
    harness
        .runtime
        .open_rom_dialog
        .sender
        .send(PathDialogResult::Selected(secondary_rom_path))
        .expect("secondary ROM selection should send");
    harness
        .process_pending_open_rom_dialog()
        .expect("secondary ROM selection should activate GAME LINK");
    harness.machine.write_bus(0xC000, 0x5A);
    harness
        .machine
        .secondary_machine_mut()
        .expect("secondary linked machine should exist")
        .write_bus(0xC000, 0x99);

    assert!(
        harness
            .execute_action(super::super::MenuAction::SetExternalPort(
                DesktopExternalPortSelection::None,
            ))
            .expect("returning to NONE should tear down GAME LINK")
            .is_none()
    );

    assert_eq!(
        harness.session.external_port_selection,
        DesktopExternalPortSelection::None
    );
    assert!(harness.session.linked_secondary_rom.is_none());
    assert_eq!(
        harness.machine.kind(),
        super::super::linked_session::DesktopEmulationSessionKind::Single
    );
    assert_eq!(
        harness.machine.linked_topology_kind(),
        LinkedTopologyKind::None
    );
    assert_eq!(
        harness.machine.external_port().attachment_kind(),
        ExternalPortAttachmentKind::None
    );
    assert_eq!(harness.machine.read_bus(0xC000), 0x5A);
}

#[test]
fn game_link_activation_rebuilds_an_advanced_primary_into_a_fresh_linked_session() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("game-link-fresh-sync", true, false, false);
    let secondary_rom_path = harness.root.join("linked-secondary.gb");
    fs::write(
        &secondary_rom_path,
        build_test_rom(32 * 1024, 0x00, 0x00, 0x00),
    )
    .expect("secondary link ROM should be writable");

    for _ in 0..256 {
        harness.machine.step_t_cycle();
    }
    assert_ne!(harness.machine.next_t_cycle(), gb_core::TCycle::ZERO);

    harness.runtime.open_rom_dialog_mode = super::super::OpenRomDialogMode::LinkedSecondary;
    harness
        .runtime
        .open_rom_dialog
        .sender
        .send(PathDialogResult::Selected(secondary_rom_path))
        .expect("secondary ROM selection should send");
    harness
        .process_pending_open_rom_dialog()
        .expect("advanced primary should still activate GAME LINK");

    assert_eq!(
        harness.machine.kind(),
        super::super::linked_session::DesktopEmulationSessionKind::LinkedDmg04TwoPlayer
    );
    assert_eq!(
        harness.machine.primary_machine().next_t_cycle(),
        gb_core::TCycle::ZERO
    );
    assert_eq!(
        harness
            .machine
            .secondary_machine()
            .expect("secondary linked machine should exist")
            .next_t_cycle(),
        gb_core::TCycle::ZERO
    );
}

#[test]
fn reset_keeps_the_linked_runtime_active_for_game_link_sessions() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("game-link-reset", true, false, false);
    let secondary_rom_path = harness.root.join("linked-secondary.gb");
    fs::write(
        &secondary_rom_path,
        build_test_rom(32 * 1024, 0x00, 0x00, 0x00),
    )
    .expect("secondary link ROM should be writable");

    harness.runtime.open_rom_dialog_mode = super::super::OpenRomDialogMode::LinkedSecondary;
    harness
        .runtime
        .open_rom_dialog
        .sender
        .send(PathDialogResult::Selected(secondary_rom_path))
        .expect("secondary ROM selection should send");
    harness
        .process_pending_open_rom_dialog()
        .expect("secondary ROM selection should activate GAME LINK");

    let primary_reset_baseline = harness.machine.read_bus(0xC000);
    let secondary_reset_baseline = harness
        .machine
        .secondary_machine_mut()
        .expect("secondary linked machine should exist")
        .read_bus(0xC000);
    harness.machine.write_bus(0xC000, 0xA5);
    harness
        .machine
        .secondary_machine_mut()
        .expect("secondary linked machine should exist")
        .write_bus(0xC000, 0x3C);

    super::super::reset_machine(
        harness.canvas.window(),
        &mut harness.session,
        &mut harness.machine,
        &mut harness.runtime,
        &mut harness.settings_store,
    )
    .expect("linked reset should succeed");

    assert_eq!(
        harness.session.external_port_selection,
        DesktopExternalPortSelection::GameLink
    );
    assert!(harness.session.linked_secondary_rom.is_some());
    assert_eq!(
        harness.machine.kind(),
        super::super::linked_session::DesktopEmulationSessionKind::LinkedDmg04TwoPlayer
    );
    assert_eq!(harness.machine.read_bus(0xC000), primary_reset_baseline);
    assert_eq!(
        harness
            .machine
            .secondary_machine_mut()
            .expect("secondary linked machine should exist")
            .read_bus(0xC000),
        secondary_reset_baseline
    );
}

#[test]
fn cgb_ir_select_game_action_switches_the_open_rom_dialog_into_secondary_mode() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("cgb-ir-action", false, false, false);
    open_cgb_primary_rom(&mut harness, "primary.gbc", 0x00, 0x00);
    harness.runtime.open_rom_dialog.pending = true;

    assert!(
        harness
            .execute_action(super::super::MenuAction::SelectCgbInfraredSecondary)
            .expect("CGB IR action should not fail when the open dialog is already pending")
            .is_none()
    );
    assert_eq!(
        harness.runtime.open_rom_dialog_mode,
        super::super::OpenRomDialogMode::CgbInfraredSecondary
    );
}

#[test]
fn cgb_ir_menu_action_ignores_non_cgb_sessions() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("cgb-ir-action-dmg", true, false, false);

    assert!(
        harness
            .execute_action(super::super::MenuAction::SelectCgbInfraredSecondary)
            .expect("CGB IR action should be a no-op outside MODEL GB COLOR")
            .is_none()
    );
    assert_eq!(
        harness.runtime.open_rom_dialog_mode,
        super::super::OpenRomDialogMode::Primary
    );
    assert!(!harness.runtime.open_rom_dialog.is_pending());
}

#[test]
fn cgb_ir_same_game_action_ignores_non_cgb_or_unloaded_sessions() {
    let _guard = crate::lock_sdl_test();
    let mut dmg_harness = FrontendHarness::new("cgb-ir-same-game-dmg", true, false, false);
    assert!(
        dmg_harness
            .execute_action(super::super::MenuAction::SetCgbInfraredSameGame)
            .expect("CGB IR SAME GAME should be a no-op outside MODEL GB COLOR")
            .is_none()
    );
    assert!(!dmg_harness.session.cgb_infrared_link_active());
    assert!(dmg_harness.session.linked_secondary_rom.is_none());
    assert_eq!(
        dmg_harness.machine.kind(),
        super::super::linked_session::DesktopEmulationSessionKind::Single
    );
    assert!(!dmg_harness.runtime.open_rom_dialog.is_pending());
    drop(dmg_harness);

    let mut unloaded_cgb_harness =
        FrontendHarness::new("cgb-ir-same-game-unloaded", false, false, false);
    assert!(
        unloaded_cgb_harness
            .execute_action(super::super::MenuAction::SetCgbInfraredSameGame)
            .expect("CGB IR SAME GAME should be a no-op without a loaded ROM")
            .is_none()
    );
    assert!(!unloaded_cgb_harness.session.cgb_infrared_link_active());
    assert!(unloaded_cgb_harness.session.linked_secondary_rom.is_none());
    assert_eq!(
        unloaded_cgb_harness.machine.kind(),
        super::super::linked_session::DesktopEmulationSessionKind::Single
    );
    assert!(!unloaded_cgb_harness.runtime.open_rom_dialog.is_pending());
}

#[test]
fn cgb_ir_select_game_action_ignores_unloaded_cgb_sessions() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("cgb-ir-select-game-unloaded", false, false, false);

    assert!(
        harness
            .execute_action(super::super::MenuAction::SelectCgbInfraredSecondary)
            .expect("CGB IR SELECT GAME should be a no-op without a loaded ROM")
            .is_none()
    );
    assert_eq!(
        harness.runtime.open_rom_dialog_mode,
        super::super::OpenRomDialogMode::Primary
    );
    assert!(!harness.runtime.open_rom_dialog.is_pending());
}

#[test]
fn cgb_ir_helper_action_toggles_runtime_video_option_and_persists_settings() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("cgb-ir-helper-toggle", false, false, false);

    assert!(!harness.runtime.video_options.show_cgb_infrared_helper);
    assert!(
        harness
            .execute_action(super::super::MenuAction::ToggleCgbInfraredHelper)
            .expect("CGB IR helper action should persist the enabled state")
            .is_none()
    );
    assert!(harness.runtime.video_options.show_cgb_infrared_helper);
    let persisted =
        fs::read_to_string(&harness.settings_path).expect("settings should be persisted");
    assert!(persisted.contains("show_cgb_infrared_helper = true"));

    assert!(
        harness
            .execute_action(super::super::MenuAction::ToggleCgbInfraredHelper)
            .expect("CGB IR helper action should persist the disabled state")
            .is_none()
    );
    assert!(!harness.runtime.video_options.show_cgb_infrared_helper);
    let persisted =
        fs::read_to_string(&harness.settings_path).expect("settings should be persisted");
    assert!(persisted.contains("show_cgb_infrared_helper = false"));
}

#[test]
fn cgb_ir_same_game_action_loads_the_primary_rom_as_the_secondary_runtime() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("cgb-ir-same-game", false, false, false);
    let primary_rom_path = open_cgb_primary_rom(&mut harness, "gold.gbc", 0x00, 0x00);
    harness.runtime.frame_blending_state.mode = DesktopFrameBlendingMode::On;
    harness.runtime.frame_blending_state.dimensions = Some(super::super::FramebufferDimensions {
        width: super::super::FRAMEBUFFER_WIDTH,
        height: super::super::FRAMEBUFFER_HEIGHT,
    });
    harness.runtime.frame_blending_state.previous_rgb_frame = vec![1, 2, 3];
    harness.runtime.frame_blending_state.has_previous_frame = true;

    assert!(
        harness
            .execute_action(super::super::MenuAction::SetCgbInfraredSameGame)
            .expect("CGB IR SAME GAME action should activate a two-console runtime")
            .is_none()
    );

    assert_eq!(
        harness.session.external_port_selection,
        DesktopExternalPortSelection::None
    );
    assert!(harness.session.cgb_infrared_link_active());
    assert_eq!(
        harness.session.linked_secondary_rom_path(),
        Some(primary_rom_path.as_path())
    );
    assert!(super::super::cgb_infrared_same_game_active(
        &harness.session
    ));
    assert_eq!(
        harness.machine.kind(),
        super::super::linked_session::DesktopEmulationSessionKind::LinkedCgbInfraredTwoPlayer
    );
    assert_eq!(
        harness.machine.linked_topology_kind(),
        LinkedTopologyKind::CgbInfrared
    );
    assert_eq!(
        harness.machine.external_port().attachment_kind(),
        ExternalPortAttachmentKind::None
    );
    assert_eq!(
        harness
            .machine
            .secondary_machine()
            .expect("secondary CGB IR machine should exist")
            .external_port()
            .attachment_kind(),
        ExternalPortAttachmentKind::None
    );
    assert_eq!(
        harness.runtime.frame_blending_state.mode,
        DesktopFrameBlendingMode::Off
    );
    assert!(harness.runtime.frame_blending_state.dimensions.is_none());
    assert!(
        harness
            .runtime
            .frame_blending_state
            .previous_rgb_frame
            .is_empty()
    );
    assert!(!harness.runtime.frame_blending_state.has_previous_frame);
}

#[test]
fn cgb_ir_secondary_selection_loads_a_two_console_infrared_runtime() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("cgb-ir-activate", false, false, false);
    open_cgb_primary_rom(&mut harness, "gold.gbc", 0x00, 0x00);
    let secondary_rom_path = write_cgb_test_rom(&harness.root, "silver.gbc", 0x00, 0x00);

    harness.runtime.open_rom_dialog_mode = super::super::OpenRomDialogMode::CgbInfraredSecondary;
    harness
        .runtime
        .open_rom_dialog
        .sender
        .send(PathDialogResult::Selected(PathBuf::from("silver.gbc")))
        .expect("CGB IR secondary ROM selection should send");
    harness
        .process_pending_open_rom_dialog()
        .expect("secondary ROM selection should activate CGB IR");

    assert_eq!(
        harness.session.external_port_selection,
        DesktopExternalPortSelection::None
    );
    assert!(harness.session.cgb_infrared_link_active());
    assert_eq!(
        harness.session.linked_secondary_rom_path(),
        Some(secondary_rom_path.as_path())
    );
    assert_eq!(
        harness.machine.kind(),
        super::super::linked_session::DesktopEmulationSessionKind::LinkedCgbInfraredTwoPlayer
    );
    assert_eq!(
        harness.machine.linked_topology_kind(),
        LinkedTopologyKind::CgbInfrared
    );
    assert_eq!(
        harness.machine.external_port().attachment_kind(),
        ExternalPortAttachmentKind::None
    );
    assert_eq!(
        harness
            .machine
            .secondary_machine()
            .expect("secondary CGB IR machine should exist")
            .external_port()
            .attachment_kind(),
        ExternalPortAttachmentKind::None
    );
    assert!(!super::super::machine_state_actions_available(
        &harness.session,
        &harness.machine
    ));
    assert!(!super::super::rewind_session_supported(
        &harness.session,
        &harness.machine
    ));
}

#[test]
fn cgb_ir_none_action_turns_active_session_off_without_opening_picker() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("cgb-ir-toggle-off", false, false, false);
    open_cgb_primary_rom(&mut harness, "gold.gbc", 0x00, 0x00);
    write_cgb_test_rom(&harness.root, "silver.gbc", 0x00, 0x00);
    harness.runtime.open_rom_dialog_mode = super::super::OpenRomDialogMode::CgbInfraredSecondary;
    harness
        .runtime
        .open_rom_dialog
        .sender
        .send(PathDialogResult::Selected(PathBuf::from("silver.gbc")))
        .expect("CGB IR secondary ROM selection should send");
    harness
        .process_pending_open_rom_dialog()
        .expect("secondary ROM selection should activate CGB IR");

    assert!(harness.session.cgb_infrared_link_active());
    assert!(
        harness
            .execute_action(super::super::MenuAction::SetCgbInfraredNone)
            .expect("CGB IR NONE action should turn the pair off")
            .is_none()
    );

    assert!(!harness.runtime.open_rom_dialog.is_pending());
    assert_eq!(
        harness.runtime.open_rom_dialog_mode,
        super::super::OpenRomDialogMode::Primary
    );
    assert!(!harness.session.cgb_infrared_link_active());
    assert!(harness.session.linked_secondary_rom.is_none());
    assert_eq!(
        harness.session.external_port_selection,
        DesktopExternalPortSelection::None
    );
    assert_eq!(
        harness.machine.kind(),
        super::super::linked_session::DesktopEmulationSessionKind::Single
    );
    assert_eq!(
        harness.machine.linked_topology_kind(),
        LinkedTopologyKind::None
    );
}

#[test]
fn pokemon_pikachu_color_action_activates_an_accessory_session() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("pokemon-pikachu-color-activate", false, false, false);
    open_cgb_primary_rom(&mut harness, "gold.gbc", 0x00, 0x00);
    harness.runtime.frame_blending_state.mode = DesktopFrameBlendingMode::On;
    harness.runtime.frame_blending_state.dimensions = Some(super::super::FramebufferDimensions {
        width: super::super::FRAMEBUFFER_WIDTH,
        height: super::super::FRAMEBUFFER_HEIGHT,
    });
    harness.runtime.frame_blending_state.previous_rgb_frame = vec![1, 2, 3];
    harness.runtime.frame_blending_state.has_previous_frame = true;

    assert!(
        harness
            .execute_action(super::super::MenuAction::SetCgbInfraredPikachuColor)
            .expect("Pokemon Pikachu Color action should activate an accessory runtime")
            .is_none()
    );

    assert_eq!(
        harness.session.external_port_selection,
        DesktopExternalPortSelection::None
    );
    assert!(!harness.session.cgb_infrared_link_active());
    assert!(harness.session.pokemon_pikachu_color_active());
    assert!(harness.session.linked_secondary_rom.is_none());
    assert_eq!(
        harness.machine.kind(),
        super::super::linked_session::DesktopEmulationSessionKind::PokemonPikachuColor
    );
    assert_eq!(
        harness.machine.linked_topology_kind(),
        LinkedTopologyKind::None
    );
    assert_eq!(
        harness.machine.external_port().attachment_kind(),
        ExternalPortAttachmentKind::None
    );
    assert_eq!(
        harness.runtime.frame_blending_state.mode,
        DesktopFrameBlendingMode::Off
    );
    assert!(harness.runtime.frame_blending_state.dimensions.is_none());
    assert!(
        harness
            .runtime
            .frame_blending_state
            .previous_rgb_frame
            .is_empty()
    );
    assert!(!harness.runtime.frame_blending_state.has_previous_frame);
    assert!(!super::super::machine_state_actions_available(
        &harness.session,
        &harness.machine
    ));
    assert!(!super::super::rewind_session_supported(
        &harness.session,
        &harness.machine
    ));
}

#[test]
fn pokemon_pikachu_color_gift_action_updates_the_accessory() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("pokemon-pikachu-color-gift", false, false, false);
    open_cgb_primary_rom(&mut harness, "gold.gbc", 0x00, 0x00);
    harness.session.pokemon_pikachu_color_gift = PokemonPikachuColorGift::Watts999;

    assert!(
        harness
            .execute_action(super::super::MenuAction::SetCgbInfraredPikachuColor)
            .expect("Pokemon Pikachu Color action should activate an accessory runtime")
            .is_none()
    );
    assert_eq!(
        harness
            .machine
            .pokemon_pikachu_color_status()
            .expect("Pokemon Pikachu Color status should be exposed")
            .gift,
        PokemonPikachuColorGift::Watts999
    );

    assert!(
        harness
            .execute_action(super::super::MenuAction::CycleCgbInfraredPikachuGift)
            .expect("gift action should cycle the accessory gift")
            .is_none()
    );

    assert_eq!(
        harness.session.pokemon_pikachu_color_gift,
        PokemonPikachuColorGift::Watts1
    );
    assert_eq!(
        harness
            .machine
            .pokemon_pikachu_color_status()
            .expect("Pokemon Pikachu Color status should stay exposed")
            .gift,
        PokemonPikachuColorGift::Watts1
    );
}

#[test]
fn cgb_ir_none_action_turns_pokemon_pikachu_color_off() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("pokemon-pikachu-color-none", false, false, false);
    open_cgb_primary_rom(&mut harness, "gold.gbc", 0x00, 0x00);
    assert!(
        harness
            .execute_action(super::super::MenuAction::SetCgbInfraredPikachuColor)
            .expect("Pokemon Pikachu Color action should activate an accessory runtime")
            .is_none()
    );

    assert!(
        harness
            .execute_action(super::super::MenuAction::SetCgbInfraredNone)
            .expect("CGB IR NONE action should turn the accessory off")
            .is_none()
    );

    assert!(!harness.session.cgb_infrared_link_active());
    assert!(!harness.session.pokemon_pikachu_color_active());
    assert!(harness.session.linked_secondary_rom.is_none());
    assert_eq!(
        harness.machine.kind(),
        super::super::linked_session::DesktopEmulationSessionKind::Single
    );
}

#[test]
fn pokemon_pikachu_color_is_mutually_exclusive_with_cgb_ir_pairs() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("pokemon-pikachu-color-exclusive", false, false, false);
    open_cgb_primary_rom(&mut harness, "gold.gbc", 0x00, 0x00);
    assert!(
        harness
            .execute_action(super::super::MenuAction::SetCgbInfraredPikachuColor)
            .expect("Pokemon Pikachu Color action should activate an accessory runtime")
            .is_none()
    );

    assert!(
        harness
            .execute_action(super::super::MenuAction::SetCgbInfraredSameGame)
            .expect("CGB IR SAME GAME should replace the accessory runtime")
            .is_none()
    );
    assert!(!harness.session.pokemon_pikachu_color_active());
    assert!(harness.session.cgb_infrared_link_active());
    assert_eq!(
        harness.machine.kind(),
        super::super::linked_session::DesktopEmulationSessionKind::LinkedCgbInfraredTwoPlayer
    );

    assert!(
        harness
            .execute_action(super::super::MenuAction::SetCgbInfraredPikachuColor)
            .expect("Pokemon Pikachu Color should replace the CGB IR pair")
            .is_none()
    );
    assert!(harness.session.pokemon_pikachu_color_active());
    assert!(!harness.session.cgb_infrared_link_active());
    assert!(harness.session.linked_secondary_rom.is_none());
    assert_eq!(
        harness.machine.kind(),
        super::super::linked_session::DesktopEmulationSessionKind::PokemonPikachuColor
    );
}

#[test]
fn selecting_none_after_cgb_ir_returns_to_a_single_primary_runtime() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("cgb-ir-detach", false, false, false);
    open_cgb_primary_rom(&mut harness, "gold.gbc", 0x00, 0x00);
    write_cgb_test_rom(&harness.root, "silver.gbc", 0x00, 0x00);
    harness.runtime.open_rom_dialog_mode = super::super::OpenRomDialogMode::CgbInfraredSecondary;
    harness
        .runtime
        .open_rom_dialog
        .sender
        .send(PathDialogResult::Selected(PathBuf::from("silver.gbc")))
        .expect("CGB IR secondary ROM selection should send");
    harness
        .process_pending_open_rom_dialog()
        .expect("secondary ROM selection should activate CGB IR");
    harness.machine.write_bus(0xC000, 0x5A);
    harness
        .machine
        .secondary_machine_mut()
        .expect("secondary CGB IR machine should exist")
        .write_bus(0xC000, 0x99);

    assert!(
        harness
            .execute_action(super::super::MenuAction::SetExternalPort(
                DesktopExternalPortSelection::None,
            ))
            .expect("returning to NONE should tear down CGB IR")
            .is_none()
    );

    assert_eq!(
        harness.session.external_port_selection,
        DesktopExternalPortSelection::None
    );
    assert!(!harness.session.cgb_infrared_link_active());
    assert!(harness.session.linked_secondary_rom.is_none());
    assert_eq!(
        harness.machine.kind(),
        super::super::linked_session::DesktopEmulationSessionKind::Single
    );
    assert_eq!(
        harness.machine.linked_topology_kind(),
        LinkedTopologyKind::None
    );
    assert_eq!(harness.machine.read_bus(0xC000), 0x5A);
}

#[test]
fn reset_keeps_the_cgb_ir_runtime_active() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("cgb-ir-reset", false, false, false);
    open_cgb_primary_rom(&mut harness, "gold.gbc", 0x00, 0x00);
    write_cgb_test_rom(&harness.root, "silver.gbc", 0x00, 0x00);
    harness.runtime.open_rom_dialog_mode = super::super::OpenRomDialogMode::CgbInfraredSecondary;
    harness
        .runtime
        .open_rom_dialog
        .sender
        .send(PathDialogResult::Selected(PathBuf::from("silver.gbc")))
        .expect("CGB IR secondary ROM selection should send");
    harness
        .process_pending_open_rom_dialog()
        .expect("secondary ROM selection should activate CGB IR");

    let primary_reset_baseline = harness.machine.read_bus(0xC000);
    let secondary_reset_baseline = harness
        .machine
        .secondary_machine_mut()
        .expect("secondary CGB IR machine should exist")
        .read_bus(0xC000);
    harness.machine.write_bus(0xC000, 0xA5);
    harness
        .machine
        .secondary_machine_mut()
        .expect("secondary CGB IR machine should exist")
        .write_bus(0xC000, 0x3C);

    super::super::reset_machine(
        harness.canvas.window(),
        &mut harness.session,
        &mut harness.machine,
        &mut harness.runtime,
        &mut harness.settings_store,
    )
    .expect("CGB IR reset should rebuild a fresh linked runtime");

    assert_eq!(
        harness.session.external_port_selection,
        DesktopExternalPortSelection::None
    );
    assert!(harness.session.cgb_infrared_link_active());
    assert!(harness.session.linked_secondary_rom.is_some());
    assert_eq!(
        harness.machine.kind(),
        super::super::linked_session::DesktopEmulationSessionKind::LinkedCgbInfraredTwoPlayer
    );
    assert_eq!(harness.machine.read_bus(0xC000), primary_reset_baseline);
    assert_eq!(
        harness
            .machine
            .secondary_machine_mut()
            .expect("secondary CGB IR machine should exist")
            .read_bus(0xC000),
        secondary_reset_baseline
    );
}

#[test]
fn reset_keeps_the_pokemon_pikachu_color_runtime_active() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("pokemon-pikachu-color-reset", false, false, false);
    open_cgb_primary_rom(&mut harness, "gold.gbc", 0x00, 0x00);
    harness.session.pokemon_pikachu_color_gift = PokemonPikachuColorGift::Watts700;
    assert!(
        harness
            .execute_action(super::super::MenuAction::SetCgbInfraredPikachuColor)
            .expect("Pokemon Pikachu Color action should activate an accessory runtime")
            .is_none()
    );
    let reset_baseline = harness.machine.read_bus(0xC000);
    harness.machine.write_bus(0xC000, 0xA5);

    super::super::reset_machine(
        harness.canvas.window(),
        &mut harness.session,
        &mut harness.machine,
        &mut harness.runtime,
        &mut harness.settings_store,
    )
    .expect("Pokemon Pikachu Color reset should rebuild a fresh accessory runtime");

    assert!(harness.session.pokemon_pikachu_color_active());
    assert!(!harness.session.cgb_infrared_link_active());
    assert!(harness.session.linked_secondary_rom.is_none());
    assert_eq!(
        harness.machine.kind(),
        super::super::linked_session::DesktopEmulationSessionKind::PokemonPikachuColor
    );
    assert_eq!(harness.machine.read_bus(0xC000), reset_baseline);
    assert_eq!(
        harness
            .machine
            .pokemon_pikachu_color_status()
            .expect("Pokemon Pikachu Color status should be exposed after reset")
            .gift,
        PokemonPikachuColorGift::Watts700
    );
}

#[test]
fn reconfigure_keeps_the_pokemon_pikachu_color_runtime_active() {
    let _guard = crate::lock_sdl_test();
    let mut harness =
        FrontendHarness::new("pokemon-pikachu-color-reconfigure", false, false, false);
    open_cgb_primary_rom(&mut harness, "gold.gbc", 0x03, 0x02);
    harness.session.pokemon_pikachu_color_gift = PokemonPikachuColorGift::Watts700;
    assert!(
        harness
            .execute_action(super::super::MenuAction::SetCgbInfraredPikachuColor)
            .expect("Pokemon Pikachu Color action should activate an accessory runtime")
            .is_none()
    );

    assert!(
        harness.runtime.save_sessions[super::super::PlayerSlot::P1.index()].is_some(),
        "P1 save session should exist before reconfigure"
    );
    assert!(
        harness.runtime.save_sessions[super::super::PlayerSlot::P2.index()].is_none(),
        "Pokemon Pikachu Color should not open a P2 save session"
    );

    assert!(
        harness
            .execute_action(super::super::MenuAction::CycleSavePolicy)
            .expect("save policy change should rebuild the accessory runtime")
            .is_none()
    );

    assert_eq!(
        harness.session.config.saves.flush_policy,
        DesktopSaveFlushPolicy::Manual
    );
    assert!(harness.session.pokemon_pikachu_color_active());
    assert!(!harness.session.cgb_infrared_link_active());
    assert!(harness.session.linked_secondary_rom.is_none());
    assert_eq!(
        harness.session.external_port_selection,
        DesktopExternalPortSelection::None
    );
    assert_eq!(
        harness.machine.kind(),
        super::super::linked_session::DesktopEmulationSessionKind::PokemonPikachuColor
    );
    assert_eq!(
        harness
            .machine
            .pokemon_pikachu_color_status()
            .expect("Pokemon Pikachu Color status should be exposed after reconfigure")
            .gift,
        PokemonPikachuColorGift::Watts700
    );
    assert!(
        harness.runtime.save_sessions[super::super::PlayerSlot::P1.index()].is_some(),
        "P1 save session should remain open after reconfigure"
    );
    assert!(
        harness.runtime.save_sessions[super::super::PlayerSlot::P2.index()].is_none(),
        "Accessory reconfigure must not create a P2 save session"
    );
}

#[test]
fn pokemon_mystery_gift_action_activates_an_accessory_session() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("pokemon-mystery-gift-activate", false, false, false);
    open_cgb_primary_rom(&mut harness, "gold.gbc", 0x00, 0x00);
    harness.runtime.frame_blending_state.mode = DesktopFrameBlendingMode::On;
    harness.runtime.frame_blending_state.dimensions = Some(super::super::FramebufferDimensions {
        width: super::super::FRAMEBUFFER_WIDTH,
        height: super::super::FRAMEBUFFER_HEIGHT,
    });
    harness.runtime.frame_blending_state.previous_rgb_frame = vec![1, 2, 3];
    harness.runtime.frame_blending_state.has_previous_frame = true;

    assert!(
        harness
            .execute_action(super::super::MenuAction::SetCgbInfraredMysteryGift)
            .expect("Pokemon Mystery Gift action should activate an accessory runtime")
            .is_none()
    );

    assert_eq!(
        harness.session.external_port_selection,
        DesktopExternalPortSelection::None
    );
    assert!(!harness.session.cgb_infrared_link_active());
    assert!(!harness.session.pokemon_pikachu_color_active());
    assert!(harness.session.pokemon_mystery_gift_active());
    assert!(harness.session.linked_secondary_rom.is_none());
    assert_eq!(
        harness.machine.kind(),
        super::super::linked_session::DesktopEmulationSessionKind::PokemonMysteryGift
    );
    assert_eq!(
        harness.machine.linked_topology_kind(),
        LinkedTopologyKind::None
    );
    assert_eq!(
        harness.machine.external_port().attachment_kind(),
        ExternalPortAttachmentKind::None
    );
    assert_eq!(
        harness.runtime.frame_blending_state.mode,
        DesktopFrameBlendingMode::Off
    );
    assert!(harness.runtime.frame_blending_state.dimensions.is_none());
    assert!(
        harness
            .runtime
            .frame_blending_state
            .previous_rgb_frame
            .is_empty()
    );
    assert!(!harness.runtime.frame_blending_state.has_previous_frame);
    assert!(!super::super::machine_state_actions_available(
        &harness.session,
        &harness.machine
    ));
    assert!(!super::super::rewind_session_supported(
        &harness.session,
        &harness.machine
    ));
}

#[test]
fn pokemon_mystery_gift_actions_update_the_accessory() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("pokemon-mystery-gift-select", false, false, false);
    open_cgb_primary_rom(&mut harness, "gold.gbc", 0x00, 0x00);
    harness.session.pokemon_mystery_gift_kind = PokemonMysteryGiftKind::Decoration;
    harness.session.pokemon_mystery_gift_code = PokemonMysteryGiftCode::new(0x24).unwrap();

    assert!(
        harness
            .execute_action(super::super::MenuAction::SetCgbInfraredMysteryGift)
            .expect("Pokemon Mystery Gift action should activate an accessory runtime")
            .is_none()
    );
    let status = harness
        .machine
        .pokemon_mystery_gift_status()
        .expect("Pokemon Mystery Gift status should be exposed");
    assert_eq!(status.kind, PokemonMysteryGiftKind::Decoration);
    assert_eq!(status.code, PokemonMysteryGiftCode::new(0x24).unwrap());

    assert!(
        harness
            .execute_action(super::super::MenuAction::CycleCgbInfraredMysteryGiftKind)
            .expect("gift-kind action should cycle the accessory gift kind")
            .is_none()
    );
    assert!(
        harness
            .execute_action(super::super::MenuAction::CycleCgbInfraredMysteryGiftCode)
            .expect("gift-code action should cycle the accessory gift code")
            .is_none()
    );

    assert_eq!(
        harness.session.pokemon_mystery_gift_kind,
        PokemonMysteryGiftKind::Item
    );
    assert_eq!(
        harness.session.pokemon_mystery_gift_code,
        PokemonMysteryGiftCode::new(0x00).unwrap()
    );
    let status = harness
        .machine
        .pokemon_mystery_gift_status()
        .expect("Pokemon Mystery Gift status should stay exposed");
    assert_eq!(status.kind, PokemonMysteryGiftKind::Item);
    assert_eq!(status.code, PokemonMysteryGiftCode::new(0x00).unwrap());
}

#[test]
fn cgb_ir_none_action_turns_pokemon_mystery_gift_off() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("pokemon-mystery-gift-none", false, false, false);
    open_cgb_primary_rom(&mut harness, "gold.gbc", 0x00, 0x00);
    assert!(
        harness
            .execute_action(super::super::MenuAction::SetCgbInfraredMysteryGift)
            .expect("Pokemon Mystery Gift action should activate an accessory runtime")
            .is_none()
    );

    assert!(
        harness
            .execute_action(super::super::MenuAction::SetCgbInfraredNone)
            .expect("CGB IR NONE action should turn the accessory off")
            .is_none()
    );

    assert!(!harness.session.cgb_infrared_link_active());
    assert!(!harness.session.pokemon_mystery_gift_active());
    assert!(harness.session.linked_secondary_rom.is_none());
    assert_eq!(
        harness.machine.kind(),
        super::super::linked_session::DesktopEmulationSessionKind::Single
    );
}

#[test]
fn pokemon_mystery_gift_is_mutually_exclusive_with_other_cgb_ir_modes() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("pokemon-mystery-gift-exclusive", false, false, false);
    open_cgb_primary_rom(&mut harness, "gold.gbc", 0x00, 0x00);
    assert!(
        harness
            .execute_action(super::super::MenuAction::SetCgbInfraredMysteryGift)
            .expect("Pokemon Mystery Gift action should activate an accessory runtime")
            .is_none()
    );

    assert!(
        harness
            .execute_action(super::super::MenuAction::SetCgbInfraredSameGame)
            .expect("CGB IR SAME GAME should replace the accessory runtime")
            .is_none()
    );
    assert!(!harness.session.pokemon_mystery_gift_active());
    assert!(harness.session.cgb_infrared_link_active());
    assert_eq!(
        harness.machine.kind(),
        super::super::linked_session::DesktopEmulationSessionKind::LinkedCgbInfraredTwoPlayer
    );

    assert!(
        harness
            .execute_action(super::super::MenuAction::SetCgbInfraredMysteryGift)
            .expect("Pokemon Mystery Gift should replace the CGB IR pair")
            .is_none()
    );
    assert!(harness.session.pokemon_mystery_gift_active());
    assert!(!harness.session.cgb_infrared_link_active());
    assert!(harness.session.linked_secondary_rom.is_none());
    assert_eq!(
        harness.machine.kind(),
        super::super::linked_session::DesktopEmulationSessionKind::PokemonMysteryGift
    );

    assert!(
        harness
            .execute_action(super::super::MenuAction::SetCgbInfraredPikachuColor)
            .expect("Pokemon Pikachu Color should replace Pokemon Mystery Gift")
            .is_none()
    );
    assert!(harness.session.pokemon_pikachu_color_active());
    assert!(!harness.session.pokemon_mystery_gift_active());
    assert_eq!(
        harness.machine.kind(),
        super::super::linked_session::DesktopEmulationSessionKind::PokemonPikachuColor
    );
}

#[test]
fn reset_keeps_the_pokemon_mystery_gift_runtime_active() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("pokemon-mystery-gift-reset", false, false, false);
    open_cgb_primary_rom(&mut harness, "gold.gbc", 0x00, 0x00);
    harness.session.pokemon_mystery_gift_kind = PokemonMysteryGiftKind::Decoration;
    harness.session.pokemon_mystery_gift_code = PokemonMysteryGiftCode::new(0x0D).unwrap();
    assert!(
        harness
            .execute_action(super::super::MenuAction::SetCgbInfraredMysteryGift)
            .expect("Pokemon Mystery Gift action should activate an accessory runtime")
            .is_none()
    );
    let reset_baseline = harness.machine.read_bus(0xC000);
    harness.machine.write_bus(0xC000, 0xA5);

    super::super::reset_machine(
        harness.canvas.window(),
        &mut harness.session,
        &mut harness.machine,
        &mut harness.runtime,
        &mut harness.settings_store,
    )
    .expect("Pokemon Mystery Gift reset should rebuild a fresh accessory runtime");

    assert!(harness.session.pokemon_mystery_gift_active());
    assert!(!harness.session.cgb_infrared_link_active());
    assert!(harness.session.linked_secondary_rom.is_none());
    assert_eq!(
        harness.machine.kind(),
        super::super::linked_session::DesktopEmulationSessionKind::PokemonMysteryGift
    );
    assert_eq!(harness.machine.read_bus(0xC000), reset_baseline);
    let status = harness
        .machine
        .pokemon_mystery_gift_status()
        .expect("Pokemon Mystery Gift status should be exposed after reset");
    assert_eq!(status.kind, PokemonMysteryGiftKind::Decoration);
    assert_eq!(status.code, PokemonMysteryGiftCode::new(0x0D).unwrap());
}

#[test]
fn reconfigure_keeps_the_pokemon_mystery_gift_runtime_active() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("pokemon-mystery-gift-reconfigure", false, false, false);
    open_cgb_primary_rom(&mut harness, "gold.gbc", 0x03, 0x02);
    harness.session.pokemon_mystery_gift_kind = PokemonMysteryGiftKind::Decoration;
    harness.session.pokemon_mystery_gift_code = PokemonMysteryGiftCode::new(0x0D).unwrap();
    assert!(
        harness
            .execute_action(super::super::MenuAction::SetCgbInfraredMysteryGift)
            .expect("Pokemon Mystery Gift action should activate an accessory runtime")
            .is_none()
    );

    assert!(
        harness.runtime.save_sessions[super::super::PlayerSlot::P1.index()].is_some(),
        "P1 save session should exist before reconfigure"
    );
    assert!(
        harness.runtime.save_sessions[super::super::PlayerSlot::P2.index()].is_none(),
        "Pokemon Mystery Gift should not open a P2 save session"
    );

    assert!(
        harness
            .execute_action(super::super::MenuAction::CycleSavePolicy)
            .expect("save policy change should rebuild the accessory runtime")
            .is_none()
    );

    assert_eq!(
        harness.session.config.saves.flush_policy,
        DesktopSaveFlushPolicy::Manual
    );
    assert!(harness.session.pokemon_mystery_gift_active());
    assert!(!harness.session.cgb_infrared_link_active());
    assert!(harness.session.linked_secondary_rom.is_none());
    assert_eq!(
        harness.session.external_port_selection,
        DesktopExternalPortSelection::None
    );
    assert_eq!(
        harness.machine.kind(),
        super::super::linked_session::DesktopEmulationSessionKind::PokemonMysteryGift
    );
    let status = harness
        .machine
        .pokemon_mystery_gift_status()
        .expect("Pokemon Mystery Gift status should be exposed after reconfigure");
    assert_eq!(status.kind, PokemonMysteryGiftKind::Decoration);
    assert_eq!(status.code, PokemonMysteryGiftCode::new(0x0D).unwrap());
    assert!(
        harness.runtime.save_sessions[super::super::PlayerSlot::P1.index()].is_some(),
        "P1 save session should remain open after reconfigure"
    );
    assert!(
        harness.runtime.save_sessions[super::super::PlayerSlot::P2.index()].is_none(),
        "Accessory reconfigure must not create a P2 save session"
    );
}

#[test]
fn cgb_ir_save_sessions_use_the_secondary_rom_with_the_p2_extension() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("cgb-ir-save-keys", false, false, false);
    open_cgb_primary_rom(&mut harness, "gold.gbc", 0x03, 0x02);
    write_cgb_test_rom(&harness.root, "silver.gbc", 0x03, 0x02);

    harness.runtime.open_rom_dialog_mode = super::super::OpenRomDialogMode::CgbInfraredSecondary;
    harness
        .runtime
        .open_rom_dialog
        .sender
        .send(PathDialogResult::Selected(PathBuf::from("silver.gbc")))
        .expect("CGB IR secondary ROM selection should send");
    harness
        .process_pending_open_rom_dialog()
        .expect("secondary ROM selection should activate CGB IR");

    let save_root = harness.root.join("saves");
    let p1_save_session = harness.runtime.save_sessions[super::super::PlayerSlot::P1.index()]
        .as_ref()
        .expect("CGB IR P1 should have a save session");
    let p2_save_session = harness.runtime.save_sessions[super::super::PlayerSlot::P2.index()]
        .as_ref()
        .expect("CGB IR P2 should have a save session");
    assert_eq!(p1_save_session.save_path(), save_root.join("gold.sav"));
    assert_eq!(p2_save_session.save_path(), save_root.join("silver.sa2"));
    assert!(harness.runtime.save_sessions[super::super::PlayerSlot::P3.index()].is_none());
    assert!(harness.runtime.save_sessions[super::super::PlayerSlot::P4.index()].is_none());
}

#[test]
fn game_link_same_game_save_sessions_use_player_slot_file_extensions() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("game-link-same-save-keys", false, false, false);
    let rom_name = "battery.gb";
    let rom_path = harness.root.join(rom_name);
    fs::write(&rom_path, build_test_rom(32 * 1024, 0x03, 0x00, 0x02))
        .expect("battery-backed ROM should be writable");

    harness
        .runtime
        .open_rom_dialog
        .sender
        .send(PathDialogResult::Selected(PathBuf::from(rom_name)))
        .expect("open ROM selection should send");
    harness
        .process_pending_open_rom_dialog()
        .expect("battery-backed ROM should load");

    assert!(
        harness
            .execute_action(super::super::MenuAction::SetGameLinkSameGame)
            .expect("SAME GAME should activate a DMG-04 session")
            .is_none()
    );

    let save_root = harness.root.join("saves");
    let p1_save_session = harness.runtime.save_sessions[super::super::PlayerSlot::P1.index()]
        .as_ref()
        .expect("GAME LINK P1 should have a save session");
    let p2_save_session = harness.runtime.save_sessions[super::super::PlayerSlot::P2.index()]
        .as_ref()
        .expect("GAME LINK P2 should have a save session");
    assert_eq!(
        harness.session.linked_secondary_rom_path(),
        Some(rom_path.as_path())
    );
    assert_eq!(p1_save_session.save_path(), save_root.join("battery.sav"));
    assert_eq!(p2_save_session.save_path(), save_root.join("battery.sa2"));
    assert!(harness.runtime.save_sessions[super::super::PlayerSlot::P3.index()].is_none());
    assert!(harness.runtime.save_sessions[super::super::PlayerSlot::P4.index()].is_none());
}

#[test]
fn cgb_ir_same_game_save_sessions_use_the_primary_rom_with_the_p2_extension() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("cgb-ir-same-game-save-keys", false, false, false);
    open_cgb_primary_rom(&mut harness, "gold.gbc", 0x03, 0x02);

    assert!(
        harness
            .execute_action(super::super::MenuAction::SetCgbInfraredSameGame)
            .expect("CGB IR SAME GAME action should activate a two-console runtime")
            .is_none()
    );

    let save_root = harness.root.join("saves");
    let p1_save_session = harness.runtime.save_sessions[super::super::PlayerSlot::P1.index()]
        .as_ref()
        .expect("CGB IR SAME GAME P1 should have a save session");
    let p2_save_session = harness.runtime.save_sessions[super::super::PlayerSlot::P2.index()]
        .as_ref()
        .expect("CGB IR SAME GAME P2 should have a save session");
    assert_eq!(p1_save_session.save_path(), save_root.join("gold.sav"));
    assert_eq!(p2_save_session.save_path(), save_root.join("gold.sa2"));
    assert!(harness.runtime.save_sessions[super::super::PlayerSlot::P3.index()].is_none());
    assert!(harness.runtime.save_sessions[super::super::PlayerSlot::P4.index()].is_none());
}

#[test]
fn game_link_select_game_action_switches_the_open_rom_dialog_into_secondary_mode() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("game-link-action", true, false, false);
    harness.runtime.open_rom_dialog.pending = true;

    assert!(
        harness
            .execute_action(super::super::MenuAction::SelectGameLinkRom)
            .expect("GAME LINK action should not fail when the open dialog is already pending")
            .is_none()
    );
    assert_eq!(
        harness.runtime.open_rom_dialog_mode,
        super::super::OpenRomDialogMode::LinkedSecondary
    );
}

#[test]
fn four_player_adapter_action_clones_the_primary_rom_without_opening_a_dialog() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("dmg07-activate", true, false, false);
    for _ in 0..256 {
        harness.machine.step_t_cycle();
    }
    assert_ne!(
        harness.machine.primary_machine().next_t_cycle(),
        gb_core::TCycle::ZERO
    );

    assert!(
        harness
            .execute_action(super::super::MenuAction::SetFourPlayerAdapter(
                super::super::DesktopDmg07PlayerCount::Three,
            ))
            .expect("4 PLAYER ADAPTER action should build a fresh DMG-07 session")
            .is_none()
    );

    assert_eq!(
        harness.session.external_port_selection,
        DesktopExternalPortSelection::FourPlayerAdapter
    );
    assert_eq!(
        harness.session.dmg07_player_count,
        Some(super::super::DesktopDmg07PlayerCount::Three)
    );
    assert!(harness.session.linked_secondary_rom.is_none());
    assert!(!harness.runtime.open_rom_dialog.is_pending());
    assert_eq!(
        harness.runtime.open_rom_dialog_mode,
        super::super::OpenRomDialogMode::Primary
    );
    assert_eq!(
        harness.machine.kind(),
        super::super::linked_session::DesktopEmulationSessionKind::LinkedDmg07 {
            player_count: super::super::DesktopDmg07PlayerCount::Three,
        }
    );
    assert_eq!(
        harness.machine.linked_topology_kind(),
        LinkedTopologyKind::Dmg07
    );
    assert_eq!(
        harness.machine.primary_machine().next_t_cycle(),
        gb_core::TCycle::ZERO
    );
    assert_dmg07_slot_port(
        &harness.machine,
        super::super::PlayerSlot::P1,
        Dmg07Port::P1,
    );
    assert_dmg07_slot_port(
        &harness.machine,
        super::super::PlayerSlot::P2,
        Dmg07Port::P2,
    );
    assert_dmg07_slot_port(
        &harness.machine,
        super::super::PlayerSlot::P3,
        Dmg07Port::P3,
    );
    assert!(
        harness
            .machine
            .machine_for_player_slot(super::super::PlayerSlot::P4)
            .is_none()
    );
}

#[test]
fn reset_keeps_the_dmg07_runtime_active_with_the_same_player_count() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("dmg07-reset", true, false, false);
    assert!(
        harness
            .execute_action(super::super::MenuAction::SetFourPlayerAdapter(
                super::super::DesktopDmg07PlayerCount::Four,
            ))
            .expect("4 PLAYER ADAPTER action should activate")
            .is_none()
    );

    let p1_reset_baseline = harness.machine.read_bus(0xC000);
    let p4_reset_baseline = harness
        .machine
        .machine_for_player_slot_mut(super::super::PlayerSlot::P4)
        .expect("P4 should map to the fourth DMG-07 machine")
        .read_bus(0xC000);
    harness.machine.write_bus(0xC000, 0xA5);
    harness
        .machine
        .machine_for_player_slot_mut(super::super::PlayerSlot::P4)
        .expect("P4 should map to the fourth DMG-07 machine")
        .write_bus(0xC000, 0x3C);

    super::super::reset_machine(
        harness.canvas.window(),
        &mut harness.session,
        &mut harness.machine,
        &mut harness.runtime,
        &mut harness.settings_store,
    )
    .expect("DMG-07 reset should rebuild a fresh linked runtime");

    assert_eq!(
        harness.session.external_port_selection,
        DesktopExternalPortSelection::FourPlayerAdapter
    );
    assert_eq!(
        harness.session.dmg07_player_count,
        Some(super::super::DesktopDmg07PlayerCount::Four)
    );
    assert_eq!(
        harness.machine.kind(),
        super::super::linked_session::DesktopEmulationSessionKind::LinkedDmg07 {
            player_count: super::super::DesktopDmg07PlayerCount::Four,
        }
    );
    assert_eq!(harness.machine.read_bus(0xC000), p1_reset_baseline);
    assert_eq!(
        harness
            .machine
            .machine_for_player_slot_mut(super::super::PlayerSlot::P4)
            .expect("P4 should stay active after reset")
            .read_bus(0xC000),
        p4_reset_baseline
    );
}

#[test]
fn dmg07_pocket_camera_frame_survives_reconfigure_and_reset_on_all_slots() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("dmg07-camera-frame-reset", false, false, false);
    let camera_rom_name = "camera.gb";
    write_test_camera_rom(&harness.root, camera_rom_name);
    harness
        .runtime
        .open_rom_dialog
        .sender
        .send(PathDialogResult::Selected(PathBuf::from(camera_rom_name)))
        .expect("Pocket Camera ROM selection should send");
    harness
        .process_pending_open_rom_dialog()
        .expect("Pocket Camera ROM should load");
    assert!(
        harness
            .execute_action(super::super::MenuAction::SetFourPlayerAdapter(
                super::super::DesktopDmg07PlayerCount::Four,
            ))
            .expect("4 PLAYER ADAPTER action should activate for a camera ROM")
            .is_none()
    );

    let png_path = write_grayscale_png(&harness.root, "camera.png", 1, 1, &[0x00]);
    harness
        .runtime
        .camera_image_dialog
        .sender
        .send(PathDialogResult::Selected(png_path))
        .expect("Pocket Camera image selection should send");
    harness
        .process_pending_camera_image_dialog()
        .expect("Pocket Camera image dialog should complete");

    let expected_black_tiles = [
        0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00,
        0xFF,
    ];
    assert_eq!(
        capture_camera_tile_bytes(
            harness
                .machine
                .machine_for_player_slot_mut(super::super::PlayerSlot::P4)
                .expect("P4 should be a cloned Pocket Camera machine")
        ),
        expected_black_tiles,
        "loading a static Pocket Camera image should apply to all DMG-07 slots"
    );

    assert!(
        harness
            .execute_action(super::super::MenuAction::CycleSavePolicy)
            .expect("save policy change should rebuild the DMG-07 camera session")
            .is_none()
    );
    assert_eq!(
        capture_camera_tile_bytes(
            harness
                .machine
                .machine_for_player_slot_mut(super::super::PlayerSlot::P4)
                .expect("P4 should remain active after reconfigure")
        ),
        expected_black_tiles,
        "DMG-07 reconfigure should reapply the session Pocket Camera frame"
    );

    super::super::reset_machine(
        harness.canvas.window(),
        &mut harness.session,
        &mut harness.machine,
        &mut harness.runtime,
        &mut harness.settings_store,
    )
    .expect("DMG-07 reset should rebuild the camera session");
    assert_eq!(
        capture_camera_tile_bytes(
            harness
                .machine
                .machine_for_player_slot_mut(super::super::PlayerSlot::P4)
                .expect("P4 should remain active after reset")
        ),
        expected_black_tiles,
        "DMG-07 reset should reapply the session Pocket Camera frame"
    );
}

#[test]
fn selecting_none_or_printer_tears_dmg07_down_to_single_p1() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("dmg07-teardown", true, false, false);
    assert!(
        harness
            .execute_action(super::super::MenuAction::SetFourPlayerAdapter(
                super::super::DesktopDmg07PlayerCount::Two,
            ))
            .expect("4 PLAYER ADAPTER action should activate")
            .is_none()
    );
    harness.machine.write_bus(0xC000, 0x44);
    harness
        .machine
        .machine_for_player_slot_mut(super::super::PlayerSlot::P2)
        .expect("P2 should be active before teardown")
        .write_bus(0xC000, 0x77);

    assert!(
        harness
            .execute_action(super::super::MenuAction::SetExternalPort(
                DesktopExternalPortSelection::None,
            ))
            .expect("NONE should tear down the DMG-07 session")
            .is_none()
    );

    assert_eq!(
        harness.session.external_port_selection,
        DesktopExternalPortSelection::None
    );
    assert_eq!(harness.session.dmg07_player_count, None);
    assert_eq!(
        harness.machine.kind(),
        super::super::linked_session::DesktopEmulationSessionKind::Single
    );
    assert_eq!(harness.machine.read_bus(0xC000), 0x44);
    assert!(
        harness
            .machine
            .machine_for_player_slot(super::super::PlayerSlot::P2)
            .is_none()
    );

    assert!(
        harness
            .execute_action(super::super::MenuAction::SetFourPlayerAdapter(
                super::super::DesktopDmg07PlayerCount::Two,
            ))
            .expect("4 PLAYER ADAPTER action should activate again")
            .is_none()
    );
    assert!(
        harness
            .execute_action(super::super::MenuAction::SetExternalPort(
                DesktopExternalPortSelection::Printer,
            ))
            .expect("PRINTER should tear down the DMG-07 session")
            .is_none()
    );

    assert_eq!(
        harness.session.external_port_selection,
        DesktopExternalPortSelection::Printer
    );
    assert_eq!(harness.session.dmg07_player_count, None);
    assert_eq!(
        harness.machine.kind(),
        super::super::linked_session::DesktopEmulationSessionKind::Single
    );
    assert_eq!(
        harness.machine.external_port().attachment_kind(),
        ExternalPortAttachmentKind::Printer
    );
}

#[test]
fn dmg07_save_sessions_use_player_slot_file_extensions() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("dmg07-save-keys", false, false, false);
    let rom_name = "battery.gb";
    let rom_path = harness.root.join(rom_name);
    fs::write(&rom_path, build_test_rom(32 * 1024, 0x03, 0x00, 0x02))
        .expect("battery-backed ROM should be writable");

    harness
        .runtime
        .open_rom_dialog
        .sender
        .send(PathDialogResult::Selected(PathBuf::from(rom_name)))
        .expect("open ROM selection should send");
    harness
        .process_pending_open_rom_dialog()
        .expect("battery-backed ROM should load");

    assert!(
        harness
            .execute_action(super::super::MenuAction::SetFourPlayerAdapter(
                super::super::DesktopDmg07PlayerCount::Four,
            ))
            .expect("4 PLAYER ADAPTER action should activate")
            .is_none()
    );

    let save_root = harness.root.join("saves");
    let expected_paths = [
        save_root.join("battery.sav"),
        save_root.join("battery.sa2"),
        save_root.join("battery.sa3"),
        save_root.join("battery.sa4"),
    ];
    for (slot, expected_path) in super::super::PlayerSlot::ALL
        .into_iter()
        .zip(expected_paths)
    {
        let save_session = harness.runtime.save_sessions[slot.index()]
            .as_ref()
            .unwrap_or_else(|| panic!("{} should have its own save session", slot.label()));
        assert_eq!(save_session.save_path(), expected_path);
    }
}
