use super::metadata::*;
use super::*;
use crate::backend::CartridgeSaveBackendError;
use crate::format::{CURRENT_MACHINE_SAVE_STATE_FORMAT_VERSION, MACHINE_SAVE_STATE_MAGIC};
use crate::wire::ByteCursor;
use gb_core::{
    CartridgeSlotState, CompatibilityPolicy, ConsoleModel, DiagnosticPolicy, ExecutionMode,
    HardwareRevision, HeuristicPolicy, HostPlatform, MachineSaveStateMetadata, OperatingMode,
    OverridePolicy, SaveStateByteFingerprint, SgbHostProfile, StartupMode, TCycle,
    ValidationPolicy,
};

fn machine_save_state_envelope() -> MachineSaveStateEnvelope {
    let mut machine = gb_core::Machine::new(
        gb_core::MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    for _ in 0..16 {
        machine.step_t_cycle();
    }
    MachineSaveStateEnvelope::new(machine.capture_save_state())
}

#[test]
fn machine_save_state_metadata_codec_covers_tags_fingerprints_and_overrides() {
    for value in [
        ConsoleModel::GameBoy,
        ConsoleModel::GameBoyPocket,
        ConsoleModel::GameBoyLight,
        ConsoleModel::GameBoyColor,
    ] {
        assert_eq!(
            decode_console_model(encode_console_model(value), "console_model")
                .expect("console model tag should decode"),
            value
        );
    }
    assert!(matches!(
        decode_console_model(0xFF, "console_model"),
        Err(CartridgeSaveBackendError::UnsupportedMachineSaveStateTag {
            field: "console_model",
            tag: 0xFF,
        })
    ));

    for value in [
        OperatingMode::Dmg,
        OperatingMode::Cgb,
        OperatingMode::GbCompatible,
        OperatingMode::CgbDmgExt,
    ] {
        assert_eq!(
            decode_operating_mode(encode_operating_mode(value), "operating_mode")
                .expect("operating mode tag should decode"),
            value
        );
    }
    assert!(matches!(
        decode_operating_mode(0xFF, "operating_mode"),
        Err(CartridgeSaveBackendError::UnsupportedMachineSaveStateTag { .. })
    ));

    for value in [
        HardwareRevision::DmgCpu,
        HardwareRevision::DmgCpuA,
        HardwareRevision::DmgCpuB,
        HardwareRevision::DmgCpuC,
        HardwareRevision::CpuMgb,
        HardwareRevision::CpuCgb,
        HardwareRevision::CpuCgbA,
        HardwareRevision::CpuCgbB,
        HardwareRevision::CpuCgbC,
        HardwareRevision::CpuCgbD,
        HardwareRevision::CpuCgbE,
    ] {
        assert_eq!(
            decode_revision(encode_revision(value), "revision")
                .expect("hardware revision tag should decode"),
            value
        );
    }
    assert!(matches!(
        decode_revision(0xFF, "revision"),
        Err(CartridgeSaveBackendError::UnsupportedMachineSaveStateTag { .. })
    ));

    for value in [
        HostPlatform::Handheld,
        HostPlatform::Sgb,
        HostPlatform::Sgb2,
    ] {
        assert_eq!(
            decode_host_platform(encode_host_platform(value), "host_platform")
                .expect("host platform tag should decode"),
            value
        );
    }
    assert!(matches!(
        decode_host_platform(0xFF, "host_platform"),
        Err(CartridgeSaveBackendError::UnsupportedMachineSaveStateTag { .. })
    ));

    for value in [
        SgbHostProfile::SgbNtsc,
        SgbHostProfile::SgbPal,
        SgbHostProfile::Sgb2Ntsc,
    ] {
        assert_eq!(
            decode_sgb_host_profile(encode_sgb_host_profile(value), "sgb_profile")
                .expect("SGB profile tag should decode"),
            value
        );
    }
    assert!(matches!(
        decode_sgb_host_profile(0xFF, "sgb_profile"),
        Err(CartridgeSaveBackendError::UnsupportedMachineSaveStateTag { .. })
    ));

    for value in [
        StartupMode::SkipBoot,
        StartupMode::CustomBoot,
        StartupMode::RealBoot,
    ] {
        assert_eq!(
            decode_startup_mode(encode_startup_mode(value), "startup_mode")
                .expect("startup mode tag should decode"),
            value
        );
    }
    assert!(matches!(
        decode_startup_mode(0xFF, "startup_mode"),
        Err(CartridgeSaveBackendError::UnsupportedMachineSaveStateTag { .. })
    ));

    for value in [
        ExecutionMode::Strict,
        ExecutionMode::Permissive,
        ExecutionMode::Experimental,
    ] {
        assert_eq!(
            decode_execution_mode(encode_execution_mode(value), "execution_mode")
                .expect("execution mode tag should decode"),
            value
        );
    }
    assert!(matches!(
        decode_execution_mode(0xFF, "execution_mode"),
        Err(CartridgeSaveBackendError::UnsupportedMachineSaveStateTag { .. })
    ));

    for value in [
        ValidationPolicy::Strict,
        ValidationPolicy::Warn,
        ValidationPolicy::Ignore,
    ] {
        assert_eq!(
            decode_validation_policy(encode_validation_policy(value), "validation_policy")
                .expect("validation policy tag should decode"),
            value
        );
    }
    assert!(matches!(
        decode_validation_policy(0xFF, "validation_policy"),
        Err(CartridgeSaveBackendError::UnsupportedMachineSaveStateTag { .. })
    ));

    for value in [
        HeuristicPolicy::Disabled,
        HeuristicPolicy::AllowExperimental,
    ] {
        assert_eq!(
            decode_heuristic_policy(encode_heuristic_policy(value), "heuristic_policy")
                .expect("heuristic policy tag should decode"),
            value
        );
    }
    assert!(matches!(
        decode_heuristic_policy(0xFF, "heuristic_policy"),
        Err(CartridgeSaveBackendError::UnsupportedMachineSaveStateTag { .. })
    ));

    for value in [
        DiagnosticPolicy::Quiet,
        DiagnosticPolicy::Standard,
        DiagnosticPolicy::Verbose,
    ] {
        assert_eq!(
            decode_diagnostic_policy(encode_diagnostic_policy(value), "diagnostic_policy")
                .expect("diagnostic policy tag should decode"),
            value
        );
    }
    assert!(matches!(
        decode_diagnostic_policy(0xFF, "diagnostic_policy"),
        Err(CartridgeSaveBackendError::UnsupportedMachineSaveStateTag { .. })
    ));

    for value in [
        CartridgeSlotState::Empty,
        CartridgeSlotState::NoMbc,
        CartridgeSlotState::Mmm01,
        CartridgeSlotState::M161,
        CartridgeSlotState::Huc1,
        CartridgeSlotState::Huc3,
        CartridgeSlotState::Mbc1,
        CartridgeSlotState::Mbc2,
        CartridgeSlotState::Mbc3,
        CartridgeSlotState::Mbc5,
        CartridgeSlotState::Mbc6,
        CartridgeSlotState::Mbc7,
        CartridgeSlotState::PocketCamera,
    ] {
        assert_eq!(
            decode_cartridge_slot_state(encode_cartridge_slot_state(value), "cartridge.state")
                .expect("cartridge slot tag should decode"),
            value
        );
    }
    assert!(matches!(
        decode_cartridge_slot_state(0xFF, "cartridge.state"),
        Err(CartridgeSaveBackendError::UnsupportedMachineSaveStateTag { .. })
    ));

    let metadata = MachineSaveStateMetadata {
        console_model: ConsoleModel::GameBoyColor,
        operating_mode: OperatingMode::GbCompatible,
        revision: HardwareRevision::CpuCgbE,
        host_platform: HostPlatform::Sgb2,
        sgb_profile: Some(SgbHostProfile::Sgb2Ntsc),
        startup_mode: StartupMode::RealBoot,
        compatibility: CompatibilityPolicy {
            execution_mode: ExecutionMode::Experimental,
            validation_policy: ValidationPolicy::Ignore,
            heuristic_policy: HeuristicPolicy::AllowExperimental,
            override_policy: OverridePolicy {
                forced_console_model: Some(ConsoleModel::GameBoyPocket),
                forced_operating_mode: Some(OperatingMode::Dmg),
                forced_host_platform: Some(HostPlatform::Sgb),
                forced_startup_mode: Some(StartupMode::SkipBoot),
            },
            diagnostic_policy: DiagnosticPolicy::Verbose,
        },
        next_t_cycle: TCycle::new(0x1234_5678),
        cartridge: gb_core::MachineCartridgeSaveStateMetadata {
            state: CartridgeSlotState::PocketCamera,
            rom_fingerprint: Some(SaveStateByteFingerprint {
                len: 1024 * 1024,
                fnv1a64: 0xA5A5_5A5A_DEAD_BEEF,
            }),
        },
        boot: gb_core::MachineBootSaveStateMetadata {
            startup_mode: StartupMode::RealBoot,
            boot_rom_mapped: true,
            boot_rom_fingerprint: Some(SaveStateByteFingerprint {
                len: 0x900,
                fnv1a64: 0x55AA_AA55_1234_5678,
            }),
        },
    };

    let mut bytes = Vec::new();
    encode_machine_save_state_metadata(&mut bytes, &metadata).expect("metadata should encode");
    let mut cursor = ByteCursor::new(&bytes);
    assert_eq!(
        decode_machine_save_state_metadata(&mut cursor).expect("metadata should decode"),
        metadata
    );
    assert_eq!(cursor.remaining(), 0);
}

#[test]
fn machine_save_state_envelope_round_trips_the_versioned_payload() {
    let envelope = machine_save_state_envelope();
    let bytes = encode_machine_save_state_envelope(&envelope).expect("encode should succeed");

    assert_eq!(
        &bytes[..MACHINE_SAVE_STATE_MAGIC.len()],
        MACHINE_SAVE_STATE_MAGIC.as_slice()
    );

    let decoded = decode_machine_save_state_envelope(&bytes).expect("decode should succeed");
    assert_eq!(decoded, envelope);
}

#[test]
fn machine_save_state_decode_rejects_invalid_headers_and_payload_shape() {
    let envelope = machine_save_state_envelope();
    let encoded = encode_machine_save_state_envelope(&envelope).expect("encode should succeed");

    let mut invalid_magic = encoded.clone();
    invalid_magic[0] = b'X';
    assert!(matches!(
        decode_machine_save_state_envelope(&invalid_magic),
        Err(CartridgeSaveBackendError::InvalidMagic { .. })
    ));

    let mut future_version = encoded.clone();
    future_version[MACHINE_SAVE_STATE_MAGIC.len()..MACHINE_SAVE_STATE_MAGIC.len() + 2]
        .copy_from_slice(&(CURRENT_MACHINE_SAVE_STATE_FORMAT_VERSION + 1).to_le_bytes());
    assert!(matches!(
        decode_machine_save_state_envelope(&future_version),
        Err(CartridgeSaveBackendError::UnsupportedFormatVersion {
            version
        }) if version == CURRENT_MACHINE_SAVE_STATE_FORMAT_VERSION + 1
    ));

    let mut invalid_model_tag = encoded.clone();
    invalid_model_tag[MACHINE_SAVE_STATE_MAGIC.len() + 2] = 0xFF;
    assert!(matches!(
        decode_machine_save_state_envelope(&invalid_model_tag),
        Err(CartridgeSaveBackendError::UnsupportedMachineSaveStateTag {
            field: "console_model",
            tag: 0xFF,
        })
    ));

    let truncated = &encoded[..encoded.len() - 1];
    assert!(matches!(
        decode_machine_save_state_envelope(truncated),
        Err(CartridgeSaveBackendError::UnexpectedEof { .. })
    ));

    let mut corrupt_payload = encoded.clone();
    let last = corrupt_payload
        .last_mut()
        .expect("payload should not be empty");
    *last ^= 0x5A;
    assert!(matches!(
        decode_machine_save_state_envelope(&corrupt_payload),
        Err(CartridgeSaveBackendError::MachineSaveStateCodec {
            operation: "decode",
            ..
        })
    ));

    let mut trailing = encoded.clone();
    trailing.push(0xAA);
    assert!(matches!(
        decode_machine_save_state_envelope(&trailing),
        Err(CartridgeSaveBackendError::TrailingBytes { remaining: 1 })
    ));
}

#[test]
fn machine_save_state_decode_rejects_metadata_that_disagrees_with_payload() {
    let envelope = machine_save_state_envelope();
    let mut encoded = encode_machine_save_state_envelope(&envelope).expect("encode should succeed");

    let operating_mode_offset = MACHINE_SAVE_STATE_MAGIC.len() + 3;
    encoded[operating_mode_offset] = encode_operating_mode(OperatingMode::GbCompatible);

    assert!(matches!(
        decode_machine_save_state_envelope(&encoded),
        Err(CartridgeSaveBackendError::MachineSaveStateMetadataMismatch)
    ));
}
