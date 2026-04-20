mod common;

use gb_core::{
    ConsoleModel, LinkedMachines, Machine, MachineConfig, SerialTransferState, StartupMode,
};

const FIXTURE_ACCEPT_ENV: &str = common::fixture_env::MACHINE;
const DMG04_CHRONOLOGY_FIXTURE_NAME: &str = "dmg04_linked_transfer_chronology.txt";

fn dmg_skip_boot_machine() -> Machine {
    Machine::new(MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot))
}

fn step_linked_t_cycles(linked: &mut LinkedMachines, t_cycles: usize) {
    for _ in 0..t_cycles {
        linked.step_t_cycle();
    }
}

#[test]
fn dmg04_two_console_link_exchanges_bytes_bidirectionally() {
    let mut linked = LinkedMachines::new(vec![dmg_skip_boot_machine(), dmg_skip_boot_machine()])
        .expect("matching machines should link");
    linked
        .attach_dmg04_cable()
        .expect("two-machine session should accept DMG-04 cable");

    step_linked_t_cycles(&mut linked, 52);

    linked
        .machine_mut(0)
        .expect("left machine")
        .write_bus(0xFF0F, 0x00);
    linked
        .machine_mut(1)
        .expect("right machine")
        .write_bus(0xFF0F, 0x00);
    linked
        .machine_mut(0)
        .expect("left machine")
        .write_bus(0xFF01, 0xA5);
    linked
        .machine_mut(0)
        .expect("left machine")
        .write_bus(0xFF02, 0x81);
    linked
        .machine_mut(1)
        .expect("right machine")
        .write_bus(0xFF01, 0x3C);
    linked
        .machine_mut(1)
        .expect("right machine")
        .write_bus(0xFF02, 0x80);

    step_linked_t_cycles(&mut linked, 8 * 512);

    assert_eq!(
        linked.machine(0).expect("left machine").serial().read_sb(),
        0x3C
    );
    assert_eq!(
        linked.machine(1).expect("right machine").serial().read_sb(),
        0xA5
    );
    assert_eq!(
        linked
            .machine(0)
            .expect("left machine")
            .serial()
            .transfer_state(),
        SerialTransferState::Idle
    );
    assert_eq!(
        linked
            .machine(1)
            .expect("right machine")
            .serial()
            .transfer_state(),
        SerialTransferState::Idle
    );
    assert_eq!(
        linked
            .machine(0)
            .expect("left machine")
            .interrupts()
            .read_if()
            & 0x08,
        0x08
    );
    assert_eq!(
        linked
            .machine(1)
            .expect("right machine")
            .interrupts()
            .read_if()
            & 0x08,
        0x08
    );
}

#[test]
fn dmg04_master_edges_advance_both_consoles_on_the_same_shared_t_cycle() {
    let mut linked = LinkedMachines::new(vec![dmg_skip_boot_machine(), dmg_skip_boot_machine()])
        .expect("matching machines should link");
    linked
        .attach_dmg04_cable()
        .expect("two-machine session should accept DMG-04 cable");

    step_linked_t_cycles(&mut linked, 52);

    linked
        .machine_mut(0)
        .expect("left machine")
        .write_bus(0xFF01, 0x81);
    linked
        .machine_mut(0)
        .expect("left machine")
        .write_bus(0xFF02, 0x81);
    linked
        .machine_mut(1)
        .expect("right machine")
        .write_bus(0xFF01, 0x00);
    linked
        .machine_mut(1)
        .expect("right machine")
        .write_bus(0xFF02, 0x80);

    step_linked_t_cycles(&mut linked, 511);

    assert_eq!(
        linked.machine(0).expect("left machine").serial().read_sb(),
        0x81
    );
    assert_eq!(
        linked.machine(1).expect("right machine").serial().read_sb(),
        0x00
    );

    let result = linked.step_t_cycle();

    assert_eq!(
        result
            .machine_context(0)
            .map(|context| context.t_cycle().get()),
        Some(563)
    );
    assert_eq!(
        result
            .machine_context(1)
            .map(|context| context.t_cycle().get()),
        Some(563)
    );
    assert_eq!(
        linked.machine(0).expect("left machine").serial().read_sb(),
        0x02
    );
    assert_eq!(
        linked.machine(1).expect("right machine").serial().read_sb(),
        0x01
    );
    assert_eq!(
        linked
            .machine(0)
            .expect("left machine")
            .serial()
            .transfer_state(),
        SerialTransferState::TransferRequested { bits_shifted: 1 }
    );
    assert_eq!(
        linked
            .machine(1)
            .expect("right machine")
            .serial()
            .transfer_state(),
        SerialTransferState::TransferRequested { bits_shifted: 1 }
    );
}

#[test]
fn dmg04_master_receives_open_line_bits_when_the_other_end_is_detached() {
    let mut linked = LinkedMachines::new(vec![dmg_skip_boot_machine(), dmg_skip_boot_machine()])
        .expect("matching machines should link");
    linked
        .attach_dmg04_cable()
        .expect("two-machine session should accept DMG-04 cable");

    step_linked_t_cycles(&mut linked, 52);

    linked
        .machine_mut(1)
        .expect("right machine")
        .set_external_port_attachment(gb_core::ExternalPortAttachmentKind::None);

    linked
        .machine_mut(0)
        .expect("left machine")
        .write_bus(0xFF01, 0x00);
    linked
        .machine_mut(0)
        .expect("left machine")
        .write_bus(0xFF02, 0x81);

    step_linked_t_cycles(&mut linked, 8 * 512);

    assert_eq!(
        linked.machine(0).expect("left machine").serial().read_sb(),
        0xFF
    );
    assert_eq!(
        linked
            .machine(0)
            .expect("left machine")
            .serial()
            .transfer_state(),
        SerialTransferState::Idle
    );
    assert_eq!(
        linked
            .machine(1)
            .expect("right machine")
            .serial()
            .transfer_state(),
        SerialTransferState::Idle
    );
}

#[test]
fn dmg04_linked_transfer_chronology_matches_the_golden_fixture() {
    let mut linked = LinkedMachines::new(vec![dmg_skip_boot_machine(), dmg_skip_boot_machine()])
        .expect("matching machines should link");
    linked
        .attach_dmg04_cable()
        .expect("two-machine session should accept DMG-04 cable");

    step_linked_t_cycles(&mut linked, 52);

    linked
        .machine_mut(0)
        .expect("left machine")
        .write_bus(0xFF0F, 0x00);
    linked
        .machine_mut(1)
        .expect("right machine")
        .write_bus(0xFF0F, 0x00);
    linked
        .machine_mut(0)
        .expect("left machine")
        .write_bus(0xFF01, 0xA5);
    linked
        .machine_mut(0)
        .expect("left machine")
        .write_bus(0xFF02, 0x81);
    linked
        .machine_mut(1)
        .expect("right machine")
        .write_bus(0xFF01, 0x3C);
    linked
        .machine_mut(1)
        .expect("right machine")
        .write_bus(0xFF02, 0x80);

    let mut chronology = String::new();
    chronology.push_str(&render_linked_serial_line(
        &linked,
        "start",
        linked.next_t_cycle().get(),
    ));

    let mut previous_left_state = linked
        .machine(0)
        .expect("left machine")
        .serial()
        .transfer_state();
    let mut previous_right_state = linked
        .machine(1)
        .expect("right machine")
        .serial()
        .transfer_state();

    for _ in 0..(8 * 512) {
        let result = linked.step_t_cycle();
        let left_state = linked
            .machine(0)
            .expect("left machine")
            .serial()
            .transfer_state();
        let right_state = linked
            .machine(1)
            .expect("right machine")
            .serial()
            .transfer_state();

        if left_state != previous_left_state || right_state != previous_right_state {
            chronology.push_str(&render_linked_serial_line(
                &linked,
                "edge",
                result
                    .machine_context(0)
                    .expect("left context should exist")
                    .t_cycle()
                    .get(),
            ));
            previous_left_state = left_state;
            previous_right_state = right_state;
        }
    }

    let fixture_path = common::paths::trace_fixture_path(DMG04_CHRONOLOGY_FIXTURE_NAME);
    common::fixtures::ensure_text_fixture(&fixture_path, &chronology, FIXTURE_ACCEPT_ENV);
}

#[test]
fn dmg04_reuses_the_last_staged_byte_when_one_side_does_not_reload_sb() {
    let mut linked = LinkedMachines::new(vec![dmg_skip_boot_machine(), dmg_skip_boot_machine()])
        .expect("matching machines should link");
    linked
        .attach_dmg04_cable()
        .expect("two-machine session should accept DMG-04 cable");

    step_linked_t_cycles(&mut linked, 52);

    linked
        .machine_mut(0)
        .expect("left machine")
        .write_bus(0xFF01, 0xA5);
    linked
        .machine_mut(0)
        .expect("left machine")
        .write_bus(0xFF02, 0x81);
    linked
        .machine_mut(1)
        .expect("right machine")
        .write_bus(0xFF01, 0x3C);
    linked
        .machine_mut(1)
        .expect("right machine")
        .write_bus(0xFF02, 0x80);

    step_linked_t_cycles(&mut linked, 8 * 512);

    assert_eq!(
        linked.machine(0).expect("left machine").serial().read_sb(),
        0x3C
    );
    assert_eq!(
        linked.machine(1).expect("right machine").serial().read_sb(),
        0xA5
    );

    linked
        .machine_mut(0)
        .expect("left machine")
        .write_bus(0xFF02, 0x81);
    linked
        .machine_mut(1)
        .expect("right machine")
        .write_bus(0xFF01, 0xF0);
    linked
        .machine_mut(1)
        .expect("right machine")
        .write_bus(0xFF02, 0x80);

    step_linked_t_cycles(&mut linked, 8 * 512);

    assert_eq!(
        linked.machine(0).expect("left machine").serial().read_sb(),
        0xF0
    );
    assert_eq!(
        linked.machine(1).expect("right machine").serial().read_sb(),
        0xA5
    );
    assert_eq!(
        linked
            .machine_mut(0)
            .expect("left machine")
            .take_serial_output_bytes(),
        vec![0xA5, 0xA5]
    );
    assert_eq!(
        linked
            .machine_mut(1)
            .expect("right machine")
            .take_serial_output_bytes(),
        vec![0x3C, 0xF0]
    );
}

#[test]
fn dmg04_double_master_mode_is_treated_as_unsupported_and_falls_back_to_open_line_input() {
    let mut linked = LinkedMachines::new(vec![dmg_skip_boot_machine(), dmg_skip_boot_machine()])
        .expect("matching machines should link");
    linked
        .attach_dmg04_cable()
        .expect("two-machine session should accept DMG-04 cable");

    step_linked_t_cycles(&mut linked, 52);

    linked
        .machine_mut(0)
        .expect("left machine")
        .write_bus(0xFF01, 0xA5);
    linked
        .machine_mut(0)
        .expect("left machine")
        .write_bus(0xFF02, 0x81);
    linked
        .machine_mut(1)
        .expect("right machine")
        .write_bus(0xFF01, 0x3C);
    linked
        .machine_mut(1)
        .expect("right machine")
        .write_bus(0xFF02, 0x81);

    step_linked_t_cycles(&mut linked, 8 * 512);

    assert_eq!(
        linked.machine(0).expect("left machine").serial().read_sb(),
        0xFF
    );
    assert_eq!(
        linked.machine(1).expect("right machine").serial().read_sb(),
        0xFF
    );
    assert_eq!(
        linked
            .machine_mut(0)
            .expect("left machine")
            .take_serial_output_bytes(),
        vec![0xA5]
    );
    assert_eq!(
        linked
            .machine_mut(1)
            .expect("right machine")
            .take_serial_output_bytes(),
        vec![0x3C]
    );
}

#[test]
fn session_level_detach_stops_dmg04_exchange_without_touching_machine_ports_directly() {
    let mut linked = LinkedMachines::new(vec![dmg_skip_boot_machine(), dmg_skip_boot_machine()])
        .expect("matching machines should link");
    linked
        .attach_dmg04_cable()
        .expect("two-machine session should accept DMG-04 cable");

    step_linked_t_cycles(&mut linked, 52);
    linked.detach_link_topology();

    linked
        .machine_mut(0)
        .expect("left machine")
        .write_bus(0xFF01, 0x00);
    linked
        .machine_mut(0)
        .expect("left machine")
        .write_bus(0xFF02, 0x81);
    linked
        .machine_mut(1)
        .expect("right machine")
        .write_bus(0xFF01, 0x3C);
    linked
        .machine_mut(1)
        .expect("right machine")
        .write_bus(0xFF02, 0x80);

    step_linked_t_cycles(&mut linked, 8 * 512);

    assert_eq!(linked.topology_kind(), gb_core::LinkedTopologyKind::None);
    assert_eq!(
        linked.machine(0).expect("left machine").serial().read_sb(),
        0xFF
    );
    assert_eq!(
        linked.machine(1).expect("right machine").serial().read_sb(),
        0x3C
    );
    assert_eq!(
        linked
            .machine(0)
            .expect("left machine")
            .serial()
            .transfer_state(),
        SerialTransferState::Idle
    );
    assert_eq!(
        linked
            .machine(1)
            .expect("right machine")
            .serial()
            .transfer_state(),
        SerialTransferState::TransferRequested { bits_shifted: 0 }
    );
}

#[test]
fn session_level_reattach_restores_dmg04_exchange_after_detach() {
    let mut linked = LinkedMachines::new(vec![dmg_skip_boot_machine(), dmg_skip_boot_machine()])
        .expect("matching machines should link");
    linked
        .attach_dmg04_cable()
        .expect("two-machine session should accept DMG-04 cable");

    step_linked_t_cycles(&mut linked, 52);
    linked.detach_link_topology();
    linked
        .attach_dmg04_cable()
        .expect("reattach should restore the DMG-04 topology");

    linked
        .machine_mut(0)
        .expect("left machine")
        .write_bus(0xFF01, 0x81);
    linked
        .machine_mut(0)
        .expect("left machine")
        .write_bus(0xFF02, 0x81);
    linked
        .machine_mut(1)
        .expect("right machine")
        .write_bus(0xFF01, 0x00);
    linked
        .machine_mut(1)
        .expect("right machine")
        .write_bus(0xFF02, 0x80);

    step_linked_t_cycles(&mut linked, 8 * 512);

    assert_eq!(linked.topology_kind(), gb_core::LinkedTopologyKind::Dmg04);
    assert_eq!(
        linked.machine(0).expect("left machine").serial().read_sb(),
        0x00
    );
    assert_eq!(
        linked.machine(1).expect("right machine").serial().read_sb(),
        0x81
    );
}

fn render_linked_serial_line(linked: &LinkedMachines, label: &str, t_cycle: u64) -> String {
    let left = linked.machine(0).expect("left machine");
    let right = linked.machine(1).expect("right machine");

    format!(
        "{label} t_cycle={t_cycle} left_sb={:#04X} left_sc={:#04X} left_state={:?} left_if={:#04X} right_sb={:#04X} right_sc={:#04X} right_state={:?} right_if={:#04X}\n",
        left.serial().read_sb(),
        left.serial().read_sc(),
        left.serial().transfer_state(),
        left.interrupts().read_if(),
        right.serial().read_sb(),
        right.serial().read_sc(),
        right.serial().transfer_state(),
        right.interrupts().read_if(),
    )
}
