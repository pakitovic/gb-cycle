use crate::options::DefaultRunBudget;

pub(crate) fn run_limit_reached(
    frame_limit: Option<u32>,
    tcycle_limit: Option<u64>,
    completed_frames: u32,
    executed_tcycles: u64,
) -> bool {
    frame_limit.is_some_and(|limit| completed_frames >= limit)
        || tcycle_limit.is_some_and(|limit| executed_tcycles >= limit)
}

pub(crate) fn default_run_limit_reached(
    default_run_budget: Option<DefaultRunBudget>,
    completed_frames: u32,
    completed_frames_at_boot_handoff: Option<u32>,
) -> bool {
    match default_run_budget {
        Some(DefaultRunBudget::SkipBootFrames { frame_limit }) => completed_frames >= frame_limit,
        Some(DefaultRunBudget::RealBootPostHandoff {
            post_handoff_frame_limit,
            safety_frame_limit,
        }) => match completed_frames_at_boot_handoff {
            Some(frames_at_handoff) => {
                completed_frames >= frames_at_handoff.saturating_add(post_handoff_frame_limit)
            }
            None => completed_frames >= safety_frame_limit,
        },
        None => false,
    }
}
