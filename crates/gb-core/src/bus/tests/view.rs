use super::*;

#[test]
fn video_views_expose_master_owned_acquire_release_and_bytes() {
    let mut oam = OamDomain::new();
    let mut vram = VramDomain::new();

    {
        let mut oam_view = OamBusView::new(BusMaster::Ppu, &mut oam);
        assert_eq!(oam_view.master(), BusMaster::Ppu);
        assert!(!oam_view.is_acquired());
        assert!(!oam_view.is_acquired_by_master());
        assert!(oam_view.read(OAM_LEN - 1).is_some());
        assert!(oam_view.read(OAM_LEN).is_none());

        oam_view.acquire();
        assert!(oam_view.is_acquired());
        assert!(oam_view.is_acquired_by_master());

        oam_view.release();
        assert!(!oam_view.is_acquired());
        assert!(!oam_view.is_acquired_by_master());
    }

    {
        let mut vram_view = VramBusView::new(BusMaster::Dma, &mut vram);
        assert_eq!(vram_view.master(), BusMaster::Dma);
        assert!(!vram_view.is_acquired());
        assert!(!vram_view.is_acquired_by_master());
        assert!(vram_view.read(VRAM_LEN - 1).is_some());
        assert!(vram_view.read(VRAM_LEN).is_none());

        vram_view.acquire();
        assert!(vram_view.is_acquired());
        assert!(vram_view.is_acquired_by_master());

        vram_view.release();
        assert!(!vram_view.is_acquired());
        assert!(!vram_view.is_acquired_by_master());
    }
}
