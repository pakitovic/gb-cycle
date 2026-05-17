use super::cgb_ir::CgbInfraredPair;
use super::dmg04::Dmg04Cable;
use super::dmg07::{Dmg07Adapter, Dmg07Participant, Dmg07Port, validate_dmg07_participants};
use crate::debugger::{TraceBuffer, TraceSink};
use crate::external_port::ExternalPortAttachmentKind;
use crate::machine::{Machine, MachineStepObserver, NoopMachineStepObserver};
use crate::scheduler::{CycleContext, GlobalScheduler, SchedulerPhase, TCycle};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedStepResult {
    contexts: Vec<CycleContext>,
}

impl LinkedStepResult {
    pub fn contexts(&self) -> &[CycleContext] {
        &self.contexts
    }

    pub fn machine_context(&self, machine_index: usize) -> Option<&CycleContext> {
        self.contexts.get(machine_index)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkedMachinesError {
    TooFewMachines {
        count: usize,
    },
    MismatchedNextTCycle {
        expected: TCycle,
        found: TCycle,
        machine_index: usize,
    },
    UnsupportedMachineCountForDmg04 {
        count: usize,
    },
    UnsupportedMachineCountForDmg07 {
        count: usize,
    },
    UnsupportedMachineCountForCgbInfrared {
        count: usize,
    },
    MissingDmg07PlayerOne,
    DuplicateDmg07Port {
        port: Dmg07Port,
    },
    DuplicateDmg07MachineIndex {
        machine_index: usize,
    },
    Dmg07MachineIndexOutOfBounds {
        machine_index: usize,
        machine_count: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LinkedTopologyKind {
    #[default]
    None,
    Dmg04,
    Dmg07,
    CgbInfrared,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
enum LinkTopology {
    #[default]
    None,
    Dmg04(Dmg04Cable),
    Dmg07(Dmg07Adapter),
    CgbInfrared(CgbInfraredPair),
}

#[derive(Debug, Clone)]
pub struct LinkedMachines<S = TraceBuffer> {
    scheduler: GlobalScheduler,
    machines: Vec<Machine<S>>,
    topology: LinkTopology,
    contexts: Vec<CycleContext>,
}

impl<S: TraceSink> LinkedMachines<S> {
    pub fn new(machines: Vec<Machine<S>>) -> Result<Self, LinkedMachinesError> {
        let count = machines.len();
        if count < 2 {
            return Err(LinkedMachinesError::TooFewMachines { count });
        }

        let expected = machines[0].next_t_cycle();
        for (machine_index, machine) in machines.iter().enumerate().skip(1) {
            let found = machine.next_t_cycle();
            if found != expected {
                return Err(LinkedMachinesError::MismatchedNextTCycle {
                    expected,
                    found,
                    machine_index,
                });
            }
        }

        let mut scheduler = GlobalScheduler::new();
        scheduler.set_next_t_cycle(expected);

        Ok(Self {
            scheduler,
            machines,
            topology: LinkTopology::None,
            contexts: vec![CycleContext::for_cycle(expected); count],
        })
    }

    pub fn scheduler(&self) -> &GlobalScheduler {
        &self.scheduler
    }

    pub fn next_t_cycle(&self) -> TCycle {
        self.scheduler.next_t_cycle()
    }

    pub fn machine_count(&self) -> usize {
        self.machines.len()
    }

    pub fn machine(&self, machine_index: usize) -> Option<&Machine<S>> {
        self.machines.get(machine_index)
    }

    pub fn machine_mut(&mut self, machine_index: usize) -> Option<&mut Machine<S>> {
        self.machines.get_mut(machine_index)
    }

    pub fn machines(&self) -> &[Machine<S>] {
        &self.machines
    }

    pub fn machines_mut(&mut self) -> &mut [Machine<S>] {
        &mut self.machines
    }

    pub fn into_machines(self) -> Vec<Machine<S>> {
        self.machines
    }

    pub fn topology_kind(&self) -> LinkedTopologyKind {
        self.topology.kind()
    }

    pub fn attach_dmg04_cable(&mut self) -> Result<(), LinkedMachinesError> {
        if self.machines.len() != 2 {
            return Err(LinkedMachinesError::UnsupportedMachineCountForDmg04 {
                count: self.machines.len(),
            });
        }

        self.detach_link_topology();

        for machine in &mut self.machines {
            machine.set_external_port_attachment(ExternalPortAttachmentKind::GameLinkDmg04);
            machine.set_dmg04_incoming_byte(None);
        }

        self.topology = LinkTopology::Dmg04(Dmg04Cable::new(0, 1));
        Ok(())
    }

    pub fn attach_dmg07_adapter(
        &mut self,
        participants: &[Dmg07Participant],
    ) -> Result<(), LinkedMachinesError> {
        validate_dmg07_participants(participants, self.machines.len())?;

        self.detach_link_topology();

        for participant in participants {
            self.machines[participant.machine_index].set_dmg07_attachment(participant.port);
            self.machines[participant.machine_index].set_dmg07_incoming_byte(None);
        }

        self.topology = LinkTopology::Dmg07(Dmg07Adapter::new(participants));
        Ok(())
    }

    pub fn attach_cgb_infrared_pair(&mut self) -> Result<(), LinkedMachinesError> {
        if self.machines.len() != 2 {
            return Err(LinkedMachinesError::UnsupportedMachineCountForCgbInfrared {
                count: self.machines.len(),
            });
        }

        self.detach_link_topology();
        for machine in &mut self.machines {
            machine.set_cgb_infrared_external_input(false);
        }

        self.topology = LinkTopology::CgbInfrared(CgbInfraredPair::new(0, 1));
        Ok(())
    }

    pub fn detach_link_topology(&mut self) {
        match &self.topology {
            LinkTopology::None => {}
            LinkTopology::Dmg04(cable) => cable.detach(&mut self.machines),
            LinkTopology::Dmg07(adapter) => adapter.detach(&mut self.machines),
            LinkTopology::CgbInfrared(pair) => pair.detach(&mut self.machines),
        }

        self.topology = LinkTopology::None;
    }

    pub fn step_t_cycle(&mut self) -> LinkedStepResult {
        self.advance_t_cycle_with_observer(&mut NoopMachineStepObserver);
        LinkedStepResult {
            contexts: self.contexts.clone(),
        }
    }

    pub fn advance_t_cycle(&mut self) {
        self.advance_t_cycle_with_observer(&mut NoopMachineStepObserver);
    }

    pub fn advance_t_cycle_with_observer<O: MachineStepObserver>(&mut self, observer: &mut O) {
        let t_cycle = self.scheduler.next_t_cycle();
        debug_assert_eq!(self.contexts.len(), self.machines.len());
        for context in &mut self.contexts {
            context.reset_for_cycle(t_cycle);
        }

        for &phase in SchedulerPhase::all() {
            for context in &mut self.contexts {
                context.enter_phase(phase);
            }

            self.prepare_phase(phase);

            for (machine, context) in self.machines.iter_mut().zip(self.contexts.iter_mut()) {
                machine.step_phase_with_context(context, observer);
            }

            self.finish_phase(phase);
        }

        let next_t_cycle = t_cycle.next();
        self.scheduler.set_next_t_cycle(next_t_cycle);
        for machine in &mut self.machines {
            machine.sync_scheduler_next_t_cycle(next_t_cycle);
        }
    }

    fn prepare_phase(&mut self, phase: SchedulerPhase) {
        let t_cycle = self.scheduler.next_t_cycle();
        match &mut self.topology {
            LinkTopology::None => {}
            LinkTopology::Dmg04(cable) => cable.prepare_phase(phase, &mut self.machines),
            LinkTopology::CgbInfrared(pair) => pair.prepare_phase(phase, &mut self.machines),
            LinkTopology::Dmg07(adapter) => {
                adapter.prepare_phase(phase, t_cycle, &mut self.machines);
            }
        }
    }

    fn finish_phase(&mut self, phase: SchedulerPhase) {
        let t_cycle = self.scheduler.next_t_cycle();
        match &mut self.topology {
            LinkTopology::None | LinkTopology::Dmg04(_) => {}
            LinkTopology::CgbInfrared(_) => {}
            LinkTopology::Dmg07(adapter) => {
                adapter.finish_phase(phase, t_cycle, &mut self.machines);
            }
        }
    }

    pub fn topology_trace_text(&self) -> Option<String> {
        match &self.topology {
            LinkTopology::None | LinkTopology::Dmg04(_) | LinkTopology::CgbInfrared(_) => None,
            LinkTopology::Dmg07(adapter) => adapter.trace_text(),
        }
    }
}

impl LinkTopology {
    const fn kind(&self) -> LinkedTopologyKind {
        match self {
            Self::None => LinkedTopologyKind::None,
            Self::Dmg04(_) => LinkedTopologyKind::Dmg04,
            Self::Dmg07(_) => LinkedTopologyKind::Dmg07,
            Self::CgbInfrared(_) => LinkedTopologyKind::CgbInfrared,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::MachineStepObserver;
    use crate::model::{ConsoleModel, MachineConfig, StartupMode};

    const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;
    const CGB_IR_SIGNAL_VISIBLE_T_CYCLES: u64 = 19_900;
    const CGB_IR_POKEMON_GSC_SHORT_PULSE_SAMPLE_T_CYCLES: u64 = 128;

    fn build_cgb_test_rom(cgb_header: u8) -> Vec<u8> {
        let mut rom = vec![0xFF; HEADER_MINIMUM_ROM_LEN.max(32 * 1024)];
        rom[0x0100] = 0xC3;
        rom[0x0101] = 0x00;
        rom[0x0102] = 0x01;
        rom[0x0143] = cgb_header;
        rom[0x0147] = 0x00;
        rom[0x0148] = 0x00;
        rom[0x0149] = 0x00;
        rom
    }

    fn dmg_skip_boot_machine() -> Machine {
        Machine::new(
            MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
        )
    }

    fn cgb_native_skip_boot_machine() -> Machine {
        let mut machine = Machine::new(
            MachineConfig::new(ConsoleModel::GameBoyColor).with_startup_mode(StartupMode::SkipBoot),
        );
        machine
            .load_cartridge(build_cgb_test_rom(0x80))
            .expect("CGB native test ROM should load");
        machine
    }

    fn cgb_compat_skip_boot_machine() -> Machine {
        let mut machine = Machine::new(
            MachineConfig::new(ConsoleModel::GameBoyColor).with_startup_mode(StartupMode::SkipBoot),
        );
        machine
            .load_cartridge(build_cgb_test_rom(0x00))
            .expect("CGB compatibility test ROM should load");
        machine
    }

    #[derive(Default)]
    struct RegionCountObserver {
        machine_regions: usize,
        ppu_regions: usize,
    }

    impl MachineStepObserver for RegionCountObserver {
        fn begin_region(&mut self, _region: crate::machine::MachineStepRegion) {
            self.machine_regions += 1;
        }

        fn begin_ppu_region(&mut self, _region: crate::ppu::PpuStepRegion) {
            self.ppu_regions += 1;
        }
    }

    #[test]
    fn linked_machines_require_at_least_two_participants() {
        let machine = dmg_skip_boot_machine();

        let error = LinkedMachines::new(vec![machine]).expect_err("single machine should fail");

        assert_eq!(error, LinkedMachinesError::TooFewMachines { count: 1 });
    }

    #[test]
    fn linked_machines_reject_mismatched_scheduler_positions() {
        let left = dmg_skip_boot_machine();
        let mut right = dmg_skip_boot_machine();
        right.step_t_cycle();

        let error = LinkedMachines::new(vec![left, right]).expect_err("mismatched cycles");

        assert_eq!(
            error,
            LinkedMachinesError::MismatchedNextTCycle {
                expected: TCycle::ZERO,
                found: TCycle::new(1),
                machine_index: 1,
            }
        );
    }

    #[test]
    fn linked_machines_advance_all_members_on_the_same_shared_t_cycle() {
        let mut linked =
            LinkedMachines::new(vec![dmg_skip_boot_machine(), dmg_skip_boot_machine()])
                .expect("matching machines should link");

        let result = linked.step_t_cycle();

        assert_eq!(linked.next_t_cycle(), TCycle::new(1));
        assert_eq!(result.contexts().len(), 2);
        assert_eq!(
            result.machine_context(0).map(CycleContext::t_cycle),
            Some(TCycle::ZERO)
        );
        assert_eq!(
            result.machine_context(1).map(CycleContext::t_cycle),
            Some(TCycle::ZERO)
        );
        assert_eq!(
            linked.machine(0).map(Machine::next_t_cycle),
            Some(TCycle::new(1))
        );
        assert_eq!(
            linked.machine(1).map(Machine::next_t_cycle),
            Some(TCycle::new(1))
        );
    }

    #[test]
    fn linked_machines_can_step_with_an_observer_without_materializing_a_result() {
        let mut linked =
            LinkedMachines::new(vec![dmg_skip_boot_machine(), dmg_skip_boot_machine()])
                .expect("matching machines should link");
        let mut observer = RegionCountObserver::default();

        linked.advance_t_cycle_with_observer(&mut observer);

        assert_eq!(linked.next_t_cycle(), TCycle::new(1));
        assert!(observer.machine_regions > 0);
        assert!(observer.ppu_regions > 0);
    }

    #[test]
    fn linked_machines_match_independent_execution_without_cross_machine_links() {
        let mut independent_left = dmg_skip_boot_machine();
        let mut independent_right = dmg_skip_boot_machine();
        independent_right.set_external_port_attachment(ExternalPortAttachmentKind::Printer);

        let mut linked = LinkedMachines::new(vec![dmg_skip_boot_machine(), {
            let mut machine = dmg_skip_boot_machine();
            machine.set_external_port_attachment(ExternalPortAttachmentKind::Printer);
            machine
        }])
        .expect("matching machines should link");

        for _ in 0..8 {
            independent_left.step_t_cycle();
            independent_right.step_t_cycle();
            linked.step_t_cycle();
        }

        assert_eq!(
            linked.machine(0).expect("left machine").snapshot(),
            independent_left.snapshot()
        );
        assert_eq!(
            linked.machine(1).expect("right machine").snapshot(),
            independent_right.snapshot()
        );
    }

    #[test]
    fn dmg04_attachment_requires_exactly_two_machines() {
        let mut linked = LinkedMachines::new(vec![
            dmg_skip_boot_machine(),
            dmg_skip_boot_machine(),
            dmg_skip_boot_machine(),
        ])
        .expect("matching machines should link");

        let error = linked
            .attach_dmg04_cable()
            .expect_err("three-machine session should reject DMG-04 cable");

        assert_eq!(
            error,
            LinkedMachinesError::UnsupportedMachineCountForDmg04 { count: 3 }
        );
    }

    #[test]
    fn attach_dmg04_cable_marks_both_external_ports_as_game_link() {
        let mut linked =
            LinkedMachines::new(vec![dmg_skip_boot_machine(), dmg_skip_boot_machine()])
                .expect("matching machines should link");

        linked
            .attach_dmg04_cable()
            .expect("two-machine session should accept DMG-04 cable");

        assert_eq!(
            linked
                .machine(0)
                .map(|machine| machine.external_port().attachment_kind()),
            Some(ExternalPortAttachmentKind::GameLinkDmg04)
        );
        assert_eq!(
            linked
                .machine(1)
                .map(|machine| machine.external_port().attachment_kind()),
            Some(ExternalPortAttachmentKind::GameLinkDmg04)
        );
        assert_eq!(linked.topology_kind(), LinkedTopologyKind::Dmg04);
    }

    #[test]
    fn detach_link_topology_clears_the_session_owned_dmg04_attachment() {
        let mut linked =
            LinkedMachines::new(vec![dmg_skip_boot_machine(), dmg_skip_boot_machine()])
                .expect("matching machines should link");

        linked
            .attach_dmg04_cable()
            .expect("two-machine session should accept DMG-04 cable");

        linked.detach_link_topology();

        assert_eq!(linked.topology_kind(), LinkedTopologyKind::None);
        assert_eq!(
            linked
                .machine(0)
                .map(|machine| machine.external_port().attachment_kind()),
            Some(ExternalPortAttachmentKind::None)
        );
        assert_eq!(
            linked
                .machine(1)
                .map(|machine| machine.external_port().attachment_kind()),
            Some(ExternalPortAttachmentKind::None)
        );
    }

    #[test]
    fn dmg04_can_be_reattached_after_session_level_detach() {
        let mut linked =
            LinkedMachines::new(vec![dmg_skip_boot_machine(), dmg_skip_boot_machine()])
                .expect("matching machines should link");

        linked
            .attach_dmg04_cable()
            .expect("two-machine session should accept DMG-04 cable");
        linked.detach_link_topology();
        linked
            .attach_dmg04_cable()
            .expect("reattach should restore DMG-04 ownership");

        assert_eq!(linked.topology_kind(), LinkedTopologyKind::Dmg04);
        assert_eq!(
            linked
                .machine(0)
                .map(|machine| machine.external_port().attachment_kind()),
            Some(ExternalPortAttachmentKind::GameLinkDmg04)
        );
        assert_eq!(
            linked
                .machine(1)
                .map(|machine| machine.external_port().attachment_kind()),
            Some(ExternalPortAttachmentKind::GameLinkDmg04)
        );
    }

    #[test]
    fn attach_dmg07_adapter_accepts_sparse_physical_ports() {
        let mut linked =
            LinkedMachines::new(vec![dmg_skip_boot_machine(), dmg_skip_boot_machine()])
                .expect("matching machines should link");

        linked
            .attach_dmg07_adapter(&[
                Dmg07Participant::new(0, Dmg07Port::P1),
                Dmg07Participant::new(1, Dmg07Port::P4),
            ])
            .expect("sparse P1/P4 adapter occupancy should be valid");

        assert_eq!(linked.topology_kind(), LinkedTopologyKind::Dmg07);
        assert_eq!(
            linked
                .machine(0)
                .map(|machine| machine.external_port().attachment_kind()),
            Some(ExternalPortAttachmentKind::FourPlayerAdapterDmg07)
        );
        assert_eq!(
            linked
                .machine(0)
                .and_then(|machine| machine.external_port().dmg07_port()),
            Some(Dmg07Port::P1)
        );
        assert_eq!(
            linked
                .machine(1)
                .and_then(|machine| machine.external_port().dmg07_port()),
            Some(Dmg07Port::P4)
        );
    }

    #[test]
    fn attach_dmg07_adapter_requires_player_one() {
        let mut linked =
            LinkedMachines::new(vec![dmg_skip_boot_machine(), dmg_skip_boot_machine()])
                .expect("matching machines should link");

        let error = linked
            .attach_dmg07_adapter(&[
                Dmg07Participant::new(0, Dmg07Port::P2),
                Dmg07Participant::new(1, Dmg07Port::P4),
            ])
            .expect_err("adapter should require P1");

        assert_eq!(error, LinkedMachinesError::MissingDmg07PlayerOne);
    }

    #[test]
    fn attach_dmg07_adapter_rejects_duplicate_ports_and_machine_indexes() {
        let mut linked =
            LinkedMachines::new(vec![dmg_skip_boot_machine(), dmg_skip_boot_machine()])
                .expect("matching machines should link");

        let duplicate_port = linked
            .attach_dmg07_adapter(&[
                Dmg07Participant::new(0, Dmg07Port::P1),
                Dmg07Participant::new(1, Dmg07Port::P1),
            ])
            .expect_err("adapter should reject duplicate ports");
        assert_eq!(
            duplicate_port,
            LinkedMachinesError::DuplicateDmg07Port {
                port: Dmg07Port::P1
            }
        );

        let duplicate_machine = linked
            .attach_dmg07_adapter(&[
                Dmg07Participant::new(0, Dmg07Port::P1),
                Dmg07Participant::new(0, Dmg07Port::P2),
            ])
            .expect_err("adapter should reject duplicate machines");
        assert_eq!(
            duplicate_machine,
            LinkedMachinesError::DuplicateDmg07MachineIndex { machine_index: 0 }
        );
    }

    #[test]
    fn attach_dmg07_adapter_rejects_out_of_bounds_machine_index() {
        let mut linked =
            LinkedMachines::new(vec![dmg_skip_boot_machine(), dmg_skip_boot_machine()])
                .expect("matching machines should link");

        let error = linked
            .attach_dmg07_adapter(&[
                Dmg07Participant::new(0, Dmg07Port::P1),
                Dmg07Participant::new(2, Dmg07Port::P2),
            ])
            .expect_err("adapter should reject missing machines");

        assert_eq!(
            error,
            LinkedMachinesError::Dmg07MachineIndexOutOfBounds {
                machine_index: 2,
                machine_count: 2,
            }
        );
    }

    #[test]
    fn detach_link_topology_clears_the_session_owned_dmg07_adapter() {
        let mut linked =
            LinkedMachines::new(vec![dmg_skip_boot_machine(), dmg_skip_boot_machine()])
                .expect("matching machines should link");

        linked
            .attach_dmg07_adapter(&[
                Dmg07Participant::new(0, Dmg07Port::P1),
                Dmg07Participant::new(1, Dmg07Port::P4),
            ])
            .expect("adapter should attach");

        linked.detach_link_topology();

        assert_eq!(linked.topology_kind(), LinkedTopologyKind::None);
        assert_eq!(
            linked
                .machine(0)
                .map(|machine| machine.external_port().attachment_kind()),
            Some(ExternalPortAttachmentKind::None)
        );
        assert_eq!(
            linked
                .machine(1)
                .map(|machine| machine.external_port().attachment_kind()),
            Some(ExternalPortAttachmentKind::None)
        );
    }

    #[test]
    fn cgb_infrared_pair_routes_emitter_light_between_two_native_cgb_machines() {
        let left = cgb_native_skip_boot_machine();
        let right = cgb_native_skip_boot_machine();
        let mut linked = LinkedMachines::new(vec![left, right]).expect("CGB machines should link");
        linked
            .attach_cgb_infrared_pair()
            .expect("two-machine CGB IR pair should attach");

        linked
            .machine_mut(0)
            .expect("left machine should exist")
            .write_bus(0xFF56, 0xC1);
        linked
            .machine_mut(1)
            .expect("right machine should exist")
            .write_bus(0xFF56, 0xC0);

        assert_eq!(
            linked
                .machine_mut(1)
                .expect("right machine should exist")
                .read_bus(0xFF56)
                & 0x02,
            0x02
        );

        for _ in 0..CGB_IR_SIGNAL_VISIBLE_T_CYCLES {
            linked.advance_t_cycle();
        }

        assert!(
            linked
                .machine(1)
                .expect("right machine should exist")
                .cgb_infrared_effective_signal_detected()
        );
        assert_eq!(
            linked
                .machine_mut(1)
                .expect("right machine should exist")
                .read_bus(0xFF56)
                & 0x02,
            0x00
        );

        linked
            .machine_mut(0)
            .expect("left machine should exist")
            .write_bus(0xFF56, 0xC0);
        linked.advance_t_cycle();

        assert_eq!(
            linked
                .machine_mut(1)
                .expect("right machine should exist")
                .read_bus(0xFF56)
                & 0x02,
            0x02
        );
    }

    #[test]
    fn cgb_infrared_pair_routes_short_pulse_to_readied_peer_sensor() {
        let left = cgb_native_skip_boot_machine();
        let right = cgb_native_skip_boot_machine();
        let mut linked = LinkedMachines::new(vec![left, right]).expect("CGB machines should link");
        linked
            .attach_cgb_infrared_pair()
            .expect("two-machine CGB IR pair should attach");

        linked
            .machine_mut(1)
            .expect("right machine should exist")
            .write_bus(0xFF56, 0xC0);

        for _ in 0..CGB_IR_SIGNAL_VISIBLE_T_CYCLES {
            linked.advance_t_cycle();
        }

        linked
            .machine_mut(0)
            .expect("left machine should exist")
            .write_bus(0xFF56, 0xC1);

        for _ in 0..CGB_IR_POKEMON_GSC_SHORT_PULSE_SAMPLE_T_CYCLES {
            linked.advance_t_cycle();
        }

        assert_eq!(
            linked
                .machine_mut(1)
                .expect("right machine should exist")
                .read_bus(0xFF56)
                & 0x02,
            0x00
        );
    }

    #[test]
    fn cgb_infrared_pair_does_not_enable_rp_in_compatibility_mode() {
        let native = cgb_native_skip_boot_machine();
        let compat = cgb_compat_skip_boot_machine();
        let mut linked =
            LinkedMachines::new(vec![native, compat]).expect("CGB machines should link");
        linked
            .attach_cgb_infrared_pair()
            .expect("two-machine CGB IR pair should attach");

        linked
            .machine_mut(0)
            .expect("native machine should exist")
            .write_bus(0xFF56, 0xC1);
        linked
            .machine_mut(1)
            .expect("compat machine should exist")
            .write_bus(0xFF56, 0xC0);

        for _ in 0..CGB_IR_SIGNAL_VISIBLE_T_CYCLES {
            linked.advance_t_cycle();
        }

        assert_eq!(
            linked
                .machine_mut(1)
                .expect("compat machine should exist")
                .read_bus(0xFF56),
            0xFF
        );
    }

    #[test]
    fn cgb_infrared_pair_requires_exactly_two_machines() {
        let mut linked = LinkedMachines::new(vec![
            cgb_native_skip_boot_machine(),
            cgb_native_skip_boot_machine(),
            cgb_native_skip_boot_machine(),
        ])
        .expect("three machines may form a linked container");

        let error = linked
            .attach_cgb_infrared_pair()
            .expect_err("CGB IR pair should reject three participants");

        assert_eq!(
            error,
            LinkedMachinesError::UnsupportedMachineCountForCgbInfrared { count: 3 }
        );
    }
}
