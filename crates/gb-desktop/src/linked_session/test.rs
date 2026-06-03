use super::{DesktopEmulationSession, DesktopEmulationSessionKind, format_linked_machines_error};
use crate::player_slots::{DesktopDmg07PlayerCount, PlayerSlot};
use gb_core::{
    ConsoleModel, Dmg07Port, ExternalPortAttachmentKind, ExternalPortAttachmentSnapshot,
    JoypadButton, LinkedMachinesError, LinkedTopologyKind, Machine, MachineConfig,
    MachineStepObserver, MachineStepRegion, StartupMode, TCycle, TraceSummaryBuffer,
};
use std::collections::HashMap;

fn dmg_skip_boot_summary_machine() -> Machine<TraceSummaryBuffer> {
    Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    )
}

#[derive(Default)]
struct CountingObserver {
    begins: HashMap<MachineStepRegion, usize>,
    ends: HashMap<MachineStepRegion, usize>,
}

impl MachineStepObserver for CountingObserver {
    fn begin_region(&mut self, region: MachineStepRegion) {
        *self.begins.entry(region).or_default() += 1;
    }

    fn end_region(&mut self, region: MachineStepRegion) {
        *self.ends.entry(region).or_default() += 1;
    }
}

#[test]
fn linked_session_rejects_mismatched_machine_timelines() {
    let primary = dmg_skip_boot_summary_machine();
    let mut secondary = dmg_skip_boot_summary_machine();
    secondary.step_t_cycle();

    let error = DesktopEmulationSession::new_linked_dmg04_two_player(primary, secondary)
        .expect_err("desynchronized machines should be rejected");

    assert!(error.contains("must share the same next T-cycle"));
    assert!(error.contains("machine index 1"));
}

#[test]
fn attach_secondary_rejects_relinking_and_detach_is_a_single_session_noop() {
    let mut session = DesktopEmulationSession::new_single(dmg_skip_boot_summary_machine());
    let next_t_cycle_before = session.next_t_cycle();
    session.detach_to_single_primary();
    assert_eq!(session.kind(), DesktopEmulationSessionKind::Single);
    assert_eq!(session.next_t_cycle(), next_t_cycle_before);
    assert!(session.secondary_machine().is_none());

    let mut desynchronized_secondary = dmg_skip_boot_summary_machine();
    desynchronized_secondary.step_t_cycle();
    let error = session
        .attach_secondary_dmg04(desynchronized_secondary)
        .expect_err("desynchronized secondary machine should be rejected");
    assert!(error.contains("must share the same next T-cycle"));
    assert_eq!(session.kind(), DesktopEmulationSessionKind::Single);
    assert_eq!(session.next_t_cycle(), next_t_cycle_before);
    assert!(session.secondary_machine().is_none());

    session
        .attach_secondary_dmg04(dmg_skip_boot_summary_machine())
        .expect("first secondary machine should attach");
    assert_eq!(
        session.kind(),
        DesktopEmulationSessionKind::LinkedDmg04TwoPlayer
    );

    let error = session
        .attach_secondary_dmg04(dmg_skip_boot_summary_machine())
        .expect_err("relinking an already linked session should fail");
    assert_eq!(
        error,
        "desktop emulation session is already running a linked DMG-04 runtime"
    );
    assert!(session.secondary_machine().is_some());
}

#[test]
fn new_linked_dmg07_maps_contiguous_player_slots_to_physical_ports() {
    let mut session = DesktopEmulationSession::new_linked_dmg07(
        vec![
            dmg_skip_boot_summary_machine(),
            dmg_skip_boot_summary_machine(),
            dmg_skip_boot_summary_machine(),
        ],
        DesktopDmg07PlayerCount::Three,
    )
    .expect("three-player desktop DMG-07 session should build");

    assert_eq!(
        session.kind(),
        DesktopEmulationSessionKind::LinkedDmg07 {
            player_count: DesktopDmg07PlayerCount::Three,
        }
    );
    assert_eq!(session.linked_topology_kind(), LinkedTopologyKind::Dmg07);
    assert_eq!(
        session.dmg07_player_count(),
        Some(DesktopDmg07PlayerCount::Three)
    );
    for (slot, port) in [
        (PlayerSlot::P1, Dmg07Port::P1),
        (PlayerSlot::P2, Dmg07Port::P2),
        (PlayerSlot::P3, Dmg07Port::P3),
    ] {
        let machine = session
            .machine_for_player_slot(slot)
            .expect("active DMG-07 slot should map to a machine");
        assert_eq!(
            machine.external_port().attachment_kind(),
            ExternalPortAttachmentKind::FourPlayerAdapterDmg07
        );
        assert_eq!(
            machine.external_port().snapshot().attachment,
            ExternalPortAttachmentSnapshot::FourPlayerAdapterDmg07 {
                port,
                incoming_byte: None,
            }
        );
    }
    assert!(session.machine_for_player_slot(PlayerSlot::P4).is_none());

    session.step_t_cycle();
    assert_eq!(session.next_t_cycle(), TCycle::new(1));
}

#[test]
fn new_linked_dmg07_rejects_wrong_machine_count_before_core_attach() {
    let error = DesktopEmulationSession::new_linked_dmg07(
        vec![
            dmg_skip_boot_summary_machine(),
            dmg_skip_boot_summary_machine(),
        ],
        DesktopDmg07PlayerCount::Four,
    )
    .expect_err("four-player desktop DMG-07 session requires four machines");

    assert_eq!(
        error,
        "DMG-07 desktop session for 4 players requires 4 machines, found 2"
    );
}

#[test]
fn step_t_cycle_with_observer_covers_single_and_linked_sessions() {
    let mut single = DesktopEmulationSession::new_single(dmg_skip_boot_summary_machine());
    let mut observer = CountingObserver::default();
    single.step_t_cycle_with_observer(&mut observer);

    assert!(
        !observer
            .begins
            .contains_key(&MachineStepRegion::ExternalEvents)
    );
    assert!(observer.begins.contains_key(&MachineStepRegion::Cpu));
    assert_eq!(
        observer.begins.get(&MachineStepRegion::Cpu),
        observer.ends.get(&MachineStepRegion::Cpu)
    );

    single
        .primary_machine_mut()
        .set_joypad_button_pressed(JoypadButton::A, true);
    let mut pending_observer = CountingObserver::default();
    single.step_t_cycle_with_observer(&mut pending_observer);
    assert!(
        pending_observer
            .begins
            .contains_key(&MachineStepRegion::ExternalEvents)
    );
    assert_eq!(
        pending_observer
            .begins
            .get(&MachineStepRegion::ExternalEvents),
        pending_observer
            .ends
            .get(&MachineStepRegion::ExternalEvents)
    );

    let mut linked = DesktopEmulationSession::new_linked_dmg04_two_player(
        dmg_skip_boot_summary_machine(),
        dmg_skip_boot_summary_machine(),
    )
    .expect("linked desktop session should build");
    let mut linked_observer = CountingObserver::default();
    linked.step_t_cycle_with_observer(&mut linked_observer);

    assert!(
        !linked_observer
            .begins
            .contains_key(&MachineStepRegion::ExternalEvents)
    );
    assert!(linked_observer.begins.contains_key(&MachineStepRegion::Cpu));
    assert_eq!(
        linked_observer.begins.get(&MachineStepRegion::Cpu),
        linked_observer.ends.get(&MachineStepRegion::Cpu)
    );
    assert_eq!(linked.next_t_cycle(), TCycle::new(1));

    linked
        .primary_machine_mut()
        .set_joypad_button_pressed(JoypadButton::A, true);
    let mut linked_pending_observer = CountingObserver::default();
    linked.step_t_cycle_with_observer(&mut linked_pending_observer);
    assert!(
        linked_pending_observer
            .begins
            .contains_key(&MachineStepRegion::ExternalEvents)
    );
    assert_eq!(
        linked_pending_observer
            .begins
            .get(&MachineStepRegion::ExternalEvents),
        linked_pending_observer
            .ends
            .get(&MachineStepRegion::ExternalEvents)
    );
    assert_eq!(linked.next_t_cycle(), TCycle::new(2));
}

#[test]
fn primary_machine_extraction_and_error_formatting_cover_remaining_linked_helpers() {
    let single = DesktopEmulationSession::new_single(dmg_skip_boot_summary_machine());
    let primary = single.into_primary_machine();
    assert_eq!(
        primary.external_port().attachment_kind(),
        gb_core::ExternalPortAttachmentKind::None
    );

    assert_eq!(
        format_linked_machines_error(LinkedMachinesError::TooFewMachines { count: 1 }),
        "linked desktop session requires at least two machines, found 1"
    );
    assert_eq!(
        format_linked_machines_error(LinkedMachinesError::UnsupportedMachineCountForDmg04 {
            count: 3
        }),
        "DMG-04 desktop sessions currently require exactly two machines, found 3"
    );
    assert_eq!(
        format_linked_machines_error(LinkedMachinesError::UnsupportedMachineCountForCgbInfrared {
            count: 3
        }),
        "CGB infrared linked sessions require exactly two machines, found 3"
    );
    assert_eq!(
        format_linked_machines_error(
            LinkedMachinesError::InvalidCgbInfraredOpticalPropagationDelay {
                requested_t_cycles: 0,
                min_t_cycles: 1,
                max_t_cycles: 256,
            }
        ),
        "CGB infrared optical propagation delay must be between 1 and 256 T-cycles, got 0"
    );
}
