pub(super) use super::*;

mod cb;
mod decode;
mod execute;
mod registers_alu;

fn power_on_cpu() -> CpuCore {
    let mut cpu = CpuCore::new(ConsoleModel::GameBoy);
    cpu.apply_startup_state(CpuStartupState::power_on_reset());
    cpu
}
