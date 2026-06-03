#[cfg(test)]
mod test;

use gb_core::JoypadButton;
use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BenchmarkStimulusTime {
    TCycle(u64),
    Frame(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BenchmarkStimulus {
    pub when: BenchmarkStimulusTime,
    pub button: JoypadButton,
    pub pressed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchmarkStimulusRuntime {
    frame_stimuli: Vec<ScheduledBenchmarkStimulus>,
    tcycle_stimuli: Vec<ScheduledBenchmarkStimulus>,
    next_frame_stimulus: usize,
    next_tcycle_stimulus: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScheduledBenchmarkStimulus {
    stimulus: BenchmarkStimulus,
    order: usize,
}

impl BenchmarkStimulusRuntime {
    pub fn new(stimuli: Vec<BenchmarkStimulus>) -> Self {
        let mut frame_stimuli = Vec::new();
        let mut tcycle_stimuli = Vec::new();
        for (order, stimulus) in stimuli.into_iter().enumerate() {
            let scheduled = ScheduledBenchmarkStimulus { stimulus, order };
            match stimulus.when {
                BenchmarkStimulusTime::Frame(_) => frame_stimuli.push(scheduled),
                BenchmarkStimulusTime::TCycle(_) => tcycle_stimuli.push(scheduled),
            }
        }
        frame_stimuli.sort_by(|left, right| {
            scheduled_frame(left)
                .cmp(&scheduled_frame(right))
                .then(left.order.cmp(&right.order))
        });
        tcycle_stimuli.sort_by(|left, right| {
            scheduled_t_cycle(left)
                .cmp(&scheduled_t_cycle(right))
                .then(left.order.cmp(&right.order))
        });
        Self {
            frame_stimuli,
            tcycle_stimuli,
            next_frame_stimulus: 0,
            next_tcycle_stimulus: 0,
        }
    }

    pub fn apply_due<F>(&mut self, t_cycle: u64, completed_frames: u64, mut apply: F)
    where
        F: FnMut(JoypadButton, bool),
    {
        let mut due = Vec::new();
        while let Some(stimulus) = self.tcycle_stimuli.get(self.next_tcycle_stimulus) {
            match scheduled_t_cycle(stimulus).cmp(&t_cycle) {
                Ordering::Less => self.next_tcycle_stimulus += 1,
                Ordering::Equal => {
                    due.push(*stimulus);
                    self.next_tcycle_stimulus += 1;
                }
                Ordering::Greater => break,
            }
        }
        while let Some(stimulus) = self.frame_stimuli.get(self.next_frame_stimulus) {
            match u64::from(scheduled_frame(stimulus)).cmp(&completed_frames) {
                Ordering::Less => self.next_frame_stimulus += 1,
                Ordering::Equal => {
                    due.push(*stimulus);
                    self.next_frame_stimulus += 1;
                }
                Ordering::Greater => break,
            }
        }
        due.sort_by_key(|stimulus| stimulus.order);
        for scheduled in due {
            apply(scheduled.stimulus.button, scheduled.stimulus.pressed);
        }
    }
}

fn scheduled_frame(stimulus: &ScheduledBenchmarkStimulus) -> u32 {
    let BenchmarkStimulusTime::Frame(frame) = stimulus.stimulus.when else {
        unreachable!("frame stimuli are stored separately from T-cycle stimuli");
    };
    frame
}

fn scheduled_t_cycle(stimulus: &ScheduledBenchmarkStimulus) -> u64 {
    let BenchmarkStimulusTime::TCycle(t_cycle) = stimulus.stimulus.when else {
        unreachable!("T-cycle stimuli are stored separately from frame stimuli");
    };
    t_cycle
}
