use super::decode::CpuInstructionKind;
use super::*;

mod arithmetic;
mod cb;
mod control_flow;
mod loads;
mod stack;

impl CpuCore {
    pub(super) fn complete_execute_machine_cycle(
        &mut self,
        opcode: u8,
        step: u8,
        bus_operation: &mut CpuBusCallback<'_>,
    ) {
        let Some(kind) = self.instruction_kind else {
            self.execution_state = CpuExecutionState::Execute {
                opcode,
                step,
                t_cycle: LAST_MACHINE_CYCLE_T,
            };
            return;
        };

        match kind {
            CpuInstructionKind::LoadRegisterImmediate { .. }
            | CpuInstructionKind::LoadRegisterPairImmediate { .. }
            | CpuInstructionKind::LoadRegisterFromHl { .. }
            | CpuInstructionKind::StoreRegisterToHl { .. }
            | CpuInstructionKind::StoreImmediateToHl
            | CpuInstructionKind::LoadAFromHlWithUpdate { .. }
            | CpuInstructionKind::StoreAToHlWithUpdate { .. }
            | CpuInstructionKind::LoadAFromAddress { .. }
            | CpuInstructionKind::StoreAToAddress { .. }
            | CpuInstructionKind::StoreSpToImmediate16
            | CpuInstructionKind::LoadSpFromHl => {
                self.execute_load_machine_cycle(kind, opcode, step, bus_operation);
            }
            CpuInstructionKind::LoadHlFromSpPlusImmediate
            | CpuInstructionKind::AddSpImmediate
            | CpuInstructionKind::AddHl { .. }
            | CpuInstructionKind::IncrementRegisterPair { .. }
            | CpuInstructionKind::DecrementRegisterPair { .. }
            | CpuInstructionKind::IncrementHlMemory
            | CpuInstructionKind::DecrementHlMemory
            | CpuInstructionKind::AluImmediate { .. }
            | CpuInstructionKind::AluFromHl { .. } => {
                self.execute_arithmetic_machine_cycle(kind, opcode, step, bus_operation);
            }
            CpuInstructionKind::RelativeJump { .. }
            | CpuInstructionKind::AbsoluteJump { .. }
            | CpuInstructionKind::Call { .. }
            | CpuInstructionKind::Return { .. }
            | CpuInstructionKind::ReturnFromInterrupt
            | CpuInstructionKind::Stop
            | CpuInstructionKind::Restart { .. } => {
                self.execute_control_flow_machine_cycle(kind, opcode, step, bus_operation);
            }
            CpuInstructionKind::PushRegisterPair { .. }
            | CpuInstructionKind::PopRegisterPair { .. } => {
                self.execute_stack_machine_cycle(kind, opcode, step, bus_operation);
            }
            CpuInstructionKind::CbPrefixed => {
                self.execute_cb_prefixed_machine_cycle(opcode, step, bus_operation);
            }
        }
    }
}
