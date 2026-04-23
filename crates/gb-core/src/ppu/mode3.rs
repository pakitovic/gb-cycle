use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DmgPrevisibleWxRetargetPlanContext {
    is_dmg_family: bool,
    drawing_mode: bool,
    window_started_this_line: bool,
    window_wy_latch: bool,
    window_enabled: bool,
    bg_enabled: bool,
    visible_pixels_output: u8,
    window_active_line_counter: u8,
    pending_previsible_trigger_x: Option<u8>,
    pending_one_hidden_prefix_resume: bool,
    live_wx_can_still_start_later_this_line: bool,
    fetcher_source: PpuBgFetcherSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DmgPrevisibleWxFollowupMarkers {
    cancel_uses_visible_wx_once: bool,
    cancel_background_override_onset_x: Option<u8>,
    retained_trigger_glitch_x: Option<u8>,
}

impl DmgPrevisibleWxFollowupMarkers {
    const fn cleared() -> Self {
        Self {
            cancel_uses_visible_wx_once: false,
            cancel_background_override_onset_x: None,
            retained_trigger_glitch_x: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DmgPrevisibleWxPlanAction {
    KeepState,
    ClearOnsetGlitch,
    ClearRetargetAndGapArtifacts,
    ArmRetarget {
        retarget: DmgPrevisibleWxRetarget,
        onset_glitch: Option<u8>,
        carry: Option<DmgPendingPrevisibleWxCarry>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DmgPrevisibleWxPlan {
    followup_markers: DmgPrevisibleWxFollowupMarkers,
    action: DmgPrevisibleWxPlanAction,
}

include!("mode3/core.rs");
include!("mode3/bg_fetch.rs");
include!("mode3/window.rs");
include!("mode3/bg_push.rs");
include!("mode3/transfer.rs");
include!("mode3/window_wx.rs");
include!("mode3/obj_fetch.rs");
