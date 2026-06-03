use super::super::*;

#[test]
fn default_run_limit_profiles_cover_skip_boot_post_handoff_and_safety_cap() {
    assert!(default_run_limit_reached(
        Some(DefaultRunBudget::SkipBootFrames {
            frame_limit: DEFAULT_SKIP_BOOT_FRAME_LIMIT,
        }),
        DEFAULT_SKIP_BOOT_FRAME_LIMIT,
        None,
    ));
    assert!(!default_run_limit_reached(
        Some(DefaultRunBudget::RealBootPostHandoff {
            post_handoff_frame_limit: DEFAULT_REAL_BOOT_POST_HANDOFF_FRAME_LIMIT,
            safety_frame_limit: DEFAULT_REAL_BOOT_SAFETY_FRAME_LIMIT,
        }),
        DEFAULT_REAL_BOOT_POST_HANDOFF_FRAME_LIMIT,
        None,
    ));
    assert!(default_run_limit_reached(
        Some(DefaultRunBudget::RealBootPostHandoff {
            post_handoff_frame_limit: DEFAULT_REAL_BOOT_POST_HANDOFF_FRAME_LIMIT,
            safety_frame_limit: DEFAULT_REAL_BOOT_SAFETY_FRAME_LIMIT,
        }),
        DEFAULT_REAL_BOOT_SAFETY_FRAME_LIMIT,
        None,
    ));
    assert!(!default_run_limit_reached(
        Some(DefaultRunBudget::RealBootPostHandoff {
            post_handoff_frame_limit: DEFAULT_REAL_BOOT_POST_HANDOFF_FRAME_LIMIT,
            safety_frame_limit: DEFAULT_REAL_BOOT_SAFETY_FRAME_LIMIT,
        }),
        121,
        Some(2),
    ));
    assert!(default_run_limit_reached(
        Some(DefaultRunBudget::RealBootPostHandoff {
            post_handoff_frame_limit: DEFAULT_REAL_BOOT_POST_HANDOFF_FRAME_LIMIT,
            safety_frame_limit: DEFAULT_REAL_BOOT_SAFETY_FRAME_LIMIT,
        }),
        122,
        Some(2),
    ));
}
