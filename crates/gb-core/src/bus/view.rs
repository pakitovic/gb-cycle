use super::BusMaster;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OamBusView<'a> {
    master: BusMaster,
    bytes: &'a [u8],
}

impl<'a> OamBusView<'a> {
    pub(crate) const fn new(master: BusMaster, bytes: &'a [u8]) -> Self {
        Self { master, bytes }
    }

    pub(crate) const fn master(self) -> BusMaster {
        self.master
    }

    pub(crate) fn bytes(self) -> &'a [u8] {
        self.bytes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct VramBusView<'a> {
    master: BusMaster,
    bytes: &'a [u8],
}

impl<'a> VramBusView<'a> {
    pub(crate) const fn new(master: BusMaster, bytes: &'a [u8]) -> Self {
        Self { master, bytes }
    }

    pub(crate) const fn master(self) -> BusMaster {
        self.master
    }

    pub(crate) fn bytes(self) -> &'a [u8] {
        self.bytes
    }
}
