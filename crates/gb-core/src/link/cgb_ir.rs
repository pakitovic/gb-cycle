use crate::debugger::TraceSink;
use crate::machine::Machine;
use crate::scheduler::SchedulerPhase;

// Provisional CGB-to-CGB optical edge delay. Shonumi's GBE+ research found that
// Super Mario Bros. DX-style IR protocols fail when the peer sees emitter changes immediately;
// local validation across several commercial CGB IR games currently favors this larger delay.
pub const DEFAULT_CGB_IR_OPTICAL_PROPAGATION_DELAY_T_CYCLES: usize = 80;
pub const MIN_CGB_IR_OPTICAL_PROPAGATION_DELAY_T_CYCLES: usize = 1;
pub const MAX_CGB_IR_OPTICAL_PROPAGATION_DELAY_T_CYCLES: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct CgbInfraredPair {
    left_machine_index: usize,
    right_machine_index: usize,
    optical_propagation_delay_t_cycles: usize,
    left_to_right_delay_line: [bool; MAX_CGB_IR_OPTICAL_PROPAGATION_DELAY_T_CYCLES],
    right_to_left_delay_line: [bool; MAX_CGB_IR_OPTICAL_PROPAGATION_DELAY_T_CYCLES],
    delay_cursor: usize,
}

impl CgbInfraredPair {
    pub(crate) const fn new(left_machine_index: usize, right_machine_index: usize) -> Self {
        Self::with_optical_propagation_delay_t_cycles(
            left_machine_index,
            right_machine_index,
            DEFAULT_CGB_IR_OPTICAL_PROPAGATION_DELAY_T_CYCLES,
        )
    }

    pub(crate) const fn with_optical_propagation_delay_t_cycles(
        left_machine_index: usize,
        right_machine_index: usize,
        optical_propagation_delay_t_cycles: usize,
    ) -> Self {
        Self {
            left_machine_index,
            right_machine_index,
            optical_propagation_delay_t_cycles,
            left_to_right_delay_line: [false; MAX_CGB_IR_OPTICAL_PROPAGATION_DELAY_T_CYCLES],
            right_to_left_delay_line: [false; MAX_CGB_IR_OPTICAL_PROPAGATION_DELAY_T_CYCLES],
            delay_cursor: 0,
        }
    }

    pub(crate) fn prepare_phase<S: TraceSink>(
        &mut self,
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
        let delayed_left_emitter = self.left_to_right_delay_line[self.delay_cursor];
        let delayed_right_emitter = self.right_to_left_delay_line[self.delay_cursor];

        self.left_to_right_delay_line[self.delay_cursor] = left_emitter_on;
        self.right_to_left_delay_line[self.delay_cursor] = right_emitter_on;
        self.delay_cursor = (self.delay_cursor + 1) % self.optical_propagation_delay_t_cycles;

        left.set_cgb_infrared_external_input(delayed_right_emitter);
        right.set_cgb_infrared_external_input(delayed_left_emitter);
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
