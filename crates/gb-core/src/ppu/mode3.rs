use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct DmgPrevisibleWxPlan {
    followup_markers: DmgPrevisibleWxFollowupMarkers,
    action: DmgPrevisibleWxPlanAction,
}

mod bg_fetch;
mod bg_push;
mod core;
mod obj_fetch;

#[cfg(test)]
pub(in crate::ppu) use self::window::{
    cgb_dmg_software_window_lcdc4_signed_to_unsigned_previous_plane_masks,
    cgb_dmg_software_window_lcdc4_unsigned_to_signed_previous_plane_masks,
    window_lcdc4_unsigned_to_signed_previous_plane_masks,
};
mod transfer;
mod window;
mod window_wx;
