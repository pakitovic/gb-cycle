use gb_core::ExternalPortAttachmentKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DesktopExternalPortSelection {
    #[default]
    None,
    Printer,
    GameLink,
    FourPlayerAdapter,
}

impl DesktopExternalPortSelection {
    pub const fn core_attachment_kind(self) -> ExternalPortAttachmentKind {
        match self {
            Self::None => ExternalPortAttachmentKind::None,
            Self::Printer => ExternalPortAttachmentKind::Printer,
            Self::GameLink | Self::FourPlayerAdapter => ExternalPortAttachmentKind::None,
        }
    }

    pub const fn supports_host_attachment(self) -> bool {
        matches!(self, Self::None | Self::Printer)
    }
}

#[cfg(test)]
mod tests {
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
}
