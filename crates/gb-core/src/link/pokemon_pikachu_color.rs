use std::collections::VecDeque;

use crate::debugger::{TraceBuffer, TraceSink};
use crate::machine::{Machine, MachineStepObserver, NoopMachineStepObserver};
use crate::scheduler::{CycleContext, GlobalScheduler, SchedulerPhase, TCycle};

const REGION_PREFIX: u8 = 0x96;
const MESSAGE_PREFIX: u8 = 0x5A;
const ACK_OKAY: u8 = 0x6C;
const POKEMON_PIKACHU_2_VERSION: u8 = 0x03;
const WESTERN_PIKACHU_NAME: [u8; 11] = [
    0x8F, 0x88, 0x8A, 0x80, 0x82, 0x87, 0x94, 0x50, 0x50, 0x50, 0x50,
];
const POKEMON_PIKACHU_COLOR_PAYLOAD_LEN: usize = 20;
const MAX_RECEIVED_BLOCK_LEN: usize = 80;

const FAST_INTERRUPT_T_CYCLES: u32 = 128;
const IR_HELLO_MARK_T_CYCLES: u32 = 4 * FAST_INTERRUPT_T_CYCLES;
const IR_HELLO_LONG_SPACE_T_CYCLES: u32 = 60 * FAST_INTERRUPT_T_CYCLES;
const IR_HELLO_MID_SPACE_T_CYCLES: u32 = 20 * FAST_INTERRUPT_T_CYCLES;
const IR_HELLO_SHORT_SPACE_T_CYCLES: u32 = 4 * FAST_INTERRUPT_T_CYCLES;
const IR_MESSAGE_PREAMBLE_MARK_T_CYCLES: u32 = 4 * FAST_INTERRUPT_T_CYCLES;
const IR_MESSAGE_PREAMBLE_SPACE_T_CYCLES: u32 = 20 * FAST_INTERRUPT_T_CYCLES;
const IR_MESSAGE_BIT_MARK_T_CYCLES: u32 = 2 * FAST_INTERRUPT_T_CYCLES;
const IR_MESSAGE_ZERO_SPACE_T_CYCLES: u32 = 4 * FAST_INTERRUPT_T_CYCLES;
const IR_MESSAGE_ONE_SPACE_T_CYCLES: u32 = 10 * FAST_INTERRUPT_T_CYCLES;
const IR_MESSAGE_TRAILER_MARK_T_CYCLES: u32 = 4 * FAST_INTERRUPT_T_CYCLES;
const IR_MESSAGE_TRAILER_SPACE_T_CYCLES: u32 = 16 * FAST_INTERRUPT_T_CYCLES;
const IR_BLOCK_LEAD_SPACE_T_CYCLES: u32 = IR_HELLO_LONG_SPACE_T_CYCLES;
const IR_SPACE_ZERO_ONE_THRESHOLD_T_CYCLES: u32 = 9 * FAST_INTERRUPT_T_CYCLES;
const IR_MIN_PREAMBLE_SPACE_T_CYCLES: u32 = 12 * FAST_INTERRUPT_T_CYCLES;
const IR_HELLO_RESPONSE_TIMEOUT_T_CYCLES: u32 = 512 * FAST_INTERRUPT_T_CYCLES;
const IR_RECEIVE_TIMEOUT_T_CYCLES: u32 = 4 * 4_194_304;
const IR_RETRY_DELAY_T_CYCLES: u32 = 512 * FAST_INTERRUPT_T_CYCLES;
const IR_COMPLETED_RESTART_DELAY_T_CYCLES: u32 = 4 * IR_RETRY_DELAY_T_CYCLES;
const ACCESSORY_TO_CGB_OPTICAL_DELAY_T_CYCLES: usize = 80;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PokemonPikachuColorGift {
    #[default]
    Watts1,
    Watts100,
    Watts200,
    Watts300,
    Watts400,
    Watts500,
    Watts600,
    Watts700,
    Watts800,
    Watts900,
    Watts999,
}

impl PokemonPikachuColorGift {
    pub const ALL: [Self; 11] = [
        Self::Watts1,
        Self::Watts100,
        Self::Watts200,
        Self::Watts300,
        Self::Watts400,
        Self::Watts500,
        Self::Watts600,
        Self::Watts700,
        Self::Watts800,
        Self::Watts900,
        Self::Watts999,
    ];

    pub const fn item_byte(self) -> u8 {
        match self {
            Self::Watts1 => 0x0D,
            Self::Watts100 => 0x00,
            Self::Watts200 => 0x09,
            Self::Watts300 => 0x13,
            Self::Watts400 => 0x15,
            Self::Watts500 => 0x17,
            Self::Watts600 => 0x10,
            Self::Watts700 => 0x11,
            Self::Watts800 => 0x16,
            Self::Watts900 => 0x12,
            Self::Watts999 => 0x22,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Watts1 => "1W",
            Self::Watts100 => "100W",
            Self::Watts200 => "200W",
            Self::Watts300 => "300W",
            Self::Watts400 => "400W",
            Self::Watts500 => "500W",
            Self::Watts600 => "600W",
            Self::Watts700 => "700W",
            Self::Watts800 => "800W",
            Self::Watts900 => "900W",
            Self::Watts999 => "999W",
        }
    }

    pub fn next(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|gift| *gift == self)
            .expect("gift should be in ALL");
        Self::ALL[(index + 1) % Self::ALL.len()]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PokemonPikachuColorRegion {
    #[default]
    Auto,
    Usa,
    Esp,
    Ita,
    Fra,
    Ger,
}

impl PokemonPikachuColorRegion {
    pub const fn code(self) -> Option<u8> {
        match self {
            Self::Auto => None,
            Self::Usa => Some(0x90),
            Self::Esp => Some(0x96),
            Self::Ita => Some(0x99),
            Self::Fra => Some(0x9A),
            Self::Ger => Some(0x9F),
        }
    }

    const fn resolve(self, observed_game_region: u8) -> Option<u8> {
        match self {
            Self::Auto if is_supported_western_region(observed_game_region) => {
                Some(observed_game_region)
            }
            Self::Auto => None,
            fixed => fixed.code(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PokemonPikachuColorStatus {
    pub gift: PokemonPikachuColorGift,
    pub region: PokemonPikachuColorRegion,
    pub resolved_region_code: Option<u8>,
    pub emitter_on: bool,
    pub game_emitter_on: bool,
    pub game_emitter_seen: bool,
    pub completed_exchange: bool,
    pub failed_exchange: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PokemonPikachuColor {
    gift: PokemonPikachuColorGift,
    region: PokemonPikachuColorRegion,
    resolved_region_code: Option<u8>,
    actions: VecDeque<ProtocolAction>,
    phase: ProtocolPhase,
    pulses: PulseQueue,
    output_active: bool,
    previous_game_emitter_on: bool,
    wait_t_cycles: u32,
    retry_delay_t_cycles: u32,
    game_emitter_seen: bool,
    completed_exchange: bool,
    failed_exchange: bool,
}

impl PokemonPikachuColor {
    pub fn new(gift: PokemonPikachuColorGift, region: PokemonPikachuColorRegion) -> Self {
        let mut accessory = Self {
            gift,
            region,
            resolved_region_code: region.code(),
            actions: VecDeque::new(),
            phase: ProtocolPhase::Idle,
            pulses: PulseQueue::new(),
            output_active: false,
            previous_game_emitter_on: false,
            wait_t_cycles: 0,
            retry_delay_t_cycles: 0,
            game_emitter_seen: false,
            completed_exchange: false,
            failed_exchange: false,
        };
        accessory.restart_exchange();
        accessory
    }

    pub const fn gift(&self) -> PokemonPikachuColorGift {
        self.gift
    }

    pub fn set_gift(&mut self, gift: PokemonPikachuColorGift) {
        if self.gift == gift {
            return;
        }
        self.gift = gift;
        self.restart_exchange();
    }

    pub const fn region(&self) -> PokemonPikachuColorRegion {
        self.region
    }

    pub fn set_region(&mut self, region: PokemonPikachuColorRegion) {
        if self.region == region {
            return;
        }
        self.region = region;
        self.restart_exchange();
    }

    pub const fn status(&self) -> PokemonPikachuColorStatus {
        PokemonPikachuColorStatus {
            gift: self.gift,
            region: self.region,
            resolved_region_code: self.resolved_region_code,
            emitter_on: self.output_active,
            game_emitter_on: self.previous_game_emitter_on,
            game_emitter_seen: self.game_emitter_seen,
            completed_exchange: self.completed_exchange,
            failed_exchange: self.failed_exchange,
        }
    }

    pub fn tick_t_cycle(&mut self, game_emitter_on: bool) -> bool {
        self.game_emitter_seen |= game_emitter_on;
        self.observe_game_emitter(game_emitter_on);
        self.drive_protocol();
        self.output_active = self.pulses.tick_t_cycle();
        self.output_active
    }

    fn observe_game_emitter(&mut self, game_emitter_on: bool) {
        if self.previous_game_emitter_on == game_emitter_on {
            match &mut self.phase {
                ProtocolPhase::ReceivingHello { receiver, .. } => {
                    receiver.tick_same_level();
                }
                ProtocolPhase::WaitingHelloResponse { receiver } => {
                    receiver.tick_same_level();
                }
                ProtocolPhase::ReceivingBlock { receiver, .. } => {
                    receiver.tick_same_level();
                }
                ProtocolPhase::WaitingBlockAck { receiver } => {
                    receiver.tick_same_level();
                }
                ProtocolPhase::Idle
                | ProtocolPhase::SendingAction
                | ProtocolPhase::RespondingToHello
                | ProtocolPhase::SendingBlock
                | ProtocolPhase::SendingBlockAck
                | ProtocolPhase::Completed
                | ProtocolPhase::Failed => {}
            }
            return;
        }

        let completed = match &mut self.phase {
            ProtocolPhase::ReceivingHello { receiver, .. } => {
                receiver.observe_transition(self.previous_game_emitter_on)
            }
            ProtocolPhase::WaitingHelloResponse { receiver } => {
                receiver.observe_transition(self.previous_game_emitter_on)
            }
            ProtocolPhase::ReceivingBlock { receiver, .. } => {
                receiver.observe_transition(self.previous_game_emitter_on)
            }
            ProtocolPhase::WaitingBlockAck { receiver } => {
                receiver.observe_transition(self.previous_game_emitter_on)
            }
            ProtocolPhase::Idle
            | ProtocolPhase::SendingAction
            | ProtocolPhase::RespondingToHello
            | ProtocolPhase::SendingBlock
            | ProtocolPhase::SendingBlockAck
            | ProtocolPhase::Completed
            | ProtocolPhase::Failed => false,
        };

        self.previous_game_emitter_on = game_emitter_on;
        if completed {
            self.finish_receiver_phase();
        }
    }

    fn drive_protocol(&mut self) {
        if matches!(self.phase, ProtocolPhase::Completed) {
            if self.retry_delay_t_cycles == 0 {
                self.restart_exchange();
            } else {
                self.retry_delay_t_cycles = self.retry_delay_t_cycles.saturating_sub(1);
            }
            return;
        }
        if matches!(self.phase, ProtocolPhase::Failed) {
            if self.retry_delay_t_cycles == 0 {
                self.restart_exchange();
            } else {
                self.retry_delay_t_cycles = self.retry_delay_t_cycles.saturating_sub(1);
            }
            return;
        }

        if self.wait_t_cycles != 0 {
            self.wait_t_cycles = self.wait_t_cycles.saturating_sub(1);
        } else if self.receiver_waiting() {
            self.fail_and_retry();
            return;
        }

        match self.phase {
            ProtocolPhase::Idle => self.start_next_action(),
            ProtocolPhase::SendingAction
            | ProtocolPhase::RespondingToHello
            | ProtocolPhase::SendingBlock
            | ProtocolPhase::SendingBlockAck => {
                if self.pulses.is_empty() {
                    self.finish_sending_phase();
                }
            }
            ProtocolPhase::ReceivingHello { .. }
            | ProtocolPhase::WaitingHelloResponse { .. }
            | ProtocolPhase::ReceivingBlock { .. }
            | ProtocolPhase::WaitingBlockAck { .. }
            | ProtocolPhase::Completed
            | ProtocolPhase::Failed => {}
        }
    }

    fn receiver_waiting(&self) -> bool {
        matches!(
            self.phase,
            ProtocolPhase::ReceivingHello { .. }
                | ProtocolPhase::WaitingHelloResponse { .. }
                | ProtocolPhase::ReceivingBlock { .. }
                | ProtocolPhase::WaitingBlockAck { .. }
        )
    }

    fn finish_receiver_phase(&mut self) {
        let phase = std::mem::replace(&mut self.phase, ProtocolPhase::Idle);
        match phase {
            ProtocolPhase::ReceivingHello { next, .. } => {
                encode_hello(&mut self.pulses);
                self.phase = ProtocolPhase::RespondingToHello;
                self.actions.push_front(next);
            }
            ProtocolPhase::WaitingHelloResponse { .. } => self.start_next_action(),
            ProtocolPhase::ReceivingBlock {
                expectation,
                receiver,
                ..
            } => match receiver.into_result() {
                Some(bytes) => {
                    if !self.handle_received_block(expectation, &bytes) {
                        self.fail_and_retry();
                        return;
                    }
                    encode_block_ack(&mut self.pulses, ACK_OKAY);
                    self.phase = ProtocolPhase::SendingBlockAck;
                }
                None => self.fail_and_retry(),
            },
            ProtocolPhase::WaitingBlockAck { receiver } => match receiver.into_result() {
                Some(bytes) if bytes == [ACK_OKAY] => self.start_next_action(),
                _ => self.fail_and_retry(),
            },
            ProtocolPhase::Idle
            | ProtocolPhase::SendingAction
            | ProtocolPhase::RespondingToHello
            | ProtocolPhase::SendingBlock
            | ProtocolPhase::SendingBlockAck
            | ProtocolPhase::Completed
            | ProtocolPhase::Failed => {}
        }
    }

    fn finish_sending_phase(&mut self) {
        let phase = std::mem::replace(&mut self.phase, ProtocolPhase::Idle);
        match phase {
            ProtocolPhase::SendingAction => {
                self.phase = ProtocolPhase::WaitingHelloResponse {
                    receiver: HelloReceiver::new(),
                };
                self.wait_t_cycles = IR_HELLO_RESPONSE_TIMEOUT_T_CYCLES;
            }
            ProtocolPhase::RespondingToHello | ProtocolPhase::SendingBlockAck => {
                self.start_next_action();
            }
            ProtocolPhase::SendingBlock => {
                self.phase = ProtocolPhase::WaitingBlockAck {
                    receiver: DataMessageReceiver::new(1),
                };
                self.wait_t_cycles = IR_RECEIVE_TIMEOUT_T_CYCLES;
            }
            ProtocolPhase::Idle
            | ProtocolPhase::ReceivingHello { .. }
            | ProtocolPhase::WaitingHelloResponse { .. }
            | ProtocolPhase::ReceivingBlock { .. }
            | ProtocolPhase::WaitingBlockAck { .. }
            | ProtocolPhase::Completed
            | ProtocolPhase::Failed => {}
        }
    }

    fn start_next_action(&mut self) {
        self.wait_t_cycles = 0;
        match self.actions.pop_front().unwrap_or(ProtocolAction::Finish) {
            ProtocolAction::Noop => self.start_next_action(),
            ProtocolAction::SendHello => {
                encode_hello(&mut self.pulses);
                self.phase = ProtocolPhase::SendingAction;
            }
            ProtocolAction::ReceiveHello => {
                self.phase = ProtocolPhase::ReceivingHello {
                    receiver: HelloReceiver::new(),
                    next: ProtocolAction::Noop,
                };
                self.wait_t_cycles = IR_RECEIVE_TIMEOUT_T_CYCLES;
            }
            ProtocolAction::SendBlock(source) => {
                let Some(bytes) = self.block_source_bytes(source) else {
                    self.fail_and_retry();
                    return;
                };
                encode_data_block(&mut self.pulses, &bytes);
                self.phase = ProtocolPhase::SendingBlock;
            }
            ProtocolAction::ReceiveBlock(expectation) => {
                self.phase = ProtocolPhase::ReceivingBlock {
                    receiver: DataBlockReceiver::new(),
                    expectation,
                };
                self.wait_t_cycles = IR_RECEIVE_TIMEOUT_T_CYCLES;
            }
            ProtocolAction::Finish => {
                self.completed_exchange = true;
                self.phase = ProtocolPhase::Completed;
                self.retry_delay_t_cycles = IR_COMPLETED_RESTART_DELAY_T_CYCLES;
            }
        }
    }

    fn handle_received_block(&mut self, expectation: ReceiveExpectation, bytes: &[u8]) -> bool {
        match expectation {
            ReceiveExpectation::RegionCode => {
                let [region_code] = bytes else {
                    return false;
                };
                let Some(resolved_region_code) = self.region.resolve(*region_code) else {
                    return false;
                };
                self.resolved_region_code = Some(resolved_region_code);
                true
            }
            ReceiveExpectation::RegionPrefix => bytes == [REGION_PREFIX],
            ReceiveExpectation::Empty => bytes.is_empty(),
            ReceiveExpectation::GamePayload => !bytes.is_empty(),
        }
    }

    fn block_source_bytes(&self, source: BlockSource) -> Option<Vec<u8>> {
        match source {
            BlockSource::RegionPrefix => Some(vec![REGION_PREFIX]),
            BlockSource::ResolvedRegionCode => self.resolved_region_code.map(|code| vec![code]),
            BlockSource::PokemonPayload => Some(self.payload().to_vec()),
            BlockSource::Empty => Some(Vec::new()),
        }
    }

    fn payload(&self) -> [u8; POKEMON_PIKACHU_COLOR_PAYLOAD_LEN] {
        pokemon_pikachu_color_payload(self.gift)
    }

    fn restart_exchange(&mut self) {
        self.resolved_region_code = self.region.code();
        self.actions = protocol_actions();
        self.phase = ProtocolPhase::Idle;
        self.pulses.clear();
        self.output_active = false;
        self.wait_t_cycles = 0;
        self.retry_delay_t_cycles = 0;
        self.game_emitter_seen = false;
        self.completed_exchange = false;
        self.failed_exchange = false;
    }

    fn fail_and_retry(&mut self) {
        self.phase = ProtocolPhase::Failed;
        self.pulses.clear();
        self.output_active = false;
        self.wait_t_cycles = 0;
        self.retry_delay_t_cycles = IR_RETRY_DELAY_T_CYCLES;
        self.failed_exchange = true;
    }
}

impl Default for PokemonPikachuColor {
    fn default() -> Self {
        Self::new(
            PokemonPikachuColorGift::default(),
            PokemonPikachuColorRegion::default(),
        )
    }
}

#[derive(Debug, Clone)]
pub struct PokemonPikachuColorSession<S = TraceBuffer> {
    scheduler: GlobalScheduler,
    machine: Machine<S>,
    accessory: PokemonPikachuColor,
    context: CycleContext,
    accessory_to_cgb_delay_line: [bool; ACCESSORY_TO_CGB_OPTICAL_DELAY_T_CYCLES],
    delay_cursor: usize,
}

impl<S: TraceSink> PokemonPikachuColorSession<S> {
    pub fn new(machine: Machine<S>, accessory: PokemonPikachuColor) -> Self {
        let next_t_cycle = machine.next_t_cycle();
        let mut scheduler = GlobalScheduler::new();
        scheduler.set_next_t_cycle(next_t_cycle);
        let mut session = Self {
            scheduler,
            machine,
            accessory,
            context: CycleContext::for_cycle(next_t_cycle),
            accessory_to_cgb_delay_line: [false; ACCESSORY_TO_CGB_OPTICAL_DELAY_T_CYCLES],
            delay_cursor: 0,
        };
        session.machine.set_cgb_infrared_external_input(false);
        session
    }

    pub fn next_t_cycle(&self) -> TCycle {
        self.scheduler.next_t_cycle()
    }

    pub fn machine(&self) -> &Machine<S> {
        &self.machine
    }

    pub fn machine_mut(&mut self) -> &mut Machine<S> {
        &mut self.machine
    }

    pub fn pokemon_pikachu_color(&self) -> &PokemonPikachuColor {
        &self.accessory
    }

    pub fn pokemon_pikachu_color_mut(&mut self) -> &mut PokemonPikachuColor {
        &mut self.accessory
    }

    pub fn into_machine(mut self) -> Machine<S> {
        self.machine.set_cgb_infrared_external_input(false);
        self.machine
    }

    pub fn step_t_cycle(&mut self) -> CycleContext {
        self.advance_t_cycle_with_observer(&mut NoopMachineStepObserver)
    }

    pub fn advance_t_cycle(&mut self) {
        self.advance_t_cycle_with_observer(&mut NoopMachineStepObserver);
    }

    pub fn advance_t_cycle_with_observer<O: MachineStepObserver>(
        &mut self,
        observer: &mut O,
    ) -> CycleContext {
        let t_cycle = self.scheduler.next_t_cycle();
        self.context.reset_for_cycle(t_cycle);

        for &phase in SchedulerPhase::all() {
            self.context.enter_phase(phase);
            if phase == SchedulerPhase::ExternalEventIngress {
                let game_emitter_on = self.machine.cgb_infrared_emitter_on();
                let accessory_output = self.accessory.tick_t_cycle(game_emitter_on);
                let delayed_output = self.accessory_to_cgb_delay_line[self.delay_cursor];
                self.accessory_to_cgb_delay_line[self.delay_cursor] = accessory_output;
                self.delay_cursor =
                    (self.delay_cursor + 1) % ACCESSORY_TO_CGB_OPTICAL_DELAY_T_CYCLES;
                self.machine.set_cgb_infrared_external_input(delayed_output);
            }
            self.machine
                .step_phase_with_context(&mut self.context, observer);
        }

        let next_t_cycle = t_cycle.next();
        self.scheduler.set_next_t_cycle(next_t_cycle);
        self.machine.sync_scheduler_next_t_cycle(next_t_cycle);
        self.context.clone()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProtocolAction {
    Noop,
    SendHello,
    ReceiveHello,
    SendBlock(BlockSource),
    ReceiveBlock(ReceiveExpectation),
    Finish,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockSource {
    RegionPrefix,
    ResolvedRegionCode,
    PokemonPayload,
    Empty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReceiveExpectation {
    RegionCode,
    RegionPrefix,
    Empty,
    GamePayload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ProtocolPhase {
    Idle,
    SendingAction,
    ReceivingHello {
        receiver: HelloReceiver,
        next: ProtocolAction,
    },
    WaitingHelloResponse {
        receiver: HelloReceiver,
    },
    RespondingToHello,
    SendingBlock,
    WaitingBlockAck {
        receiver: DataMessageReceiver,
    },
    ReceivingBlock {
        receiver: DataBlockReceiver,
        expectation: ReceiveExpectation,
    },
    SendingBlockAck,
    Completed,
    Failed,
}

fn protocol_actions() -> VecDeque<ProtocolAction> {
    use ProtocolAction::*;
    [
        SendHello,
        SendBlock(BlockSource::RegionPrefix),
        SendBlock(BlockSource::Empty),
        ReceiveHello,
        ReceiveBlock(ReceiveExpectation::RegionCode),
        ReceiveBlock(ReceiveExpectation::Empty),
        SendHello,
        SendBlock(BlockSource::PokemonPayload),
        SendBlock(BlockSource::Empty),
        ReceiveHello,
        ReceiveBlock(ReceiveExpectation::RegionPrefix),
        ReceiveBlock(ReceiveExpectation::Empty),
        SendHello,
        SendBlock(BlockSource::ResolvedRegionCode),
        SendBlock(BlockSource::Empty),
        ReceiveHello,
        ReceiveBlock(ReceiveExpectation::GamePayload),
        ReceiveBlock(ReceiveExpectation::Empty),
        SendHello,
        SendBlock(BlockSource::Empty),
        Finish,
    ]
    .into_iter()
    .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PulseQueue {
    pulses: VecDeque<Pulse>,
}

impl PulseQueue {
    fn new() -> Self {
        Self {
            pulses: VecDeque::new(),
        }
    }

    fn clear(&mut self) {
        self.pulses.clear();
    }

    fn is_empty(&self) -> bool {
        self.pulses.is_empty()
    }

    fn push(&mut self, level: bool, t_cycles: u32) {
        if t_cycles == 0 {
            return;
        }
        if let Some(back) = self.pulses.back_mut()
            && back.level == level
        {
            back.remaining_t_cycles = back.remaining_t_cycles.saturating_add(t_cycles);
            return;
        }
        self.pulses.push_back(Pulse {
            level,
            remaining_t_cycles: t_cycles,
        });
    }

    fn tick_t_cycle(&mut self) -> bool {
        let Some(front) = self.pulses.front_mut() else {
            return false;
        };
        let level = front.level;
        front.remaining_t_cycles = front.remaining_t_cycles.saturating_sub(1);
        if front.remaining_t_cycles == 0 {
            self.pulses.pop_front();
        }
        level
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Pulse {
    level: bool,
    remaining_t_cycles: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HelloReceiver {
    previous_segment_t_cycles: u32,
    short_pulses_seen: u8,
    completed: bool,
}

impl HelloReceiver {
    fn new() -> Self {
        Self {
            previous_segment_t_cycles: 0,
            short_pulses_seen: 0,
            completed: false,
        }
    }

    fn tick_same_level(&mut self) {
        self.previous_segment_t_cycles = self.previous_segment_t_cycles.saturating_add(1);
    }

    fn observe_transition(&mut self, completed_level: bool) -> bool {
        let duration = self.previous_segment_t_cycles;
        self.previous_segment_t_cycles = 1;
        if completed_level && is_short_mark(duration) {
            self.short_pulses_seen = self.short_pulses_seen.saturating_add(1);
            self.completed = self.short_pulses_seen >= 2;
        }
        self.completed
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DataBlockReceiver {
    state: DataBlockReceiverState,
}

impl DataBlockReceiver {
    fn new() -> Self {
        Self {
            state: DataBlockReceiverState::Header(DataMessageReceiver::new(2)),
        }
    }

    fn tick_same_level(&mut self) {
        match &mut self.state {
            DataBlockReceiverState::Header(receiver)
            | DataBlockReceiverState::Payload { receiver, .. }
            | DataBlockReceiverState::Checksum { receiver, .. } => receiver.tick_same_level(),
            DataBlockReceiverState::Done(_) | DataBlockReceiverState::Failed => {}
        }
    }

    fn observe_transition(&mut self, completed_level: bool) -> bool {
        let message_completed = match &mut self.state {
            DataBlockReceiverState::Header(receiver)
            | DataBlockReceiverState::Payload { receiver, .. }
            | DataBlockReceiverState::Checksum { receiver, .. } => {
                receiver.observe_transition(completed_level)
            }
            DataBlockReceiverState::Done(_) => return true,
            DataBlockReceiverState::Failed => return true,
        };
        if message_completed {
            self.advance_message();
        }
        matches!(
            self.state,
            DataBlockReceiverState::Done(_) | DataBlockReceiverState::Failed
        )
    }

    fn into_result(self) -> Option<Vec<u8>> {
        match self.state {
            DataBlockReceiverState::Done(bytes) => Some(bytes),
            DataBlockReceiverState::Header(_)
            | DataBlockReceiverState::Payload { .. }
            | DataBlockReceiverState::Checksum { .. }
            | DataBlockReceiverState::Failed => None,
        }
    }

    fn advance_message(&mut self) {
        let state = std::mem::replace(&mut self.state, DataBlockReceiverState::Failed);
        let next_state = match state {
            DataBlockReceiverState::Header(receiver) => {
                let Some(bytes) = receiver.into_result() else {
                    self.state = DataBlockReceiverState::Failed;
                    return;
                };
                let [prefix, len] = bytes.as_slice() else {
                    self.state = DataBlockReceiverState::Failed;
                    return;
                };
                if *prefix != MESSAGE_PREFIX || usize::from(*len) > MAX_RECEIVED_BLOCK_LEN {
                    self.state = DataBlockReceiverState::Failed;
                    return;
                }
                DataBlockReceiverState::Payload {
                    receiver: DataMessageReceiver::new(usize::from(*len)),
                    checksum: u16::from(MESSAGE_PREFIX) + u16::from(*len),
                }
            }
            DataBlockReceiverState::Payload {
                receiver, checksum, ..
            } => {
                let Some(payload) = receiver.into_result() else {
                    self.state = DataBlockReceiverState::Failed;
                    return;
                };
                let checksum = payload
                    .iter()
                    .fold(checksum, |sum, byte| sum.wrapping_add(u16::from(*byte)));
                DataBlockReceiverState::Checksum {
                    receiver: DataMessageReceiver::new(2),
                    payload,
                    checksum,
                }
            }
            DataBlockReceiverState::Checksum {
                receiver,
                payload,
                checksum,
            } => {
                let Some(bytes) = receiver.into_result() else {
                    self.state = DataBlockReceiverState::Failed;
                    return;
                };
                let [lo, hi] = bytes.as_slice() else {
                    self.state = DataBlockReceiverState::Failed;
                    return;
                };
                let received = u16::from_le_bytes([*lo, *hi]);
                if received == checksum {
                    DataBlockReceiverState::Done(payload)
                } else {
                    DataBlockReceiverState::Failed
                }
            }
            DataBlockReceiverState::Done(bytes) => DataBlockReceiverState::Done(bytes),
            DataBlockReceiverState::Failed => DataBlockReceiverState::Failed,
        };
        self.state = next_state;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DataBlockReceiverState {
    Header(DataMessageReceiver),
    Payload {
        receiver: DataMessageReceiver,
        checksum: u16,
    },
    Checksum {
        receiver: DataMessageReceiver,
        payload: Vec<u8>,
        checksum: u16,
    },
    Done(Vec<u8>),
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DataMessageReceiver {
    expected_len: usize,
    state: DataMessageReceiverState,
    previous_segment_t_cycles: u32,
    bits: Vec<u8>,
    bytes: Vec<u8>,
}

impl DataMessageReceiver {
    fn new(expected_len: usize) -> Self {
        Self {
            expected_len,
            state: DataMessageReceiverState::WaitingForPreambleMark,
            previous_segment_t_cycles: 0,
            bits: Vec::with_capacity(expected_len.saturating_mul(8)),
            bytes: Vec::with_capacity(expected_len),
        }
    }

    fn tick_same_level(&mut self) {
        self.previous_segment_t_cycles = self.previous_segment_t_cycles.saturating_add(1);
    }

    fn observe_transition(&mut self, completed_level: bool) -> bool {
        let duration = self.previous_segment_t_cycles;
        self.previous_segment_t_cycles = 1;
        match self.state {
            DataMessageReceiverState::WaitingForPreambleMark => {
                if completed_level && is_short_mark(duration) {
                    self.state = DataMessageReceiverState::WaitingForPreambleSpace;
                }
            }
            DataMessageReceiverState::WaitingForPreambleSpace => {
                if !completed_level && duration >= IR_MIN_PREAMBLE_SPACE_T_CYCLES {
                    if self.expected_len == 0 {
                        self.state = DataMessageReceiverState::WaitingForTrailerMark;
                    } else {
                        self.state = DataMessageReceiverState::ReceivingBits;
                    }
                }
            }
            DataMessageReceiverState::ReceivingBits => {
                if !completed_level {
                    self.push_bit(if duration < IR_SPACE_ZERO_ONE_THRESHOLD_T_CYCLES {
                        0
                    } else {
                        1
                    });
                    if self.bits.len() == self.expected_len * 8 {
                        self.state = DataMessageReceiverState::WaitingForTrailerMark;
                    }
                }
            }
            DataMessageReceiverState::WaitingForTrailerMark => {
                if completed_level && is_short_mark(duration) {
                    self.finish();
                }
            }
            DataMessageReceiverState::Done => {}
        }
        matches!(self.state, DataMessageReceiverState::Done)
    }

    fn push_bit(&mut self, bit: u8) {
        self.bits.push(bit);
        if self.bits.len().is_multiple_of(8) {
            let start = self.bits.len() - 8;
            let byte = self.bits[start..]
                .iter()
                .fold(0_u8, |byte, bit| byte.wrapping_shl(1) | (*bit & 0x01));
            self.bytes.push(byte);
        }
    }

    fn finish(&mut self) {
        if self.bytes.len() == self.expected_len {
            self.state = DataMessageReceiverState::Done;
        }
    }

    fn into_result(self) -> Option<Vec<u8>> {
        matches!(self.state, DataMessageReceiverState::Done).then_some(self.bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DataMessageReceiverState {
    WaitingForPreambleMark,
    WaitingForPreambleSpace,
    ReceivingBits,
    WaitingForTrailerMark,
    Done,
}

fn encode_hello(queue: &mut PulseQueue) {
    queue.push(false, IR_HELLO_LONG_SPACE_T_CYCLES);
    queue.push(true, IR_HELLO_MARK_T_CYCLES);
    queue.push(false, IR_HELLO_MID_SPACE_T_CYCLES);
    queue.push(true, IR_HELLO_MARK_T_CYCLES);
    queue.push(false, IR_HELLO_SHORT_SPACE_T_CYCLES);
}

fn encode_data_block(queue: &mut PulseQueue, payload: &[u8]) {
    let len = u8::try_from(payload.len()).expect("Pokemon Pikachu Color blocks fit in u8");
    let checksum = data_block_checksum(payload);
    queue.push(false, IR_BLOCK_LEAD_SPACE_T_CYCLES);
    encode_data_message(queue, &[MESSAGE_PREFIX, len]);
    encode_data_message(queue, payload);
    encode_data_message(queue, &checksum.to_le_bytes());
}

fn encode_block_ack(queue: &mut PulseQueue, status: u8) {
    queue.push(false, IR_BLOCK_LEAD_SPACE_T_CYCLES);
    encode_data_message(queue, &[status]);
}

fn encode_data_message(queue: &mut PulseQueue, bytes: &[u8]) {
    queue.push(false, IR_HELLO_SHORT_SPACE_T_CYCLES);
    queue.push(true, IR_MESSAGE_PREAMBLE_MARK_T_CYCLES);
    queue.push(false, IR_MESSAGE_PREAMBLE_SPACE_T_CYCLES);
    for byte in bytes {
        for bit_index in (0..8).rev() {
            queue.push(true, IR_MESSAGE_BIT_MARK_T_CYCLES);
            let bit = (byte >> bit_index) & 0x01;
            let space = if bit == 0 {
                IR_MESSAGE_ZERO_SPACE_T_CYCLES
            } else {
                IR_MESSAGE_ONE_SPACE_T_CYCLES
            };
            queue.push(false, space);
        }
    }
    queue.push(true, IR_MESSAGE_TRAILER_MARK_T_CYCLES);
    queue.push(false, IR_MESSAGE_TRAILER_SPACE_T_CYCLES);
}

fn data_block_checksum(payload: &[u8]) -> u16 {
    payload.iter().fold(
        u16::from(MESSAGE_PREFIX) + payload.len() as u16,
        |sum, byte| sum.wrapping_add(u16::from(*byte)),
    )
}

fn pokemon_pikachu_color_payload(
    gift: PokemonPikachuColorGift,
) -> [u8; POKEMON_PIKACHU_COLOR_PAYLOAD_LEN] {
    let mut payload = [0_u8; POKEMON_PIKACHU_COLOR_PAYLOAD_LEN];
    payload[0] = POKEMON_PIKACHU_2_VERSION;
    payload[3..14].copy_from_slice(&WESTERN_PIKACHU_NAME);
    payload[16] = gift.item_byte();
    payload
}

const fn is_supported_western_region(region_code: u8) -> bool {
    matches!(region_code, 0x90 | 0x96 | 0x99 | 0x9A | 0x9F)
}

fn is_short_mark(duration: u32) -> bool {
    (IR_MESSAGE_BIT_MARK_T_CYCLES / 2..=IR_HELLO_MARK_T_CYCLES * 3).contains(&duration)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ConsoleModel, MachineConfig, StartupMode};

    const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;

    fn done_data_message(bytes: &[u8]) -> DataMessageReceiver {
        DataMessageReceiver {
            expected_len: bytes.len(),
            state: DataMessageReceiverState::Done,
            previous_segment_t_cycles: 0,
            bits: Vec::new(),
            bytes: bytes.to_vec(),
        }
    }

    fn drive_data_message_receiver(
        pulses: &mut PulseQueue,
        receiver: &mut DataMessageReceiver,
    ) -> Option<Vec<u8>> {
        let mut previous = false;
        while !pulses.is_empty() {
            let level = pulses.tick_t_cycle();
            if level == previous {
                receiver.tick_same_level();
            } else {
                if receiver.observe_transition(previous) {
                    break;
                }
                previous = level;
            }
        }
        receiver.clone().into_result()
    }

    fn drive_data_block_receiver(
        pulses: &mut PulseQueue,
        receiver: &mut DataBlockReceiver,
    ) -> Option<Vec<u8>> {
        let mut previous = false;
        while !pulses.is_empty() {
            let level = pulses.tick_t_cycle();
            if level == previous {
                receiver.tick_same_level();
            } else {
                if receiver.observe_transition(previous) {
                    break;
                }
                previous = level;
            }
        }
        receiver.clone().into_result()
    }

    fn accessory_with_phase(phase: ProtocolPhase) -> PokemonPikachuColor {
        PokemonPikachuColor {
            phase,
            ..PokemonPikachuColor::default()
        }
    }

    fn cgb_native_skip_boot_machine() -> Machine {
        let mut machine = Machine::new(
            MachineConfig::new(ConsoleModel::GameBoyColor).with_startup_mode(StartupMode::SkipBoot),
        );
        let mut rom = vec![0xFF; HEADER_MINIMUM_ROM_LEN.max(32 * 1024)];
        rom[0x0100] = 0xC3;
        rom[0x0101] = 0x00;
        rom[0x0102] = 0x01;
        rom[0x0143] = 0x80;
        rom[0x0147] = 0x00;
        rom[0x0148] = 0x00;
        rom[0x0149] = 0x00;
        machine
            .load_cartridge(rom)
            .expect("CGB native test ROM should load");
        machine
    }

    #[test]
    fn gifts_map_to_documented_item_bytes() {
        let item_bytes: Vec<u8> = PokemonPikachuColorGift::ALL
            .into_iter()
            .map(PokemonPikachuColorGift::item_byte)
            .collect();

        assert_eq!(
            item_bytes,
            vec![
                0x0D, 0x00, 0x09, 0x13, 0x15, 0x17, 0x10, 0x11, 0x16, 0x12, 0x22
            ]
        );
    }

    #[test]
    fn public_gift_region_and_status_helpers_cover_all_variants() {
        assert_eq!(
            PokemonPikachuColorGift::ALL.map(PokemonPikachuColorGift::label),
            [
                "1W", "100W", "200W", "300W", "400W", "500W", "600W", "700W", "800W", "900W",
                "999W",
            ]
        );
        assert_eq!(
            PokemonPikachuColorGift::Watts999.next(),
            PokemonPikachuColorGift::Watts1
        );
        assert_eq!(PokemonPikachuColorRegion::Auto.code(), None);
        assert_eq!(PokemonPikachuColorRegion::Usa.code(), Some(0x90));
        assert_eq!(PokemonPikachuColorRegion::Esp.code(), Some(0x96));
        assert_eq!(PokemonPikachuColorRegion::Ita.code(), Some(0x99));
        assert_eq!(PokemonPikachuColorRegion::Fra.code(), Some(0x9A));
        assert_eq!(PokemonPikachuColorRegion::Ger.code(), Some(0x9F));

        let mut accessory = PokemonPikachuColor::new(
            PokemonPikachuColorGift::Watts400,
            PokemonPikachuColorRegion::Fra,
        );
        assert_eq!(accessory.gift(), PokemonPikachuColorGift::Watts400);
        assert_eq!(accessory.region(), PokemonPikachuColorRegion::Fra);
        assert_eq!(
            accessory.status(),
            PokemonPikachuColorStatus {
                gift: PokemonPikachuColorGift::Watts400,
                region: PokemonPikachuColorRegion::Fra,
                resolved_region_code: Some(0x9A),
                emitter_on: false,
                game_emitter_on: false,
                game_emitter_seen: false,
                completed_exchange: false,
                failed_exchange: false,
            }
        );

        accessory.tick_t_cycle(true);
        assert!(accessory.status().game_emitter_on);
        assert!(accessory.status().game_emitter_seen);
        accessory.set_gift(PokemonPikachuColorGift::Watts400);
        assert!(accessory.status().game_emitter_seen);
        accessory.set_gift(PokemonPikachuColorGift::Watts500);
        assert_eq!(accessory.gift(), PokemonPikachuColorGift::Watts500);
        assert!(!accessory.status().game_emitter_seen);
        accessory.set_region(PokemonPikachuColorRegion::Fra);
        assert_eq!(accessory.status().resolved_region_code, Some(0x9A));
        accessory.set_region(PokemonPikachuColorRegion::Auto);
        assert_eq!(accessory.region(), PokemonPikachuColorRegion::Auto);
        assert_eq!(accessory.status().resolved_region_code, None);
    }

    #[test]
    fn payload_uses_pokemon_pikachu_2_identity_and_selected_item() {
        let payload = pokemon_pikachu_color_payload(PokemonPikachuColorGift::Watts999);

        assert_eq!(
            payload,
            [
                0x03, 0x00, 0x00, 0x8F, 0x88, 0x8A, 0x80, 0x82, 0x87, 0x94, 0x50, 0x50, 0x50, 0x50,
                0x00, 0x00, 0x22, 0x00, 0x00, 0x00,
            ]
        );
    }

    #[test]
    fn checksum_matches_decoded_gbe_plus_payload_blocks_without_reusing_waveform_data() {
        assert_eq!(
            data_block_checksum(&pokemon_pikachu_color_payload(
                PokemonPikachuColorGift::Watts1
            )),
            0x057C
        );
        assert_eq!(
            data_block_checksum(&pokemon_pikachu_color_payload(
                PokemonPikachuColorGift::Watts100
            )),
            0x056F
        );
        assert_eq!(
            data_block_checksum(&pokemon_pikachu_color_payload(
                PokemonPikachuColorGift::Watts500
            )),
            0x0586
        );
    }

    #[test]
    fn data_message_round_trips_msb_first() {
        let mut pulses = PulseQueue::new();
        encode_data_message(&mut pulses, &[0x96]);
        let mut receiver = DataMessageReceiver::new(1);

        assert_eq!(
            drive_data_message_receiver(&mut pulses, &mut receiver),
            Some(vec![0x96])
        );
    }

    #[test]
    fn data_message_receiver_covers_empty_and_incomplete_messages() {
        let mut pulses = PulseQueue::new();
        encode_data_message(&mut pulses, &[]);
        let mut receiver = DataMessageReceiver::new(0);
        assert_eq!(
            drive_data_message_receiver(&mut pulses, &mut receiver),
            Some(Vec::new())
        );

        let mut pulses = PulseQueue::new();
        encode_data_message(&mut pulses, &[0x96]);
        let mut receiver = DataMessageReceiver::new(2);
        assert_eq!(
            drive_data_message_receiver(&mut pulses, &mut receiver),
            None
        );

        let mut receiver = done_data_message(&[]);
        assert!(receiver.observe_transition(false));
        assert_eq!(receiver.into_result(), Some(Vec::new()));
    }

    #[test]
    fn encoder_uses_gold_silver_crystal_ir_timer_cadence() {
        let mut pulses = PulseQueue::new();
        encode_hello(&mut pulses);
        assert_eq!(
            pulses.pulses.iter().copied().collect::<Vec<_>>(),
            vec![
                Pulse {
                    level: false,
                    remaining_t_cycles: 7_680
                },
                Pulse {
                    level: true,
                    remaining_t_cycles: 512
                },
                Pulse {
                    level: false,
                    remaining_t_cycles: 2_560
                },
                Pulse {
                    level: true,
                    remaining_t_cycles: 512
                },
                Pulse {
                    level: false,
                    remaining_t_cycles: 512
                },
            ]
        );

        let mut pulses = PulseQueue::new();
        encode_data_message(&mut pulses, &[0x80]);
        assert_eq!(
            pulses.pulses.iter().take(7).copied().collect::<Vec<_>>(),
            vec![
                Pulse {
                    level: false,
                    remaining_t_cycles: 512
                },
                Pulse {
                    level: true,
                    remaining_t_cycles: 512
                },
                Pulse {
                    level: false,
                    remaining_t_cycles: 2_560
                },
                Pulse {
                    level: true,
                    remaining_t_cycles: 256
                },
                Pulse {
                    level: false,
                    remaining_t_cycles: 1_280
                },
                Pulse {
                    level: true,
                    remaining_t_cycles: 256
                },
                Pulse {
                    level: false,
                    remaining_t_cycles: 512
                },
            ]
        );
    }

    #[test]
    fn data_block_round_trips_payload_and_checksum() {
        let payload = [REGION_PREFIX];
        let mut pulses = PulseQueue::new();
        encode_data_block(&mut pulses, &payload);
        let mut receiver = DataBlockReceiver::new();

        assert_eq!(
            drive_data_block_receiver(&mut pulses, &mut receiver),
            Some(payload.to_vec())
        );
    }

    #[test]
    fn data_block_receiver_rejects_malformed_headers_payloads_and_checksums() {
        let mut receiver = DataBlockReceiver {
            state: DataBlockReceiverState::Header(DataMessageReceiver::new(2)),
        };
        receiver.advance_message();
        assert!(matches!(receiver.state, DataBlockReceiverState::Failed));
        assert_eq!(receiver.into_result(), None);

        let mut receiver = DataBlockReceiver {
            state: DataBlockReceiverState::Header(done_data_message(&[MESSAGE_PREFIX])),
        };
        receiver.advance_message();
        assert!(matches!(receiver.state, DataBlockReceiverState::Failed));

        let mut receiver = DataBlockReceiver {
            state: DataBlockReceiverState::Header(done_data_message(&[0x00, 0x01])),
        };
        receiver.advance_message();
        assert!(matches!(receiver.state, DataBlockReceiverState::Failed));

        let mut receiver = DataBlockReceiver {
            state: DataBlockReceiverState::Header(done_data_message(&[
                MESSAGE_PREFIX,
                MAX_RECEIVED_BLOCK_LEN as u8 + 1,
            ])),
        };
        receiver.advance_message();
        assert!(matches!(receiver.state, DataBlockReceiverState::Failed));

        let mut receiver = DataBlockReceiver {
            state: DataBlockReceiverState::Payload {
                receiver: DataMessageReceiver::new(1),
                checksum: 0,
            },
        };
        receiver.advance_message();
        assert!(matches!(receiver.state, DataBlockReceiverState::Failed));

        let mut receiver = DataBlockReceiver {
            state: DataBlockReceiverState::Checksum {
                receiver: DataMessageReceiver::new(2),
                payload: vec![REGION_PREFIX],
                checksum: 0,
            },
        };
        receiver.advance_message();
        assert!(matches!(receiver.state, DataBlockReceiverState::Failed));

        let mut receiver = DataBlockReceiver {
            state: DataBlockReceiverState::Checksum {
                receiver: done_data_message(&[0x00]),
                payload: vec![REGION_PREFIX],
                checksum: 0,
            },
        };
        receiver.advance_message();
        assert!(matches!(receiver.state, DataBlockReceiverState::Failed));

        let mut receiver = DataBlockReceiver {
            state: DataBlockReceiverState::Checksum {
                receiver: done_data_message(&[0x00, 0x00]),
                payload: vec![REGION_PREFIX],
                checksum: data_block_checksum(&[REGION_PREFIX]),
            },
        };
        receiver.advance_message();
        assert!(matches!(receiver.state, DataBlockReceiverState::Failed));

        let mut receiver = DataBlockReceiver {
            state: DataBlockReceiverState::Done(vec![REGION_PREFIX]),
        };
        assert!(receiver.observe_transition(false));
        receiver.advance_message();
        assert_eq!(receiver.into_result(), Some(vec![REGION_PREFIX]));

        let mut receiver = DataBlockReceiver {
            state: DataBlockReceiverState::Failed,
        };
        assert!(receiver.observe_transition(false));
        receiver.advance_message();
        assert_eq!(receiver.into_result(), None);
    }

    #[test]
    fn pulse_queue_ignores_zero_merges_levels_and_reports_idle_low() {
        let mut pulses = PulseQueue::new();
        assert!(!pulses.tick_t_cycle());

        pulses.push(true, 0);
        assert!(pulses.is_empty());
        pulses.push(true, 2);
        pulses.push(true, 3);
        pulses.push(false, 1);
        assert_eq!(
            pulses.pulses.iter().copied().collect::<Vec<_>>(),
            vec![
                Pulse {
                    level: true,
                    remaining_t_cycles: 5,
                },
                Pulse {
                    level: false,
                    remaining_t_cycles: 1,
                },
            ]
        );
        for _ in 0..5 {
            assert!(pulses.tick_t_cycle());
        }
        assert!(!pulses.tick_t_cycle());
        assert!(!pulses.tick_t_cycle());
    }

    #[test]
    fn block_sources_and_receive_expectations_cover_success_and_failure_paths() {
        let mut accessory = PokemonPikachuColor::default();
        assert_eq!(
            accessory.block_source_bytes(BlockSource::RegionPrefix),
            Some(vec![REGION_PREFIX])
        );
        assert_eq!(
            accessory.block_source_bytes(BlockSource::ResolvedRegionCode),
            None
        );
        assert_eq!(
            accessory.block_source_bytes(BlockSource::PokemonPayload),
            Some(pokemon_pikachu_color_payload(PokemonPikachuColorGift::Watts1).to_vec())
        );
        assert_eq!(
            accessory.block_source_bytes(BlockSource::Empty),
            Some(Vec::new())
        );

        assert!(accessory.handle_received_block(ReceiveExpectation::RegionCode, &[0x96]));
        assert_eq!(accessory.status().resolved_region_code, Some(0x96));
        assert!(
            !PokemonPikachuColor::default()
                .handle_received_block(ReceiveExpectation::RegionCode, &[0x00])
        );
        assert!(
            !PokemonPikachuColor::default()
                .handle_received_block(ReceiveExpectation::RegionCode, &[0x96, 0x00])
        );
        let mut fixed_region = PokemonPikachuColor::new(
            PokemonPikachuColorGift::Watts1,
            PokemonPikachuColorRegion::Usa,
        );
        assert!(fixed_region.handle_received_block(ReceiveExpectation::RegionCode, &[0xF3]));
        assert_eq!(fixed_region.status().resolved_region_code, Some(0x90));

        assert!(
            accessory.handle_received_block(ReceiveExpectation::RegionPrefix, &[REGION_PREFIX])
        );
        assert!(!accessory.handle_received_block(ReceiveExpectation::RegionPrefix, &[0x00]));
        assert!(accessory.handle_received_block(ReceiveExpectation::Empty, &[]));
        assert!(!accessory.handle_received_block(ReceiveExpectation::Empty, &[0x00]));
        assert!(accessory.handle_received_block(ReceiveExpectation::GamePayload, &[0x01]));
        assert!(!accessory.handle_received_block(ReceiveExpectation::GamePayload, &[]));
    }

    #[test]
    fn auto_region_accepts_western_region_codes_only() {
        let region = PokemonPikachuColorRegion::Auto;

        for code in [0x90, 0x96, 0x99, 0x9A, 0x9F] {
            assert_eq!(region.resolve(code), Some(code));
        }
        assert_eq!(region.resolve(0xF3), None);
        assert_eq!(region.resolve(0x00), None);
    }

    #[test]
    fn outgoing_hello_waits_for_game_response_before_first_block() {
        let mut accessory = PokemonPikachuColor::default();

        for _ in 0..20_000 {
            accessory.tick_t_cycle(false);
            if matches!(accessory.phase, ProtocolPhase::WaitingHelloResponse { .. }) {
                break;
            }
            assert!(
                !matches!(
                    accessory.phase,
                    ProtocolPhase::SendingBlock | ProtocolPhase::Failed
                ),
                "accessory must not send its first data block before the game answers its hello"
            );
        }
        assert!(matches!(
            accessory.phase,
            ProtocolPhase::WaitingHelloResponse { .. }
        ));

        for _ in 0..8_000 {
            accessory.tick_t_cycle(false);
            assert!(
                matches!(accessory.phase, ProtocolPhase::WaitingHelloResponse { .. }),
                "accessory should wait for the game hello response instead of sending data early"
            );
        }

        let mut game_hello = PulseQueue::new();
        encode_hello(&mut game_hello);
        let mut reached_first_block = false;
        while !game_hello.is_empty() {
            let game_emitter_on = game_hello.tick_t_cycle();
            accessory.tick_t_cycle(game_emitter_on);
            if matches!(accessory.phase, ProtocolPhase::SendingBlock) {
                reached_first_block = true;
                break;
            }
        }

        for _ in 0..20_000 {
            if reached_first_block {
                break;
            }
            accessory.tick_t_cycle(false);
            if matches!(accessory.phase, ProtocolPhase::SendingBlock) {
                reached_first_block = true;
                break;
            }
            assert!(!matches!(accessory.phase, ProtocolPhase::Failed));
        }

        assert!(
            reached_first_block,
            "accessory should send the first data block after receiving the game hello response"
        );
    }

    #[test]
    fn missed_outgoing_hello_retries_on_short_role_a_cadence() {
        let mut accessory = PokemonPikachuColor::default();
        let mut entered_hello_wait = false;
        let mut saw_timeout = false;
        let mut saw_retry_hello = false;

        for _ in 0..IR_HELLO_RESPONSE_TIMEOUT_T_CYCLES
            + IR_RETRY_DELAY_T_CYCLES
            + IR_HELLO_LONG_SPACE_T_CYCLES * 4
        {
            accessory.tick_t_cycle(false);
            entered_hello_wait |=
                matches!(accessory.phase, ProtocolPhase::WaitingHelloResponse { .. });
            saw_timeout |= entered_hello_wait && matches!(accessory.phase, ProtocolPhase::Failed);
            if saw_timeout && matches!(accessory.phase, ProtocolPhase::SendingAction) {
                saw_retry_hello = true;
                break;
            }
        }

        assert!(entered_hello_wait);
        assert!(saw_timeout);
        assert!(
            saw_retry_hello,
            "PP2 role A should retry hello promptly when the game misses the first one"
        );
    }

    #[test]
    fn completed_exchange_rearms_for_another_mystery_gift_round() {
        let mut accessory = PokemonPikachuColor::default();
        accessory.actions.clear();
        accessory.phase = ProtocolPhase::Idle;

        accessory.tick_t_cycle(false);
        assert!(matches!(accessory.phase, ProtocolPhase::Completed));
        assert!(accessory.status().completed_exchange);

        let mut saw_second_round_hello = false;
        for _ in 0..IR_COMPLETED_RESTART_DELAY_T_CYCLES + IR_HELLO_LONG_SPACE_T_CYCLES * 2 {
            accessory.tick_t_cycle(false);
            if matches!(accessory.phase, ProtocolPhase::SendingAction) {
                saw_second_round_hello = true;
                break;
            }
        }

        assert!(
            saw_second_round_hello,
            "successful PP2 transfers should rearm without toggling IR NONE"
        );
        assert!(!accessory.status().completed_exchange);
    }

    #[test]
    fn protocol_phase_transitions_cover_ack_success_and_failure_paths() {
        let mut accessory = accessory_with_phase(ProtocolPhase::ReceivingHello {
            receiver: HelloReceiver::new(),
            next: ProtocolAction::ReceiveBlock(ReceiveExpectation::Empty),
        });
        accessory.finish_receiver_phase();
        assert!(matches!(accessory.phase, ProtocolPhase::RespondingToHello));
        assert!(matches!(
            accessory.actions.front(),
            Some(ProtocolAction::ReceiveBlock(ReceiveExpectation::Empty))
        ));

        accessory.pulses.clear();
        accessory.phase = ProtocolPhase::RespondingToHello;
        accessory.finish_sending_phase();
        assert!(matches!(
            accessory.phase,
            ProtocolPhase::ReceivingBlock { .. }
        ));

        accessory.actions.clear();
        accessory.phase = ProtocolPhase::SendingBlockAck;
        accessory.finish_sending_phase();
        assert!(matches!(accessory.phase, ProtocolPhase::Completed));

        accessory.actions = [ProtocolAction::SendBlock(BlockSource::ResolvedRegionCode)]
            .into_iter()
            .collect();
        accessory.resolved_region_code = None;
        accessory.phase = ProtocolPhase::Idle;
        accessory.start_next_action();
        assert!(matches!(accessory.phase, ProtocolPhase::Failed));
        assert!(accessory.status().failed_exchange);

        let mut accessory = accessory_with_phase(ProtocolPhase::ReceivingBlock {
            receiver: DataBlockReceiver {
                state: DataBlockReceiverState::Done(vec![REGION_PREFIX]),
            },
            expectation: ReceiveExpectation::RegionPrefix,
        });
        accessory.finish_receiver_phase();
        assert!(matches!(accessory.phase, ProtocolPhase::SendingBlockAck));

        let mut accessory = accessory_with_phase(ProtocolPhase::ReceivingBlock {
            receiver: DataBlockReceiver {
                state: DataBlockReceiverState::Failed,
            },
            expectation: ReceiveExpectation::Empty,
        });
        accessory.finish_receiver_phase();
        assert!(matches!(accessory.phase, ProtocolPhase::Failed));

        let mut accessory = accessory_with_phase(ProtocolPhase::WaitingBlockAck {
            receiver: done_data_message(&[ACK_OKAY]),
        });
        accessory.actions.clear();
        accessory.finish_receiver_phase();
        assert!(matches!(accessory.phase, ProtocolPhase::Completed));

        let mut accessory = accessory_with_phase(ProtocolPhase::WaitingBlockAck {
            receiver: done_data_message(&[0x00]),
        });
        accessory.finish_receiver_phase();
        assert!(matches!(accessory.phase, ProtocolPhase::Failed));
    }

    #[test]
    fn session_routes_accessory_light_to_cgb_sensor_and_clears_it_on_exit() {
        let mut accessor_session = PokemonPikachuColorSession::new(
            cgb_native_skip_boot_machine(),
            PokemonPikachuColor::default(),
        );
        assert_eq!(
            accessor_session.next_t_cycle(),
            accessor_session.machine().next_t_cycle()
        );
        assert_eq!(
            accessor_session.pokemon_pikachu_color().gift(),
            PokemonPikachuColorGift::Watts1
        );
        accessor_session
            .pokemon_pikachu_color_mut()
            .set_gift(PokemonPikachuColorGift::Watts100);
        assert_eq!(
            accessor_session.pokemon_pikachu_color().gift(),
            PokemonPikachuColorGift::Watts100
        );
        let _ = accessor_session.step_t_cycle();
        let mut observer = NoopMachineStepObserver;
        let _ = accessor_session.advance_t_cycle_with_observer(&mut observer);

        let mut session = PokemonPikachuColorSession::new(
            cgb_native_skip_boot_machine(),
            PokemonPikachuColor::default(),
        );
        session.machine_mut().write_bus(0xFF56, 0xC0);
        session
            .pokemon_pikachu_color_mut()
            .pulses
            .push(true, 20_000);

        for _ in 0..20_000 + ACCESSORY_TO_CGB_OPTICAL_DELAY_T_CYCLES {
            session.advance_t_cycle();
        }

        assert_eq!(session.machine_mut().read_bus(0xFF56) & 0x02, 0x00);

        let mut machine = session.into_machine();
        machine.write_bus(0xFF56, 0xC0);
        for _ in 0..128 {
            machine.step_t_cycle();
        }
        assert_eq!(machine.read_bus(0xFF56) & 0x02, 0x02);
    }
}
