use crate::backend::CartridgeSaveBackendError;
use crate::wire::{ByteCursor, write_bool, write_u64};
use gb_core::{
    CartridgeSlotState, CompatibilityPolicy, ConsoleModel, DiagnosticPolicy, ExecutionMode,
    HardwareRevision, HeuristicPolicy, HostPlatform, MachineSaveStateMetadata, OperatingMode,
    OverridePolicy, SaveStateByteFingerprint, SgbHostProfile, StartupMode, TCycle,
    ValidationPolicy,
};

pub(super) fn encode_machine_save_state_metadata(
    bytes: &mut Vec<u8>,
    metadata: &MachineSaveStateMetadata,
) -> Result<(), CartridgeSaveBackendError> {
    bytes.push(encode_console_model(metadata.console_model));
    bytes.push(encode_operating_mode(metadata.operating_mode));
    bytes.push(encode_revision(metadata.revision));
    bytes.push(encode_host_platform(metadata.host_platform));
    encode_optional_tag(bytes, metadata.sgb_profile.map(encode_sgb_host_profile));
    bytes.push(encode_startup_mode(metadata.startup_mode));
    encode_compatibility_policy(bytes, &metadata.compatibility);
    write_u64(bytes, metadata.next_t_cycle.get());
    bytes.push(encode_cartridge_slot_state(metadata.cartridge.state));
    encode_fingerprint(bytes, metadata.cartridge.rom_fingerprint);
    bytes.push(encode_startup_mode(metadata.boot.startup_mode));
    write_bool(bytes, metadata.boot.boot_rom_mapped);
    encode_fingerprint(bytes, metadata.boot.boot_rom_fingerprint);
    Ok(())
}

pub(super) fn decode_machine_save_state_metadata(
    cursor: &mut ByteCursor<'_>,
) -> Result<MachineSaveStateMetadata, CartridgeSaveBackendError> {
    let console_model = decode_console_model(cursor.read_u8()?, "console_model")?;
    let operating_mode = decode_operating_mode(cursor.read_u8()?, "operating_mode")?;
    let revision = decode_revision(cursor.read_u8()?, "revision")?;
    let host_platform = decode_host_platform(cursor.read_u8()?, "host_platform")?;
    let sgb_profile = decode_optional_tag(cursor, "sgb_profile")?
        .map(|tag| decode_sgb_host_profile(tag, "sgb_profile"))
        .transpose()?;
    let startup_mode = decode_startup_mode(cursor.read_u8()?, "startup_mode")?;
    let compatibility = decode_compatibility_policy(cursor)?;
    let next_t_cycle = TCycle::new(cursor.read_u64()?);
    let cartridge_state = decode_cartridge_slot_state(cursor.read_u8()?, "cartridge.state")?;
    let rom_fingerprint = decode_fingerprint(cursor, "cartridge.rom_fingerprint")?;
    let boot_startup_mode = decode_startup_mode(cursor.read_u8()?, "boot.startup_mode")?;
    let boot_rom_mapped = cursor.read_bool("boot.boot_rom_mapped")?;
    let boot_rom_fingerprint = decode_fingerprint(cursor, "boot.boot_rom_fingerprint")?;

    Ok(MachineSaveStateMetadata {
        console_model,
        operating_mode,
        revision,
        host_platform,
        sgb_profile,
        startup_mode,
        compatibility,
        next_t_cycle,
        cartridge: gb_core::MachineCartridgeSaveStateMetadata {
            state: cartridge_state,
            rom_fingerprint,
        },
        boot: gb_core::MachineBootSaveStateMetadata {
            startup_mode: boot_startup_mode,
            boot_rom_mapped,
            boot_rom_fingerprint,
        },
    })
}

pub(super) fn encode_compatibility_policy(bytes: &mut Vec<u8>, policy: &CompatibilityPolicy) {
    bytes.push(encode_execution_mode(policy.execution_mode));
    bytes.push(encode_validation_policy(policy.validation_policy));
    bytes.push(encode_heuristic_policy(policy.heuristic_policy));
    encode_override_policy(bytes, &policy.override_policy);
    bytes.push(encode_diagnostic_policy(policy.diagnostic_policy));
}

pub(super) fn decode_compatibility_policy(
    cursor: &mut ByteCursor<'_>,
) -> Result<CompatibilityPolicy, CartridgeSaveBackendError> {
    Ok(CompatibilityPolicy {
        execution_mode: decode_execution_mode(cursor.read_u8()?, "compatibility.execution_mode")?,
        validation_policy: decode_validation_policy(
            cursor.read_u8()?,
            "compatibility.validation_policy",
        )?,
        heuristic_policy: decode_heuristic_policy(
            cursor.read_u8()?,
            "compatibility.heuristic_policy",
        )?,
        override_policy: decode_override_policy(cursor)?,
        diagnostic_policy: decode_diagnostic_policy(
            cursor.read_u8()?,
            "compatibility.diagnostic_policy",
        )?,
    })
}

pub(super) fn encode_override_policy(bytes: &mut Vec<u8>, policy: &OverridePolicy) {
    encode_optional_tag(bytes, policy.forced_console_model.map(encode_console_model));
    encode_optional_tag(
        bytes,
        policy.forced_operating_mode.map(encode_operating_mode),
    );
    encode_optional_tag(bytes, policy.forced_host_platform.map(encode_host_platform));
    encode_optional_tag(bytes, policy.forced_startup_mode.map(encode_startup_mode));
}

pub(super) fn decode_override_policy(
    cursor: &mut ByteCursor<'_>,
) -> Result<OverridePolicy, CartridgeSaveBackendError> {
    Ok(OverridePolicy {
        forced_console_model: decode_optional_tag(cursor, "override.forced_console_model")?
            .map(|tag| decode_console_model(tag, "override.forced_console_model"))
            .transpose()?,
        forced_operating_mode: decode_optional_tag(cursor, "override.forced_operating_mode")?
            .map(|tag| decode_operating_mode(tag, "override.forced_operating_mode"))
            .transpose()?,
        forced_host_platform: decode_optional_tag(cursor, "override.forced_host_platform")?
            .map(|tag| decode_host_platform(tag, "override.forced_host_platform"))
            .transpose()?,
        forced_startup_mode: decode_optional_tag(cursor, "override.forced_startup_mode")?
            .map(|tag| decode_startup_mode(tag, "override.forced_startup_mode"))
            .transpose()?,
    })
}

pub(super) fn encode_optional_tag(bytes: &mut Vec<u8>, tag: Option<u8>) {
    write_bool(bytes, tag.is_some());
    if let Some(tag) = tag {
        bytes.push(tag);
    }
}

pub(super) fn decode_optional_tag(
    cursor: &mut ByteCursor<'_>,
    field: &'static str,
) -> Result<Option<u8>, CartridgeSaveBackendError> {
    let present = cursor.read_bool(field)?;
    if present {
        Ok(Some(cursor.read_u8()?))
    } else {
        Ok(None)
    }
}

pub(super) fn encode_fingerprint(
    bytes: &mut Vec<u8>,
    fingerprint: Option<SaveStateByteFingerprint>,
) {
    write_bool(bytes, fingerprint.is_some());
    if let Some(fingerprint) = fingerprint {
        write_u64(bytes, fingerprint.len);
        write_u64(bytes, fingerprint.fnv1a64);
    }
}

pub(super) fn decode_fingerprint(
    cursor: &mut ByteCursor<'_>,
    field: &'static str,
) -> Result<Option<SaveStateByteFingerprint>, CartridgeSaveBackendError> {
    let present = cursor.read_bool(field)?;
    if !present {
        return Ok(None);
    }

    Ok(Some(SaveStateByteFingerprint {
        len: cursor.read_u64()?,
        fnv1a64: cursor.read_u64()?,
    }))
}

pub(super) fn encode_console_model(value: ConsoleModel) -> u8 {
    match value {
        ConsoleModel::GameBoy => 1,
        ConsoleModel::GameBoyPocket => 2,
        ConsoleModel::GameBoyColor => 3,
        ConsoleModel::GameBoyLight => 4,
        ConsoleModel::GameBoyAdvance => 5,
    }
}

pub(super) fn decode_console_model(
    tag: u8,
    field: &'static str,
) -> Result<ConsoleModel, CartridgeSaveBackendError> {
    match tag {
        0 => Ok(ConsoleModel::GameBoy),
        1 => Ok(ConsoleModel::GameBoy),
        2 => Ok(ConsoleModel::GameBoyPocket),
        3 => Ok(ConsoleModel::GameBoyColor),
        4 => Ok(ConsoleModel::GameBoyLight),
        5 => Ok(ConsoleModel::GameBoyAdvance),
        _ => unsupported_machine_save_state_tag(field, tag),
    }
}

pub(super) fn encode_operating_mode(value: OperatingMode) -> u8 {
    match value {
        OperatingMode::Dmg => 0,
        OperatingMode::Cgb => 1,
        OperatingMode::GbCompatible => 2,
        OperatingMode::CgbDmgExt => 3,
    }
}

pub(super) fn decode_operating_mode(
    tag: u8,
    field: &'static str,
) -> Result<OperatingMode, CartridgeSaveBackendError> {
    match tag {
        0 => Ok(OperatingMode::Dmg),
        1 => Ok(OperatingMode::Cgb),
        2 => Ok(OperatingMode::GbCompatible),
        3 => Ok(OperatingMode::CgbDmgExt),
        _ => unsupported_machine_save_state_tag(field, tag),
    }
}

pub(super) fn encode_revision(value: HardwareRevision) -> u8 {
    match value {
        HardwareRevision::DmgCpu0 => 0,
        HardwareRevision::DmgCpuA => 1,
        HardwareRevision::DmgCpuB => 2,
        HardwareRevision::DmgCpuC => 3,
        HardwareRevision::CpuMgb => 4,
        HardwareRevision::CpuCgb => 5,
        HardwareRevision::CpuCgbA => 6,
        HardwareRevision::CpuCgbB => 7,
        HardwareRevision::CpuCgbC => 8,
        HardwareRevision::CpuCgbD => 9,
        HardwareRevision::CpuCgbE => 10,
        HardwareRevision::CpuAgbA => 11,
    }
}

pub(super) fn decode_revision(
    tag: u8,
    field: &'static str,
) -> Result<HardwareRevision, CartridgeSaveBackendError> {
    match tag {
        0 => Ok(HardwareRevision::DmgCpu0),
        1 => Ok(HardwareRevision::DmgCpuA),
        2 => Ok(HardwareRevision::DmgCpuB),
        3 => Ok(HardwareRevision::DmgCpuC),
        4 => Ok(HardwareRevision::CpuMgb),
        5 => Ok(HardwareRevision::CpuCgb),
        6 => Ok(HardwareRevision::CpuCgbA),
        7 => Ok(HardwareRevision::CpuCgbB),
        8 => Ok(HardwareRevision::CpuCgbC),
        9 => Ok(HardwareRevision::CpuCgbD),
        10 => Ok(HardwareRevision::CpuCgbE),
        11 => Ok(HardwareRevision::CpuAgbA),
        _ => unsupported_machine_save_state_tag(field, tag),
    }
}

pub(super) fn encode_host_platform(value: HostPlatform) -> u8 {
    match value {
        HostPlatform::Handheld => 0,
        HostPlatform::Sgb => 1,
        HostPlatform::Sgb2 => 2,
    }
}

pub(super) fn decode_host_platform(
    tag: u8,
    field: &'static str,
) -> Result<HostPlatform, CartridgeSaveBackendError> {
    match tag {
        0 => Ok(HostPlatform::Handheld),
        1 => Ok(HostPlatform::Sgb),
        2 => Ok(HostPlatform::Sgb2),
        _ => unsupported_machine_save_state_tag(field, tag),
    }
}

pub(super) fn encode_sgb_host_profile(value: SgbHostProfile) -> u8 {
    match value {
        SgbHostProfile::SgbNtsc => 0,
        SgbHostProfile::SgbPal => 1,
        SgbHostProfile::Sgb2Ntsc => 2,
    }
}

pub(super) fn decode_sgb_host_profile(
    tag: u8,
    field: &'static str,
) -> Result<SgbHostProfile, CartridgeSaveBackendError> {
    match tag {
        0 => Ok(SgbHostProfile::SgbNtsc),
        1 => Ok(SgbHostProfile::SgbPal),
        2 => Ok(SgbHostProfile::Sgb2Ntsc),
        _ => unsupported_machine_save_state_tag(field, tag),
    }
}

pub(super) fn encode_startup_mode(value: StartupMode) -> u8 {
    match value {
        StartupMode::SkipBoot => 0,
        StartupMode::RealBoot => 1,
        StartupMode::CustomBoot => 2,
    }
}

pub(super) fn decode_startup_mode(
    tag: u8,
    field: &'static str,
) -> Result<StartupMode, CartridgeSaveBackendError> {
    match tag {
        0 => Ok(StartupMode::SkipBoot),
        1 => Ok(StartupMode::RealBoot),
        2 => Ok(StartupMode::CustomBoot),
        _ => unsupported_machine_save_state_tag(field, tag),
    }
}

pub(super) fn encode_execution_mode(value: ExecutionMode) -> u8 {
    match value {
        ExecutionMode::Strict => 0,
        ExecutionMode::Permissive => 1,
        ExecutionMode::Experimental => 2,
    }
}

pub(super) fn decode_execution_mode(
    tag: u8,
    field: &'static str,
) -> Result<ExecutionMode, CartridgeSaveBackendError> {
    match tag {
        0 => Ok(ExecutionMode::Strict),
        1 => Ok(ExecutionMode::Permissive),
        2 => Ok(ExecutionMode::Experimental),
        _ => unsupported_machine_save_state_tag(field, tag),
    }
}

pub(super) fn encode_validation_policy(value: ValidationPolicy) -> u8 {
    match value {
        ValidationPolicy::Strict => 0,
        ValidationPolicy::Warn => 1,
        ValidationPolicy::Ignore => 2,
    }
}

pub(super) fn decode_validation_policy(
    tag: u8,
    field: &'static str,
) -> Result<ValidationPolicy, CartridgeSaveBackendError> {
    match tag {
        0 => Ok(ValidationPolicy::Strict),
        1 => Ok(ValidationPolicy::Warn),
        2 => Ok(ValidationPolicy::Ignore),
        _ => unsupported_machine_save_state_tag(field, tag),
    }
}

pub(super) fn encode_heuristic_policy(value: HeuristicPolicy) -> u8 {
    match value {
        HeuristicPolicy::Disabled => 0,
        HeuristicPolicy::AllowExperimental => 1,
    }
}

pub(super) fn decode_heuristic_policy(
    tag: u8,
    field: &'static str,
) -> Result<HeuristicPolicy, CartridgeSaveBackendError> {
    match tag {
        0 => Ok(HeuristicPolicy::Disabled),
        1 => Ok(HeuristicPolicy::AllowExperimental),
        _ => unsupported_machine_save_state_tag(field, tag),
    }
}

pub(super) fn encode_diagnostic_policy(value: DiagnosticPolicy) -> u8 {
    match value {
        DiagnosticPolicy::Quiet => 0,
        DiagnosticPolicy::Standard => 1,
        DiagnosticPolicy::Verbose => 2,
    }
}

pub(super) fn decode_diagnostic_policy(
    tag: u8,
    field: &'static str,
) -> Result<DiagnosticPolicy, CartridgeSaveBackendError> {
    match tag {
        0 => Ok(DiagnosticPolicy::Quiet),
        1 => Ok(DiagnosticPolicy::Standard),
        2 => Ok(DiagnosticPolicy::Verbose),
        _ => unsupported_machine_save_state_tag(field, tag),
    }
}

pub(super) fn encode_cartridge_slot_state(value: CartridgeSlotState) -> u8 {
    match value {
        CartridgeSlotState::Empty => 0,
        CartridgeSlotState::NoMbc => 1,
        CartridgeSlotState::Mmm01 => 2,
        CartridgeSlotState::M161 => 3,
        CartridgeSlotState::Huc1 => 4,
        CartridgeSlotState::Huc3 => 5,
        CartridgeSlotState::Mbc1 => 6,
        CartridgeSlotState::Mbc2 => 7,
        CartridgeSlotState::Mbc3 => 8,
        CartridgeSlotState::Mbc5 => 9,
        CartridgeSlotState::PocketCamera => 10,
        CartridgeSlotState::Mbc6 => 11,
        CartridgeSlotState::Mbc7 => 12,
    }
}

pub(super) fn decode_cartridge_slot_state(
    tag: u8,
    field: &'static str,
) -> Result<CartridgeSlotState, CartridgeSaveBackendError> {
    match tag {
        0 => Ok(CartridgeSlotState::Empty),
        1 => Ok(CartridgeSlotState::NoMbc),
        2 => Ok(CartridgeSlotState::Mmm01),
        3 => Ok(CartridgeSlotState::M161),
        4 => Ok(CartridgeSlotState::Huc1),
        5 => Ok(CartridgeSlotState::Huc3),
        6 => Ok(CartridgeSlotState::Mbc1),
        7 => Ok(CartridgeSlotState::Mbc2),
        8 => Ok(CartridgeSlotState::Mbc3),
        9 => Ok(CartridgeSlotState::Mbc5),
        10 => Ok(CartridgeSlotState::PocketCamera),
        11 => Ok(CartridgeSlotState::Mbc6),
        12 => Ok(CartridgeSlotState::Mbc7),
        _ => unsupported_machine_save_state_tag(field, tag),
    }
}

pub(super) fn unsupported_machine_save_state_tag<T>(
    field: &'static str,
    tag: u8,
) -> Result<T, CartridgeSaveBackendError> {
    Err(CartridgeSaveBackendError::UnsupportedMachineSaveStateTag { field, tag })
}
