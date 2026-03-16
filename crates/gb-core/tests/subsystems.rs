use gb_core::{
    ApuStatus, BootStatus, BusStatus, CartridgeSlotState, ConsoleModel, CpuStatus, DmaStatus,
    InterruptControllerStatus, JoypadStatus, Machine, MachineConfig, PpuStatus, SerialStatus,
    StartupMode, TimerStatus,
};

#[test]
fn machine_keeps_phase_2_1_foundations_ready_with_a_live_cpu_scaffold() {
    let machine = Machine::new(
        MachineConfig::new(ConsoleModel::Mgb).with_startup_mode(StartupMode::RealBoot),
    );

    assert_eq!(machine.cpu().status(), CpuStatus::Ready);
    assert_eq!(machine.bus().status(), BusStatus::Ready);
    assert_eq!(machine.apu().status(), ApuStatus::Ready);
    assert_eq!(machine.ppu().status(), PpuStatus::RegistersReady);
    assert_eq!(machine.dma().status(), DmaStatus::Ready);
    assert_eq!(machine.timer().status(), TimerStatus::Ready);
    assert_eq!(machine.serial().status(), SerialStatus::Ready);
    assert_eq!(machine.boot().status(), BootStatus::Ready);
    assert_eq!(
        machine.interrupts().status(),
        InterruptControllerStatus::Ready
    );
    assert_eq!(machine.joypad().status(), JoypadStatus::Ready);
    assert_eq!(machine.cartridge().state(), CartridgeSlotState::Empty);
}

#[test]
fn model_and_startup_configuration_flow_into_the_stubbed_boundaries() {
    let machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg0).with_startup_mode(StartupMode::SkipBoot),
    );

    assert_eq!(machine.cpu().status(), CpuStatus::Ready);
    assert_eq!(machine.cpu().console_model(), ConsoleModel::Dmg0);
    assert_eq!(machine.bus().console_model(), ConsoleModel::Dmg0);
    assert_eq!(machine.apu().console_model(), ConsoleModel::Dmg0);
    assert_eq!(machine.ppu().console_model(), ConsoleModel::Dmg0);
    assert_eq!(machine.dma().console_model(), ConsoleModel::Dmg0);
    assert_eq!(machine.timer().console_model(), ConsoleModel::Dmg0);
    assert_eq!(machine.serial().console_model(), ConsoleModel::Dmg0);
    assert_eq!(machine.boot().console_model(), ConsoleModel::Dmg0);
    assert_eq!(machine.interrupts().console_model(), ConsoleModel::Dmg0);
    assert_eq!(machine.joypad().console_model(), ConsoleModel::Dmg0);
    assert_eq!(machine.boot().startup_mode(), StartupMode::SkipBoot);
}
