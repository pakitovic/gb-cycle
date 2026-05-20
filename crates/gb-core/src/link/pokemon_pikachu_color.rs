use crate::debugger::{TraceBuffer, TraceSink};
use crate::machine::{Machine, MachineStepObserver, NoopMachineStepObserver};
use crate::scheduler::{CycleContext, GlobalScheduler, SchedulerPhase, TCycle};

#[cfg(test)]
use super::mystery_gift_protocol::data_block_checksum;
use super::mystery_gift_protocol::{
    ACCESSORY_TO_CGB_OPTICAL_DELAY_T_CYCLES, MYSTERY_GIFT_PAYLOAD_LEN,
    MYSTERY_GIFT_SINGLE_PAYLOAD_VERSION, MysteryGiftRegion, MysteryGiftRoleAProtocol,
};

const WESTERN_PIKACHU_NAME: [u8; 11] = [
    0x8F, 0x88, 0x8A, 0x80, 0x82, 0x87, 0x94, 0x50, 0x50, 0x50, 0x50,
];

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

    const fn protocol_region(self) -> MysteryGiftRegion {
        match self {
            Self::Auto => MysteryGiftRegion::Auto,
            Self::Usa => MysteryGiftRegion::Fixed(0x90),
            Self::Esp => MysteryGiftRegion::Fixed(0x96),
            Self::Ita => MysteryGiftRegion::Fixed(0x99),
            Self::Fra => MysteryGiftRegion::Fixed(0x9A),
            Self::Ger => MysteryGiftRegion::Fixed(0x9F),
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
    protocol: MysteryGiftRoleAProtocol,
}

impl PokemonPikachuColor {
    pub fn new(gift: PokemonPikachuColorGift, region: PokemonPikachuColorRegion) -> Self {
        Self {
            gift,
            region,
            protocol: MysteryGiftRoleAProtocol::new(
                region.protocol_region(),
                pokemon_pikachu_color_payload(gift),
            ),
        }
    }

    pub const fn gift(&self) -> PokemonPikachuColorGift {
        self.gift
    }

    pub fn set_gift(&mut self, gift: PokemonPikachuColorGift) {
        if self.gift == gift {
            return;
        }
        self.gift = gift;
        self.protocol
            .set_payload(pokemon_pikachu_color_payload(gift));
    }

    pub const fn region(&self) -> PokemonPikachuColorRegion {
        self.region
    }

    pub fn set_region(&mut self, region: PokemonPikachuColorRegion) {
        if self.region == region {
            return;
        }
        self.region = region;
        self.protocol.set_region(region.protocol_region());
    }

    pub fn status(&self) -> PokemonPikachuColorStatus {
        let protocol = self.protocol.status();
        PokemonPikachuColorStatus {
            gift: self.gift,
            region: self.region,
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

fn pokemon_pikachu_color_payload(gift: PokemonPikachuColorGift) -> [u8; MYSTERY_GIFT_PAYLOAD_LEN] {
    let mut payload = [0_u8; MYSTERY_GIFT_PAYLOAD_LEN];
    payload[0] = MYSTERY_GIFT_SINGLE_PAYLOAD_VERSION;
    payload[3..14].copy_from_slice(&WESTERN_PIKACHU_NAME);
    payload[16] = gift.item_byte();
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
