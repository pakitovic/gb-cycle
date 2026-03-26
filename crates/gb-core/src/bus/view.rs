use super::BusMaster;
use super::video::{OamDomain, VramDomain};

pub(crate) type OamBusView<'a> = VideoBusView<'a, OamDomain>;
pub(crate) type VramBusView<'a> = VideoBusView<'a, VramDomain>;

#[derive(Debug)]
pub(crate) struct VideoBusView<'a, D> {
    master: BusMaster,
    domain: &'a mut D,
}

impl<'a, D> VideoBusView<'a, D>
where
    D: VideoDomain,
{
    pub(crate) fn new(master: BusMaster, domain: &'a mut D) -> Self {
        Self { master, domain }
    }

    pub(crate) const fn master(&self) -> BusMaster {
        self.master
    }

    pub(crate) fn is_acquired_by_master(&self) -> bool {
        self.domain.is_acquired_by(self.master)
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        self.domain.bytes()
    }

    pub(crate) fn is_acquired(&self) -> bool {
        self.domain.is_acquired()
    }
}

pub(crate) trait VideoDomain {
    fn is_acquired(&self) -> bool;
    fn is_acquired_by(&self, master: BusMaster) -> bool;
    fn bytes(&self) -> &[u8];
}

impl<'a> VideoBusView<'a, OamDomain> {
    #[cfg(test)]
    pub(crate) fn acquire(&mut self) {
        self.domain.acquire(self.master);
    }

    #[cfg(test)]
    pub(crate) fn release(&mut self) {
        self.domain.release(self.master);
    }
}

impl<'a> VideoBusView<'a, VramDomain> {
    #[cfg(test)]
    pub(crate) fn acquire(&mut self) {
        self.domain.acquire(self.master);
    }

    #[cfg(test)]
    pub(crate) fn release(&mut self) {
        self.domain.release(self.master);
    }
}

impl VideoDomain for OamDomain {
    fn is_acquired(&self) -> bool {
        OamDomain::is_acquired(self)
    }

    fn is_acquired_by(&self, master: BusMaster) -> bool {
        OamDomain::is_acquired_by(self, master)
    }

    fn bytes(&self) -> &[u8] {
        OamDomain::bytes(self)
    }
}

impl VideoDomain for VramDomain {
    fn is_acquired(&self) -> bool {
        VramDomain::is_acquired(self)
    }

    fn is_acquired_by(&self, master: BusMaster) -> bool {
        VramDomain::is_acquired_by(self, master)
    }

    fn bytes(&self) -> &[u8] {
        VramDomain::bytes(self)
    }
}
