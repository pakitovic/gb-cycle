pub(in crate::apu) const NR10_ADDRESS: u16 = 0xFF10;
pub(in crate::apu) const NR11_ADDRESS: u16 = 0xFF11;
pub(in crate::apu) const NR12_ADDRESS: u16 = 0xFF12;
pub(in crate::apu) const NR13_ADDRESS: u16 = 0xFF13;
pub(in crate::apu) const NR14_ADDRESS: u16 = 0xFF14;
pub(in crate::apu) const UNUSED_NR15_ADDRESS: u16 = 0xFF15;
pub(in crate::apu) const NR21_ADDRESS: u16 = 0xFF16;
pub(in crate::apu) const NR22_ADDRESS: u16 = 0xFF17;
pub(in crate::apu) const NR23_ADDRESS: u16 = 0xFF18;
pub(in crate::apu) const NR24_ADDRESS: u16 = 0xFF19;
pub(in crate::apu) const NR30_ADDRESS: u16 = 0xFF1A;
pub(in crate::apu) const NR31_ADDRESS: u16 = 0xFF1B;
pub(in crate::apu) const NR32_ADDRESS: u16 = 0xFF1C;
pub(in crate::apu) const NR33_ADDRESS: u16 = 0xFF1D;
pub(in crate::apu) const NR34_ADDRESS: u16 = 0xFF1E;
pub(in crate::apu) const UNUSED_NR1F_ADDRESS: u16 = 0xFF1F;
pub(in crate::apu) const NR41_ADDRESS: u16 = 0xFF20;
pub(in crate::apu) const NR42_ADDRESS: u16 = 0xFF21;
pub(in crate::apu) const NR43_ADDRESS: u16 = 0xFF22;
pub(in crate::apu) const NR44_ADDRESS: u16 = 0xFF23;
pub(in crate::apu) const NR50_ADDRESS: u16 = 0xFF24;
pub(in crate::apu) const NR51_ADDRESS: u16 = 0xFF25;
pub(in crate::apu) const NR52_ADDRESS: u16 = 0xFF26;
pub(in crate::apu) const WAVE_RAM_START_ADDRESS: u16 = 0xFF30;
pub(in crate::apu) const WAVE_RAM_END_ADDRESS: u16 = 0xFF3F;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ApuRegister {
    Nr10,
    Nr11,
    Nr12,
    Nr13,
    Nr14,
    UnusedNr15,
    Nr21,
    Nr22,
    Nr23,
    Nr24,
    Nr30,
    Nr31,
    Nr32,
    Nr33,
    Nr34,
    UnusedNr1f,
    Nr41,
    Nr42,
    Nr43,
    Nr44,
    Nr50,
    Nr51,
    Nr52,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Channel1Register {
    Nr10,
    Nr11,
    Nr12,
    Nr13,
    Nr14,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Channel2Register {
    Nr21,
    Nr22,
    Nr23,
    Nr24,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Channel3Register {
    Nr30,
    Nr31,
    Nr32,
    Nr33,
    Nr34,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Channel4Register {
    Nr41,
    Nr42,
    Nr43,
    Nr44,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MasterRegister {
    Nr50,
    Nr51,
    Nr52,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ApuRegisterOwner {
    Channel1(Channel1Register),
    Channel2(Channel2Register),
    Channel3(Channel3Register),
    Channel4(Channel4Register),
    Master(MasterRegister),
    Unused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ApuMmioRegister {
    Register(ApuRegister),
    WaveRam(usize),
    Unmapped,
}

impl ApuRegister {
    #[cfg(test)]
    pub(super) const fn address(self) -> u16 {
        match self {
            Self::Nr10 => NR10_ADDRESS,
            Self::Nr11 => NR11_ADDRESS,
            Self::Nr12 => NR12_ADDRESS,
            Self::Nr13 => NR13_ADDRESS,
            Self::Nr14 => NR14_ADDRESS,
            Self::UnusedNr15 => UNUSED_NR15_ADDRESS,
            Self::Nr21 => NR21_ADDRESS,
            Self::Nr22 => NR22_ADDRESS,
            Self::Nr23 => NR23_ADDRESS,
            Self::Nr24 => NR24_ADDRESS,
            Self::Nr30 => NR30_ADDRESS,
            Self::Nr31 => NR31_ADDRESS,
            Self::Nr32 => NR32_ADDRESS,
            Self::Nr33 => NR33_ADDRESS,
            Self::Nr34 => NR34_ADDRESS,
            Self::UnusedNr1f => UNUSED_NR1F_ADDRESS,
            Self::Nr41 => NR41_ADDRESS,
            Self::Nr42 => NR42_ADDRESS,
            Self::Nr43 => NR43_ADDRESS,
            Self::Nr44 => NR44_ADDRESS,
            Self::Nr50 => NR50_ADDRESS,
            Self::Nr51 => NR51_ADDRESS,
            Self::Nr52 => NR52_ADDRESS,
        }
    }

    pub(super) const fn owner(self) -> ApuRegisterOwner {
        match self {
            Self::Nr10 => ApuRegisterOwner::Channel1(Channel1Register::Nr10),
            Self::Nr11 => ApuRegisterOwner::Channel1(Channel1Register::Nr11),
            Self::Nr12 => ApuRegisterOwner::Channel1(Channel1Register::Nr12),
            Self::Nr13 => ApuRegisterOwner::Channel1(Channel1Register::Nr13),
            Self::Nr14 => ApuRegisterOwner::Channel1(Channel1Register::Nr14),
            Self::UnusedNr15 => ApuRegisterOwner::Unused,
            Self::Nr21 => ApuRegisterOwner::Channel2(Channel2Register::Nr21),
            Self::Nr22 => ApuRegisterOwner::Channel2(Channel2Register::Nr22),
            Self::Nr23 => ApuRegisterOwner::Channel2(Channel2Register::Nr23),
            Self::Nr24 => ApuRegisterOwner::Channel2(Channel2Register::Nr24),
            Self::Nr30 => ApuRegisterOwner::Channel3(Channel3Register::Nr30),
            Self::Nr31 => ApuRegisterOwner::Channel3(Channel3Register::Nr31),
            Self::Nr32 => ApuRegisterOwner::Channel3(Channel3Register::Nr32),
            Self::Nr33 => ApuRegisterOwner::Channel3(Channel3Register::Nr33),
            Self::Nr34 => ApuRegisterOwner::Channel3(Channel3Register::Nr34),
            Self::UnusedNr1f => ApuRegisterOwner::Unused,
            Self::Nr41 => ApuRegisterOwner::Channel4(Channel4Register::Nr41),
            Self::Nr42 => ApuRegisterOwner::Channel4(Channel4Register::Nr42),
            Self::Nr43 => ApuRegisterOwner::Channel4(Channel4Register::Nr43),
            Self::Nr44 => ApuRegisterOwner::Channel4(Channel4Register::Nr44),
            Self::Nr50 => ApuRegisterOwner::Master(MasterRegister::Nr50),
            Self::Nr51 => ApuRegisterOwner::Master(MasterRegister::Nr51),
            Self::Nr52 => ApuRegisterOwner::Master(MasterRegister::Nr52),
        }
    }
}

impl ApuMmioRegister {
    pub(super) const fn decode(address: u16) -> Self {
        match address {
            NR10_ADDRESS => Self::Register(ApuRegister::Nr10),
            NR11_ADDRESS => Self::Register(ApuRegister::Nr11),
            NR12_ADDRESS => Self::Register(ApuRegister::Nr12),
            NR13_ADDRESS => Self::Register(ApuRegister::Nr13),
            NR14_ADDRESS => Self::Register(ApuRegister::Nr14),
            UNUSED_NR15_ADDRESS => Self::Register(ApuRegister::UnusedNr15),
            NR21_ADDRESS => Self::Register(ApuRegister::Nr21),
            NR22_ADDRESS => Self::Register(ApuRegister::Nr22),
            NR23_ADDRESS => Self::Register(ApuRegister::Nr23),
            NR24_ADDRESS => Self::Register(ApuRegister::Nr24),
            NR30_ADDRESS => Self::Register(ApuRegister::Nr30),
            NR31_ADDRESS => Self::Register(ApuRegister::Nr31),
            NR32_ADDRESS => Self::Register(ApuRegister::Nr32),
            NR33_ADDRESS => Self::Register(ApuRegister::Nr33),
            NR34_ADDRESS => Self::Register(ApuRegister::Nr34),
            UNUSED_NR1F_ADDRESS => Self::Register(ApuRegister::UnusedNr1f),
            NR41_ADDRESS => Self::Register(ApuRegister::Nr41),
            NR42_ADDRESS => Self::Register(ApuRegister::Nr42),
            NR43_ADDRESS => Self::Register(ApuRegister::Nr43),
            NR44_ADDRESS => Self::Register(ApuRegister::Nr44),
            NR50_ADDRESS => Self::Register(ApuRegister::Nr50),
            NR51_ADDRESS => Self::Register(ApuRegister::Nr51),
            NR52_ADDRESS => Self::Register(ApuRegister::Nr52),
            WAVE_RAM_START_ADDRESS..=WAVE_RAM_END_ADDRESS => {
                Self::WaveRam((address - WAVE_RAM_START_ADDRESS) as usize)
            }
            _ => Self::Unmapped,
        }
    }

    pub(super) const fn should_observe_register_write(self) -> bool {
        matches!(self, Self::Register(_))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ApuMmioRegister, ApuRegister, ApuRegisterOwner, Channel1Register, Channel2Register,
        Channel3Register, Channel4Register, MasterRegister, NR10_ADDRESS, NR52_ADDRESS,
        UNUSED_NR1F_ADDRESS, WAVE_RAM_END_ADDRESS, WAVE_RAM_START_ADDRESS,
    };

    #[test]
    fn decode_distinguishes_apu_registers_wave_ram_and_unmapped_addresses() {
        assert_eq!(
            ApuMmioRegister::decode(NR10_ADDRESS),
            ApuMmioRegister::Register(ApuRegister::Nr10)
        );
        assert_eq!(
            ApuMmioRegister::decode(UNUSED_NR1F_ADDRESS),
            ApuMmioRegister::Register(ApuRegister::UnusedNr1f)
        );
        assert_eq!(
            ApuMmioRegister::decode(WAVE_RAM_START_ADDRESS),
            ApuMmioRegister::WaveRam(0)
        );
        assert_eq!(
            ApuMmioRegister::decode(WAVE_RAM_END_ADDRESS),
            ApuMmioRegister::WaveRam(0x0F)
        );
        assert_eq!(ApuMmioRegister::decode(0xFF27), ApuMmioRegister::Unmapped);
    }

    #[test]
    fn observation_policy_covers_only_the_ff10_ff26_register_window() {
        assert!(ApuMmioRegister::decode(NR52_ADDRESS).should_observe_register_write());
        assert!(ApuMmioRegister::decode(0xFF15).should_observe_register_write());
        assert!(!ApuMmioRegister::decode(0xFF27).should_observe_register_write());
        assert!(!ApuMmioRegister::decode(WAVE_RAM_START_ADDRESS).should_observe_register_write());
    }

    #[test]
    fn register_addresses_round_trip_through_decode() {
        let registers = [
            ApuRegister::Nr10,
            ApuRegister::Nr11,
            ApuRegister::Nr12,
            ApuRegister::Nr13,
            ApuRegister::Nr14,
            ApuRegister::UnusedNr15,
            ApuRegister::Nr21,
            ApuRegister::Nr22,
            ApuRegister::Nr23,
            ApuRegister::Nr24,
            ApuRegister::Nr30,
            ApuRegister::Nr31,
            ApuRegister::Nr32,
            ApuRegister::Nr33,
            ApuRegister::Nr34,
            ApuRegister::UnusedNr1f,
            ApuRegister::Nr41,
            ApuRegister::Nr42,
            ApuRegister::Nr43,
            ApuRegister::Nr44,
            ApuRegister::Nr50,
            ApuRegister::Nr51,
            ApuRegister::Nr52,
        ];

        for register in registers {
            assert_eq!(
                ApuMmioRegister::decode(register.address()),
                ApuMmioRegister::Register(register)
            );
        }
    }

    #[test]
    fn register_owner_routes_each_register_block_explicitly() {
        assert_eq!(
            ApuRegister::Nr10.owner(),
            ApuRegisterOwner::Channel1(Channel1Register::Nr10)
        );
        assert_eq!(
            ApuRegister::Nr23.owner(),
            ApuRegisterOwner::Channel2(Channel2Register::Nr23)
        );
        assert_eq!(
            ApuRegister::Nr34.owner(),
            ApuRegisterOwner::Channel3(Channel3Register::Nr34)
        );
        assert_eq!(
            ApuRegister::Nr42.owner(),
            ApuRegisterOwner::Channel4(Channel4Register::Nr42)
        );
        assert_eq!(
            ApuRegister::Nr52.owner(),
            ApuRegisterOwner::Master(MasterRegister::Nr52)
        );
        assert_eq!(ApuRegister::UnusedNr15.owner(), ApuRegisterOwner::Unused);
        assert_eq!(ApuRegister::UnusedNr1f.owner(), ApuRegisterOwner::Unused);
    }
}
