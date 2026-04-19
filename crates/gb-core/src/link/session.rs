use super::dmg04::Dmg04Cable;
use crate::debugger::{TraceBuffer, TraceSink};
use crate::external_port::ExternalPortAttachmentKind;
use crate::machine::{Machine, MachineStepObserver, NoopMachineStepObserver};
use crate::scheduler::{CycleContext, GlobalScheduler, SchedulerPhase, TCycle};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedStepResult {
    contexts: Vec<CycleContext>,
}

impl LinkedStepResult {
    pub fn contexts(&self) -> &[CycleContext] {
        &self.contexts
    }

    pub fn machine_context(&self, machine_index: usize) -> Option<&CycleContext> {
        self.contexts.get(machine_index)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkedMachinesError {
    TooFewMachines {
        count: usize,
    },
    MismatchedNextTCycle {
        expected: TCycle,
        found: TCycle,
        machine_index: usize,
    },
    UnsupportedMachineCountForDmg04 {
        count: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LinkedTopologyKind {
    #[default]
    None,
    Dmg04,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
enum LinkTopology {
    #[default]
    None,
    Dmg04(Dmg04Cable),
}

#[derive(Debug, Clone)]
pub struct LinkedMachines<S = TraceBuffer> {
    scheduler: GlobalScheduler,
    machines: Vec<Machine<S>>,
    topology: LinkTopology,
    contexts: Vec<CycleContext>,
}

impl<S: TraceSink> LinkedMachines<S> {
    pub fn new(machines: Vec<Machine<S>>) -> Result<Self, LinkedMachinesError> {
        let count = machines.len();
        if count < 2 {
            return Err(LinkedMachinesError::TooFewMachines { count });
        }

        let expected = machines[0].next_t_cycle();
        for (machine_index, machine) in machines.iter().enumerate().skip(1) {
            let found = machine.next_t_cycle();
            if found != expected {
                return Err(LinkedMachinesError::MismatchedNextTCycle {
                    expected,
                    found,
                    machine_index,
                });
            }
        }

        let mut scheduler = GlobalScheduler::new();
        scheduler.set_next_t_cycle(expected);

        Ok(Self {
            scheduler,
            machines,
            topology: LinkTopology::None,
            contexts: vec![CycleContext::for_cycle(expected); count],
        })
    }

    pub fn scheduler(&self) -> &GlobalScheduler {
        &self.scheduler
    }

    pub fn next_t_cycle(&self) -> TCycle {
        self.scheduler.next_t_cycle()
    }

    pub fn machine_count(&self) -> usize {
        self.machines.len()
    }

    pub fn machine(&self, machine_index: usize) -> Option<&Machine<S>> {
        self.machines.get(machine_index)
    }

    pub fn machine_mut(&mut self, machine_index: usize) -> Option<&mut Machine<S>> {
        self.machines.get_mut(machine_index)
    }

    pub fn machines(&self) -> &[Machine<S>] {
        &self.machines
    }

    pub fn machines_mut(&mut self) -> &mut [Machine<S>] {
        &mut self.machines
    }

    pub fn into_machines(self) -> Vec<Machine<S>> {
        self.machines
    }

    pub fn topology_kind(&self) -> LinkedTopologyKind {
        self.topology.kind()
    }

    pub fn attach_dmg04_cable(&mut self) -> Result<(), LinkedMachinesError> {
        if self.machines.len() != 2 {
            return Err(LinkedMachinesError::UnsupportedMachineCountForDmg04 {
                count: self.machines.len(),
            });
        }

        self.detach_link_topology();

        for machine in &mut self.machines {
            machine.set_external_port_attachment(ExternalPortAttachmentKind::GameLinkDmg04);
            machine.set_dmg04_incoming_byte(None);
        }

        self.topology = LinkTopology::Dmg04(Dmg04Cable::new(0, 1));
        Ok(())
    }

    pub fn detach_link_topology(&mut self) {
        match self.topology {
            LinkTopology::None => {}
            LinkTopology::Dmg04(cable) => cable.detach(&mut self.machines),
        }

        self.topology = LinkTopology::None;
    }

    pub fn step_t_cycle(&mut self) -> LinkedStepResult {
        self.advance_t_cycle_with_observer(&mut NoopMachineStepObserver);
        LinkedStepResult {
            contexts: self.contexts.clone(),
        }
    }

    pub fn advance_t_cycle(&mut self) {
        self.advance_t_cycle_with_observer(&mut NoopMachineStepObserver);
    }

    pub fn advance_t_cycle_with_observer<O: MachineStepObserver>(&mut self, observer: &mut O) {
        let t_cycle = self.scheduler.next_t_cycle();
        debug_assert_eq!(self.contexts.len(), self.machines.len());
        for context in &mut self.contexts {
            context.reset_for_cycle(t_cycle);
        }

        for &phase in SchedulerPhase::all() {
            for context in &mut self.contexts {
                context.enter_phase(phase);
            }

            self.prepare_phase(phase);

            for (machine, context) in self.machines.iter_mut().zip(self.contexts.iter_mut()) {
                machine.step_phase_with_context(context, observer);
            }
        }

        let next_t_cycle = t_cycle.next();
        self.scheduler.set_next_t_cycle(next_t_cycle);
        for machine in &mut self.machines {
            machine.sync_scheduler_next_t_cycle(next_t_cycle);
        }
    }

    fn prepare_phase(&mut self, phase: SchedulerPhase) {
        match self.topology {
            LinkTopology::None => {}
            LinkTopology::Dmg04(cable) => cable.prepare_phase(phase, &mut self.machines),
        }
    }
}

impl LinkTopology {
    const fn kind(self) -> LinkedTopologyKind {
        match self {
            Self::None => LinkedTopologyKind::None,
            Self::Dmg04(_) => LinkedTopologyKind::Dmg04,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::MachineStepObserver;
    use crate::model::{ConsoleModel, MachineConfig, StartupMode};

    fn dmg_skip_boot_machine() -> Machine {
        Machine::new(MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot))
    }

    #[derive(Default)]
    struct RegionCountObserver {
        machine_regions: usize,
        ppu_regions: usize,
    }

    impl MachineStepObserver for RegionCountObserver {
        fn begin_region(&mut self, _region: crate::machine::MachineStepRegion) {
            self.machine_regions += 1;
        }

        fn begin_ppu_region(&mut self, _region: crate::ppu::PpuStepRegion) {
            self.ppu_regions += 1;
        }
    }

    #[test]
    fn linked_machines_require_at_least_two_participants() {
        let machine = dmg_skip_boot_machine();

        let error = LinkedMachines::new(vec![machine]).expect_err("single machine should fail");

        assert_eq!(error, LinkedMachinesError::TooFewMachines { count: 1 });
    }

    #[test]
    fn linked_machines_reject_mismatched_scheduler_positions() {
        let left = dmg_skip_boot_machine();
        let mut right = dmg_skip_boot_machine();
        right.step_t_cycle();

        let error = LinkedMachines::new(vec![left, right]).expect_err("mismatched cycles");

        assert_eq!(
            error,
            LinkedMachinesError::MismatchedNextTCycle {
                expected: TCycle::ZERO,
                found: TCycle::new(1),
                machine_index: 1,
            }
        );
    }

    #[test]
    fn linked_machines_advance_all_members_on_the_same_shared_t_cycle() {
        let mut linked =
            LinkedMachines::new(vec![dmg_skip_boot_machine(), dmg_skip_boot_machine()])
                .expect("matching machines should link");

        let result = linked.step_t_cycle();

        assert_eq!(linked.next_t_cycle(), TCycle::new(1));
        assert_eq!(result.contexts().len(), 2);
        assert_eq!(
            result.machine_context(0).map(CycleContext::t_cycle),
            Some(TCycle::ZERO)
        );
        assert_eq!(
            result.machine_context(1).map(CycleContext::t_cycle),
            Some(TCycle::ZERO)
        );
        assert_eq!(
            linked.machine(0).map(Machine::next_t_cycle),
            Some(TCycle::new(1))
        );
        assert_eq!(
            linked.machine(1).map(Machine::next_t_cycle),
            Some(TCycle::new(1))
        );
    }

    #[test]
    fn linked_machines_can_step_with_an_observer_without_materializing_a_result() {
        let mut linked =
            LinkedMachines::new(vec![dmg_skip_boot_machine(), dmg_skip_boot_machine()])
                .expect("matching machines should link");
        let mut observer = RegionCountObserver::default();

        linked.advance_t_cycle_with_observer(&mut observer);

        assert_eq!(linked.next_t_cycle(), TCycle::new(1));
        assert!(observer.machine_regions > 0);
        assert!(observer.ppu_regions > 0);
    }

    #[test]
    fn linked_machines_match_independent_execution_without_cross_machine_links() {
        let mut independent_left = dmg_skip_boot_machine();
        let mut independent_right = dmg_skip_boot_machine();
        independent_right.set_external_port_attachment(ExternalPortAttachmentKind::Printer);

        let mut linked = LinkedMachines::new(vec![dmg_skip_boot_machine(), {
            let mut machine = dmg_skip_boot_machine();
            machine.set_external_port_attachment(ExternalPortAttachmentKind::Printer);
            machine
        }])
        .expect("matching machines should link");

        for _ in 0..8 {
            independent_left.step_t_cycle();
            independent_right.step_t_cycle();
            linked.step_t_cycle();
        }

        assert_eq!(
            linked.machine(0).expect("left machine").snapshot(),
            independent_left.snapshot()
        );
        assert_eq!(
            linked.machine(1).expect("right machine").snapshot(),
            independent_right.snapshot()
        );
    }

    #[test]
    fn dmg04_attachment_requires_exactly_two_machines() {
        let mut linked = LinkedMachines::new(vec![
            dmg_skip_boot_machine(),
            dmg_skip_boot_machine(),
            dmg_skip_boot_machine(),
        ])
        .expect("matching machines should link");

        let error = linked
            .attach_dmg04_cable()
            .expect_err("three-machine session should reject DMG-04 cable");

        assert_eq!(
            error,
            LinkedMachinesError::UnsupportedMachineCountForDmg04 { count: 3 }
        );
    }

    #[test]
    fn attach_dmg04_cable_marks_both_external_ports_as_game_link() {
        let mut linked =
            LinkedMachines::new(vec![dmg_skip_boot_machine(), dmg_skip_boot_machine()])
                .expect("matching machines should link");

        linked
            .attach_dmg04_cable()
            .expect("two-machine session should accept DMG-04 cable");

        assert_eq!(
            linked
                .machine(0)
                .map(|machine| machine.external_port().attachment_kind()),
            Some(ExternalPortAttachmentKind::GameLinkDmg04)
        );
        assert_eq!(
            linked
                .machine(1)
                .map(|machine| machine.external_port().attachment_kind()),
            Some(ExternalPortAttachmentKind::GameLinkDmg04)
        );
        assert_eq!(linked.topology_kind(), LinkedTopologyKind::Dmg04);
    }

    #[test]
    fn detach_link_topology_clears_the_session_owned_dmg04_attachment() {
        let mut linked =
            LinkedMachines::new(vec![dmg_skip_boot_machine(), dmg_skip_boot_machine()])
                .expect("matching machines should link");

        linked
            .attach_dmg04_cable()
            .expect("two-machine session should accept DMG-04 cable");

        linked.detach_link_topology();

        assert_eq!(linked.topology_kind(), LinkedTopologyKind::None);
        assert_eq!(
            linked
                .machine(0)
                .map(|machine| machine.external_port().attachment_kind()),
            Some(ExternalPortAttachmentKind::None)
        );
        assert_eq!(
            linked
                .machine(1)
                .map(|machine| machine.external_port().attachment_kind()),
            Some(ExternalPortAttachmentKind::None)
        );
    }

    #[test]
    fn dmg04_can_be_reattached_after_session_level_detach() {
        let mut linked =
            LinkedMachines::new(vec![dmg_skip_boot_machine(), dmg_skip_boot_machine()])
                .expect("matching machines should link");

        linked
            .attach_dmg04_cable()
            .expect("two-machine session should accept DMG-04 cable");
        linked.detach_link_topology();
        linked
            .attach_dmg04_cable()
            .expect("reattach should restore DMG-04 ownership");

        assert_eq!(linked.topology_kind(), LinkedTopologyKind::Dmg04);
        assert_eq!(
            linked
                .machine(0)
                .map(|machine| machine.external_port().attachment_kind()),
            Some(ExternalPortAttachmentKind::GameLinkDmg04)
        );
        assert_eq!(
            linked
                .machine(1)
                .map(|machine| machine.external_port().attachment_kind()),
            Some(ExternalPortAttachmentKind::GameLinkDmg04)
        );
    }
}
