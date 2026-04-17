use crate::debugger::TraceSink;
use crate::external_port::ExternalPortAttachmentKind;
use crate::machine::{Dmg04EndpointState, Machine};
use crate::scheduler::SchedulerPhase;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Dmg04Cable {
    left_machine_index: usize,
    right_machine_index: usize,
}

impl Dmg04Cable {
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
        match phase {
            SchedulerPhase::ExternalEventIngress => {
                self.prepare_external_event_ingress(machines);
            }
            SchedulerPhase::AutonomousPeripheralTicks => {
                self.prepare_autonomous_peripheral_ticks(machines);
            }
            _ => {}
        }
    }

    pub(crate) fn detach<S: TraceSink>(&self, machines: &mut [Machine<S>]) {
        let (left, right) =
            indexed_pair_mut(machines, self.left_machine_index, self.right_machine_index);
        left.set_external_port_attachment(ExternalPortAttachmentKind::None);
        right.set_external_port_attachment(ExternalPortAttachmentKind::None);
    }

    fn prepare_external_event_ingress<S: TraceSink>(&self, machines: &mut [Machine<S>]) {
        let (left, right) =
            indexed_pair_mut(machines, self.left_machine_index, self.right_machine_index);
        let left_state = left.dmg04_endpoint_state();
        let right_state = right.dmg04_endpoint_state();

        if left_state.internal_clock_edge_pending && right_state.waiting_for_external_clock {
            right.queue_external_serial_clock();
        }
        if right_state.internal_clock_edge_pending && left_state.waiting_for_external_clock {
            left.queue_external_serial_clock();
        }
    }

    fn prepare_autonomous_peripheral_ticks<S: TraceSink>(&self, machines: &mut [Machine<S>]) {
        let (left, right) =
            indexed_pair_mut(machines, self.left_machine_index, self.right_machine_index);
        let left_state = left.dmg04_endpoint_state();
        let right_state = right.dmg04_endpoint_state();

        match resolve_dmg04_exchange(left_state, right_state) {
            Some(Dmg04Exchange {
                left_incoming_byte,
                right_incoming_byte,
            }) => {
                left.set_dmg04_incoming_byte(Some(left_incoming_byte));
                right.set_dmg04_incoming_byte(Some(right_incoming_byte));
            }
            None => {
                left.set_dmg04_incoming_byte(None);
                right.set_dmg04_incoming_byte(None);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Dmg04Exchange {
    left_incoming_byte: u8,
    right_incoming_byte: u8,
}

fn resolve_dmg04_exchange(
    left: Dmg04EndpointState,
    right: Dmg04EndpointState,
) -> Option<Dmg04Exchange> {
    if left.internal_clock_edge_pending && right.waiting_for_external_clock {
        return Some(Dmg04Exchange {
            left_incoming_byte: right.staged_outgoing_byte,
            right_incoming_byte: left.staged_outgoing_byte,
        });
    }

    if right.internal_clock_edge_pending && left.waiting_for_external_clock {
        return Some(Dmg04Exchange {
            left_incoming_byte: right.staged_outgoing_byte,
            right_incoming_byte: left.staged_outgoing_byte,
        });
    }

    None
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
