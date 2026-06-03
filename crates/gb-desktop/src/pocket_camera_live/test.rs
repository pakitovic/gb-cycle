use super::*;
use std::ptr::NonNull;

fn live_input_with_state() -> PocketCameraLiveInput {
    PocketCameraLiveInput {
        subsystem: None,
        unavailable_reason: None,
        camera: None,
        enabled: true,
        warmup_frames_remaining: 3,
        frames_delivered: 7,
        polls_without_frame: 9,
        camera_name: Some("fake camera".to_string()),
        poll_error: None,
    }
}

#[test]
fn unavailable_live_input_reports_start_error_without_enabling() {
    let mut live = PocketCameraLiveInput::unavailable_for_tests("camera backend disabled");

    assert!(!live.is_enabled());
    assert_eq!(live.start(), Err("camera backend disabled".to_string()));
    assert!(!live.is_enabled());
    assert_eq!(
        live.poll_frame().expect("disabled poll should succeed"),
        None
    );
}

#[test]
fn live_input_state_accessors_and_stop_reset_session_state() {
    let mut live = live_input_with_state();

    assert!(live.is_enabled());
    assert_eq!(live.camera_name(), Some("fake camera"));
    assert_eq!(live.frames_delivered(), 7);
    assert_eq!(live.polls_without_frame(), 9);
    assert_eq!(live.permission_state_label(), None);
    assert_eq!(live.start(), Ok(()));

    live.stop();

    assert!(!live.is_enabled());
    assert_eq!(live.camera_name(), None);
    assert_eq!(live.frames_delivered(), 0);
    assert_eq!(live.polls_without_frame(), 0);
}

#[test]
fn live_input_with_missing_open_camera_self_disables_on_poll() {
    let mut live = live_input_with_state();

    assert_eq!(live.poll_frame(), Ok(None));

    assert!(!live.is_enabled());
    assert_eq!(live.camera_name(), None);
    assert_eq!(live.frames_delivered(), 0);
    assert_eq!(live.polls_without_frame(), 0);
}

#[test]
fn live_input_without_subsystem_or_reason_reports_the_default_start_error() {
    let mut live = live_input_with_state();
    live.stop();

    assert_eq!(
        live.start(),
        Err("SDL3 camera subsystem is not available".to_string())
    );
}

#[test]
fn low_level_camera_helpers_handle_invalid_ids_and_enumeration() {
    let _guard = crate::lock_sdl_test();

    assert!(OpenCamera::open(camera::SDL_CameraID::new(0)).is_err());
    assert_eq!(camera_name(camera::SDL_CameraID::new(0)), "camera 0");
    let _ = connected_cameras();
}

#[test]
fn rgb24_pixels_to_grayscale_respects_pitch_padding() {
    let pixels = [
        255, 0, 0, 0, 255, 0, 99, 99, 99, 0, 0, 255, 255, 255, 255, 77, 77, 77,
    ];

    let grayscale = rgb24_pixels_to_grayscale(2, 2, 9, pixels.as_ptr())
        .expect("pitched RGB24 rows should convert");

    assert_eq!(grayscale, vec![76, 150, 29, 255]);
}

#[test]
fn rgb24_pixels_to_grayscale_rejects_short_pitch() {
    let pixels = [0; 6];

    assert_eq!(
        rgb24_pixels_to_grayscale(3, 1, 8, pixels.as_ptr()),
        Err("camera frame pitch is smaller than its RGB row width".to_string())
    );
}

#[test]
fn rgb24_pixels_to_grayscale_reports_overflow_and_missing_pixels() {
    let pixels = [0; 3];

    assert_eq!(
        rgb24_pixels_to_grayscale(usize::MAX, 1, usize::MAX, pixels.as_ptr()),
        Err("camera RGB row width overflowed".to_string())
    );
    assert_eq!(
        rgb24_pixels_to_grayscale(1, usize::MAX, usize::MAX, pixels.as_ptr()),
        Err("camera pixel buffer length overflowed".to_string())
    );
    assert_eq!(
        rgb24_pixels_to_grayscale(1, 1, 3, std::ptr::null()),
        Err("camera frame has no pixel buffer".to_string())
    );
}

#[test]
fn pocket_camera_frame_from_rgb24_surface_preserves_dimensions() {
    let _guard = crate::lock_sdl_test();

    let surface = unsafe { surface::SDL_CreateSurface(2, 1, pixels::SDL_PIXELFORMAT_RGB24) };
    let surface = NonNull::new(surface).expect("RGB24 test surface should be created");
    let row = [255, 0, 0, 0, 255, 0];
    unsafe {
        std::ptr::copy_nonoverlapping(row.as_ptr(), (*surface.as_ptr()).pixels.cast(), row.len())
    };

    let frame = unsafe { pocket_camera_frame_from_surface(surface.as_ptr()) }
        .expect("RGB24 surface should convert to a Pocket Camera frame");
    unsafe { surface::SDL_DestroySurface(surface.as_ptr()) };

    assert_eq!(frame.width, 2);
    assert_eq!(frame.height, 1);
    assert_eq!(frame.grayscale_pixels.len(), 2);
}

#[test]
fn pocket_camera_frame_from_convertible_surface_uses_rgb24_conversion() {
    let _guard = crate::lock_sdl_test();

    let surface = unsafe { surface::SDL_CreateSurface(1, 1, pixels::SDL_PIXELFORMAT_RGBA32) };
    let surface = NonNull::new(surface).expect("RGBA32 test surface should be created");
    let pixel = [128, 128, 128, 128];
    unsafe {
        std::ptr::copy_nonoverlapping(
            pixel.as_ptr(),
            (*surface.as_ptr()).pixels.cast(),
            pixel.len(),
        )
    };

    let frame = unsafe { pocket_camera_frame_from_surface(surface.as_ptr()) }
        .expect("RGBA32 surface should convert through SDL");
    unsafe { surface::SDL_DestroySurface(surface.as_ptr()) };

    assert_eq!(frame.width, 1);
    assert_eq!(frame.height, 1);
    assert_eq!(frame.grayscale_pixels.len(), 1);
}

#[test]
fn pocket_camera_frame_from_surface_rejects_null_surface() {
    assert_eq!(
        unsafe { pocket_camera_frame_from_surface(std::ptr::null_mut()) },
        Err("SDL3 camera produced a null surface".to_string())
    );
}

#[test]
fn rgb24_surface_to_pocket_camera_frame_rejects_invalid_surface_metadata() {
    let _guard = crate::lock_sdl_test();

    let surface = unsafe { surface::SDL_CreateSurface(1, 1, pixels::SDL_PIXELFORMAT_RGB24) };
    let surface = NonNull::new(surface).expect("RGB24 test surface should be created");
    let original_width = unsafe { (*surface.as_ptr()).w };
    let original_height = unsafe { (*surface.as_ptr()).h };
    let original_pitch = unsafe { (*surface.as_ptr()).pitch };

    unsafe { (*surface.as_ptr()).w = -1 };
    assert_eq!(
        unsafe { rgb24_surface_to_pocket_camera_frame(surface.as_ptr()) },
        Err("camera frame width is negative".to_string())
    );

    unsafe {
        (*surface.as_ptr()).w = original_width;
        (*surface.as_ptr()).h = -1;
    }
    assert_eq!(
        unsafe { rgb24_surface_to_pocket_camera_frame(surface.as_ptr()) },
        Err("camera frame height is negative".to_string())
    );

    unsafe {
        (*surface.as_ptr()).h = original_height;
        (*surface.as_ptr()).pitch = -1;
    }
    assert_eq!(
        unsafe { rgb24_surface_to_pocket_camera_frame(surface.as_ptr()) },
        Err("camera frame pitch is negative".to_string())
    );

    unsafe {
        (*surface.as_ptr()).pitch = original_pitch;
        (*surface.as_ptr()).w = 0;
    }
    assert_eq!(
        unsafe { rgb24_surface_to_pocket_camera_frame(surface.as_ptr()) },
        Err("camera frame has zero dimensions".to_string())
    );

    unsafe {
        (*surface.as_ptr()).w = i32::from(u16::MAX) + 1;
        (*surface.as_ptr()).h = original_height;
    }
    assert_eq!(
        unsafe { rgb24_surface_to_pocket_camera_frame(surface.as_ptr()) },
        Err("camera frame width exceeds Pocket Camera API limits".to_string())
    );

    unsafe {
        (*surface.as_ptr()).w = original_width;
        (*surface.as_ptr()).h = i32::from(u16::MAX) + 1;
    }
    assert_eq!(
        unsafe { rgb24_surface_to_pocket_camera_frame(surface.as_ptr()) },
        Err("camera frame height exceeds Pocket Camera API limits".to_string())
    );

    unsafe {
        (*surface.as_ptr()).h = original_height;
        (*surface.as_ptr()).pitch = original_pitch;
        surface::SDL_DestroySurface(surface.as_ptr());
    }
}

#[test]
fn live_camera_frames_are_mirrored_horizontally_for_selfie_orientation() {
    let mut grayscale = vec![1, 2, 3, 4, 5, 6];

    mirror_frame_horizontally(&mut grayscale, 3);

    assert_eq!(grayscale, vec![3, 2, 1, 6, 5, 4]);
}

#[test]
fn mirror_frame_with_zero_width_is_a_noop() {
    let mut grayscale = vec![1, 2, 3];

    mirror_frame_horizontally(&mut grayscale, 0);

    assert_eq!(grayscale, vec![1, 2, 3]);
}

#[test]
fn camera_permission_state_labels_cover_known_sdl_states() {
    assert_eq!(
        camera_permission_state_label(camera::SDL_CAMERA_PERMISSION_STATE_DENIED),
        "denied"
    );
    assert_eq!(
        camera_permission_state_label(camera::SDL_CAMERA_PERMISSION_STATE_PENDING),
        "pending"
    );
    assert_eq!(
        camera_permission_state_label(camera::SDL_CAMERA_PERMISSION_STATE_APPROVED),
        "approved"
    );
    assert_eq!(
        camera_permission_state_label(camera::SDL_CameraPermissionState::new(99)),
        "unknown"
    );
}

#[test]
fn reading_sdl_error_is_safe_when_sdl_has_no_error() {
    let _guard = crate::lock_sdl_test();

    let _ = sdl_error();
}
