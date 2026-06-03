use super::*;
use crate::backend::InMemoryCartridgeSaveBackend;
use crate::time::{
    CartridgeSaveTimeSource, FixedCartridgeSaveTimeSource, SystemCartridgeSaveTimeSource,
};
use gb_core::{CartridgePersistenceMetadata, CartridgePersistenceProfile, CartridgeRamPayloadKind};

#[test]
fn default_backends_time_sources_and_battery_policy_are_explicit() {
    let _ = SystemCartridgeSaveTimeSource.now_unix_seconds();

    let empty_backend = InMemoryCartridgeSaveBackend::new();
    assert!(empty_backend.is_empty());

    let default_backend = InMemoryCartridgeSaveBackend::default();
    assert_eq!(default_backend.len(), 0);

    assert_eq!(FixedCartridgeSaveTimeSource::new(42).now_unix_seconds(), 42);

    assert!(uses_battery_backed_hardware_persistence(
        CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: true,
            profile: CartridgePersistenceProfile::PersistentRtc,
        }
    ));
    assert!(uses_battery_backed_hardware_persistence(
        CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: false,
            profile: CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear { byte_len: 8 },
            },
        }
    ));
    assert!(!uses_battery_backed_hardware_persistence(
        CartridgePersistenceMetadata {
            has_battery: false,
            has_rtc: true,
            profile: CartridgePersistenceProfile::PersistentRtc,
        }
    ));
    assert!(!uses_battery_backed_hardware_persistence(
        CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: false,
            profile: CartridgePersistenceProfile::None,
        }
    ));
    assert!(uses_battery_backed_hardware_persistence(
        CartridgePersistenceMetadata {
            has_battery: false,
            has_rtc: false,
            profile: CartridgePersistenceProfile::PersistentEeprom { byte_len: 256 },
        }
    ));
}
