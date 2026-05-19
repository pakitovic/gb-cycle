use crate::debugger::{TraceBuffer, TraceSink};
use crate::machine::{Machine, MachineStepObserver, NoopMachineStepObserver};
use crate::scheduler::{CycleContext, GlobalScheduler, SchedulerPhase, TCycle};

#[cfg(test)]
use super::mystery_gift_protocol::data_block_checksum;
use super::mystery_gift_protocol::{
    ACCESSORY_TO_CGB_OPTICAL_DELAY_T_CYCLES, MYSTERY_GIFT_PAYLOAD_LEN,
    MYSTERY_GIFT_SINGLE_PAYLOAD_VERSION, MysteryGiftRegion, MysteryGiftRoleAProtocol,
};

const GB_CYCLE_NAME: [u8; 11] = [
    0x86, 0x81, 0xE3, 0x82, 0x98, 0x82, 0x8B, 0x84, 0x50, 0x50, 0x50,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PokemonMysteryGiftKind {
    #[default]
    Item,
    Decoration,
}

impl PokemonMysteryGiftKind {
    pub const ALL: [Self; 2] = [Self::Item, Self::Decoration];

    pub const fn gift_type_byte(self) -> u8 {
        match self {
            Self::Item => 0x00,
            Self::Decoration => 0x01,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Item => "GIFT ITEM",
            Self::Decoration => "GIFT DECORATION",
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::Item => Self::Decoration,
            Self::Decoration => Self::Item,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PokemonMysteryGiftCode(u8);

impl PokemonMysteryGiftCode {
    pub const MIN: u8 = 0x00;
    pub const MAX: u8 = 0x24;
    pub const ALL: [Self; 37] = [
        Self(0x00),
        Self(0x01),
        Self(0x02),
        Self(0x03),
        Self(0x04),
        Self(0x05),
        Self(0x06),
        Self(0x07),
        Self(0x08),
        Self(0x09),
        Self(0x0A),
        Self(0x0B),
        Self(0x0C),
        Self(0x0D),
        Self(0x0E),
        Self(0x0F),
        Self(0x10),
        Self(0x11),
        Self(0x12),
        Self(0x13),
        Self(0x14),
        Self(0x15),
        Self(0x16),
        Self(0x17),
        Self(0x18),
        Self(0x19),
        Self(0x1A),
        Self(0x1B),
        Self(0x1C),
        Self(0x1D),
        Self(0x1E),
        Self(0x1F),
        Self(0x20),
        Self(0x21),
        Self(0x22),
        Self(0x23),
        Self(0x24),
    ];

    pub const fn new(value: u8) -> Option<Self> {
        if value <= Self::MAX {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn value(self) -> u8 {
        self.0
    }

    pub const fn item_label(self) -> &'static str {
        match self.0 {
            0x00 => "BERRY",
            0x01 => "PRZCUREBERRY",
            0x02 => "MINT BERRY",
            0x03 => "ICE BERRY",
            0x04 => "BURNT BERRY",
            0x05 => "PSNCUREBERRY",
            0x06 => "GUARD SPEC.",
            0x07 => "X DEFEND",
            0x08 => "X ATTACK",
            0x09 => "BITTER BERRY",
            0x0A => "DIRE HIT",
            0x0B => "X SPECIAL",
            0x0C => "X ACCURACY",
            0x0D => "EON MAIL",
            0x0E => "MORPH MAIL",
            0x0F => "MUSIC MAIL",
            0x10 => "MIRACLEBERRY",
            0x11 => "GOLD BERRY",
            0x12 => "REVIVE",
            0x13 => "GREAT BALL",
            0x14 => "SUPER REPEL",
            0x15 => "MAX REPEL",
            0x16 => "ELIXIR",
            0x17 => "ETHER",
            0x18 => "WATER STONE",
            0x19 => "FIRE STONE",
            0x1A => "LEAF STONE",
            0x1B => "THUNDERSTONE",
            0x1C => "MAX ETHER",
            0x1D => "MAX ELIXIR",
            0x1E => "MAX REVIVE",
            0x1F => "SCOPE LENS",
            0x20 => "HP UP",
            0x21 => "PP UP",
            0x22 => "RARE CANDY",
            0x23 => "BLUESKY MAIL",
            0x24 => "MIRAGE MAIL",
            _ => unreachable!(),
        }
    }

    pub const fn decoration_label(self) -> &'static str {
        match self.0 {
            0x00 => "JIGGLYPUFF DOLL",
            0x01 => "POLIWAG DOLL",
            0x02 => "DIGLETT DOLL",
            0x03 => "STARYU DOLL",
            0x04 => "MAGIKARP DOLL",
            0x05 => "ODDISH DOLL",
            0x06 => "GENGAR DOLL",
            0x07 => "SHELLDER DOLL",
            0x08 => "GRIMER DOLL",
            0x09 => "VOLTORB DOLL",
            0x0A => "CLEFAIRY POSTER",
            0x0B => "JIGGLYPUFF POSTER",
            0x0C => "SUPER NES",
            0x0D => "WEEDLE DOLL",
            0x0E => "GEODUDE DOLL",
            0x0F => "MACHOP DOLL",
            0x10 => "MAGNA PLANT",
            0x11 => "TROPIC PLANT",
            0x12 => "NES",
            0x13 => "NINTENDO 64",
            0x14 => "BULBASAUR DOLL",
            0x15 => "SQUIRTLE DOLL",
            0x16 => "PINK BED",
            0x17 => "POLKADOT BED",
            0x18 => "RED CARPET",
            0x19 => "BLUE CARPET",
            0x1A => "YELLOW CARPET",
            0x1B => "GREEN CARPET",
            0x1C => "JUMBO PLANT",
            0x1D => "VIRTUAL BOY",
            0x1E => "BIG ONIX DOLL",
            0x1F => "PIKACHU POSTER",
            0x20 => "BIG LAPRAS DOLL",
            0x21 => "SURF PIKACHU DOLL",
            0x22 => "PIKACHU BED",
            0x23 => "UNOWN DOLL",
            0x24 => "TENTACOOL DOLL",
            _ => unreachable!(),
        }
    }

    pub const fn label(self, kind: PokemonMysteryGiftKind) -> &'static str {
        match kind {
            PokemonMysteryGiftKind::Item => self.item_label(),
            PokemonMysteryGiftKind::Decoration => self.decoration_label(),
        }
    }

    pub fn next(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|code| *code == self)
            .expect("gift code should be in ALL");
        Self::ALL[(index + 1) % Self::ALL.len()]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PokemonMysteryGiftStatus {
    pub kind: PokemonMysteryGiftKind,
    pub code: PokemonMysteryGiftCode,
    pub resolved_region_code: Option<u8>,
    pub emitter_on: bool,
    pub game_emitter_on: bool,
    pub game_emitter_seen: bool,
    pub completed_exchange: bool,
    pub failed_exchange: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PokemonMysteryGift {
    kind: PokemonMysteryGiftKind,
    code: PokemonMysteryGiftCode,
    protocol: MysteryGiftRoleAProtocol,
}

impl PokemonMysteryGift {
    pub fn new(kind: PokemonMysteryGiftKind, code: PokemonMysteryGiftCode) -> Self {
        Self {
            kind,
            code,
            protocol: MysteryGiftRoleAProtocol::new(
                MysteryGiftRegion::Auto,
                pokemon_mystery_gift_payload(kind, code),
            ),
        }
    }

    pub const fn kind(&self) -> PokemonMysteryGiftKind {
        self.kind
    }

    pub fn set_kind(&mut self, kind: PokemonMysteryGiftKind) {
        if self.kind == kind {
            return;
        }
        self.kind = kind;
        self.protocol
            .set_payload(pokemon_mystery_gift_payload(kind, self.code));
    }

    pub const fn code(&self) -> PokemonMysteryGiftCode {
        self.code
    }

    pub fn set_code(&mut self, code: PokemonMysteryGiftCode) {
        if self.code == code {
            return;
        }
        self.code = code;
        self.protocol
            .set_payload(pokemon_mystery_gift_payload(self.kind, code));
    }

    pub fn status(&self) -> PokemonMysteryGiftStatus {
        let protocol = self.protocol.status();
        PokemonMysteryGiftStatus {
            kind: self.kind,
            code: self.code,
            resolved_region_code: protocol.resolved_region_code,
            emitter_on: protocol.emitter_on,
            game_emitter_on: protocol.game_emitter_on,
            game_emitter_seen: protocol.game_emitter_seen,
            completed_exchange: protocol.completed_exchange,
            failed_exchange: protocol.failed_exchange,
        }
    }

    pub fn tick_t_cycle(&mut self, game_emitter_on: bool) -> bool {
        self.protocol.tick_t_cycle(game_emitter_on)
    }

    #[cfg(test)]
    fn push_test_pulse(&mut self, level: bool, t_cycles: u32) {
        self.protocol.push_test_pulse(level, t_cycles);
    }
}

impl Default for PokemonMysteryGift {
    fn default() -> Self {
        Self::new(
            PokemonMysteryGiftKind::default(),
            PokemonMysteryGiftCode::default(),
        )
    }
}

#[derive(Debug, Clone)]
pub struct PokemonMysteryGiftSession<S = TraceBuffer> {
    scheduler: GlobalScheduler,
    machine: Machine<S>,
    accessory: PokemonMysteryGift,
    context: CycleContext,
    accessory_to_cgb_delay_line: [bool; ACCESSORY_TO_CGB_OPTICAL_DELAY_T_CYCLES],
    delay_cursor: usize,
}

impl<S: TraceSink> PokemonMysteryGiftSession<S> {
    pub fn new(machine: Machine<S>, accessory: PokemonMysteryGift) -> Self {
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

    pub fn pokemon_mystery_gift(&self) -> &PokemonMysteryGift {
        &self.accessory
    }

    pub fn pokemon_mystery_gift_mut(&mut self) -> &mut PokemonMysteryGift {
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

fn pokemon_mystery_gift_payload(
    kind: PokemonMysteryGiftKind,
    code: PokemonMysteryGiftCode,
) -> [u8; MYSTERY_GIFT_PAYLOAD_LEN] {
    let mut payload = [0_u8; MYSTERY_GIFT_PAYLOAD_LEN];
    payload[0] = MYSTERY_GIFT_SINGLE_PAYLOAD_VERSION;
    payload[3..14].copy_from_slice(&GB_CYCLE_NAME);
    payload[15] = kind.gift_type_byte();
    payload[16] = code.value();
    payload[17] = code.value();
    payload
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ConsoleModel, MachineConfig, StartupMode};

    const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;

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
    fn gift_kind_helpers_cover_labels_and_payload_bytes() {
        assert_eq!(
            PokemonMysteryGiftKind::ALL,
            [
                PokemonMysteryGiftKind::Item,
                PokemonMysteryGiftKind::Decoration
            ]
        );
        assert_eq!(PokemonMysteryGiftKind::Item.gift_type_byte(), 0x00);
        assert_eq!(PokemonMysteryGiftKind::Decoration.gift_type_byte(), 0x01);
        assert_eq!(PokemonMysteryGiftKind::Item.label(), "GIFT ITEM");
        assert_eq!(
            PokemonMysteryGiftKind::Decoration.label(),
            "GIFT DECORATION"
        );
        assert_eq!(
            PokemonMysteryGiftKind::Item.next(),
            PokemonMysteryGiftKind::Decoration
        );
        assert_eq!(
            PokemonMysteryGiftKind::Decoration.next(),
            PokemonMysteryGiftKind::Item
        );
    }

    #[test]
    fn gift_code_helpers_cover_full_documented_table_without_ui_codes() {
        assert_eq!(PokemonMysteryGiftCode::ALL.len(), 37);
        assert_eq!(
            PokemonMysteryGiftCode::new(PokemonMysteryGiftCode::MIN),
            Some(PokemonMysteryGiftCode(0x00))
        );
        assert_eq!(
            PokemonMysteryGiftCode::new(PokemonMysteryGiftCode::MAX),
            Some(PokemonMysteryGiftCode(0x24))
        );
        assert_eq!(PokemonMysteryGiftCode::new(0x25), None);
        assert_eq!(
            PokemonMysteryGiftCode(0x24).next(),
            PokemonMysteryGiftCode(0x00)
        );

        assert_eq!(PokemonMysteryGiftCode(0x00).item_label(), "BERRY");
        assert_eq!(PokemonMysteryGiftCode(0x0D).item_label(), "EON MAIL");
        assert_eq!(PokemonMysteryGiftCode(0x24).item_label(), "MIRAGE MAIL");
        assert_eq!(
            PokemonMysteryGiftCode(0x00).decoration_label(),
            "JIGGLYPUFF DOLL"
        );
        assert_eq!(
            PokemonMysteryGiftCode(0x0D).decoration_label(),
            "WEEDLE DOLL"
        );
        assert_eq!(
            PokemonMysteryGiftCode(0x24).decoration_label(),
            "TENTACOOL DOLL"
        );
        assert_eq!(
            PokemonMysteryGiftCode(0x13).label(PokemonMysteryGiftKind::Item),
            "GREAT BALL"
        );
        assert_eq!(
            PokemonMysteryGiftCode(0x13).label(PokemonMysteryGiftKind::Decoration),
            "NINTENDO 64"
        );

        let values: Vec<u8> = PokemonMysteryGiftCode::ALL
            .into_iter()
            .map(PokemonMysteryGiftCode::value)
            .collect();
        assert_eq!(values, (0x00..=0x24).collect::<Vec<_>>());
    }

    #[test]
    fn payload_uses_gb_cycle_identity_and_selected_item_or_decoration() {
        let item_payload = pokemon_mystery_gift_payload(
            PokemonMysteryGiftKind::Item,
            PokemonMysteryGiftCode(0x0D),
        );
        assert_eq!(
            item_payload,
            [
                0x03, 0x00, 0x00, 0x86, 0x81, 0xE3, 0x82, 0x98, 0x82, 0x8B, 0x84, 0x50, 0x50, 0x50,
                0x00, 0x00, 0x0D, 0x0D, 0x00, 0x00,
            ]
        );

        let decoration_payload = pokemon_mystery_gift_payload(
            PokemonMysteryGiftKind::Decoration,
            PokemonMysteryGiftCode(0x0D),
        );
        assert_eq!(decoration_payload[15], 0x01);
        assert_eq!(decoration_payload[16], 0x0D);
        assert_eq!(decoration_payload[17], 0x0D);
    }

    #[test]
    fn checksum_covers_custom_payload_variants() {
        assert_eq!(
            data_block_checksum(&pokemon_mystery_gift_payload(
                PokemonMysteryGiftKind::Item,
                PokemonMysteryGiftCode(0x0D),
            )),
            0x0610
        );
        assert_eq!(
            data_block_checksum(&pokemon_mystery_gift_payload(
                PokemonMysteryGiftKind::Decoration,
                PokemonMysteryGiftCode(0x24),
            )),
            0x063F
        );
    }

    #[test]
    fn public_status_helpers_restart_when_selection_changes() {
        let mut accessory =
            PokemonMysteryGift::new(PokemonMysteryGiftKind::Item, PokemonMysteryGiftCode(0x0D));
        assert_eq!(accessory.kind(), PokemonMysteryGiftKind::Item);
        assert_eq!(accessory.code(), PokemonMysteryGiftCode(0x0D));
        assert_eq!(
            accessory.status(),
            PokemonMysteryGiftStatus {
                kind: PokemonMysteryGiftKind::Item,
                code: PokemonMysteryGiftCode(0x0D),
                resolved_region_code: None,
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
        accessory.set_kind(PokemonMysteryGiftKind::Item);
        assert!(accessory.status().game_emitter_seen);
        accessory.set_kind(PokemonMysteryGiftKind::Decoration);
        assert_eq!(accessory.kind(), PokemonMysteryGiftKind::Decoration);
        assert!(!accessory.status().game_emitter_seen);

        accessory.tick_t_cycle(true);
        assert!(accessory.status().game_emitter_seen);
        accessory.set_code(PokemonMysteryGiftCode(0x0D));
        assert!(accessory.status().game_emitter_seen);
        accessory.set_code(PokemonMysteryGiftCode(0x24));
        assert_eq!(accessory.code(), PokemonMysteryGiftCode(0x24));
        assert!(!accessory.status().game_emitter_seen);
    }

    #[test]
    fn session_routes_accessory_light_to_cgb_sensor_and_clears_it_on_exit() {
        let mut accessor_session = PokemonMysteryGiftSession::new(
            cgb_native_skip_boot_machine(),
            PokemonMysteryGift::default(),
        );
        assert_eq!(
            accessor_session.next_t_cycle(),
            accessor_session.machine().next_t_cycle()
        );
        assert_eq!(
            accessor_session.pokemon_mystery_gift().kind(),
            PokemonMysteryGiftKind::Item
        );
        accessor_session
            .pokemon_mystery_gift_mut()
            .set_kind(PokemonMysteryGiftKind::Decoration);
        assert_eq!(
            accessor_session.pokemon_mystery_gift().kind(),
            PokemonMysteryGiftKind::Decoration
        );
        let _ = accessor_session.step_t_cycle();
        let mut observer = NoopMachineStepObserver;
        let _ = accessor_session.advance_t_cycle_with_observer(&mut observer);

        let mut session = PokemonMysteryGiftSession::new(
            cgb_native_skip_boot_machine(),
            PokemonMysteryGift::default(),
        );
        session.machine_mut().write_bus(0xFF56, 0xC0);
        session
            .pokemon_mystery_gift_mut()
            .push_test_pulse(true, 20_000);

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
