use super::*;

#[test]
fn stem_output_paths_use_sidecar_channel_suffixes() {
    assert_eq!(channel_stem_suffix(ApuRecordedChannel::Ch1), "ch1");
    assert_eq!(channel_stem_suffix(ApuRecordedChannel::Ch4), "ch4");
    assert_eq!(
        stem_output_path(&PathBuf::from("/tmp/zelda.wav"), ApuRecordedChannel::Ch4,)
            .expect("stem output path"),
        PathBuf::from("/tmp/zelda.ch4.wav")
    );
    assert_eq!(
        stem_output_path(&PathBuf::from("/tmp/zelda.aifc"), ApuRecordedChannel::Ch2,)
            .expect("stem output path"),
        PathBuf::from("/tmp/zelda.ch2.aifc")
    );
    assert!(
        stem_output_path(&PathBuf::from("/tmp/zelda"), ApuRecordedChannel::Ch1)
            .expect_err("paths without extensions should fail")
            .contains("supported extension")
    );
    assert!(
        stem_output_path(&PathBuf::from("/"), ApuRecordedChannel::Ch1)
            .expect_err("paths without filename stems should fail")
            .contains("filename stem")
    );
}

#[test]
fn automatic_audio_recordings_use_an_audios_sidecar_directory() {
    let root = temp_recording_path("dir");
    fs::create_dir_all(&root).expect("root directory");
    let rom_path = root.join("zelda.gb");
    fs::write(&rom_path, b"rom").expect("rom file");

    let first = resolve_next_audio_recording_output_path(Some(&rom_path), &root)
        .expect("first automatic path");
    assert_eq!(first, root.join("audios/zelda-0.wav"));

    fs::create_dir_all(first.parent().expect("audio output parent")).expect("audio dir");
    fs::write(&first, b"existing").expect("existing recording");

    let second = resolve_next_audio_recording_output_path(Some(&rom_path), &root)
        .expect("second automatic path");
    assert_eq!(second, root.join("audios/zelda-1.wav"));

    let _ = fs::remove_file(first);
    let _ = fs::remove_file(rom_path);
    let _ = fs::remove_dir_all(root.join("audios"));
    let _ = fs::remove_dir(root);
}

#[test]
fn automatic_audio_recording_helpers_fall_back_without_a_loaded_rom() {
    let root = temp_recording_path("dir");
    fs::create_dir_all(&root).expect("root directory");

    assert_eq!(
        audio_recording_output_directory(None, &root),
        root.join("audios")
    );
    assert_eq!(audio_recording_output_stem(None), "gb-cycle");

    let path = resolve_next_audio_recording_output_path(None, &root)
        .expect("automatic recording path without a rom");
    assert_eq!(path, root.join("audios/gb-cycle-0.wav"));

    let _ = fs::remove_dir_all(root.join("audios"));
    let _ = fs::remove_dir(root);
}
