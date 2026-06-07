mod border;
mod borrowed;
mod host;
mod protocol;
mod shell;

#[cfg(test)]
mod test;

pub use borrowed::{
    SGB_BORROWED_BORDER_EXTRACTION_FRAME_LIMIT, SgbBorrowedBorder,
    extract_initial_sgb_borrowed_border, sgb_header_accepts_borrowed_border,
};
pub use host::{
    DeterministicHleSgbHostBackend, SgbAudioState, SgbCommandState, SgbHost, SgbHostBackend,
    SgbHostBackendKind, SgbHostBackendRequest, SgbHostBackendRequestKind, SgbHostBackendResponse,
    SgbHostSaveState, SgbHostSnapshot, SgbMultiplayerState, SgbPacketGateState, SgbPacketTrace,
    SgbPacketTransportState, SgbSnesHostState, SgbStartupState, SgbSystemControlState,
    SgbVideoState,
};
pub(crate) use protocol::SgbVramTransferDisplayState;

pub use protocol::{
    SGB_ATF_BYTES, SGB_ATF_COUNT, SGB_ATF_TOTAL_BYTES, SGB_ATTR_MAP_CELLS, SGB_ATTR_MAP_HEIGHT,
    SGB_ATTR_MAP_WIDTH, SGB_BORDER_PALETTE_COLORS, SGB_BORDER_PALETTE_COUNT, SGB_BORDER_TILE_BYTES,
    SGB_BORDER_TILE_COUNT, SGB_BORDER_TILE_DATA_BYTES, SGB_BORDER_TILEMAP_ENTRIES,
    SGB_BORDER_TILEMAP_STORED_HEIGHT, SGB_BORDER_TILEMAP_VISIBLE_HEIGHT, SGB_BORDER_TILEMAP_WIDTH,
    SGB_COMMAND_MAX_PACKETS, SGB_COMMAND_PACKET_BYTES, SGB_CONTROLLER_COUNT,
    SGB_DATA_SND_INLINE_BYTES, SGB_FRAME_HEIGHT, SGB_FRAME_PIXELS, SGB_FRAME_WIDTH,
    SGB_LCD_FRAME_ORIGIN_X, SGB_LCD_FRAME_ORIGIN_Y, SGB_LCD_HEIGHT, SGB_LCD_PIXELS, SGB_LCD_WIDTH,
    SGB_OBJ_OAM_PAYLOAD_BYTES, SGB_SCREEN_PALETTE_COLORS, SGB_SCREEN_PALETTE_COUNT,
    SGB_SNES_DATA_TRN_BYTES, SGB_SYSTEM_PALETTE_COUNT, SGB_VRAM_TRANSFER_BYTES, SgbApuRamAddress,
    SgbAttributeFileState, SgbAttributeMap, SgbAttributeState, SgbBorderMapEntry, SgbBorderPalette,
    SgbBorderState, SgbBorderTileData, SgbBorderTileMap, SgbChrTransferSelection,
    SgbChrTransferTileType, SgbCommandAcceptance, SgbCompletedVramTransfer, SgbDataSendRequest,
    SgbDataTransferRequest, SgbFrameCompositionError, SgbHostAudioRequest, SgbHostStatus,
    SgbJoypLineState, SgbJumpRequest, SgbLcdCompositionError, SgbLcdRgb555Frame, SgbObjOamPayload,
    SgbObjTransferState, SgbPacketTraceStatus, SgbPacketTransportPhase, SgbPalSetOptions,
    SgbPaletteState, SgbPendingVramTransfer, SgbPlayerPaletteOverrideState, SgbRealBootAsset,
    SgbRgb555Color, SgbScreenMask, SgbScreenPalette, SgbSnesAddress, SgbSnesHostRequest,
    SgbSoundEffectControl, SgbSoundRequest, SgbSoundTransferPacket, SgbSoundTransferRequest,
    SgbSystemPaletteState, SgbVramTransferBuffer, SgbVramTransferError, SgbVramTransferPhase,
    SgbVramTransferSourceMode, SgbVramTransferState, SgbVramTransferTarget,
};
pub use shell::{
    SGB_SHELL_BORDER_FADE_FRAMES, SgbShellBorderTransitionPhase, SgbShellBorderTransitionState,
    SgbShellState,
};
