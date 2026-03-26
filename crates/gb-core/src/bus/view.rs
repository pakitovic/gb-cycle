use super::BusMaster;
use super::video::{OamDomain, VramDomain};

#[derive(Debug)]
pub(crate) struct OamBusView<'a> {
    master: BusMaster,
    domain: &'a mut OamDomain,
}

impl<'a> OamBusView<'a> {
    pub(crate) fn new(master: BusMaster, domain: &'a mut OamDomain) -> Self {
        Self { master, domain }
    }

    pub(crate) const fn master(&self) -> BusMaster {
        self.master
    }

    #[allow(dead_code)]
    pub(crate) fn acquire(&mut self) {
        self.domain.acquire(self.master);
    }

    #[allow(dead_code)]
    pub(crate) fn release(&mut self) {
        self.domain.release(self.master);
    }

    #[allow(dead_code)]
    #[allow(dead_code)]
    pub(crate) fn is_acquired(&self) -> bool {
        self.domain.is_acquired()
    }

    pub(crate) fn is_acquired_by_this(&self) -> bool {
        self.domain.is_acquired_by(self.master)
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        self.domain.bytes()
    }
}

#[derive(Debug)]
pub(crate) struct VramBusView<'a> {
    master: BusMaster,
    domain: &'a mut VramDomain,
}

impl<'a> VramBusView<'a> {
    pub(crate) fn new(master: BusMaster, domain: &'a mut VramDomain) -> Self {
        Self { master, domain }
    }

    pub(crate) const fn master(&self) -> BusMaster {
        self.master
    }

    #[allow(dead_code)]
    pub(crate) fn acquire(&mut self) {
        self.domain.acquire(self.master);
    }

    #[allow(dead_code)]
    pub(crate) fn release(&mut self) {
        self.domain.release(self.master);
    }

    #[allow(dead_code)]
    pub(crate) fn is_acquired(&self) -> bool {
        self.domain.is_acquired()
    }

    pub(crate) fn is_acquired_by_this(&self) -> bool {
        self.domain.is_acquired_by(self.master)
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        self.domain.bytes()
    }
}
