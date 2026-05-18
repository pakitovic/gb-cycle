use crate::player_slots::{DesktopDmg07PlayerCount, PlayerSlot};
#[cfg(test)]
use gb_core::LinkedTopologyKind;
use gb_core::{
    ConsoleModel, Dmg07Participant, Dmg07Port, LinkedMachines, LinkedMachinesError, Machine,
    MachineConfig, MachineStepObserver, StartupMode, TraceSummaryBuffer,
};
use std::ops::{Deref, DerefMut};

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopEmulationSessionKind {
    Single,
    LinkedDmg04TwoPlayer,
    LinkedCgbInfraredTwoPlayer,
    LinkedDmg07 {
        player_count: DesktopDmg07PlayerCount,
    },
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone)]
pub enum DesktopEmulationSession {
    Single(Box<Machine<TraceSummaryBuffer>>),
    LinkedDmg04TwoPlayer(Box<LinkedMachines<TraceSummaryBuffer>>),
    LinkedCgbInfraredTwoPlayer(Box<LinkedMachines<TraceSummaryBuffer>>),
    LinkedDmg07 {
        linked: Box<LinkedMachines<TraceSummaryBuffer>>,
        player_count: DesktopDmg07PlayerCount,
    },
}

impl DesktopEmulationSession {
    pub fn new_single(machine: Machine<TraceSummaryBuffer>) -> Self {
        Self::Single(Box::new(machine))
    }

    pub fn new_linked_dmg04_two_player(
        primary_machine: Machine<TraceSummaryBuffer>,
        secondary_machine: Machine<TraceSummaryBuffer>,
    ) -> Result<Self, String> {
        let mut linked = LinkedMachines::new(vec![primary_machine, secondary_machine])
            .map_err(format_linked_machines_error)?;
        linked
            .attach_dmg04_cable()
            .map_err(format_linked_machines_error)?;
        Ok(Self::LinkedDmg04TwoPlayer(Box::new(linked)))
    }

    pub fn new_linked_cgb_infrared_two_player(
        primary_machine: Machine<TraceSummaryBuffer>,
        secondary_machine: Machine<TraceSummaryBuffer>,
    ) -> Result<Self, String> {
        let mut linked = LinkedMachines::new(vec![primary_machine, secondary_machine])
            .map_err(format_linked_machines_error)?;
        linked
            .attach_cgb_infrared_pair()
            .map_err(format_linked_machines_error)?;
        Ok(Self::LinkedCgbInfraredTwoPlayer(Box::new(linked)))
    }

    pub fn new_linked_dmg07(
        machines: Vec<Machine<TraceSummaryBuffer>>,
        player_count: DesktopDmg07PlayerCount,
    ) -> Result<Self, String> {
        if machines.len() != player_count.get() {
            return Err(format!(
                "DMG-07 desktop session for {} players requires {} machines, found {}",
                player_count.get(),
                player_count.get(),
                machines.len()
            ));
        }

        let mut linked = LinkedMachines::new(machines).map_err(format_linked_machines_error)?;
        let participants = dmg07_participants_for_player_count(player_count);
        linked
            .attach_dmg07_adapter(&participants)
            .map_err(format_linked_machines_error)?;
        Ok(Self::LinkedDmg07 {
            linked: Box::new(linked),
            player_count,
        })
    }

    #[cfg(test)]
    pub const fn kind(&self) -> DesktopEmulationSessionKind {
        match self {
            Self::Single(_) => DesktopEmulationSessionKind::Single,
            Self::LinkedDmg04TwoPlayer(_) => DesktopEmulationSessionKind::LinkedDmg04TwoPlayer,
            Self::LinkedCgbInfraredTwoPlayer(_) => {
                DesktopEmulationSessionKind::LinkedCgbInfraredTwoPlayer
            }
            Self::LinkedDmg07 { player_count, .. } => DesktopEmulationSessionKind::LinkedDmg07 {
                player_count: *player_count,
            },
        }
    }

    #[cfg(test)]
    pub fn linked_topology_kind(&self) -> LinkedTopologyKind {
        match self {
            Self::Single(_) => LinkedTopologyKind::None,
            Self::LinkedDmg04TwoPlayer(linked) => linked.topology_kind(),
            Self::LinkedCgbInfraredTwoPlayer(linked) => linked.topology_kind(),
            Self::LinkedDmg07 { linked, .. } => linked.topology_kind(),
        }
    }

    pub fn primary_machine(&self) -> &Machine<TraceSummaryBuffer> {
        match self {
            Self::Single(machine) => machine,
            Self::LinkedDmg04TwoPlayer(linked) => linked
                .machine(0)
                .expect("linked desktop session should always have a primary machine"),
            Self::LinkedCgbInfraredTwoPlayer(linked) => linked
                .machine(0)
                .expect("linked desktop session should always have a primary machine"),
            Self::LinkedDmg07 { linked, .. } => linked
                .machine(0)
                .expect("linked desktop session should always have a primary machine"),
        }
    }

    pub fn primary_machine_mut(&mut self) -> &mut Machine<TraceSummaryBuffer> {
        match self {
            Self::Single(machine) => machine,
            Self::LinkedDmg04TwoPlayer(linked) => linked
                .machine_mut(0)
                .expect("linked desktop session should always have a primary machine"),
            Self::LinkedCgbInfraredTwoPlayer(linked) => linked
                .machine_mut(0)
                .expect("linked desktop session should always have a primary machine"),
            Self::LinkedDmg07 { linked, .. } => linked
                .machine_mut(0)
                .expect("linked desktop session should always have a primary machine"),
        }
    }

    pub fn secondary_machine(&self) -> Option<&Machine<TraceSummaryBuffer>> {
        match self {
            Self::Single(_) => None,
            Self::LinkedDmg04TwoPlayer(linked) => linked.machine(1),
            Self::LinkedCgbInfraredTwoPlayer(linked) => linked.machine(1),
            Self::LinkedDmg07 { linked, .. } => linked.machine(1),
        }
    }

    pub fn secondary_machine_mut(&mut self) -> Option<&mut Machine<TraceSummaryBuffer>> {
        match self {
            Self::Single(_) => None,
            Self::LinkedDmg04TwoPlayer(linked) => linked.machine_mut(1),
            Self::LinkedCgbInfraredTwoPlayer(linked) => linked.machine_mut(1),
            Self::LinkedDmg07 { linked, .. } => linked.machine_mut(1),
        }
    }

    pub fn machine_for_player_slot(
        &self,
        slot: PlayerSlot,
    ) -> Option<&Machine<TraceSummaryBuffer>> {
        match slot {
            PlayerSlot::P1 => Some(self.primary_machine()),
            PlayerSlot::P2 => self.secondary_machine(),
            PlayerSlot::P3 | PlayerSlot::P4 => match self {
                Self::LinkedDmg07 { linked, .. } => linked.machine(slot.machine_index()),
                Self::Single(_)
                | Self::LinkedDmg04TwoPlayer(_)
                | Self::LinkedCgbInfraredTwoPlayer(_) => None,
            },
        }
    }

    pub fn machine_for_player_slot_mut(
        &mut self,
        slot: PlayerSlot,
    ) -> Option<&mut Machine<TraceSummaryBuffer>> {
        match slot {
            PlayerSlot::P1 => Some(self.primary_machine_mut()),
            PlayerSlot::P2 => self.secondary_machine_mut(),
            PlayerSlot::P3 | PlayerSlot::P4 => match self {
                Self::LinkedDmg07 { linked, .. } => linked.machine_mut(slot.machine_index()),
                Self::Single(_)
                | Self::LinkedDmg04TwoPlayer(_)
                | Self::LinkedCgbInfraredTwoPlayer(_) => None,
            },
        }
    }

    pub const fn is_linked_dmg04_two_player(&self) -> bool {
        matches!(self, Self::LinkedDmg04TwoPlayer(_))
    }

    pub const fn is_linked_cgb_infrared_two_player(&self) -> bool {
        matches!(self, Self::LinkedCgbInfraredTwoPlayer(_))
    }

    pub const fn dmg07_player_count(&self) -> Option<DesktopDmg07PlayerCount> {
        match self {
            Self::LinkedDmg07 { player_count, .. } => Some(*player_count),
            Self::Single(_)
            | Self::LinkedDmg04TwoPlayer(_)
            | Self::LinkedCgbInfraredTwoPlayer(_) => None,
        }
    }

    pub const fn is_linked_dmg07(&self) -> bool {
        matches!(self, Self::LinkedDmg07 { .. })
    }

    pub fn attach_secondary_dmg04(
        &mut self,
        secondary_machine: Machine<TraceSummaryBuffer>,
    ) -> Result<(), String> {
        let expected = match self {
            Self::Single(machine) => machine.next_t_cycle(),
            Self::LinkedDmg04TwoPlayer(_) => {
                return Err(
                    "desktop emulation session is already running a linked DMG-04 runtime"
                        .to_string(),
                );
            }
            Self::LinkedDmg07 { .. } => {
                return Err(
                    "desktop emulation session is already running a linked DMG-07 runtime"
                        .to_string(),
                );
            }
            Self::LinkedCgbInfraredTwoPlayer(_) => {
                return Err(
                    "desktop emulation session is already running a linked CGB IR runtime"
                        .to_string(),
                );
            }
        };
        let found = secondary_machine.next_t_cycle();
        if found != expected {
            return Err(format_linked_machines_error(
                LinkedMachinesError::MismatchedNextTCycle {
                    expected,
                    found,
                    machine_index: 1,
                },
            ));
        }

        let current_session =
            std::mem::replace(self, Self::new_single(placeholder_summary_machine()));
        let Self::Single(primary_machine) = current_session else {
            unreachable!("linked desktop session should have been rejected before replacement");
        };

        let next_session = Self::new_linked_dmg04_two_player(*primary_machine, secondary_machine)
            .expect("validated desktop DMG-04 session should build successfully");
        *self = next_session;
        Ok(())
    }

    pub fn attach_secondary_cgb_infrared(
        &mut self,
        secondary_machine: Machine<TraceSummaryBuffer>,
    ) -> Result<(), String> {
        let expected = match self {
            Self::Single(machine) => machine.next_t_cycle(),
            Self::LinkedDmg04TwoPlayer(_) => {
                return Err(
                    "desktop emulation session is already running a linked DMG-04 runtime"
                        .to_string(),
                );
            }
            Self::LinkedDmg07 { .. } => {
                return Err(
                    "desktop emulation session is already running a linked DMG-07 runtime"
                        .to_string(),
                );
            }
            Self::LinkedCgbInfraredTwoPlayer(_) => {
                return Err(
                    "desktop emulation session is already running a linked CGB IR runtime"
                        .to_string(),
                );
            }
        };
        let found = secondary_machine.next_t_cycle();
        if found != expected {
            return Err(format_linked_machines_error(
                LinkedMachinesError::MismatchedNextTCycle {
                    expected,
                    found,
                    machine_index: 1,
                },
            ));
        }

        let current_session =
            std::mem::replace(self, Self::new_single(placeholder_summary_machine()));
        let Self::Single(primary_machine) = current_session else {
            unreachable!("linked desktop session should have been rejected before replacement");
        };

        let next_session =
            Self::new_linked_cgb_infrared_two_player(*primary_machine, secondary_machine)
                .expect("validated desktop CGB IR session should build successfully");
        *self = next_session;
        Ok(())
    }

    pub fn detach_to_single_primary(&mut self) {
        if matches!(self, Self::Single(_)) {
            return;
        }

        let linked_session =
            std::mem::replace(self, Self::new_single(placeholder_summary_machine()));
        *self = Self::new_single(linked_session.into_primary_machine());
    }

    pub fn step_t_cycle(&mut self) {
        match self {
            Self::Single(machine) => {
                let _ = machine.step_t_cycle();
            }
            Self::LinkedDmg04TwoPlayer(linked) => {
                linked.advance_t_cycle();
            }
            Self::LinkedCgbInfraredTwoPlayer(linked) => {
                linked.advance_t_cycle();
            }
            Self::LinkedDmg07 { linked, .. } => {
                linked.advance_t_cycle();
            }
        }
    }

    pub fn step_t_cycle_with_observer<O: MachineStepObserver>(&mut self, observer: &mut O) {
        match self {
            Self::Single(machine) => {
                let _ = machine.step_t_cycle_with_observer(observer);
            }
            Self::LinkedDmg04TwoPlayer(linked) => {
                linked.advance_t_cycle_with_observer(observer);
            }
            Self::LinkedCgbInfraredTwoPlayer(linked) => {
                linked.advance_t_cycle_with_observer(observer);
            }
            Self::LinkedDmg07 { linked, .. } => {
                linked.advance_t_cycle_with_observer(observer);
            }
        }
    }

    pub fn into_primary_machine(self) -> Machine<TraceSummaryBuffer> {
        match self {
            Self::Single(machine) => *machine,
            Self::LinkedDmg04TwoPlayer(mut linked) => {
                linked.detach_link_topology();
                linked
                    .into_machines()
                    .into_iter()
                    .next()
                    .expect("linked desktop session should keep the primary machine")
            }
            Self::LinkedCgbInfraredTwoPlayer(mut linked) => {
                linked.detach_link_topology();
                linked
                    .into_machines()
                    .into_iter()
                    .next()
                    .expect("linked desktop session should keep the primary machine")
            }
            Self::LinkedDmg07 { mut linked, .. } => {
                linked.detach_link_topology();
                linked
                    .into_machines()
                    .into_iter()
                    .next()
                    .expect("linked desktop session should keep the primary machine")
            }
        }
    }
}

fn dmg07_participants_for_player_count(
    player_count: DesktopDmg07PlayerCount,
) -> Vec<Dmg07Participant> {
    Dmg07Port::ALL
        .into_iter()
        .take(player_count.get())
        .enumerate()
        .map(|(machine_index, port)| Dmg07Participant::new(machine_index, port))
        .collect()
}

impl Deref for DesktopEmulationSession {
    type Target = Machine<TraceSummaryBuffer>;

    fn deref(&self) -> &Self::Target {
        self.primary_machine()
    }
}

impl DerefMut for DesktopEmulationSession {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.primary_machine_mut()
    }
}

fn format_linked_machines_error(error: LinkedMachinesError) -> String {
    match error {
        LinkedMachinesError::TooFewMachines { count } => {
            format!("linked desktop session requires at least two machines, found {count}")
        }
        LinkedMachinesError::MismatchedNextTCycle {
            expected,
            found,
            machine_index,
        } => format!(
            "linked desktop session machines must share the same next T-cycle; expected {expected:?}, found {found:?} at machine index {machine_index}"
        ),
        LinkedMachinesError::UnsupportedMachineCountForDmg04 { count } => {
            format!("DMG-04 desktop sessions currently require exactly two machines, found {count}")
        }
        LinkedMachinesError::UnsupportedMachineCountForDmg07 { count } => {
            format!("DMG-07 linked sessions require two to four machines, found {count}")
        }
        LinkedMachinesError::UnsupportedMachineCountForCgbInfrared { count } => {
            format!("CGB infrared linked sessions require exactly two machines, found {count}")
        }
        LinkedMachinesError::MissingDmg07PlayerOne => {
            "DMG-07 linked sessions require adapter port P1".to_string()
        }
        LinkedMachinesError::DuplicateDmg07Port { port } => {
            format!("DMG-07 linked session uses adapter port {port:?} more than once")
        }
        LinkedMachinesError::DuplicateDmg07MachineIndex { machine_index } => {
            format!("DMG-07 linked session uses machine index {machine_index} more than once")
        }
        LinkedMachinesError::Dmg07MachineIndexOutOfBounds {
            machine_index,
            machine_count,
        } => format!(
            "DMG-07 linked session references machine index {machine_index}, but only {machine_count} machines exist"
        ),
    }
}

fn placeholder_summary_machine() -> Machine<TraceSummaryBuffer> {
    Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    )
}

#[cfg(test)]
mod tests {
    use super::{
        DesktopEmulationSession, DesktopEmulationSessionKind, format_linked_machines_error,
    };
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
            format_linked_machines_error(
                LinkedMachinesError::UnsupportedMachineCountForCgbInfrared { count: 3 }
            ),
            "CGB infrared linked sessions require exactly two machines, found 3"
        );
    }
}
