use crate::player_slots::{DesktopDmg07PlayerCount, PlayerSlot};
#[cfg(test)]
use gb_core::LinkedTopologyKind;
use gb_core::{
    DEFAULT_CGB_IR_OPTICAL_PROPAGATION_DELAY_T_CYCLES, Dmg07Participant, Dmg07Port, LinkedMachines,
    LinkedMachinesError, MAX_CGB_IR_OPTICAL_PROPAGATION_DELAY_T_CYCLES,
    MIN_CGB_IR_OPTICAL_PROPAGATION_DELAY_T_CYCLES, Machine, MachineStepObserver,
    PokemonMysteryGift, PokemonMysteryGiftCode, PokemonMysteryGiftKind, PokemonMysteryGiftSession,
    PokemonMysteryGiftStatus, PokemonPikachuColor, PokemonPikachuColorGift,
    PokemonPikachuColorRegion, PokemonPikachuColorSession, PokemonPikachuColorStatus,
    TraceSummaryBuffer,
};
use std::ops::{Deref, DerefMut};

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopEmulationSessionKind {
    Single,
    LinkedDmg04TwoPlayer,
    LinkedCgbInfraredTwoPlayer,
    PokemonPikachuColor,
    PokemonMysteryGift,
    LinkedDmg07 {
        player_count: DesktopDmg07PlayerCount,
    },
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone)]
pub enum DesktopEmulationSession {
    Transitioning,
    Single(Box<Machine<TraceSummaryBuffer>>),
    LinkedDmg04TwoPlayer(Box<LinkedMachines<TraceSummaryBuffer>>),
    LinkedCgbInfraredTwoPlayer(Box<LinkedMachines<TraceSummaryBuffer>>),
    PokemonPikachuColor(Box<PokemonPikachuColorSession<TraceSummaryBuffer>>),
    PokemonMysteryGift(Box<PokemonMysteryGiftSession<TraceSummaryBuffer>>),
    LinkedDmg07 {
        linked: Box<LinkedMachines<TraceSummaryBuffer>>,
        player_count: DesktopDmg07PlayerCount,
    },
}

impl DesktopEmulationSession {
    pub fn new_single(machine: Machine<TraceSummaryBuffer>) -> Self {
        Self::Single(Box::new(machine))
    }

    pub fn new_pokemon_pikachu_color(
        machine: Machine<TraceSummaryBuffer>,
        gift: PokemonPikachuColorGift,
        region: PokemonPikachuColorRegion,
    ) -> Self {
        Self::PokemonPikachuColor(Box::new(PokemonPikachuColorSession::new(
            machine,
            PokemonPikachuColor::new(gift, region),
        )))
    }

    pub fn new_pokemon_mystery_gift(
        machine: Machine<TraceSummaryBuffer>,
        kind: PokemonMysteryGiftKind,
        code: PokemonMysteryGiftCode,
    ) -> Self {
        Self::PokemonMysteryGift(Box::new(PokemonMysteryGiftSession::new(
            machine,
            PokemonMysteryGift::new(kind, code),
        )))
    }

    pub fn new_linked_dmg04_two_player(
        primary_machine: Machine<TraceSummaryBuffer>,
        secondary_machine: Machine<TraceSummaryBuffer>,
    ) -> Result<Self, String> {
        Self::new_linked_dmg04_two_player_from_machines(primary_machine, secondary_machine)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new_linked_cgb_infrared_two_player(
        primary_machine: Machine<TraceSummaryBuffer>,
        secondary_machine: Machine<TraceSummaryBuffer>,
    ) -> Result<Self, String> {
        Self::new_linked_cgb_infrared_two_player_with_optical_delay(
            primary_machine,
            secondary_machine,
            DEFAULT_CGB_IR_OPTICAL_PROPAGATION_DELAY_T_CYCLES,
        )
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new_linked_cgb_infrared_two_player_with_optical_delay(
        primary_machine: Machine<TraceSummaryBuffer>,
        secondary_machine: Machine<TraceSummaryBuffer>,
        optical_propagation_delay_t_cycles: usize,
    ) -> Result<Self, String> {
        Self::new_linked_cgb_infrared_two_player_from_machines(
            primary_machine,
            secondary_machine,
            optical_propagation_delay_t_cycles,
        )
    }

    fn new_linked_dmg04_two_player_from_machines(
        primary_machine: Machine<TraceSummaryBuffer>,
        secondary_machine: Machine<TraceSummaryBuffer>,
    ) -> Result<Self, String> {
        let mut linked = LinkedMachines::new(linked_machine_pair_from_values(
            primary_machine,
            secondary_machine,
        ))
        .map_err(format_linked_machines_error)?;
        linked
            .attach_dmg04_cable()
            .map_err(format_linked_machines_error)?;
        Ok(Self::LinkedDmg04TwoPlayer(Box::new(linked)))
    }

    fn new_linked_dmg04_two_player_from_primary_box(
        primary_machine: Box<Machine<TraceSummaryBuffer>>,
        secondary_machine: Machine<TraceSummaryBuffer>,
    ) -> Result<Self, String> {
        let mut linked = LinkedMachines::new(linked_machine_pair_from_primary_box(
            primary_machine,
            secondary_machine,
        ))
        .map_err(format_linked_machines_error)?;
        linked
            .attach_dmg04_cable()
            .map_err(format_linked_machines_error)?;
        Ok(Self::LinkedDmg04TwoPlayer(Box::new(linked)))
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn new_linked_cgb_infrared_two_player_from_machines(
        primary_machine: Machine<TraceSummaryBuffer>,
        secondary_machine: Machine<TraceSummaryBuffer>,
        optical_propagation_delay_t_cycles: usize,
    ) -> Result<Self, String> {
        Self::new_linked_cgb_infrared_two_player_from_vec(
            linked_machine_pair_from_values(primary_machine, secondary_machine),
            optical_propagation_delay_t_cycles,
        )
    }

    fn new_linked_cgb_infrared_two_player_from_primary_box(
        primary_machine: Box<Machine<TraceSummaryBuffer>>,
        secondary_machine: Machine<TraceSummaryBuffer>,
        optical_propagation_delay_t_cycles: usize,
    ) -> Result<Self, String> {
        Self::new_linked_cgb_infrared_two_player_from_vec(
            linked_machine_pair_from_primary_box(primary_machine, secondary_machine),
            optical_propagation_delay_t_cycles,
        )
    }

    fn new_linked_cgb_infrared_two_player_from_vec(
        machines: Vec<Machine<TraceSummaryBuffer>>,
        optical_propagation_delay_t_cycles: usize,
    ) -> Result<Self, String> {
        let mut linked = LinkedMachines::new(machines).map_err(format_linked_machines_error)?;
        linked
            .attach_cgb_infrared_pair_with_optical_propagation_delay(
                optical_propagation_delay_t_cycles,
            )
            .map_err(format_linked_machines_error)?;
        Ok(Self::LinkedCgbInfraredTwoPlayer(Box::new(linked)))
    }

    pub fn new_linked_dmg07(
        machines: Vec<Machine<TraceSummaryBuffer>>,
        player_count: DesktopDmg07PlayerCount,
    ) -> Result<Self, String> {
        if machines.len() != player_count.get() {
            return Err(format!(
                "DMG-07 desktop session for {} players requires {} machines, found {}",
                player_count.get(),
                player_count.get(),
                machines.len()
            ));
        }

        let mut linked = LinkedMachines::new(machines).map_err(format_linked_machines_error)?;
        let participants = dmg07_participants_for_player_count(player_count);
        linked
            .attach_dmg07_adapter(&participants)
            .map_err(format_linked_machines_error)?;
        Ok(Self::LinkedDmg07 {
            linked: Box::new(linked),
            player_count,
        })
    }

    #[cfg(test)]
    pub fn kind(&self) -> DesktopEmulationSessionKind {
        match self {
            Self::Transitioning => {
                unreachable!("desktop emulation session should not be observed while transitioning")
            }
            Self::Single(_) => DesktopEmulationSessionKind::Single,
            Self::LinkedDmg04TwoPlayer(_) => DesktopEmulationSessionKind::LinkedDmg04TwoPlayer,
            Self::LinkedCgbInfraredTwoPlayer(_) => {
                DesktopEmulationSessionKind::LinkedCgbInfraredTwoPlayer
            }
            Self::PokemonPikachuColor(_) => DesktopEmulationSessionKind::PokemonPikachuColor,
            Self::PokemonMysteryGift(_) => DesktopEmulationSessionKind::PokemonMysteryGift,
            Self::LinkedDmg07 { player_count, .. } => DesktopEmulationSessionKind::LinkedDmg07 {
                player_count: *player_count,
            },
        }
    }

    #[cfg(test)]
    pub fn linked_topology_kind(&self) -> LinkedTopologyKind {
        match self {
            Self::Transitioning => LinkedTopologyKind::None,
            Self::Single(_) => LinkedTopologyKind::None,
            Self::LinkedDmg04TwoPlayer(linked) => linked.topology_kind(),
            Self::LinkedCgbInfraredTwoPlayer(linked) => linked.topology_kind(),
            Self::PokemonPikachuColor(_) => LinkedTopologyKind::None,
            Self::PokemonMysteryGift(_) => LinkedTopologyKind::None,
            Self::LinkedDmg07 { linked, .. } => linked.topology_kind(),
        }
    }

    pub fn primary_machine(&self) -> &Machine<TraceSummaryBuffer> {
        match self {
            Self::Transitioning => {
                unreachable!("desktop emulation session should not be observed while transitioning")
            }
            Self::Single(machine) => machine,
            Self::LinkedDmg04TwoPlayer(linked) => linked
                .machine(0)
                .expect("linked desktop session should always have a primary machine"),
            Self::LinkedCgbInfraredTwoPlayer(linked) => linked
                .machine(0)
                .expect("linked desktop session should always have a primary machine"),
            Self::PokemonPikachuColor(session) => session.machine(),
            Self::PokemonMysteryGift(session) => session.machine(),
            Self::LinkedDmg07 { linked, .. } => linked
                .machine(0)
                .expect("linked desktop session should always have a primary machine"),
        }
    }

    pub fn primary_machine_mut(&mut self) -> &mut Machine<TraceSummaryBuffer> {
        match self {
            Self::Transitioning => {
                unreachable!("desktop emulation session should not be observed while transitioning")
            }
            Self::Single(machine) => machine,
            Self::LinkedDmg04TwoPlayer(linked) => linked
                .machine_mut(0)
                .expect("linked desktop session should always have a primary machine"),
            Self::LinkedCgbInfraredTwoPlayer(linked) => linked
                .machine_mut(0)
                .expect("linked desktop session should always have a primary machine"),
            Self::PokemonPikachuColor(session) => session.machine_mut(),
            Self::PokemonMysteryGift(session) => session.machine_mut(),
            Self::LinkedDmg07 { linked, .. } => linked
                .machine_mut(0)
                .expect("linked desktop session should always have a primary machine"),
        }
    }

    pub fn secondary_machine(&self) -> Option<&Machine<TraceSummaryBuffer>> {
        match self {
            Self::Transitioning => None,
            Self::Single(_) => None,
            Self::PokemonPikachuColor(_) => None,
            Self::PokemonMysteryGift(_) => None,
            Self::LinkedDmg04TwoPlayer(linked) => linked.machine(1),
            Self::LinkedCgbInfraredTwoPlayer(linked) => linked.machine(1),
            Self::LinkedDmg07 { linked, .. } => linked.machine(1),
        }
    }

    pub fn secondary_machine_mut(&mut self) -> Option<&mut Machine<TraceSummaryBuffer>> {
        match self {
            Self::Transitioning => None,
            Self::Single(_) => None,
            Self::PokemonPikachuColor(_) => None,
            Self::PokemonMysteryGift(_) => None,
            Self::LinkedDmg04TwoPlayer(linked) => linked.machine_mut(1),
            Self::LinkedCgbInfraredTwoPlayer(linked) => linked.machine_mut(1),
            Self::LinkedDmg07 { linked, .. } => linked.machine_mut(1),
        }
    }

    pub fn machine_for_player_slot(
        &self,
        slot: PlayerSlot,
    ) -> Option<&Machine<TraceSummaryBuffer>> {
        match slot {
            PlayerSlot::P1 => Some(self.primary_machine()),
            PlayerSlot::P2 => self.secondary_machine(),
            PlayerSlot::P3 | PlayerSlot::P4 => match self {
                Self::LinkedDmg07 { linked, .. } => linked.machine(slot.machine_index()),
                Self::Transitioning
                | Self::Single(_)
                | Self::PokemonPikachuColor(_)
                | Self::PokemonMysteryGift(_)
                | Self::LinkedDmg04TwoPlayer(_)
                | Self::LinkedCgbInfraredTwoPlayer(_) => None,
            },
        }
    }

    pub fn machine_for_player_slot_mut(
        &mut self,
        slot: PlayerSlot,
    ) -> Option<&mut Machine<TraceSummaryBuffer>> {
        match slot {
            PlayerSlot::P1 => Some(self.primary_machine_mut()),
            PlayerSlot::P2 => self.secondary_machine_mut(),
            PlayerSlot::P3 | PlayerSlot::P4 => match self {
                Self::LinkedDmg07 { linked, .. } => linked.machine_mut(slot.machine_index()),
                Self::Transitioning
                | Self::Single(_)
                | Self::PokemonPikachuColor(_)
                | Self::PokemonMysteryGift(_)
                | Self::LinkedDmg04TwoPlayer(_)
                | Self::LinkedCgbInfraredTwoPlayer(_) => None,
            },
        }
    }

    pub const fn is_linked_dmg04_two_player(&self) -> bool {
        matches!(self, Self::LinkedDmg04TwoPlayer(_))
    }

    pub const fn is_linked_cgb_infrared_two_player(&self) -> bool {
        matches!(self, Self::LinkedCgbInfraredTwoPlayer(_))
    }

    pub const fn is_pokemon_pikachu_color(&self) -> bool {
        matches!(self, Self::PokemonPikachuColor(_))
    }

    pub const fn is_pokemon_mystery_gift(&self) -> bool {
        matches!(self, Self::PokemonMysteryGift(_))
    }

    pub fn set_pokemon_pikachu_color_gift(&mut self, gift: PokemonPikachuColorGift) {
        if let Self::PokemonPikachuColor(session) = self {
            session.pokemon_pikachu_color_mut().set_gift(gift);
        }
    }

    pub fn pokemon_pikachu_color_status(&self) -> Option<PokemonPikachuColorStatus> {
        match self {
            Self::PokemonPikachuColor(session) => Some(session.pokemon_pikachu_color().status()),
            _ => None,
        }
    }

    pub fn set_pokemon_mystery_gift_kind(&mut self, kind: PokemonMysteryGiftKind) {
        if let Self::PokemonMysteryGift(session) = self {
            session.pokemon_mystery_gift_mut().set_kind(kind);
        }
    }

    pub fn set_pokemon_mystery_gift_code(&mut self, code: PokemonMysteryGiftCode) {
        if let Self::PokemonMysteryGift(session) = self {
            session.pokemon_mystery_gift_mut().set_code(code);
        }
    }

    pub fn pokemon_mystery_gift_status(&self) -> Option<PokemonMysteryGiftStatus> {
        match self {
            Self::PokemonMysteryGift(session) => Some(session.pokemon_mystery_gift().status()),
            _ => None,
        }
    }

    pub const fn dmg07_player_count(&self) -> Option<DesktopDmg07PlayerCount> {
        match self {
            Self::LinkedDmg07 { player_count, .. } => Some(*player_count),
            Self::Transitioning
            | Self::Single(_)
            | Self::PokemonPikachuColor(_)
            | Self::PokemonMysteryGift(_)
            | Self::LinkedDmg04TwoPlayer(_)
            | Self::LinkedCgbInfraredTwoPlayer(_) => None,
        }
    }

    pub const fn is_linked_dmg07(&self) -> bool {
        matches!(self, Self::LinkedDmg07 { .. })
    }

    pub fn attach_secondary_dmg04(
        &mut self,
        secondary_machine: Machine<TraceSummaryBuffer>,
    ) -> Result<(), String> {
        let expected = match self {
            Self::Transitioning => {
                return Err("desktop emulation session is already transitioning".to_string());
            }
            Self::Single(machine) => machine.next_t_cycle(),
            Self::LinkedDmg04TwoPlayer(_) => {
                return Err(
                    "desktop emulation session is already running a linked DMG-04 runtime"
                        .to_string(),
                );
            }
            Self::LinkedDmg07 { .. } => {
                return Err(
                    "desktop emulation session is already running a linked DMG-07 runtime"
                        .to_string(),
                );
            }
            Self::LinkedCgbInfraredTwoPlayer(_) => {
                return Err(
                    "desktop emulation session is already running a linked CGB IR runtime"
                        .to_string(),
                );
            }
            Self::PokemonPikachuColor(_) => {
                return Err(
                    "desktop emulation session is already running a Pokemon Pikachu Color runtime"
                        .to_string(),
                );
            }
            Self::PokemonMysteryGift(_) => {
                return Err(
                    "desktop emulation session is already running a Pokemon Mystery Gift runtime"
                        .to_string(),
                );
            }
        };
        let found = secondary_machine.next_t_cycle();
        if found != expected {
            return Err(format_linked_machines_error(
                LinkedMachinesError::MismatchedNextTCycle {
                    expected,
                    found,
                    machine_index: 1,
                },
            ));
        }

        let current_session = std::mem::replace(self, Self::Transitioning);
        let Self::Single(primary_machine) = current_session else {
            unreachable!("linked desktop session should have been rejected before replacement");
        };

        let next_session =
            Self::new_linked_dmg04_two_player_from_primary_box(primary_machine, secondary_machine)
                .expect("validated desktop DMG-04 session should build successfully");
        *self = next_session;
        Ok(())
    }

    pub fn attach_secondary_cgb_infrared(
        &mut self,
        secondary_machine: Machine<TraceSummaryBuffer>,
    ) -> Result<(), String> {
        self.attach_secondary_cgb_infrared_with_optical_delay(
            secondary_machine,
            DEFAULT_CGB_IR_OPTICAL_PROPAGATION_DELAY_T_CYCLES,
        )
    }

    pub fn attach_secondary_cgb_infrared_with_optical_delay(
        &mut self,
        secondary_machine: Machine<TraceSummaryBuffer>,
        optical_propagation_delay_t_cycles: usize,
    ) -> Result<(), String> {
        validate_cgb_ir_optical_propagation_delay_t_cycles(optical_propagation_delay_t_cycles)?;

        let expected = match self {
            Self::Transitioning => {
                return Err("desktop emulation session is already transitioning".to_string());
            }
            Self::Single(machine) => machine.next_t_cycle(),
            Self::LinkedDmg04TwoPlayer(_) => {
                return Err(
                    "desktop emulation session is already running a linked DMG-04 runtime"
                        .to_string(),
                );
            }
            Self::LinkedDmg07 { .. } => {
                return Err(
                    "desktop emulation session is already running a linked DMG-07 runtime"
                        .to_string(),
                );
            }
            Self::LinkedCgbInfraredTwoPlayer(_) => {
                return Err(
                    "desktop emulation session is already running a linked CGB IR runtime"
                        .to_string(),
                );
            }
            Self::PokemonPikachuColor(_) => {
                return Err(
                    "desktop emulation session is already running a Pokemon Pikachu Color runtime"
                        .to_string(),
                );
            }
            Self::PokemonMysteryGift(_) => {
                return Err(
                    "desktop emulation session is already running a Pokemon Mystery Gift runtime"
                        .to_string(),
                );
            }
        };
        let found = secondary_machine.next_t_cycle();
        if found != expected {
            return Err(format_linked_machines_error(
                LinkedMachinesError::MismatchedNextTCycle {
                    expected,
                    found,
                    machine_index: 1,
                },
            ));
        }

        let current_session = std::mem::replace(self, Self::Transitioning);
        let Self::Single(primary_machine) = current_session else {
            unreachable!("linked desktop session should have been rejected before replacement");
        };

        let next_session = Self::new_linked_cgb_infrared_two_player_from_primary_box(
            primary_machine,
            secondary_machine,
            optical_propagation_delay_t_cycles,
        )
        .expect("validated desktop CGB IR session should build successfully");
        *self = next_session;
        Ok(())
    }

    pub fn detach_to_single_primary(&mut self) {
        if matches!(self, Self::Transitioning | Self::Single(_)) {
            return;
        }

        let linked_session = std::mem::replace(self, Self::Transitioning);
        *self = Self::new_single(linked_session.into_primary_machine());
    }

    pub fn step_t_cycle(&mut self) {
        match self {
            Self::Transitioning => {
                unreachable!("desktop emulation session should not be stepped while transitioning")
            }
            Self::Single(machine) => {
                let _ = machine.step_t_cycle();
            }
            Self::LinkedDmg04TwoPlayer(linked) => {
                linked.advance_t_cycle();
            }
            Self::LinkedCgbInfraredTwoPlayer(linked) => {
                linked.advance_t_cycle();
            }
            Self::PokemonPikachuColor(session) => {
                session.advance_t_cycle();
            }
            Self::PokemonMysteryGift(session) => {
                session.advance_t_cycle();
            }
            Self::LinkedDmg07 { linked, .. } => {
                linked.advance_t_cycle();
            }
        }
    }

    pub fn step_t_cycle_with_observer<O: MachineStepObserver>(&mut self, observer: &mut O) {
        match self {
            Self::Transitioning => {
                unreachable!("desktop emulation session should not be stepped while transitioning")
            }
            Self::Single(machine) => {
                let _ = machine.step_t_cycle_with_observer(observer);
            }
            Self::LinkedDmg04TwoPlayer(linked) => {
                linked.advance_t_cycle_with_observer(observer);
            }
            Self::LinkedCgbInfraredTwoPlayer(linked) => {
                linked.advance_t_cycle_with_observer(observer);
            }
            Self::PokemonPikachuColor(session) => {
                session.advance_t_cycle_with_observer(observer);
            }
            Self::PokemonMysteryGift(session) => {
                session.advance_t_cycle_with_observer(observer);
            }
            Self::LinkedDmg07 { linked, .. } => {
                linked.advance_t_cycle_with_observer(observer);
            }
        }
    }

    pub fn into_primary_machine(self) -> Machine<TraceSummaryBuffer> {
        match self {
            Self::Transitioning => {
                unreachable!("desktop emulation session should not be consumed while transitioning")
            }
            Self::Single(machine) => *machine,
            Self::LinkedDmg04TwoPlayer(mut linked) => {
                linked.detach_link_topology();
                linked
                    .into_machines()
                    .into_iter()
                    .next()
                    .expect("linked desktop session should keep the primary machine")
            }
            Self::LinkedCgbInfraredTwoPlayer(mut linked) => {
                linked.detach_link_topology();
                linked
                    .into_machines()
                    .into_iter()
                    .next()
                    .expect("linked desktop session should keep the primary machine")
            }
            Self::PokemonPikachuColor(session) => session.into_machine(),
            Self::PokemonMysteryGift(session) => session.into_machine(),
            Self::LinkedDmg07 { mut linked, .. } => {
                linked.detach_link_topology();
                linked
                    .into_machines()
                    .into_iter()
                    .next()
                    .expect("linked desktop session should keep the primary machine")
            }
        }
    }
}

fn dmg07_participants_for_player_count(
    player_count: DesktopDmg07PlayerCount,
) -> Vec<Dmg07Participant> {
    Dmg07Port::ALL
        .into_iter()
        .take(player_count.get())
        .enumerate()
        .map(|(machine_index, port)| Dmg07Participant::new(machine_index, port))
        .collect()
}

impl Deref for DesktopEmulationSession {
    type Target = Machine<TraceSummaryBuffer>;

    fn deref(&self) -> &Self::Target {
        self.primary_machine()
    }
}

impl DerefMut for DesktopEmulationSession {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.primary_machine_mut()
    }
}

fn format_linked_machines_error(error: LinkedMachinesError) -> String {
    match error {
        LinkedMachinesError::TooFewMachines { count } => {
            format!("linked desktop session requires at least two machines, found {count}")
        }
        LinkedMachinesError::MismatchedNextTCycle {
            expected,
            found,
            machine_index,
        } => format!(
            "linked desktop session machines must share the same next T-cycle; expected {expected:?}, found {found:?} at machine index {machine_index}"
        ),
        LinkedMachinesError::UnsupportedMachineCountForDmg04 { count } => {
            format!("DMG-04 desktop sessions currently require exactly two machines, found {count}")
        }
        LinkedMachinesError::UnsupportedMachineCountForDmg07 { count } => {
            format!("DMG-07 linked sessions require two to four machines, found {count}")
        }
        LinkedMachinesError::UnsupportedExternalPortAttachment {
            machine_index,
            attachment_kind,
        } => format!(
            "linked session cannot attach {attachment_kind:?} to machine index {machine_index}"
        ),
        LinkedMachinesError::UnsupportedMachineCountForCgbInfrared { count } => {
            format!("CGB infrared linked sessions require exactly two machines, found {count}")
        }
        LinkedMachinesError::InvalidCgbInfraredOpticalPropagationDelay {
            requested_t_cycles,
            min_t_cycles,
            max_t_cycles,
        } => format!(
            "CGB infrared optical propagation delay must be between {min_t_cycles} and {max_t_cycles} T-cycles, got {requested_t_cycles}"
        ),
        LinkedMachinesError::MissingDmg07PlayerOne => {
            "DMG-07 linked sessions require adapter port P1".to_string()
        }
        LinkedMachinesError::DuplicateDmg07Port { port } => {
            format!("DMG-07 linked session uses adapter port {port:?} more than once")
        }
        LinkedMachinesError::DuplicateDmg07MachineIndex { machine_index } => {
            format!("DMG-07 linked session uses machine index {machine_index} more than once")
        }
        LinkedMachinesError::Dmg07MachineIndexOutOfBounds {
            machine_index,
            machine_count,
        } => format!(
            "DMG-07 linked session references machine index {machine_index}, but only {machine_count} machines exist"
        ),
    }
}

fn validate_cgb_ir_optical_propagation_delay_t_cycles(
    requested_t_cycles: usize,
) -> Result<(), String> {
    if (MIN_CGB_IR_OPTICAL_PROPAGATION_DELAY_T_CYCLES
        ..=MAX_CGB_IR_OPTICAL_PROPAGATION_DELAY_T_CYCLES)
        .contains(&requested_t_cycles)
    {
        Ok(())
    } else {
        Err(format_linked_machines_error(
            LinkedMachinesError::InvalidCgbInfraredOpticalPropagationDelay {
                requested_t_cycles,
                min_t_cycles: MIN_CGB_IR_OPTICAL_PROPAGATION_DELAY_T_CYCLES,
                max_t_cycles: MAX_CGB_IR_OPTICAL_PROPAGATION_DELAY_T_CYCLES,
            },
        ))
    }
}

fn linked_machine_pair_from_values(
    primary_machine: Machine<TraceSummaryBuffer>,
    secondary_machine: Machine<TraceSummaryBuffer>,
) -> Vec<Machine<TraceSummaryBuffer>> {
    // Do not use `vec![primary_machine, secondary_machine]` here. The macro can materialize a large temporary pair on the test stack under coverage-instrumented builds before moving the machines into the heap-backed vector.
    #[allow(clippy::vec_init_then_push)]
    {
        let mut machines = Vec::with_capacity(2);
        machines.push(primary_machine);
        machines.push(secondary_machine);
        machines
    }
}

fn linked_machine_pair_from_primary_box(
    primary_machine: Box<Machine<TraceSummaryBuffer>>,
    secondary_machine: Machine<TraceSummaryBuffer>,
) -> Vec<Machine<TraceSummaryBuffer>> {
    // Keep the primary machine boxed until the vector allocation exists so session swaps do not need a placeholder machine or a stack-backed pair.
    #[allow(clippy::vec_init_then_push)]
    {
        let mut machines = Vec::with_capacity(2);
        machines.push(*primary_machine);
        machines.push(secondary_machine);
        machines
    }
}

#[cfg(test)]
mod test;
