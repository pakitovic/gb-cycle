use gb_core::PocketCameraFrame;
use sdl3::sys::{camera, error, pixels, stdinc, surface};
use std::ffi::CStr;
use std::ptr::NonNull;
use std::slice;

const LIVE_CAMERA_WARMUP_FRAMES: u8 = 5;

pub struct PocketCameraLiveInput {
    subsystem: Option<sdl3::CameraSubsystem>,
    unavailable_reason: Option<String>,
    camera: Option<OpenCamera>,
    enabled: bool,
    warmup_frames_remaining: u8,
    frames_delivered: u64,
    polls_without_frame: u16,
    camera_name: Option<String>,
    #[cfg(test)]
    poll_error: Option<String>,
}

impl PocketCameraLiveInput {
    pub fn new(subsystem: Result<sdl3::CameraSubsystem, String>) -> Self {
        match subsystem {
            Ok(subsystem) => Self {
                subsystem: Some(subsystem),
                unavailable_reason: None,
                camera: None,
                enabled: false,
                warmup_frames_remaining: 0,
                frames_delivered: 0,
                polls_without_frame: 0,
                camera_name: None,
                #[cfg(test)]
                poll_error: None,
            },
            Err(error) => Self {
                subsystem: None,
                unavailable_reason: Some(error),
                camera: None,
                enabled: false,
                warmup_frames_remaining: 0,
                frames_delivered: 0,
                polls_without_frame: 0,
                camera_name: None,
                #[cfg(test)]
                poll_error: None,
            },
        }
    }

    #[cfg(test)]
    pub fn unavailable_for_tests(reason: impl Into<String>) -> Self {
        Self::new(Err(reason.into()))
    }

    #[cfg(test)]
    pub fn enabled_without_camera_for_tests() -> Self {
        Self {
            subsystem: None,
            unavailable_reason: None,
            camera: None,
            enabled: true,
            warmup_frames_remaining: LIVE_CAMERA_WARMUP_FRAMES,
            frames_delivered: 0,
            polls_without_frame: 0,
            camera_name: Some("test camera".to_string()),
            poll_error: None,
        }
    }

    #[cfg(test)]
    pub fn enabled_with_poll_error_for_tests(error: impl Into<String>) -> Self {
        Self {
            subsystem: None,
            unavailable_reason: None,
            camera: None,
            enabled: true,
            warmup_frames_remaining: LIVE_CAMERA_WARMUP_FRAMES,
            frames_delivered: 0,
            polls_without_frame: 0,
            camera_name: Some("test camera".to_string()),
            poll_error: Some(error.into()),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn camera_name(&self) -> Option<&str> {
        self.camera_name.as_deref()
    }

    pub fn start(&mut self) -> Result<(), String> {
        if self.enabled {
            return Ok(());
        }
        if self.subsystem.is_none() {
            return Err(self
                .unavailable_reason
                .clone()
                .unwrap_or_else(|| "SDL3 camera subsystem is not available".to_string()));
        }

        let cameras = connected_cameras()?;
        let Some(camera_id) = cameras.first().copied() else {
            return Err("no SDL3 camera devices are currently available".to_string());
        };
        let camera_name = camera_name(camera_id);
        let camera = OpenCamera::open(camera_id)?;
        self.camera = Some(camera);
        self.enabled = true;
        self.warmup_frames_remaining = LIVE_CAMERA_WARMUP_FRAMES;
        self.frames_delivered = 0;
        self.polls_without_frame = 0;
        self.camera_name = Some(camera_name);
        Ok(())
    }

    pub fn stop(&mut self) {
        self.camera = None;
        self.enabled = false;
        self.warmup_frames_remaining = 0;
        self.frames_delivered = 0;
        self.polls_without_frame = 0;
        self.camera_name = None;
    }

    pub fn poll_frame(&mut self) -> Result<Option<PocketCameraFrame>, String> {
        if !self.enabled {
            return Ok(None);
        }

        #[cfg(test)]
        if let Some(error) = self.poll_error.clone() {
            return Err(error);
        }

        let Some(camera) = self.camera.as_ref() else {
            self.stop();
            return Ok(None);
        };

        let permission_state = camera.permission_state();
        if permission_state == camera::SDL_CAMERA_PERMISSION_STATE_DENIED {
            self.stop();
            return Err("camera permission was denied by the operating system".to_string());
        }

        let mut latest_frame = None;
        loop {
            let Some(frame) = camera.acquire_frame()? else {
                break;
            };
            let frame_result = frame.to_pocket_camera_frame();
            if self.warmup_frames_remaining > 0 {
                self.warmup_frames_remaining -= 1;
                continue;
            }
            latest_frame = Some(frame_result?);
        }
        if latest_frame.is_some() {
            self.frames_delivered += 1;
            self.polls_without_frame = 0;
        } else {
            self.polls_without_frame = self.polls_without_frame.saturating_add(1);
        }
        Ok(latest_frame)
    }

    pub fn frames_delivered(&self) -> u64 {
        self.frames_delivered
    }

    pub fn polls_without_frame(&self) -> u16 {
        self.polls_without_frame
    }

    pub fn permission_state_label(&self) -> Option<&'static str> {
        self.camera
            .as_ref()
            .map(|camera| camera_permission_state_label(camera.permission_state()))
    }
}

struct OpenCamera {
    raw: NonNull<camera::SDL_Camera>,
}

impl OpenCamera {
    fn open(camera_id: camera::SDL_CameraID) -> Result<Self, String> {
        // SAFETY: `camera_id` comes from SDL_GetCameras. Passing a null spec lets
        // SDL pick the camera's native stream; this avoids platform backends that
        // open successfully but never deliver frames for uncommon requested sizes
        // like the Pocket Camera's final 128x112 input.
        let raw = unsafe { camera::SDL_OpenCamera(camera_id, std::ptr::null()) };
        let raw = NonNull::new(raw).ok_or_else(|| {
            let error = sdl_error();
            if error.is_empty() {
                "failed to open SDL3 camera".to_string()
            } else {
                format!("failed to open SDL3 camera: {error}")
            }
        })?;
        Ok(Self { raw })
    }

    fn permission_state(&self) -> camera::SDL_CameraPermissionState {
        // SAFETY: `raw` is a live SDL_Camera owned by this wrapper until Drop.
        unsafe { camera::SDL_GetCameraPermissionState(self.raw.as_ptr()) }
    }

    fn acquire_frame(&self) -> Result<Option<CameraFrame<'_>>, String> {
        let mut timestamp_ns = 0;
        // SAFETY: `raw` is a live SDL_Camera and `timestamp_ns` is a valid out pointer.
        let surface =
            unsafe { camera::SDL_AcquireCameraFrame(self.raw.as_ptr(), &mut timestamp_ns) };
        let Some(surface) = NonNull::new(surface) else {
            return Ok(None);
        };
        Ok(Some(CameraFrame {
            camera: self,
            surface,
        }))
    }
}

impl Drop for OpenCamera {
    fn drop(&mut self) {
        // SAFETY: `raw` is owned by this wrapper and is closed exactly once here.
        unsafe { camera::SDL_CloseCamera(self.raw.as_ptr()) };
    }
}

struct CameraFrame<'camera> {
    camera: &'camera OpenCamera,
    surface: NonNull<surface::SDL_Surface>,
}

impl CameraFrame<'_> {
    fn to_pocket_camera_frame(&self) -> Result<PocketCameraFrame, String> {
        // SAFETY: `surface` is an SDL camera frame that remains valid until this
        // CameraFrame is dropped and releases it back to SDL.
        let mut frame = unsafe { pocket_camera_frame_from_surface(self.surface.as_ptr()) }?;
        mirror_frame_horizontally(&mut frame.grayscale_pixels, usize::from(frame.width));
        Ok(frame)
    }
}

impl Drop for CameraFrame<'_> {
    fn drop(&mut self) {
        // SAFETY: `surface` was returned by SDL_AcquireCameraFrame for `camera.raw`
        // and must be returned with SDL_ReleaseCameraFrame exactly once.
        unsafe {
            camera::SDL_ReleaseCameraFrame(self.camera.raw.as_ptr(), self.surface.as_ptr());
        }
    }
}

fn connected_cameras() -> Result<Vec<camera::SDL_CameraID>, String> {
    let mut count = 0;
    // SAFETY: `count` is a valid out pointer. SDL returns an allocation that we
    // copy immediately and free with SDL_free below.
    let cameras = unsafe { camera::SDL_GetCameras(&mut count) };
    if cameras.is_null() {
        if count == 0 {
            return Ok(Vec::new());
        }
        let error = sdl_error();
        return Err(if error.is_empty() {
            "failed to enumerate SDL3 cameras".to_string()
        } else {
            format!("failed to enumerate SDL3 cameras: {error}")
        });
    }

    let count = usize::try_from(count).map_err(|_| "SDL3 returned a negative camera count")?;
    // SAFETY: SDL_GetCameras returned at least `count` entries before the
    // terminating zero. We copy the IDs before freeing the SDL allocation.
    let ids = unsafe { slice::from_raw_parts(cameras, count) }.to_vec();
    // SAFETY: `cameras` was allocated by SDL_GetCameras and must be released by SDL_free.
    unsafe { stdinc::SDL_free(cameras.cast()) };
    Ok(ids)
}

fn camera_name(camera_id: camera::SDL_CameraID) -> String {
    // SAFETY: `camera_id` comes from SDL_GetCameras. SDL owns the returned string.
    let name = unsafe { camera::SDL_GetCameraName(camera_id) };
    if name.is_null() {
        format!("camera {}", camera_id.value())
    } else {
        // SAFETY: SDL_GetCameraName returns a NUL-terminated C string while the
        // device remains known to SDL; we copy it immediately.
        unsafe { CStr::from_ptr(name) }
            .to_string_lossy()
            .into_owned()
    }
}

unsafe fn pocket_camera_frame_from_surface(
    source: *mut surface::SDL_Surface,
) -> Result<PocketCameraFrame, String> {
    if source.is_null() {
        return Err("SDL3 camera produced a null surface".to_string());
    }

    // SAFETY: caller guarantees `source` points to a live SDL_Surface.
    if unsafe { (*source).format } == pixels::SDL_PIXELFORMAT_RGB24 {
        // SAFETY: source is a live RGB24 surface.
        unsafe { rgb24_surface_to_pocket_camera_frame(source) }
    } else {
        // SAFETY: source is a live surface; SDL returns a newly allocated converted
        // surface or NULL on failure.
        let converted =
            unsafe { surface::SDL_ConvertSurface(source, pixels::SDL_PIXELFORMAT_RGB24) };
        let Some(converted) = NonNull::new(converted) else {
            let error = sdl_error();
            return Err(if error.is_empty() {
                "failed to convert SDL3 camera frame to RGB24".to_string()
            } else {
                format!("failed to convert SDL3 camera frame to RGB24: {error}")
            });
        };
        // SAFETY: converted points to a live RGB24 SDL surface allocated above.
        let frame = unsafe { rgb24_surface_to_pocket_camera_frame(converted.as_ptr()) };
        // SAFETY: converted was allocated by SDL_ConvertSurface and is no longer used.
        unsafe { surface::SDL_DestroySurface(converted.as_ptr()) };
        frame
    }
}

unsafe fn rgb24_surface_to_pocket_camera_frame(
    source: *mut surface::SDL_Surface,
) -> Result<PocketCameraFrame, String> {
    // SAFETY: caller guarantees `source` points to a live SDL_Surface. These
    // fields are read-only SDL surface metadata.
    let width =
        usize::try_from(unsafe { (*source).w }).map_err(|_| "camera frame width is negative")?;
    // SAFETY: caller guarantees `source` points to a live SDL_Surface.
    let height =
        usize::try_from(unsafe { (*source).h }).map_err(|_| "camera frame height is negative")?;
    // SAFETY: caller guarantees `source` points to a live SDL_Surface.
    let pitch = usize::try_from(unsafe { (*source).pitch })
        .map_err(|_| "camera frame pitch is negative")?;
    if width == 0 || height == 0 {
        return Err("camera frame has zero dimensions".to_string());
    }
    let width_u16 =
        u16::try_from(width).map_err(|_| "camera frame width exceeds Pocket Camera API limits")?;
    let height_u16 = u16::try_from(height)
        .map_err(|_| "camera frame height exceeds Pocket Camera API limits")?;

    // SAFETY: source is a live SDL_Surface.
    let must_unlock = unsafe { surface::SDL_MUSTLOCK(source) };
    if must_unlock {
        // SAFETY: source is a live SDL_Surface that SDL reports requires locking.
        let locked = unsafe { surface::SDL_LockSurface(source) };
        if !locked {
            let error = sdl_error();
            return Err(if error.is_empty() {
                "failed to lock SDL3 camera frame".to_string()
            } else {
                format!("failed to lock SDL3 camera frame: {error}")
            });
        }
    }

    // SAFETY: source is a live SDL_Surface and, if needed, is locked above so
    // its pixels can be accessed directly until it is unlocked below.
    let pixels = unsafe { (*source).pixels };
    let result = rgb24_pixels_to_grayscale(width, height, pitch, pixels.cast());
    if must_unlock {
        // SAFETY: source was successfully locked above.
        unsafe { surface::SDL_UnlockSurface(source) };
    }

    Ok(PocketCameraFrame {
        width: width_u16,
        height: height_u16,
        grayscale_pixels: result?,
    })
}

fn rgb24_pixels_to_grayscale(
    width: usize,
    height: usize,
    pitch: usize,
    pixels: *const u8,
) -> Result<Vec<u8>, String> {
    let row_bytes = width
        .checked_mul(3)
        .ok_or_else(|| "camera RGB row width overflowed".to_string())?;
    if pitch < row_bytes {
        return Err("camera frame pitch is smaller than its RGB row width".to_string());
    }
    let len = pitch
        .checked_mul(height.saturating_sub(1))
        .and_then(|prefix| prefix.checked_add(row_bytes))
        .ok_or_else(|| "camera pixel buffer length overflowed".to_string())?;
    if pixels.is_null() {
        return Err("camera frame has no pixel buffer".to_string());
    }
    // SAFETY: callers pass SDL surface memory whose accessible byte range covers
    // the computed pitched rows.
    let pixels = unsafe { slice::from_raw_parts(pixels, len) };
    let mut grayscale_pixels = Vec::with_capacity(width * height);
    for y in 0..height {
        let row_start = y * pitch;
        let row = &pixels[row_start..row_start + row_bytes];
        for rgb in row.chunks_exact(3) {
            grayscale_pixels.push(grayscale_from_rgb(rgb[0], rgb[1], rgb[2]));
        }
    }
    Ok(grayscale_pixels)
}

fn grayscale_from_rgb(red: u8, green: u8, blue: u8) -> u8 {
    ((299_u32 * u32::from(red) + 587_u32 * u32::from(green) + 114_u32 * u32::from(blue) + 500)
        / 1000) as u8
}

fn mirror_frame_horizontally(grayscale_pixels: &mut [u8], width: usize) {
    if width == 0 {
        return;
    }
    for row in grayscale_pixels.chunks_exact_mut(width) {
        row.reverse();
    }
}

fn camera_permission_state_label(state: camera::SDL_CameraPermissionState) -> &'static str {
    if state == camera::SDL_CAMERA_PERMISSION_STATE_DENIED {
        "denied"
    } else if state == camera::SDL_CAMERA_PERMISSION_STATE_PENDING {
        "pending"
    } else if state == camera::SDL_CAMERA_PERMISSION_STATE_APPROVED {
        "approved"
    } else {
        "unknown"
    }
}

fn sdl_error() -> String {
    let error = error::SDL_GetError();
    if error.is_null() {
        String::new()
    } else {
        // SAFETY: SDL_GetError returns a NUL-terminated string owned by SDL; we
        // copy it immediately.
        unsafe { CStr::from_ptr(error) }
            .to_string_lossy()
            .into_owned()
    }
}

#[cfg(test)]
mod tests {
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
            std::ptr::copy_nonoverlapping(
                row.as_ptr(),
                (*surface.as_ptr()).pixels.cast(),
                row.len(),
            )
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
}
