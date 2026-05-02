use super::*;

#[test]
fn ppu_save_state_defaults_missing_cgb_palette_state() {
    let state = Ppu::new(ConsoleModel::GameBoyColor).capture_save_state();
    let mut serialized =
        serde_json::to_value(&state).expect("PPU save-state should serialize to JSON");

    let fields = serialized
        .as_object_mut()
        .expect("PPU save-state should serialize as a JSON object");
    assert!(
        fields.remove("cgb_palettes").is_some(),
        "fixture must model pre-CGB-palette save states"
    );

    let restored: PpuSaveState = serde_json::from_value(serialized)
        .expect("missing CGB palettes should deserialize through the default state");

    assert_eq!(restored.cgb_palettes, CgbPaletteState::default());
}
