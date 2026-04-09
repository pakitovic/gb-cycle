#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct FrameSequencerState {
    pub(super) step: u8,
    pub(super) length_clock_count: u64,
    pub(super) sweep_clock_count: u64,
    pub(super) envelope_clock_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct FrameSequencerClocks {
    pub(super) length: bool,
    pub(super) sweep: bool,
    pub(super) envelope: bool,
}

pub(crate) const fn div_apu_phase_from_system_counter(system_counter: u16) -> u8 {
    ((system_counter >> 13) & 0x07) as u8
}

impl FrameSequencerState {
    pub(super) fn apply_startup_phase(&mut self, div_apu: u8) {
        self.step = div_apu & 0x07;
        self.length_clock_count = 0;
        self.sweep_clock_count = 0;
        self.envelope_clock_count = 0;
    }

    pub(super) fn advance(&mut self) -> FrameSequencerClocks {
        let clocks = match self.step {
            0 => FrameSequencerClocks {
                length: true,
                ..FrameSequencerClocks::default()
            },
            2 | 6 => FrameSequencerClocks {
                length: true,
                sweep: true,
                ..FrameSequencerClocks::default()
            },
            4 => FrameSequencerClocks {
                length: true,
                ..FrameSequencerClocks::default()
            },
            7 => FrameSequencerClocks {
                envelope: true,
                ..FrameSequencerClocks::default()
            },
            _ => FrameSequencerClocks::default(),
        };

        if clocks.length {
            self.length_clock_count += 1;
        }
        if clocks.sweep {
            self.sweep_clock_count += 1;
        }
        if clocks.envelope {
            self.envelope_clock_count += 1;
        }

        self.step = (self.step + 1) & 0x07;
        clocks
    }
}
