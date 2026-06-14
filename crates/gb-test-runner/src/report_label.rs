use gb_core::HardwareRevision;

pub(crate) const REPORT_REVISION_SUFFIXES: [&str; 13] = [
    "(DMG-CPU-0)",
    "(DMG-CPU-A)",
    "(DMG-CPU-B)",
    "(DMG-CPU-C)",
    "(CPU-MGB)",
    "(CPU-CGB-0)",
    "(CPU-CGB-A)",
    "(CPU-CGB-B)",
    "(CPU-CGB-C)",
    "(CPU-CGB-D)",
    "(CPU-CGB-E)",
    "(CPU-AGB-0)",
    "(CPU-AGB-A)",
];

pub(crate) const fn hardware_revision_report_suffix(revision: HardwareRevision) -> &'static str {
    match revision {
        HardwareRevision::DmgCpu0 => "(DMG-CPU-0)",
        HardwareRevision::DmgCpuA => "(DMG-CPU-A)",
        HardwareRevision::DmgCpuB => "(DMG-CPU-B)",
        HardwareRevision::DmgCpuC => "(DMG-CPU-C)",
        HardwareRevision::CpuMgb => "(CPU-MGB)",
        HardwareRevision::CpuCgb0 => "(CPU-CGB-0)",
        HardwareRevision::CpuCgbA => "(CPU-CGB-A)",
        HardwareRevision::CpuCgbB => "(CPU-CGB-B)",
        HardwareRevision::CpuCgbC => "(CPU-CGB-C)",
        HardwareRevision::CpuCgbD => "(CPU-CGB-D)",
        HardwareRevision::CpuCgbE => "(CPU-CGB-E)",
        HardwareRevision::CpuAgb0 => "(CPU-AGB-0)",
        HardwareRevision::CpuAgbA => "(CPU-AGB-A)",
    }
}
