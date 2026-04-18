use super::decode::InstructionExecutionGroup;
use super::*;

mod arithmetic;
mod cb;
mod control_flow;
mod loads;
mod stack;

impl CpuCore {
    pub(super) fn complete_execute_machine_cycle(
        &mut self,
        step: u8,
        bus_operation: &mut CpuExternalCallback<'_>,
    ) {
        if !cfg!(test) && self.in_flight.execution_group.is_some() {
            debug_assert_eq!(
                self.in_flight.execution_group,
                self.in_flight.kind.map(CpuInstructionKind::execution_group),
                "decoded execution group must stay coherent with the decoded instruction kind",
            );
        }

        if !cfg!(test) {
            debug_assert!(
                self.in_flight.opcode().is_some() || self.in_flight.kind.is_none(),
                "execute state should retain a latched opcode when a decoded instruction is still in flight",
            );
        }

        let opcode = self.in_flight.opcode().unwrap_or(0);
        let Some((kind, execution_group)) = self.in_flight.execution_descriptor() else {
            if !cfg!(test) {
                debug_assert!(
                    false,
                    "execute state should retain a decoded instruction descriptor while machine cycles are in flight",
                );
            }
            self.execution_state = CpuExecutionState::Execute {
                step,
                t_cycle: LAST_MACHINE_CYCLE_T,
            };
            return;
        };

        match execution_group {
            InstructionExecutionGroup::Load => {
                self.execute_load_machine_cycle(kind, opcode, step, bus_operation);
            }
            InstructionExecutionGroup::Arithmetic => {
                self.execute_arithmetic_machine_cycle(kind, opcode, step, bus_operation);
            }
            InstructionExecutionGroup::ControlFlow => {
                self.execute_control_flow_machine_cycle(kind, opcode, step, bus_operation);
            }
            InstructionExecutionGroup::Stack => {
                self.execute_stack_machine_cycle(kind, opcode, step, bus_operation);
            }
            InstructionExecutionGroup::CbPrefixed => {
                self.execute_cb_prefixed_machine_cycle(opcode, step, bus_operation);
            }
        }
    }
}
