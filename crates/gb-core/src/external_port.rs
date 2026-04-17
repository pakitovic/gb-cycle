mod printer;

pub use printer::{
    PrintedPage, PrinterCommand, PrinterMargins, PrinterPrintArgs, PrinterSnapshot,
    PrinterStatusBits,
};

use crate::serial::SerialPeer;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ExternalPortAttachmentKind {
    #[default]
    None,
    Loopback,
    Printer,
    GameLinkDmg04,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ExternalPortResetPolicy {
    #[default]
    PreserveAttachmentKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExternalPort {
    attachment: ExternalPortAttachment,
    reset_policy: ExternalPortResetPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalPortSnapshot {
    pub attachment_kind: ExternalPortAttachmentKind,
    pub reset_policy: ExternalPortResetPolicy,
    pub printer: Option<PrinterSnapshot>,
    pub dmg04_incoming_byte: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
enum ExternalPortAttachment {
    #[default]
    None,
    Loopback(LoopbackAttachmentState),
    Printer(printer::PrinterDevice),
    GameLinkDmg04(Dmg04AttachmentState),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
struct LoopbackAttachmentState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
struct Dmg04AttachmentState {
    incoming_byte: Option<u8>,
}

impl ExternalPort {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn attachment_kind(&self) -> ExternalPortAttachmentKind {
        self.attachment.kind()
    }

    pub fn reset_policy(&self) -> ExternalPortResetPolicy {
        self.reset_policy
    }

    pub fn set_reset_policy(&mut self, reset_policy: ExternalPortResetPolicy) {
        self.reset_policy = reset_policy;
    }

    pub fn set_attachment_kind(&mut self, attachment_kind: ExternalPortAttachmentKind) {
        self.attachment = ExternalPortAttachment::from_kind(attachment_kind);
    }

    pub fn apply_startup_reset(&mut self) {
        self.attachment = match self.reset_policy {
            ExternalPortResetPolicy::PreserveAttachmentKind => {
                ExternalPortAttachment::from_kind(self.attachment.kind())
            }
        };
    }

    pub fn snapshot(&self) -> ExternalPortSnapshot {
        ExternalPortSnapshot {
            attachment_kind: self.attachment.kind(),
            reset_policy: self.reset_policy,
            printer: self.attachment.printer_snapshot(),
            dmg04_incoming_byte: self.attachment.dmg04_incoming_byte(),
        }
    }

    pub fn take_printed_pages(&mut self) -> Vec<PrintedPage> {
        match &mut self.attachment {
            ExternalPortAttachment::Printer(printer) => printer.take_printed_pages(),
            _ => Vec::new(),
        }
    }

    pub(crate) fn tick_t_cycle(&mut self) {
        if let ExternalPortAttachment::Printer(printer) = &mut self.attachment {
            printer.tick_t_cycle();
        }
    }

    pub(crate) fn handle_completed_serial_byte(&mut self, outgoing_byte: u8) {
        if let ExternalPortAttachment::Printer(printer) = &mut self.attachment {
            printer.receive_serial_byte(outgoing_byte);
        }
    }

    pub(crate) fn set_dmg04_incoming_byte(&mut self, incoming_byte: Option<u8>) {
        if let ExternalPortAttachment::GameLinkDmg04(endpoint) = &mut self.attachment {
            endpoint.incoming_byte = incoming_byte;
        }
    }

    pub(crate) fn serial_peer(&self) -> SerialPeer {
        match &self.attachment {
            ExternalPortAttachment::None => SerialPeer::Disconnected,
            ExternalPortAttachment::Loopback(_) => SerialPeer::Loopback,
            ExternalPortAttachment::Printer(printer) => SerialPeer::StagedIncomingByte {
                byte: printer.staged_response_byte(),
            },
            ExternalPortAttachment::GameLinkDmg04(endpoint) => endpoint.serial_peer(),
        }
    }
}

impl ExternalPortAttachment {
    const fn kind(&self) -> ExternalPortAttachmentKind {
        match self {
            Self::None => ExternalPortAttachmentKind::None,
            Self::Loopback(_) => ExternalPortAttachmentKind::Loopback,
            Self::Printer(_) => ExternalPortAttachmentKind::Printer,
            Self::GameLinkDmg04(_) => ExternalPortAttachmentKind::GameLinkDmg04,
        }
    }

    fn from_kind(kind: ExternalPortAttachmentKind) -> Self {
        match kind {
            ExternalPortAttachmentKind::None => Self::None,
            ExternalPortAttachmentKind::Loopback => Self::Loopback(LoopbackAttachmentState),
            ExternalPortAttachmentKind::Printer => Self::Printer(printer::PrinterDevice::new()),
            ExternalPortAttachmentKind::GameLinkDmg04 => {
                Self::GameLinkDmg04(Dmg04AttachmentState::default())
            }
        }
    }

    fn printer_snapshot(&self) -> Option<PrinterSnapshot> {
        match self {
            Self::Printer(printer) => Some(printer.snapshot()),
            _ => None,
        }
    }

    const fn dmg04_incoming_byte(&self) -> Option<u8> {
        match self {
            Self::GameLinkDmg04(endpoint) => endpoint.incoming_byte,
            _ => None,
        }
    }
}

impl Dmg04AttachmentState {
    const fn serial_peer(&self) -> SerialPeer {
        match self.incoming_byte {
            Some(byte) => SerialPeer::StagedIncomingByte { byte },
            None => SerialPeer::Disconnected,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_external_port_starts_disconnected() {
        let external_port = ExternalPort::new();

        assert_eq!(
            external_port.attachment_kind(),
            ExternalPortAttachmentKind::None
        );
        assert_eq!(
            external_port.reset_policy(),
            ExternalPortResetPolicy::PreserveAttachmentKind
        );
        assert_eq!(external_port.serial_peer(), SerialPeer::Disconnected);
        assert!(external_port.snapshot().printer.is_none());
        assert!(external_port.snapshot().dmg04_incoming_byte.is_none());
    }

    #[test]
    fn reset_policy_is_publicly_configurable() {
        let mut external_port = ExternalPort::new();

        external_port.set_reset_policy(ExternalPortResetPolicy::PreserveAttachmentKind);

        assert_eq!(
            external_port.reset_policy(),
            ExternalPortResetPolicy::PreserveAttachmentKind
        );
    }

    #[test]
    fn loopback_attachment_maps_to_the_existing_serial_peer_boundary() {
        let mut external_port = ExternalPort::new();

        external_port.set_attachment_kind(ExternalPortAttachmentKind::Loopback);

        assert_eq!(
            external_port.attachment_kind(),
            ExternalPortAttachmentKind::Loopback
        );
        assert_eq!(external_port.serial_peer(), SerialPeer::Loopback);
    }

    #[test]
    fn printer_attachment_starts_with_a_zero_response_byte() {
        let mut external_port = ExternalPort::new();

        external_port.set_attachment_kind(ExternalPortAttachmentKind::Printer);

        assert_eq!(
            external_port.serial_peer(),
            SerialPeer::StagedIncomingByte { byte: 0x00 }
        );
        assert!(external_port.snapshot().printer.is_some());
    }

    #[test]
    fn dmg04_attachment_starts_with_a_disconnected_incoming_line() {
        let mut external_port = ExternalPort::new();

        external_port.set_attachment_kind(ExternalPortAttachmentKind::GameLinkDmg04);

        assert_eq!(
            external_port.attachment_kind(),
            ExternalPortAttachmentKind::GameLinkDmg04
        );
        assert_eq!(external_port.serial_peer(), SerialPeer::Disconnected);
        assert_eq!(external_port.snapshot().dmg04_incoming_byte, None);

        external_port.set_dmg04_incoming_byte(Some(0x3C));

        assert_eq!(
            external_port.serial_peer(),
            SerialPeer::StagedIncomingByte { byte: 0x3C }
        );
        assert_eq!(external_port.snapshot().dmg04_incoming_byte, Some(0x3C));
    }

    #[test]
    fn startup_reset_preserves_the_attachment_kind() {
        let mut external_port = ExternalPort::new();
        external_port.set_attachment_kind(ExternalPortAttachmentKind::GameLinkDmg04);
        external_port.set_dmg04_incoming_byte(Some(0xA5));

        external_port.apply_startup_reset();

        assert_eq!(
            external_port.attachment_kind(),
            ExternalPortAttachmentKind::GameLinkDmg04
        );
        assert_eq!(external_port.serial_peer(), SerialPeer::Disconnected);
        assert_eq!(external_port.snapshot().dmg04_incoming_byte, None);
    }
}
