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
        let opcode = self.current_opcode().unwrap_or(0);
        let Some(kind) = self.in_flight.kind else {
            self.execution_state = CpuExecutionState::Execute {
                step,
                t_cycle: LAST_MACHINE_CYCLE_T,
            };
            return;
        };
        let execution_group = self
            .in_flight
            .execution_group
            .unwrap_or_else(|| kind.execution_group());

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
