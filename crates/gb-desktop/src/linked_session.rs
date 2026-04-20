#[cfg(test)]
use gb_core::LinkedTopologyKind;
use gb_core::{
    ConsoleModel, LinkedMachines, LinkedMachinesError, Machine, MachineConfig, MachineStepObserver,
    StartupMode, TraceSummaryBuffer,
};
use std::ops::{Deref, DerefMut};

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopEmulationSessionKind {
    Single,
    LinkedDmg04TwoPlayer,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone)]
pub enum DesktopEmulationSession {
    Single(Box<Machine<TraceSummaryBuffer>>),
    LinkedDmg04TwoPlayer(Box<LinkedMachines<TraceSummaryBuffer>>),
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

    #[cfg(test)]
    pub const fn kind(&self) -> DesktopEmulationSessionKind {
        match self {
            Self::Single(_) => DesktopEmulationSessionKind::Single,
            Self::LinkedDmg04TwoPlayer(_) => DesktopEmulationSessionKind::LinkedDmg04TwoPlayer,
        }
    }

    #[cfg(test)]
    pub fn linked_topology_kind(&self) -> LinkedTopologyKind {
        match self {
            Self::Single(_) => LinkedTopologyKind::None,
            Self::LinkedDmg04TwoPlayer(linked) => linked.topology_kind(),
        }
    }

    pub fn primary_machine(&self) -> &Machine<TraceSummaryBuffer> {
        match self {
            Self::Single(machine) => machine,
            Self::LinkedDmg04TwoPlayer(linked) => linked
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
        }
    }

    pub fn secondary_machine(&self) -> Option<&Machine<TraceSummaryBuffer>> {
        match self {
            Self::Single(_) => None,
            Self::LinkedDmg04TwoPlayer(linked) => linked.machine(1),
        }
    }

    pub fn secondary_machine_mut(&mut self) -> Option<&mut Machine<TraceSummaryBuffer>> {
        match self {
            Self::Single(_) => None,
            Self::LinkedDmg04TwoPlayer(linked) => linked.machine_mut(1),
        }
    }

    pub const fn is_linked_dmg04_two_player(&self) -> bool {
        matches!(self, Self::LinkedDmg04TwoPlayer(_))
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
        }
    }
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
    }
}

fn placeholder_summary_machine() -> Machine<TraceSummaryBuffer> {
    Machine::new_summary(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    )
}

#[cfg(test)]
mod tests {
    use super::{
        DesktopEmulationSession, DesktopEmulationSessionKind, format_linked_machines_error,
    };
    use gb_core::{
        ConsoleModel, LinkedMachinesError, Machine, MachineConfig, MachineStepObserver,
        MachineStepRegion, StartupMode, TCycle, TraceSummaryBuffer,
    };
    use std::collections::HashMap;

    fn dmg_skip_boot_summary_machine() -> Machine<TraceSummaryBuffer> {
        Machine::new_summary(
            MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
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
    fn step_t_cycle_with_observer_covers_single_and_linked_sessions() {
        let mut single = DesktopEmulationSession::new_single(dmg_skip_boot_summary_machine());
        let mut observer = CountingObserver::default();
        single.step_t_cycle_with_observer(&mut observer);

        assert!(
            observer
                .begins
                .contains_key(&MachineStepRegion::ExternalEvents)
        );
        assert!(observer.begins.contains_key(&MachineStepRegion::Cpu));
        assert_eq!(
            observer.begins.get(&MachineStepRegion::Cpu),
            observer.ends.get(&MachineStepRegion::Cpu)
        );

        let mut linked = DesktopEmulationSession::new_linked_dmg04_two_player(
            dmg_skip_boot_summary_machine(),
            dmg_skip_boot_summary_machine(),
        )
        .expect("linked desktop session should build");
        let mut linked_observer = CountingObserver::default();
        linked.step_t_cycle_with_observer(&mut linked_observer);

        assert!(
            linked_observer
                .begins
                .contains_key(&MachineStepRegion::ExternalEvents)
        );
        assert!(linked_observer.begins.contains_key(&MachineStepRegion::Cpu));
        assert_eq!(
            linked_observer
                .begins
                .get(&MachineStepRegion::ExternalEvents),
            linked_observer.ends.get(&MachineStepRegion::ExternalEvents)
        );
        assert_eq!(linked.next_t_cycle(), TCycle::new(1));
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
    }
}
