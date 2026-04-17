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
        let current_session =
            std::mem::replace(self, Self::new_single(placeholder_summary_machine()));
        let Self::Single(primary_machine) = current_session else {
            *self = current_session;
            return Err(
                "desktop emulation session is already running a linked DMG-04 runtime".to_string(),
            );
        };

        let next_session = Self::new_linked_dmg04_two_player(*primary_machine, secondary_machine)?;
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
                let _ = linked.step_t_cycle();
            }
        }
    }

    pub fn step_t_cycle_with_observer<O: MachineStepObserver>(&mut self, observer: &mut O) {
        match self {
            Self::Single(machine) => {
                let _ = machine.step_t_cycle_with_observer(observer);
            }
            Self::LinkedDmg04TwoPlayer(linked) => {
                let _ = linked.step_t_cycle();
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
