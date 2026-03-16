use gb_core::{BootRomAssets, BootRomKind, ConsoleModel, Machine, MachineConfig, StartupMode};

const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;
const BOOT_ROM_LEN: usize = 0x0100;
const PHASE_2_ENTRY_OPCODE: u8 = 0xD3;

fn build_test_rom(program: &[u8], boot_opcode: u8) -> Vec<u8> {
    let mut rom = vec![0xFF; HEADER_MINIMUM_ROM_LEN.max(32 * 1024)];
    rom[0x0000] = boot_opcode;
    for (offset, byte) in program.iter().copied().enumerate() {
        rom[0x0100 + offset] = byte;
    }
    rom[0x0147] = 0x00;
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;
    rom
}

fn build_phase_2_trace_boot_rom() -> Vec<u8> {
    let mut rom = vec![0x00; BOOT_ROM_LEN];
    rom[0x0000..0x0003].copy_from_slice(&[0xC3, 0xFB, 0x00]);
    rom[0x00FB..0x00FD].copy_from_slice(&[0x3E, 0x01]);
    rom[0x00FD..0x0100].copy_from_slice(&[0xEA, 0x50, 0xFF]);
    rom
}

fn build_phase_2_real_boot_rom(entry_opcode: u8) -> Vec<u8> {
    let mut rom = build_test_rom(&[entry_opcode], 0x12);
    rom[0x0100] = entry_opcode;
    rom
}

fn step_machine_t_cycles(machine: &mut Machine, steps: usize) {
    for _ in 0..steps {
        machine.step_t_cycle();
    }
}

fn step_machine_until(
    machine: &mut Machine,
    max_steps: usize,
    predicate: impl Fn(&Machine) -> bool,
) {
    for _ in 0..max_steps {
        if predicate(machine) {
            return;
        }
        machine.step_t_cycle();
    }

    assert!(
        predicate(machine),
        "predicate was not satisfied within {max_steps} T-cycles"
    );
}

fn assert_trace_fragments_in_order(trace: &str, fragments: &[&str]) {
    let mut search_start = 0;

    for fragment in fragments {
        let relative_index = trace[search_start..]
            .find(fragment)
            .unwrap_or_else(|| panic!("trace does not contain fragment: {fragment}"));
        search_start += relative_index + fragment.len();
    }
}

#[test]
fn phase_2_trace_shows_fetch_operand_if_visibility_and_interrupt_acceptance() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0x3E, 0x12, 0xFB, 0x00], 0x12))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFFFF, 0x01);
    machine.write_bus(0xFF0F, 0x01);

    step_machine_t_cycles(&mut machine, 16);

    let trace = machine.tracer().sink().render_text();

    assert_trace_fragments_in_order(
        &trace,
        &[
            "subsystem=cpu level=trace message=\"t_cycle=3 phase=cpu_micro_operation",
            "last_bus_activity=opcode_fetch@0x0100=0x3E",
            "subsystem=cpu level=trace message=\"t_cycle=7 phase=cpu_micro_operation",
            "last_bus_activity=operand_read@0x0101=0x12",
            "subsystem=interrupts level=trace message=\"t_cycle=15 phase=interrupt_aggregation console_model=Dmg status=Ready if=0xE1 ie=0x01\"",
            "subsystem=interrupts level=trace message=\"t_cycle=15 phase=cpu_wake_interrupt_evaluation console_model=Dmg status=Ready if=0xE0 ie=0x01\"",
            "subsystem=cpu level=trace message=\"t_cycle=15 phase=cpu_wake_interrupt_evaluation",
            "execution_state=ServiceInterrupt { source: VBlank, step: 0, t_cycle: 0 }",
        ],
    );
}

#[test]
fn phase_2_trace_shows_boot_handoff_before_the_first_cartridge_fetch() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg)
            .with_startup_mode(StartupMode::RealBoot)
            .with_boot_rom_assets(
                BootRomAssets::none()
                    .with_bytes(BootRomKind::Dmg, build_phase_2_trace_boot_rom())
                    .expect("phase 2 boot-trace ROM should validate"),
            ),
    );

    machine
        .load_cartridge(build_phase_2_real_boot_rom(PHASE_2_ENTRY_OPCODE))
        .expect("NoMBC test ROM should load");

    step_machine_until(&mut machine, 48, |machine| {
        machine.cpu().current_opcode() == Some(PHASE_2_ENTRY_OPCODE)
    });

    let trace = machine.tracer().sink().render_text();

    assert_trace_fragments_in_order(
        &trace,
        &[
            "subsystem=cpu level=trace message=\"t_cycle=39 phase=cpu_micro_operation",
            "last_bus_activity=data_write@0xFF50=",
            "subsystem=boot level=trace message=\"t_cycle=39 phase=mmio_side_effect_commit console_model=Dmg startup_mode=RealBoot status=Ready boot_rom_kind=Dmg boot_rom_mapped=false\"",
            "subsystem=cpu level=trace message=\"t_cycle=43 phase=cpu_micro_operation",
            "last_bus_activity=opcode_fetch@0x0100=0xD3",
        ],
    );
}
