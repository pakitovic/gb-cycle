use crate::debugger::TraceSink;
use crate::machine::Machine;
use crate::scheduler::SchedulerPhase;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct CgbInfraredPair {
    left_machine_index: usize,
    right_machine_index: usize,
}

impl CgbInfraredPair {
    pub(crate) const fn new(left_machine_index: usize, right_machine_index: usize) -> Self {
        Self {
            left_machine_index,
            right_machine_index,
        }
    }

    pub(crate) fn prepare_phase<S: TraceSink>(
        &self,
        phase: SchedulerPhase,
        machines: &mut [Machine<S>],
    ) {
        if phase != SchedulerPhase::ExternalEventIngress {
            return;
        }

        let (left, right) =
            indexed_pair_mut(machines, self.left_machine_index, self.right_machine_index);
        let left_emitter_on = left.cgb_infrared_emitter_on();
        let right_emitter_on = right.cgb_infrared_emitter_on();

        left.set_cgb_infrared_external_input(right_emitter_on);
        right.set_cgb_infrared_external_input(left_emitter_on);
    }

    pub(crate) fn detach<S: TraceSink>(&self, machines: &mut [Machine<S>]) {
        let (left, right) =
            indexed_pair_mut(machines, self.left_machine_index, self.right_machine_index);
        left.set_cgb_infrared_external_input(false);
        right.set_cgb_infrared_external_input(false);
    }
}

fn indexed_pair_mut<T>(slice: &mut [T], left_index: usize, right_index: usize) -> (&mut T, &mut T) {
    assert_ne!(left_index, right_index, "pair indexes must be distinct");
    assert!(
        left_index < slice.len() && right_index < slice.len(),
        "pair indexes must be in bounds",
    );

    if left_index < right_index {
        let (left_half, right_half) = slice.split_at_mut(right_index);
        (&mut left_half[left_index], &mut right_half[0])
    } else {
        let (right_half, left_half) = slice.split_at_mut(left_index);
        (&mut left_half[0], &mut right_half[right_index])
    }
}
