use super::DesktopExternalPortSelection;
use gb_core::ExternalPortAttachmentKind;

#[test]
fn desktop_external_port_selection_maps_host_and_core_attachment_policies() {
    let cases = [
        (
            DesktopExternalPortSelection::None,
            ExternalPortAttachmentKind::None,
            true,
        ),
        (
            DesktopExternalPortSelection::Printer,
            ExternalPortAttachmentKind::Printer,
            true,
        ),
        (
            DesktopExternalPortSelection::GameLink,
            ExternalPortAttachmentKind::None,
            false,
        ),
        (
            DesktopExternalPortSelection::FourPlayerAdapter,
            ExternalPortAttachmentKind::None,
            false,
        ),
    ];

    for (selection, expected_kind, expected_host_support) in cases {
        assert_eq!(selection.core_attachment_kind(), expected_kind);
        assert_eq!(selection.supports_host_attachment(), expected_host_support);
    }

    assert_eq!(
        DesktopExternalPortSelection::default(),
        DesktopExternalPortSelection::None
    );
}
