use super::*;

#[test]
fn save_flush_policy_helpers_match_runtime_expectations() {
    assert!(!DesktopSaveFlushPolicy::Manual.flush_on_close());
    assert!(!DesktopSaveFlushPolicy::Manual.flush_each_frame_boundary());
    assert_eq!(DesktopSaveFlushPolicy::Manual.debounce_window(), None);

    assert!(DesktopSaveFlushPolicy::OnClose.flush_on_close());
    assert!(!DesktopSaveFlushPolicy::OnClose.flush_each_frame_boundary());

    assert!(DesktopSaveFlushPolicy::OnWrite.flush_on_close());
    assert!(DesktopSaveFlushPolicy::OnWrite.flush_each_frame_boundary());
    assert_eq!(DesktopSaveFlushPolicy::OnWrite.debounce_window(), None);

    assert!(DesktopSaveFlushPolicy::Debounced.flush_on_close());
    assert!(DesktopSaveFlushPolicy::Debounced.flush_each_frame_boundary());
    assert_eq!(
        DesktopSaveFlushPolicy::Debounced.debounce_window(),
        Some(DEFAULT_SAVE_FLUSH_DEBOUNCE)
    );
}
