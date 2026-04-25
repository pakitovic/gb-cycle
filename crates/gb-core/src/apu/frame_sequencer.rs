const FRAME_SEQUENCER_STEP_MASK: u8 = 0x07;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct FrameSequencerState {
    pub(super) step: u8,
    pub(super) length_clock_count: u64,
    pub(super) sweep_clock_count: u64,
    pub(super) envelope_clock_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct FrameSequencerClocks {
    pub(super) length: bool,
    pub(super) sweep: bool,
    pub(super) envelope: bool,
}

pub(crate) const fn div_apu_phase_from_system_counter(system_counter: u16) -> u8 {
    ((system_counter >> 13) & FRAME_SEQUENCER_STEP_MASK as u16) as u8
}

pub(super) const fn frame_sequencer_step_clocks_length(step: u8) -> bool {
    matches!(step & FRAME_SEQUENCER_STEP_MASK, 0 | 2 | 4 | 6)
}

pub(super) const fn frame_sequencer_step_clocks_sweep(step: u8) -> bool {
    matches!(step & FRAME_SEQUENCER_STEP_MASK, 2 | 6)
}

pub(super) const fn frame_sequencer_step_clocks_envelope(step: u8) -> bool {
    step & FRAME_SEQUENCER_STEP_MASK == 7
}

pub(super) const fn frame_sequencer_clocks(step: u8) -> FrameSequencerClocks {
    FrameSequencerClocks {
        length: frame_sequencer_step_clocks_length(step),
        sweep: frame_sequencer_step_clocks_sweep(step),
        envelope: frame_sequencer_step_clocks_envelope(step),
    }
}

impl FrameSequencerState {
    pub(super) fn apply_startup_phase(&mut self, div_apu: u8) {
        self.step = div_apu & FRAME_SEQUENCER_STEP_MASK;
        self.length_clock_count = 0;
        self.sweep_clock_count = 0;
        self.envelope_clock_count = 0;
    }

    pub(super) fn advance(&mut self) -> FrameSequencerClocks {
        let clocks = frame_sequencer_clocks(self.step);

        if clocks.length {
            self.length_clock_count += 1;
        }
        if clocks.sweep {
            self.sweep_clock_count += 1;
        }
        if clocks.envelope {
            self.envelope_clock_count += 1;
        }

        self.step = (self.step + 1) & FRAME_SEQUENCER_STEP_MASK;
        clocks
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FrameSequencerClocks, frame_sequencer_clocks, frame_sequencer_step_clocks_envelope,
        frame_sequencer_step_clocks_length, frame_sequencer_step_clocks_sweep,
    };

    #[test]
    fn frame_sequencer_schedule_helpers_share_one_truth_table() {
        let expected = [
            FrameSequencerClocks {
                length: true,
                sweep: false,
                envelope: false,
            },
            FrameSequencerClocks::default(),
            FrameSequencerClocks {
                length: true,
                sweep: true,
                envelope: false,
            },
            FrameSequencerClocks::default(),
            FrameSequencerClocks {
                length: true,
                sweep: false,
                envelope: false,
            },
            FrameSequencerClocks::default(),
            FrameSequencerClocks {
                length: true,
                sweep: true,
                envelope: false,
            },
            FrameSequencerClocks {
                length: false,
                sweep: false,
                envelope: true,
            },
        ];

        for (step, expected_clocks) in expected.into_iter().enumerate() {
            let step = step as u8;
            assert_eq!(frame_sequencer_clocks(step), expected_clocks);
            assert_eq!(
                frame_sequencer_step_clocks_length(step),
                expected_clocks.length
            );
            assert_eq!(
                frame_sequencer_step_clocks_sweep(step),
                expected_clocks.sweep
            );
            assert_eq!(
                frame_sequencer_step_clocks_envelope(step),
                expected_clocks.envelope
            );
        }
    }
}
