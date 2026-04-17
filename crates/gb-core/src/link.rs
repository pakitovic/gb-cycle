use crate::debugger::{TraceBuffer, TraceSink};
use crate::machine::{Machine, NoopMachineStepObserver};
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
}

#[derive(Debug, Clone)]
pub struct LinkedMachines<S = TraceBuffer> {
    scheduler: GlobalScheduler,
    machines: Vec<Machine<S>>,
}

impl<S: TraceSink> LinkedMachines<S> {
    pub fn new(machines: Vec<Machine<S>>) -> Result<Self, LinkedMachinesError> {
        if machines.len() < 2 {
            return Err(LinkedMachinesError::TooFewMachines {
                count: machines.len(),
            });
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

    pub fn step_t_cycle(&mut self) -> LinkedStepResult {
        let t_cycle = self.scheduler.next_t_cycle();
        let mut contexts = Vec::with_capacity(self.machines.len());
        for _ in 0..self.machines.len() {
            contexts.push(CycleContext::for_cycle(t_cycle));
        }

        for &phase in SchedulerPhase::all() {
            for context in &mut contexts {
                context.enter_phase(phase);
            }

            for (machine, context) in self.machines.iter_mut().zip(contexts.iter_mut()) {
                machine.step_phase_with_context(context, &mut NoopMachineStepObserver);
            }
        }

        let next_t_cycle = t_cycle.next();
        self.scheduler.set_next_t_cycle(next_t_cycle);
        for machine in &mut self.machines {
            machine.sync_scheduler_next_t_cycle(next_t_cycle);
        }

        LinkedStepResult { contexts }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::external_port::ExternalPortAttachmentKind;
    use crate::model::{ConsoleModel, MachineConfig, StartupMode};

    fn dmg_skip_boot_machine() -> Machine {
        Machine::new(MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot))
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
}
