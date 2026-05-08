mod printer;

pub use printer::{
    PrintedPage, PrinterCommand, PrinterMargins, PrinterPrintArgs, PrinterSnapshot,
    PrinterStatusBits,
};

use crate::link::Dmg07Port;
use crate::serial::SerialPeer;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum ExternalPortAttachmentKind {
    #[default]
    None,
    Loopback,
    Printer,
    GameLinkDmg04,
    FourPlayerAdapterDmg07,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum ExternalPortResetPolicy {
    #[default]
    PreserveAttachmentKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct ExternalPort {
    attachment: ExternalPortAttachment,
    reset_policy: ExternalPortResetPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ExternalPortSaveState {
    attachment: ExternalPortAttachment,
    reset_policy: ExternalPortResetPolicy,
}

impl ExternalPortSaveState {
    pub(crate) fn dynamic_payload_bytes(&self) -> usize {
        self.attachment.dynamic_payload_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ExternalPortSnapshot {
    pub reset_policy: ExternalPortResetPolicy,
    pub attachment: ExternalPortAttachmentSnapshot,
}

impl ExternalPortSnapshot {
    pub const fn attachment_kind(&self) -> ExternalPortAttachmentKind {
        self.attachment.kind()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ExternalPortAttachmentSnapshot {
    None,
    Loopback,
    Printer(PrinterSnapshot),
    GameLinkDmg04 {
        incoming_byte: Option<u8>,
    },
    FourPlayerAdapterDmg07 {
        port: Dmg07Port,
        incoming_byte: Option<u8>,
    },
}

impl ExternalPortAttachmentSnapshot {
    pub const fn kind(&self) -> ExternalPortAttachmentKind {
        match self {
            Self::None => ExternalPortAttachmentKind::None,
            Self::Loopback => ExternalPortAttachmentKind::Loopback,
            Self::Printer(_) => ExternalPortAttachmentKind::Printer,
            Self::GameLinkDmg04 { .. } => ExternalPortAttachmentKind::GameLinkDmg04,
            Self::FourPlayerAdapterDmg07 { .. } => {
                ExternalPortAttachmentKind::FourPlayerAdapterDmg07
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
enum ExternalPortAttachment {
    #[default]
    None,
    Loopback(LoopbackAttachmentState),
    Printer(printer::PrinterDevice),
    GameLinkDmg04(Dmg04AttachmentState),
    FourPlayerAdapterDmg07(Dmg07AttachmentState),
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
struct LoopbackAttachmentState;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
struct Dmg04AttachmentState {
    incoming_byte: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
struct Dmg07AttachmentState {
    port: Dmg07Port,
    incoming_byte: Option<u8>,
}

impl Default for Dmg07AttachmentState {
    fn default() -> Self {
        Self {
            port: Dmg07Port::P1,
            incoming_byte: None,
        }
    }
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

    pub(crate) fn capture_save_state(&self) -> ExternalPortSaveState {
        ExternalPortSaveState {
            attachment: self.attachment.clone(),
            reset_policy: self.reset_policy,
        }
    }

    pub(crate) fn restore_save_state(&mut self, state: &ExternalPortSaveState) {
        self.attachment = state.attachment.clone();
        self.reset_policy = state.reset_policy;
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
                self.attachment.reset_transient_state()
            }
        };
    }

    pub fn snapshot(&self) -> ExternalPortSnapshot {
        ExternalPortSnapshot {
            reset_policy: self.reset_policy,
            attachment: self.attachment.snapshot(),
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

    pub(crate) fn requires_t_cycle_tick(&self) -> bool {
        matches!(&self.attachment, ExternalPortAttachment::Printer(_))
    }

    pub(crate) fn requires_serial_peer_refresh_after_t_cycle(&self) -> bool {
        matches!(&self.attachment, ExternalPortAttachment::Printer(_))
    }

    pub(crate) fn handles_completed_serial_byte(&self) -> bool {
        matches!(&self.attachment, ExternalPortAttachment::Printer(_))
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

    pub(crate) fn set_dmg07_attachment(&mut self, port: Dmg07Port) {
        self.attachment = ExternalPortAttachment::FourPlayerAdapterDmg07(Dmg07AttachmentState {
            port,
            incoming_byte: None,
        });
    }

    pub(crate) fn dmg07_port(&self) -> Option<Dmg07Port> {
        match self.attachment {
            ExternalPortAttachment::FourPlayerAdapterDmg07(endpoint) => Some(endpoint.port),
            _ => None,
        }
    }

    pub(crate) fn set_dmg07_incoming_byte(&mut self, incoming_byte: Option<u8>) {
        if let ExternalPortAttachment::FourPlayerAdapterDmg07(endpoint) = &mut self.attachment {
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
            ExternalPortAttachment::FourPlayerAdapterDmg07(endpoint) => endpoint.serial_peer(),
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
            Self::FourPlayerAdapterDmg07(_) => ExternalPortAttachmentKind::FourPlayerAdapterDmg07,
        }
    }

    fn dynamic_payload_bytes(&self) -> usize {
        match self {
            Self::Printer(printer) => printer.dynamic_payload_bytes(),
            Self::None
            | Self::Loopback(_)
            | Self::GameLinkDmg04(_)
            | Self::FourPlayerAdapterDmg07(_) => 0,
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
            ExternalPortAttachmentKind::FourPlayerAdapterDmg07 => {
                Self::FourPlayerAdapterDmg07(Dmg07AttachmentState::default())
            }
        }
    }

    fn reset_transient_state(&self) -> Self {
        match self {
            Self::None => Self::None,
            Self::Loopback(_) => Self::Loopback(LoopbackAttachmentState),
            Self::Printer(_) => Self::Printer(printer::PrinterDevice::new()),
            Self::GameLinkDmg04(_) => Self::GameLinkDmg04(Dmg04AttachmentState::default()),
            Self::FourPlayerAdapterDmg07(endpoint) => {
                Self::FourPlayerAdapterDmg07(Dmg07AttachmentState {
                    port: endpoint.port,
                    incoming_byte: None,
                })
            }
        }
    }

    fn snapshot(&self) -> ExternalPortAttachmentSnapshot {
        match self {
            Self::None => ExternalPortAttachmentSnapshot::None,
            Self::Loopback(_) => ExternalPortAttachmentSnapshot::Loopback,
            Self::Printer(printer) => ExternalPortAttachmentSnapshot::Printer(printer.snapshot()),
            Self::GameLinkDmg04(endpoint) => ExternalPortAttachmentSnapshot::GameLinkDmg04 {
                incoming_byte: endpoint.incoming_byte,
            },
            Self::FourPlayerAdapterDmg07(endpoint) => {
                ExternalPortAttachmentSnapshot::FourPlayerAdapterDmg07 {
                    port: endpoint.port,
                    incoming_byte: endpoint.incoming_byte,
                }
            }
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

impl Dmg07AttachmentState {
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
        assert_eq!(
            external_port.snapshot().attachment,
            ExternalPortAttachmentSnapshot::None
        );
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
        assert!(matches!(
            external_port.snapshot().attachment,
            ExternalPortAttachmentSnapshot::Printer(_)
        ));
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
        assert_eq!(
            external_port.snapshot().attachment,
            ExternalPortAttachmentSnapshot::GameLinkDmg04 {
                incoming_byte: None
            }
        );

        external_port.set_dmg04_incoming_byte(Some(0x3C));

        assert_eq!(
            external_port.serial_peer(),
            SerialPeer::StagedIncomingByte { byte: 0x3C }
        );
        assert_eq!(
            external_port.snapshot().attachment,
            ExternalPortAttachmentSnapshot::GameLinkDmg04 {
                incoming_byte: Some(0x3C)
            }
        );
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
        assert_eq!(
            external_port.snapshot().attachment,
            ExternalPortAttachmentSnapshot::GameLinkDmg04 {
                incoming_byte: None
            }
        );
    }

    #[test]
    fn dmg07_attachment_preserves_port_and_clears_transient_incoming_on_reset() {
        let mut external_port = ExternalPort::new();
        external_port.set_dmg07_attachment(Dmg07Port::P4);
        external_port.set_dmg07_incoming_byte(Some(0xFE));

        assert_eq!(
            external_port.attachment_kind(),
            ExternalPortAttachmentKind::FourPlayerAdapterDmg07
        );
        assert_eq!(external_port.dmg07_port(), Some(Dmg07Port::P4));
        assert_eq!(
            external_port.serial_peer(),
            SerialPeer::StagedIncomingByte { byte: 0xFE }
        );
        assert_eq!(
            external_port.snapshot().attachment,
            ExternalPortAttachmentSnapshot::FourPlayerAdapterDmg07 {
                port: Dmg07Port::P4,
                incoming_byte: Some(0xFE),
            }
        );

        external_port.apply_startup_reset();

        assert_eq!(
            external_port.attachment_kind(),
            ExternalPortAttachmentKind::FourPlayerAdapterDmg07
        );
        assert_eq!(external_port.dmg07_port(), Some(Dmg07Port::P4));
        assert_eq!(external_port.serial_peer(), SerialPeer::Disconnected);
        assert_eq!(
            external_port.snapshot().attachment,
            ExternalPortAttachmentSnapshot::FourPlayerAdapterDmg07 {
                port: Dmg07Port::P4,
                incoming_byte: None,
            }
        );
    }
}
