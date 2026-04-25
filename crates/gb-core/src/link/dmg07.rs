use std::collections::BTreeSet;

use crate::debugger::TraceSink;
use crate::external_port::ExternalPortAttachmentKind;
use crate::machine::Machine;
use crate::scheduler::{SchedulerPhase, TCycle};

const DMG07_PING_HEADER: u8 = 0xFE;
const DMG07_ACK: u8 = 0x88;
const DMG07_TRANSMISSION_MARKER: u8 = 0xAA;
const DMG07_TRANSMISSION_INDICATOR: u8 = 0xCC;
const DMG07_RESTART_MARKER: u8 = 0xFF;
const DMG07_RESTART_INDICATOR: u8 = 0xFF;
const DMG07_PING_PACKET_BYTES: usize = 4;
const DMG07_INDICATOR_BYTES: usize = 4;
const DMG07_MAX_SIZE: usize = 4;
const DMG07_INITIAL_BYTE_DELAY_T_CYCLES: u64 = 4_096;
const DMG07_SERIAL_BIT_PERIOD_T_CYCLES: u64 = 67;
const DMG07_PING_INTER_BYTE_DELAY_T_CYCLES: u64 = 5_956;
const DMG07_PING_BASE_INTER_PACKET_DELAY_T_CYCLES: u64 = 51_548;
const DMG07_RATE_LOW_NIBBLE_STEP_DELAY_T_CYCLES: u64 = 4_194;
const DMG07_TRANSMISSION_BASE_INTER_BYTE_DELAY_T_CYCLES: u64 = 3_720;
const DMG07_TRANSMISSION_RATE_STEP_DELAY_T_CYCLES: u64 = 445;
const DMG07_TRANSMISSION_BASE_PACKET_PERIOD_T_CYCLES: u64 = 71_303;

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
pub enum Dmg07Port {
    #[default]
    P1,
    P2,
    P3,
    P4,
}

impl Dmg07Port {
    pub const ALL: [Self; 4] = [Self::P1, Self::P2, Self::P3, Self::P4];

    pub const fn index(self) -> usize {
        match self {
            Self::P1 => 0,
            Self::P2 => 1,
            Self::P3 => 2,
            Self::P4 => 3,
        }
    }

    pub const fn id_byte(self) -> u8 {
        self.index() as u8 + 1
    }

    const fn connection_bit(self) -> u8 {
        0x10 << self.index()
    }

    pub fn from_manifest_name(name: &str) -> Option<Self> {
        match name {
            "p1" | "P1" => Some(Self::P1),
            "p2" | "P2" => Some(Self::P2),
            "p3" | "P3" => Some(Self::P3),
            "p4" | "P4" => Some(Self::P4),
            _ => None,
        }
    }

    pub const fn manifest_name(self) -> &'static str {
        match self {
            Self::P1 => "p1",
            Self::P2 => "p2",
            Self::P3 => "p3",
            Self::P4 => "p4",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Dmg07Participant {
    pub machine_index: usize,
    pub port: Dmg07Port,
}

impl Dmg07Participant {
    pub const fn new(machine_index: usize, port: Dmg07Port) -> Self {
        Self {
            machine_index,
            port,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Dmg07Adapter {
    machine_by_port: [Option<usize>; 4],
    protocol: Dmg07Protocol,
    byte_engine: Dmg07ByteEngine,
    clocked_ports_for_byte: [bool; 4],
    trace: Vec<String>,
}

impl Dmg07Adapter {
    pub(crate) fn new(participants: &[Dmg07Participant]) -> Self {
        let mut machine_by_port = [None; 4];
        for participant in participants {
            machine_by_port[participant.port.index()] = Some(participant.machine_index);
        }

        Self {
            machine_by_port,
            protocol: Dmg07Protocol::new(),
            byte_engine: Dmg07ByteEngine::new(),
            clocked_ports_for_byte: [false; 4],
            trace: Vec::new(),
        }
    }

    pub(crate) fn prepare_phase<S: TraceSink>(
        &mut self,
        phase: SchedulerPhase,
        t_cycle: TCycle,
        machines: &mut [Machine<S>],
    ) {
        if phase != SchedulerPhase::ExternalEventIngress {
            return;
        }

        if !self.byte_engine.should_queue_bit(t_cycle) {
            return;
        }

        for port in Dmg07Port::ALL {
            let Some(machine_index) = self.machine_by_port[port.index()] else {
                continue;
            };
            let incoming_byte = self.protocol.incoming_byte_for(port);
            let machine = &mut machines[machine_index];
            let endpoint = machine.dmg07_endpoint_state();
            if endpoint.waiting_for_external_clock {
                machine.set_dmg07_incoming_byte(Some(incoming_byte));
                machine.queue_external_serial_clock();
                self.clocked_ports_for_byte[port.index()] = true;
            }
        }

        self.byte_engine.record_queued_bit(t_cycle);
    }

    pub(crate) fn finish_phase<S: TraceSink>(
        &mut self,
        phase: SchedulerPhase,
        t_cycle: TCycle,
        machines: &mut [Machine<S>],
    ) {
        if phase != SchedulerPhase::AutonomousPeripheralTicks
            || !self.byte_engine.take_byte_completion_pending()
        {
            return;
        }

        let mut outgoing_by_port = [None; 4];
        for port in Dmg07Port::ALL {
            let Some(machine_index) = self.machine_by_port[port.index()] else {
                continue;
            };
            if self.clocked_ports_for_byte[port.index()] {
                outgoing_by_port[port.index()] =
                    machines[machine_index].latest_completed_serial_output_byte();
            }
            machines[machine_index].set_dmg07_incoming_byte(None);
        }
        self.clocked_ports_for_byte = [false; 4];

        if let Some(trace_line) = self.protocol.complete_byte(outgoing_by_port) {
            self.trace.push(format!("t_cycle={t_cycle} {trace_line}"));
        }
        let delay = self.protocol.delay_after_completed_byte_t_cycles();
        self.byte_engine.schedule_next_byte(t_cycle, delay);
    }

    pub(crate) fn detach<S: TraceSink>(&self, machines: &mut [Machine<S>]) {
        for machine_index in self.machine_by_port.iter().copied().flatten() {
            machines[machine_index].set_external_port_attachment(ExternalPortAttachmentKind::None);
        }
    }

    pub(crate) fn trace_text(&self) -> Option<String> {
        if self.trace.is_empty() {
            None
        } else {
            Some(format!("{}\n", self.trace.join("\n")))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Dmg07ByteEngine {
    next_bit_t_cycle: TCycle,
    queued_bits_for_byte: u8,
    byte_completion_pending: bool,
}

impl Dmg07ByteEngine {
    const fn new() -> Self {
        Self {
            next_bit_t_cycle: TCycle::new(DMG07_INITIAL_BYTE_DELAY_T_CYCLES),
            queued_bits_for_byte: 0,
            byte_completion_pending: false,
        }
    }

    fn should_queue_bit(&self, t_cycle: TCycle) -> bool {
        !self.byte_completion_pending && t_cycle >= self.next_bit_t_cycle
    }

    fn record_queued_bit(&mut self, t_cycle: TCycle) {
        self.queued_bits_for_byte += 1;
        if self.queued_bits_for_byte == 8 {
            self.byte_completion_pending = true;
        } else {
            self.next_bit_t_cycle = TCycle::new(t_cycle.get() + DMG07_SERIAL_BIT_PERIOD_T_CYCLES);
        }
    }

    fn take_byte_completion_pending(&mut self) -> bool {
        std::mem::take(&mut self.byte_completion_pending)
    }

    fn schedule_next_byte(&mut self, t_cycle: TCycle, delay_t_cycles: u64) {
        self.queued_bits_for_byte = 0;
        self.next_bit_t_cycle = TCycle::new(t_cycle.get() + delay_t_cycles.max(1));
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Dmg07Protocol {
    phase: Dmg07ProtocolPhase,
    connected: [bool; 4],
    ping_response_index: [u8; 4],
    rate: u8,
    size: u8,
    transition_marker_count: u8,
    restart_marker_count: u8,
    transmission_restart_requested: bool,
    transmission_buffer: Dmg07TransmissionBuffer,
    trace_transition: Option<String>,
    delay_after_completed_byte_t_cycles: u64,
}

impl Dmg07Protocol {
    fn new() -> Self {
        Self {
            phase: Dmg07ProtocolPhase::Ping { byte_index: 0 },
            connected: [false; 4],
            ping_response_index: [0; 4],
            rate: 0,
            size: 1,
            transition_marker_count: 0,
            restart_marker_count: 0,
            transmission_restart_requested: false,
            transmission_buffer: Dmg07TransmissionBuffer::new(),
            trace_transition: None,
            delay_after_completed_byte_t_cycles: DMG07_PING_INTER_BYTE_DELAY_T_CYCLES,
        }
    }

    fn incoming_byte_for(&self, port: Dmg07Port) -> u8 {
        match self.phase {
            Dmg07ProtocolPhase::Ping { byte_index: 0 } => DMG07_PING_HEADER,
            Dmg07ProtocolPhase::Ping { .. } => self.status_byte_for(port),
            Dmg07ProtocolPhase::TransmissionIndicator { .. } => DMG07_TRANSMISSION_INDICATOR,
            Dmg07ProtocolPhase::Transmission { .. } => {
                self.transmission_buffer.current_byte(self.size as usize)
            }
            Dmg07ProtocolPhase::PingRestartIndicator { .. } => DMG07_RESTART_INDICATOR,
        }
    }

    fn complete_byte(&mut self, outgoing_by_port: [Option<u8>; 4]) -> Option<String> {
        self.trace_transition = None;
        let completed_phase = self.phase;

        match completed_phase {
            Dmg07ProtocolPhase::Ping { byte_index } => {
                self.complete_ping_byte(byte_index, outgoing_by_port);
            }
            Dmg07ProtocolPhase::TransmissionIndicator { byte_index } => {
                self.complete_transmission_indicator_byte(byte_index);
            }
            Dmg07ProtocolPhase::Transmission { byte_index } => {
                self.complete_transmission_byte(byte_index, outgoing_by_port);
            }
            Dmg07ProtocolPhase::PingRestartIndicator { byte_index } => {
                self.complete_ping_restart_indicator_byte(byte_index);
            }
        }
        self.delay_after_completed_byte_t_cycles =
            self.delay_after_completed_phase_t_cycles(completed_phase);

        self.trace_transition.take()
    }

    fn delay_after_completed_byte_t_cycles(&self) -> u64 {
        self.delay_after_completed_byte_t_cycles
    }

    fn delay_after_completed_phase_t_cycles(&self, completed_phase: Dmg07ProtocolPhase) -> u64 {
        match completed_phase {
            Dmg07ProtocolPhase::Ping { byte_index }
                if byte_index + 1 == DMG07_PING_PACKET_BYTES =>
            {
                self.ping_inter_packet_delay_t_cycles()
            }
            Dmg07ProtocolPhase::Ping { .. } => DMG07_PING_INTER_BYTE_DELAY_T_CYCLES,
            Dmg07ProtocolPhase::TransmissionIndicator { .. }
            | Dmg07ProtocolPhase::PingRestartIndicator { .. } => {
                self.transmission_inter_byte_delay_t_cycles()
            }
            Dmg07ProtocolPhase::Transmission { byte_index }
                if byte_index + 1 == self.packet_len() =>
            {
                self.transmission_packet_boundary_delay_t_cycles()
            }
            Dmg07ProtocolPhase::Transmission { .. } => {
                self.transmission_inter_byte_delay_t_cycles()
            }
        }
    }

    fn complete_ping_byte(&mut self, byte_index: usize, outgoing_by_port: [Option<u8>; 4]) {
        let p1_outgoing = outgoing_by_port[Dmg07Port::P1.index()];
        let transition_marker_packet = self.transition_marker_count > 0
            || (self.ping_response_index[Dmg07Port::P1.index()] == 0
                && p1_outgoing == Some(DMG07_TRANSMISSION_MARKER));

        if transition_marker_packet {
            self.track_ping_transition_marker(p1_outgoing);
        } else {
            self.consume_ping_response_byte(outgoing_by_port);
        }

        if byte_index + 1 == DMG07_PING_PACKET_BYTES {
            if self.transition_marker_count >= 3 {
                self.phase = Dmg07ProtocolPhase::TransmissionIndicator { byte_index: 0 };
                self.transition_marker_count = 0;
                self.trace_transition = Some(format!(
                    "dmg07 transition=transmission_indicator rate={} size={}",
                    self.rate, self.size
                ));
            } else {
                self.transition_marker_count = 0;
                self.phase = Dmg07ProtocolPhase::Ping { byte_index: 0 };
            }
        } else {
            self.phase = Dmg07ProtocolPhase::Ping {
                byte_index: byte_index + 1,
            };
        }
    }

    fn complete_transmission_indicator_byte(&mut self, byte_index: usize) {
        if byte_index + 1 == DMG07_INDICATOR_BYTES {
            self.phase = Dmg07ProtocolPhase::Transmission { byte_index: 0 };
            self.transmission_buffer.reset();
            self.trace_transition = Some("dmg07 transition=transmission".to_string());
        } else {
            self.phase = Dmg07ProtocolPhase::TransmissionIndicator {
                byte_index: byte_index + 1,
            };
        }
    }

    fn complete_transmission_byte(&mut self, byte_index: usize, outgoing_by_port: [Option<u8>; 4]) {
        let size = self.size as usize;
        self.transmission_buffer
            .capture_current_byte(size, self.connected, outgoing_by_port);

        self.track_restart_marker(byte_index, outgoing_by_port[Dmg07Port::P1.index()]);

        let packet_len = size * 4;
        self.transmission_buffer.advance(size);
        if byte_index + 1 == packet_len {
            if self.transmission_restart_requested {
                self.phase = Dmg07ProtocolPhase::PingRestartIndicator { byte_index: 0 };
                self.transmission_restart_requested = false;
                self.restart_marker_count = 0;
                self.trace_transition = Some("dmg07 transition=ping_restart_indicator".to_string());
            } else {
                self.restart_marker_count = 0;
                self.phase = Dmg07ProtocolPhase::Transmission { byte_index: 0 };
            }
        } else {
            self.phase = Dmg07ProtocolPhase::Transmission {
                byte_index: byte_index + 1,
            };
        }
    }

    fn complete_ping_restart_indicator_byte(&mut self, byte_index: usize) {
        if byte_index + 1 == DMG07_INDICATOR_BYTES {
            self.phase = Dmg07ProtocolPhase::Ping { byte_index: 0 };
            self.connected = [false; 4];
            self.ping_response_index = [0; 4];
            self.trace_transition = Some("dmg07 transition=ping".to_string());
        } else {
            self.phase = Dmg07ProtocolPhase::PingRestartIndicator {
                byte_index: byte_index + 1,
            };
        }
    }

    fn status_byte_for(&self, port: Dmg07Port) -> u8 {
        let mut status = port.id_byte();
        for candidate in Dmg07Port::ALL {
            if self.connected[candidate.index()] {
                status |= candidate.connection_bit();
            }
        }
        status
    }

    fn consume_ping_response_byte(&mut self, outgoing_by_port: [Option<u8>; 4]) {
        for port in Dmg07Port::ALL {
            let port_index = port.index();
            let outgoing = outgoing_by_port[port_index];
            match self.ping_response_index[port_index] {
                0 => {
                    if outgoing == Some(DMG07_ACK) {
                        self.ping_response_index[port_index] = 1;
                    } else {
                        self.connected[port_index] = false;
                    }
                }
                1 => {
                    if outgoing == Some(DMG07_ACK) {
                        self.connected[port_index] = true;
                        self.ping_response_index[port_index] = 2;
                    } else {
                        self.connected[port_index] = false;
                        self.ping_response_index[port_index] = 0;
                    }
                }
                2 => {
                    if port == Dmg07Port::P1
                        && let Some(rate) = outgoing
                    {
                        self.rate = rate;
                    }
                    self.ping_response_index[port_index] = 3;
                }
                3 => {
                    if port == Dmg07Port::P1
                        && let Some(size @ 1..=4) = outgoing
                    {
                        self.size = size;
                    }
                    self.ping_response_index[port_index] = 0;
                }
                _ => {
                    self.ping_response_index[port_index] = 0;
                }
            }
        }
    }

    fn packet_len(&self) -> usize {
        self.size as usize * 4
    }

    fn ping_inter_packet_delay_t_cycles(&self) -> u64 {
        DMG07_PING_BASE_INTER_PACKET_DELAY_T_CYCLES
            + u64::from(self.rate & 0x0F) * DMG07_RATE_LOW_NIBBLE_STEP_DELAY_T_CYCLES
    }

    fn transmission_inter_byte_delay_t_cycles(&self) -> u64 {
        DMG07_TRANSMISSION_BASE_INTER_BYTE_DELAY_T_CYCLES
            + u64::from(self.rate >> 4) * DMG07_TRANSMISSION_RATE_STEP_DELAY_T_CYCLES
    }

    fn transmission_packet_boundary_delay_t_cycles(&self) -> u64 {
        let packet_len = self.packet_len() as u64;
        let byte_transfer_span_t_cycles = 7 * DMG07_SERIAL_BIT_PERIOD_T_CYCLES;
        let inter_byte_delay_t_cycles = self.transmission_inter_byte_delay_t_cycles();
        let elapsed_before_boundary_delay = packet_len * byte_transfer_span_t_cycles
            + packet_len.saturating_sub(1) * inter_byte_delay_t_cycles;
        let minimum_packet_period_t_cycles = DMG07_TRANSMISSION_BASE_PACKET_PERIOD_T_CYCLES
            + u64::from(self.rate & 0x0F) * DMG07_RATE_LOW_NIBBLE_STEP_DELAY_T_CYCLES;

        inter_byte_delay_t_cycles
            .max(minimum_packet_period_t_cycles.saturating_sub(elapsed_before_boundary_delay))
    }

    fn track_ping_transition_marker(&mut self, p1_outgoing: Option<u8>) {
        if p1_outgoing == Some(DMG07_TRANSMISSION_MARKER) {
            self.transition_marker_count = self.transition_marker_count.saturating_add(1);
        } else if self.transition_marker_count < 3 {
            self.transition_marker_count = 0;
        }
    }

    fn track_restart_marker(&mut self, byte_index: usize, p1_outgoing: Option<u8>) {
        if byte_index == 0 {
            self.restart_marker_count = 0;
        }

        if p1_outgoing == Some(DMG07_RESTART_MARKER) {
            self.restart_marker_count = self.restart_marker_count.saturating_add(1);
            if self.restart_marker_count >= 3 {
                self.transmission_restart_requested = true;
            }
        } else if self.restart_marker_count < 3 {
            self.restart_marker_count = 0;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Dmg07TransmissionBuffer {
    slots: [[u8; DMG07_MAX_SIZE]; 8],
    position: usize,
}

impl Dmg07TransmissionBuffer {
    const fn new() -> Self {
        Self {
            slots: [[0; DMG07_MAX_SIZE]; 8],
            position: 0,
        }
    }

    fn reset(&mut self) {
        self.slots = [[0; DMG07_MAX_SIZE]; 8];
        self.position = 0;
    }

    fn current_byte(&self, size: usize) -> u8 {
        let ring_position = self.ring_position(size);
        let slot_index = ring_position / size;
        let byte_offset = ring_position % size;
        self.slots[slot_index][byte_offset]
    }

    fn capture_current_byte(
        &mut self,
        size: usize,
        connected: [bool; 4],
        outgoing_by_port: [Option<u8>; 4],
    ) {
        let packet_position = self.packet_position(size);
        if packet_position == 0 || packet_position > size {
            return;
        }

        let packet_len = packet_len_for_size(size);
        let ring_len = ring_len_for_size(size);
        let target_position = (self.position - 1 + packet_len) % ring_len;
        let target_slot_base = target_position / size;
        let target_offset = target_position % size;

        for port in Dmg07Port::ALL {
            self.slots[target_slot_base + port.index()][target_offset] = if connected[port.index()]
            {
                outgoing_by_port[port.index()].unwrap_or(0)
            } else {
                0
            };
        }
    }

    fn advance(&mut self, size: usize) {
        self.position = (self.position + 1) % ring_len_for_size(size);
    }

    fn packet_position(&self, size: usize) -> usize {
        self.position % packet_len_for_size(size)
    }

    fn ring_position(&self, size: usize) -> usize {
        self.position % ring_len_for_size(size)
    }
}

fn packet_len_for_size(size: usize) -> usize {
    size * 4
}

fn ring_len_for_size(size: usize) -> usize {
    packet_len_for_size(size) * 2
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Dmg07ProtocolPhase {
    Ping { byte_index: usize },
    TransmissionIndicator { byte_index: usize },
    Transmission { byte_index: usize },
    PingRestartIndicator { byte_index: usize },
}

pub(crate) fn validate_dmg07_participants(
    participants: &[Dmg07Participant],
    machine_count: usize,
) -> Result<(), crate::link::LinkedMachinesError> {
    if !(2..=4).contains(&participants.len()) {
        return Err(
            crate::link::LinkedMachinesError::UnsupportedMachineCountForDmg07 {
                count: participants.len(),
            },
        );
    }

    let mut seen_ports = BTreeSet::new();
    let mut seen_machine_indexes = BTreeSet::new();
    let mut has_p1 = false;
    for participant in participants {
        if participant.machine_index >= machine_count {
            return Err(
                crate::link::LinkedMachinesError::Dmg07MachineIndexOutOfBounds {
                    machine_index: participant.machine_index,
                    machine_count,
                },
            );
        }
        if !seen_machine_indexes.insert(participant.machine_index) {
            return Err(
                crate::link::LinkedMachinesError::DuplicateDmg07MachineIndex {
                    machine_index: participant.machine_index,
                },
            );
        }
        if !seen_ports.insert(participant.port) {
            return Err(crate::link::LinkedMachinesError::DuplicateDmg07Port {
                port: participant.port,
            });
        }
        has_p1 |= participant.port == Dmg07Port::P1;
    }

    if !has_p1 {
        return Err(crate::link::LinkedMachinesError::MissingDmg07PlayerOne);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_ack_updates_status_mid_packet() {
        let mut protocol = Dmg07Protocol::new();

        assert_eq!(protocol.incoming_byte_for(Dmg07Port::P1), DMG07_PING_HEADER);
        protocol.complete_byte([Some(DMG07_ACK), None, None, None]);
        assert_eq!(protocol.incoming_byte_for(Dmg07Port::P1), 0x01);
        protocol.complete_byte([Some(DMG07_ACK), None, None, None]);

        assert_eq!(protocol.incoming_byte_for(Dmg07Port::P1), 0x11);
        assert_eq!(protocol.incoming_byte_for(Dmg07Port::P4), 0x14);
    }

    #[test]
    fn ping_replies_configure_rate_and_size() {
        let mut protocol = Dmg07Protocol::new();

        protocol.complete_byte([Some(DMG07_ACK), None, None, None]);
        protocol.complete_byte([Some(DMG07_ACK), None, None, None]);
        protocol.complete_byte([Some(0x20), None, None, None]);
        protocol.complete_byte([Some(0x03), None, None, None]);

        assert_eq!(protocol.rate, 0x20);
        assert_eq!(protocol.size, 3);
    }

    #[test]
    fn adapter_byte_engine_uses_clustered_serial_clock_period() {
        let mut engine = Dmg07ByteEngine::new();

        assert!(!engine.should_queue_bit(TCycle::new(DMG07_INITIAL_BYTE_DELAY_T_CYCLES - 1)));
        assert!(engine.should_queue_bit(TCycle::new(DMG07_INITIAL_BYTE_DELAY_T_CYCLES)));

        let first_bit = TCycle::new(DMG07_INITIAL_BYTE_DELAY_T_CYCLES);
        engine.record_queued_bit(first_bit);

        assert!(!engine.should_queue_bit(TCycle::new(
            DMG07_INITIAL_BYTE_DELAY_T_CYCLES + DMG07_SERIAL_BIT_PERIOD_T_CYCLES - 1
        )));
        assert!(engine.should_queue_bit(TCycle::new(
            DMG07_INITIAL_BYTE_DELAY_T_CYCLES + DMG07_SERIAL_BIT_PERIOD_T_CYCLES
        )));
    }

    #[test]
    fn rate_configures_ping_and_transmission_packet_timing() {
        let mut protocol = Dmg07Protocol::new();

        protocol.complete_byte([Some(DMG07_ACK), None, None, None]);
        protocol.complete_byte([Some(DMG07_ACK), None, None, None]);
        protocol.complete_byte([Some(0x28), None, None, None]);
        protocol.complete_byte([Some(0x04), None, None, None]);
        assert_eq!(
            protocol.delay_after_completed_byte_t_cycles(),
            DMG07_PING_BASE_INTER_PACKET_DELAY_T_CYCLES
                + 8 * DMG07_RATE_LOW_NIBBLE_STEP_DELAY_T_CYCLES
        );

        protocol.phase = Dmg07ProtocolPhase::Transmission { byte_index: 15 };
        protocol.rate = 0x28;
        protocol.size = 4;
        let inter_byte_delay = protocol.transmission_inter_byte_delay_t_cycles();
        assert_eq!(
            inter_byte_delay,
            DMG07_TRANSMISSION_BASE_INTER_BYTE_DELAY_T_CYCLES
                + 2 * DMG07_TRANSMISSION_RATE_STEP_DELAY_T_CYCLES
        );

        let packet_len = protocol.packet_len() as u64;
        let elapsed_before_boundary_delay = packet_len * 7 * DMG07_SERIAL_BIT_PERIOD_T_CYCLES
            + packet_len.saturating_sub(1) * inter_byte_delay;
        let packet_boundary_delay = protocol.transmission_packet_boundary_delay_t_cycles();
        assert_eq!(
            elapsed_before_boundary_delay + packet_boundary_delay,
            DMG07_TRANSMISSION_BASE_PACKET_PERIOD_T_CYCLES
                + 8 * DMG07_RATE_LOW_NIBBLE_STEP_DELAY_T_CYCLES
        );
    }

    #[test]
    fn ping_accepts_three_aa_bytes_plus_filler_as_transmission_request() {
        let mut protocol = Dmg07Protocol::new();

        protocol.complete_byte([Some(DMG07_TRANSMISSION_MARKER), None, None, None]);
        protocol.complete_byte([Some(DMG07_TRANSMISSION_MARKER), None, None, None]);
        protocol.complete_byte([Some(DMG07_TRANSMISSION_MARKER), None, None, None]);
        let trace = protocol.complete_byte([Some(0x00), None, None, None]);

        assert!(matches!(
            protocol.phase,
            Dmg07ProtocolPhase::TransmissionIndicator { byte_index: 0 }
        ));
        assert!(
            trace
                .expect("transition trace")
                .contains("transmission_indicator")
        );
    }

    #[test]
    fn transmission_request_preserves_ping_configuration_and_connections() {
        let mut protocol = Dmg07Protocol::new();

        protocol.complete_byte([Some(DMG07_ACK), Some(DMG07_ACK), None, Some(DMG07_ACK)]);
        protocol.complete_byte([Some(DMG07_ACK), Some(DMG07_ACK), None, Some(DMG07_ACK)]);
        protocol.complete_byte([Some(0x28), Some(0x00), None, Some(0x00)]);
        protocol.complete_byte([Some(0x04), Some(0x00), None, Some(0x00)]);

        assert_eq!(protocol.connected, [true, true, false, true]);
        assert_eq!(protocol.rate, 0x28);
        assert_eq!(protocol.size, 4);

        protocol.complete_byte([Some(DMG07_TRANSMISSION_MARKER), None, None, None]);
        protocol.complete_byte([Some(DMG07_TRANSMISSION_MARKER), None, None, None]);
        protocol.complete_byte([Some(DMG07_TRANSMISSION_MARKER), None, None, None]);
        let trace = protocol.complete_byte([Some(0x00), None, None, None]);

        assert!(matches!(
            protocol.phase,
            Dmg07ProtocolPhase::TransmissionIndicator { byte_index: 0 }
        ));
        assert_eq!(protocol.connected, [true, true, false, true]);
        assert_eq!(protocol.rate, 0x28);
        assert_eq!(protocol.size, 4);
        assert!(
            trace
                .expect("transition trace")
                .contains("transmission_indicator rate=40 size=4")
        );
    }

    #[test]
    fn transmission_uses_size_window_and_preserves_sparse_port_slots() {
        let mut protocol = Dmg07Protocol::new();
        protocol.phase = Dmg07ProtocolPhase::Transmission { byte_index: 0 };
        protocol.connected = [true, false, false, true];
        protocol.size = 2;

        protocol.complete_byte([Some(0xAA), Some(0xBB), Some(0xCC), Some(0xDD)]);
        assert_eq!(protocol.incoming_byte_for(Dmg07Port::P1), 0x00);

        protocol.complete_byte([Some(0xA2), Some(0xB2), Some(0xC2), Some(0xD2)]);
        protocol.complete_byte([Some(0xA3), Some(0xB3), Some(0xC3), Some(0xD3)]);
        for filler_index in 3..8 {
            protocol.complete_byte([Some(filler_index as u8), Some(0xEE), Some(0xEE), Some(0xEE)]);
        }

        for expected_byte in [0xA2, 0xA3, 0x00, 0x00, 0x00, 0x00, 0xD2, 0xD3] {
            assert_eq!(protocol.incoming_byte_for(Dmg07Port::P1), expected_byte);
            protocol.complete_byte([Some(0), Some(0), Some(0), Some(0)]);
        }
    }

    #[test]
    fn transmission_broadcasts_buffered_slots_in_physical_port_order() {
        let mut protocol = Dmg07Protocol::new();
        protocol.phase = Dmg07ProtocolPhase::Transmission { byte_index: 0 };
        protocol.connected = [true, true, false, false];
        protocol.size = 4;

        for byte_index in 0..16 {
            let p1 = if (1..=4).contains(&byte_index) {
                0xA0 + (byte_index - 1) as u8
            } else {
                0xEE
            };
            let p2 = if (1..=4).contains(&byte_index) {
                0xB0 + (byte_index - 1) as u8
            } else {
                0xEE
            };
            protocol.complete_byte([Some(p1), Some(p2), Some(0xEE), Some(0xEE)]);
        }

        let expected = [
            0xA0, 0xA1, 0xA2, 0xA3, 0xB0, 0xB1, 0xB2, 0xB3, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];

        for (byte_index, expected_byte) in expected.iter().copied().enumerate() {
            assert_eq!(protocol.incoming_byte_for(Dmg07Port::P1), expected_byte);
            assert_eq!(protocol.incoming_byte_for(Dmg07Port::P2), expected_byte);
            protocol.complete_byte([Some(0), Some(0), Some(0), Some(0)]);
            if byte_index < expected.len() - 1 {
                assert!(matches!(
                    protocol.phase,
                    Dmg07ProtocolPhase::Transmission {
                        byte_index: next_index
                    } if next_index == byte_index + 1
                ));
            }
        }
    }

    #[test]
    fn restart_requires_three_aligned_ff_bytes() {
        let mut protocol = Dmg07Protocol::new();
        protocol.phase = Dmg07ProtocolPhase::Transmission { byte_index: 0 };
        protocol.size = 1;

        protocol.complete_byte([Some(DMG07_RESTART_MARKER), None, None, None]);
        protocol.complete_byte([Some(DMG07_RESTART_MARKER), None, None, None]);
        protocol.complete_byte([Some(DMG07_RESTART_MARKER), None, None, None]);
        let trace = protocol.complete_byte([Some(0x00), None, None, None]);

        assert!(matches!(
            protocol.phase,
            Dmg07ProtocolPhase::PingRestartIndicator { byte_index: 0 }
        ));
        assert!(
            trace
                .expect("transition trace")
                .contains("ping_restart_indicator")
        );
    }
}
