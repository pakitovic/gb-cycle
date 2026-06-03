use gb_core::{DMG_T_CYCLES_PER_FRAME, DMG_T_CYCLES_PER_SECOND};

pub fn target_frame_rate_hz() -> f64 {
    DMG_T_CYCLES_PER_SECOND as f64 / DMG_T_CYCLES_PER_FRAME as f64
}

pub fn target_frames_for_duration(duration_seconds: u32) -> u32 {
    (f64::from(duration_seconds) * target_frame_rate_hz()).ceil() as u32
}

pub fn target_tcycles_for_duration(duration_seconds: u32) -> u64 {
    u64::from(target_frames_for_duration(duration_seconds)) * DMG_T_CYCLES_PER_FRAME
}
