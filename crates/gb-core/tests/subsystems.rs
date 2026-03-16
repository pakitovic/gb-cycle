use gb_core::{
    BootStatus, BusStatus, CartridgeSlotState, ConsoleModel, CpuStatus, DmaStatus, Machine,
    MachineConfig, PpuStatus, StartupMode, TimerStatus,
};

#[test]
fn machine_owns_all_phase0_stubbed_subsystems() {
    let machine = Machine::new(
        MachineConfig::new(ConsoleModel::Mgb).with_startup_mode(StartupMode::RealBoot),
    );

    assert_eq!(machine.cpu().status(), CpuStatus::Stub);
    assert_eq!(machine.bus().status(), BusStatus::Stub);
    assert_eq!(machine.ppu().status(), PpuStatus::Stub);
    assert_eq!(machine.dma().status(), DmaStatus::Stub);
    assert_eq!(machine.timer().status(), TimerStatus::Stub);
    assert_eq!(machine.boot().status(), BootStatus::Stub);
    assert_eq!(machine.cartridge().state(), CartridgeSlotState::Empty);
}

#[test]
fn model_and_startup_configuration_flow_into_the_stubbed_boundaries() {
    let machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg0).with_startup_mode(StartupMode::SkipBoot),
    );

    assert_eq!(machine.cpu().console_model(), ConsoleModel::Dmg0);
    assert_eq!(machine.bus().console_model(), ConsoleModel::Dmg0);
    assert_eq!(machine.ppu().console_model(), ConsoleModel::Dmg0);
    assert_eq!(machine.dma().console_model(), ConsoleModel::Dmg0);
    assert_eq!(machine.timer().console_model(), ConsoleModel::Dmg0);
    assert_eq!(machine.boot().console_model(), ConsoleModel::Dmg0);
    assert_eq!(machine.boot().startup_mode(), StartupMode::SkipBoot);
}
