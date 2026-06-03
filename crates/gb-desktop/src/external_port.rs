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
mod test;
