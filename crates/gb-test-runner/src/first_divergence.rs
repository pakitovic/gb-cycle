use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use gb_core::ExecutionMode;
use serde::{Deserialize, Serialize};

use crate::{
    RomCaseValidationError, RomExecutionError, RomRunner, RomSuite, RomSuiteValidationError,
    RomTestCase, RunnerMachine, SameBoyCaseBundleExecutionError, SameBoyCaseBundleRunner,
    budget_exhausted,
};

const FNV1A64_OFFSET: u64 = 14_695_981_039_346_656_037;
const FNV1A64_PRIME: u64 = 1_099_511_628_211;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirstDivergenceCompareMode {
    Framebuffer,
    State,
}

impl FirstDivergenceCompareMode {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Framebuffer => "framebuffer",
            Self::State => "state",
        }
    }
}

#[derive(Debug)]
pub enum FirstDivergenceExecutionError {
    InvalidProbeInterval {
        probe_interval_tcycles: u64,
    },
    InvalidSuite(RomSuiteValidationError),
    InvalidCase {
        case_id: String,
        error: RomCaseValidationError,
    },
    NonStrictCase {
        case_id: String,
        actual: ExecutionMode,
    },
    UnsupportedExternalStimuli {
        case_id: String,
    },
    ResolveRomPath {
        case_id: String,
        source: Box<RomExecutionError>,
    },
    PrepareMachine {
        case_id: String,
        source: Box<RomExecutionError>,
    },
    ReadRom {
        path: PathBuf,
        source: io::Error,
    },
    CartridgeLoad {
        path: PathBuf,
        source: gb_core::CartridgeLoadError,
    },
    SameBoy(SameBoyCaseBundleExecutionError),
    CreateDirectory {
        path: PathBuf,
        source: io::Error,
    },
    CreateProbeFile {
        path: PathBuf,
        source: io::Error,
    },
    WriteProbeFile {
        path: PathBuf,
        source: io::Error,
    },
    ReadProbeFile {
        path: PathBuf,
        source: io::Error,
    },
    DecodeProbeJson {
        path: PathBuf,
        line: usize,
        source: serde_json::Error,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DifferentialProbeSnapshot {
    pub t_cycles: u64,
    pub pc: u16,
    pub sp: u16,
    pub af: u16,
    pub bc: u16,
    pub de: u16,
    pub hl: u16,
    pub ime: bool,
    pub div: u8,
    pub tima: u8,
    pub tma: u8,
    pub tac: u8,
    pub interrupt_flags: u8,
    pub interrupt_enable: u8,
    pub lcdc: u8,
    pub stat: u8,
    pub ly: u8,
    pub line_dot: u16,
    pub scy: u8,
    pub scx: u8,
    pub lyc: u8,
    pub bgp: u8,
    pub obp0: u8,
    pub obp1: u8,
    pub wy: u8,
    pub wx: u8,
    pub vram_hash: String,
    pub oam_hash: String,
    pub wram_hash: String,
    pub hram_hash: String,
    pub framebuffer_hash: String,
    pub serial_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeFieldMismatch {
    pub field: String,
    pub local: String,
    pub oracle: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FirstDivergenceCaseOutcome {
    Matched,
    Diverged {
        first_probe_index: usize,
        window_start_tcycles: u64,
        local_tcycles: Option<u64>,
        oracle_tcycles: Option<u64>,
        mismatches: Vec<ProbeFieldMismatch>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirstDivergenceCaseReport {
    pub case_id: String,
    pub local_probe_path: PathBuf,
    pub oracle_probe_path: PathBuf,
    pub local_probe_count: usize,
    pub oracle_probe_count: usize,
    pub outcome: FirstDivergenceCaseOutcome,
}

impl FirstDivergenceCaseReport {
    pub fn matched(&self) -> bool {
        matches!(self.outcome, FirstDivergenceCaseOutcome::Matched)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirstDivergenceSuiteReport {
    pub suite_name: String,
    pub compare_mode: FirstDivergenceCompareMode,
    pub probe_interval_tcycles: u64,
    pub probe_root: PathBuf,
    pub cases: Vec<FirstDivergenceCaseReport>,
}

impl FirstDivergenceSuiteReport {
    pub fn all_matched(&self) -> bool {
        self.cases.iter().all(FirstDivergenceCaseReport::matched)
    }
}

#[derive(Debug, Clone)]
pub struct FirstDivergenceRunner {
    rom_runner: RomRunner,
    sameboy_runner: SameBoyCaseBundleRunner,
    probe_root: PathBuf,
    compare_mode: FirstDivergenceCompareMode,
    probe_interval_tcycles: u64,
}

impl FirstDivergenceRunner {
    pub fn new(probe_root: impl Into<PathBuf>) -> Self {
        let probe_root = probe_root.into();
        Self {
            rom_runner: RomRunner::new(),
            sameboy_runner: SameBoyCaseBundleRunner::new(&probe_root),
            probe_root,
            compare_mode: FirstDivergenceCompareMode::Framebuffer,
            probe_interval_tcycles: 70_224,
        }
    }

    pub fn with_rom_runner(mut self, rom_runner: RomRunner) -> Self {
        self.sameboy_runner = self.sameboy_runner.with_rom_runner(rom_runner.clone());
        self.rom_runner = rom_runner;
        self
    }

    pub fn with_sameboy_runner(mut self, sameboy_runner: SameBoyCaseBundleRunner) -> Self {
        self.sameboy_runner = sameboy_runner;
        self
    }

    pub fn with_compare_mode(mut self, compare_mode: FirstDivergenceCompareMode) -> Self {
        self.compare_mode = compare_mode;
        self
    }

    pub fn with_probe_interval_tcycles(mut self, probe_interval_tcycles: u64) -> Self {
        assert!(
            probe_interval_tcycles > 0,
            "probe interval T-cycle cadence must be greater than zero"
        );
        self.probe_interval_tcycles = probe_interval_tcycles;
        self
    }

    pub fn run_suite(
        &self,
        suite: &RomSuite,
    ) -> Result<FirstDivergenceSuiteReport, FirstDivergenceExecutionError> {
        if self.probe_interval_tcycles == 0 {
            return Err(FirstDivergenceExecutionError::InvalidProbeInterval {
                probe_interval_tcycles: self.probe_interval_tcycles,
            });
        }

        suite
            .validate()
            .map_err(FirstDivergenceExecutionError::InvalidSuite)?;

        let mut cases = Vec::with_capacity(suite.cases.len());
        for case in &suite.cases {
            cases.push(self.run_case(case)?);
        }

        Ok(FirstDivergenceSuiteReport {
            suite_name: suite.name.clone(),
            compare_mode: self.compare_mode,
            probe_interval_tcycles: self.probe_interval_tcycles,
            probe_root: self.probe_root.clone(),
            cases,
        })
    }

    fn run_case(
        &self,
        case: &RomTestCase,
    ) -> Result<FirstDivergenceCaseReport, FirstDivergenceExecutionError> {
        if case.execution_mode != ExecutionMode::Strict {
            return Err(FirstDivergenceExecutionError::NonStrictCase {
                case_id: case.id.clone(),
                actual: case.execution_mode,
            });
        }
        if !case.external_stimuli.stimuli().is_empty() {
            return Err(FirstDivergenceExecutionError::UnsupportedExternalStimuli {
                case_id: case.id.clone(),
            });
        }

        let case_dir = self.probe_root.join(&case.id);
        fs::create_dir_all(&case_dir).map_err(|source| {
            FirstDivergenceExecutionError::CreateDirectory {
                path: case_dir.clone(),
                source,
            }
        })?;
        let local_probe_path = case_dir.join("local_probes.jsonl");
        let oracle_probe_path = case_dir.join("sameboy_probes.jsonl");

        let local_probes = self.capture_local_probes(case, &local_probe_path)?;
        self.sameboy_runner
            .run_probe_case(case, &oracle_probe_path, self.probe_interval_tcycles)
            .map_err(FirstDivergenceExecutionError::SameBoy)?;
        let oracle_probes = read_probe_json_lines(&oracle_probe_path)?;
        let outcome = compare_probe_sequences(&local_probes, &oracle_probes, self.compare_mode);

        Ok(FirstDivergenceCaseReport {
            case_id: case.id.clone(),
            local_probe_path,
            oracle_probe_path,
            local_probe_count: local_probes.len(),
            oracle_probe_count: oracle_probes.len(),
            outcome,
        })
    }

    fn capture_local_probes(
        &self,
        case: &RomTestCase,
        probe_path: &Path,
    ) -> Result<Vec<DifferentialProbeSnapshot>, FirstDivergenceExecutionError> {
        case.validate()
            .map_err(|error| FirstDivergenceExecutionError::InvalidCase {
                case_id: case.id.clone(),
                error,
            })?;

        let rom_path = self
            .rom_runner
            .resolve_case_rom_path(case)
            .map_err(|source| FirstDivergenceExecutionError::ResolveRomPath {
                case_id: case.id.clone(),
                source: Box::new(source),
            })?;
        let rom_bytes =
            fs::read(&rom_path).map_err(|source| FirstDivergenceExecutionError::ReadRom {
                path: rom_path.clone(),
                source,
            })?;
        let boot_rom_assets = self
            .rom_runner
            .load_boot_rom_assets(case)
            .map_err(|source| FirstDivergenceExecutionError::PrepareMachine {
                case_id: case.id.clone(),
                source: Box::new(source),
            })?;
        let mut machine = RunnerMachine::new(case, boot_rom_assets);
        machine.load_cartridge(rom_bytes).map_err(|source| {
            FirstDivergenceExecutionError::CartridgeLoad {
                path: rom_path,
                source,
            }
        })?;
        self.rom_runner
            .apply_startup_cartridge_state(case, &mut machine);
        self.rom_runner
            .apply_startup_memory_writes(case, &mut machine);

        let mut probes = Vec::new();
        let mut serial_bytes = Vec::new();
        let mut executed_t_cycles = 0_u64;
        let mut completed_frames = 0_u32;
        let mut at_frame_origin = machine.at_frame_origin();
        let mut next_probe_tcycles = self.probe_interval_tcycles;
        let mut last_probe_tcycles = 0_u64;
        probes.push(capture_probe_snapshot(
            &machine,
            executed_t_cycles,
            &serial_bytes,
        ));

        while !budget_exhausted(case.timeout, executed_t_cycles, completed_frames) {
            machine.step_t_cycle();
            executed_t_cycles += 1;
            serial_bytes.extend(machine.take_serial_output_bytes());

            let now_at_frame_origin = machine.at_frame_origin();
            if now_at_frame_origin && !at_frame_origin {
                completed_frames += 1;
            }
            at_frame_origin = now_at_frame_origin;

            while executed_t_cycles >= next_probe_tcycles {
                probes.push(capture_probe_snapshot(
                    &machine,
                    executed_t_cycles,
                    &serial_bytes,
                ));
                last_probe_tcycles = executed_t_cycles;
                next_probe_tcycles += self.probe_interval_tcycles;
            }
        }

        if executed_t_cycles != last_probe_tcycles {
            probes.push(capture_probe_snapshot(
                &machine,
                executed_t_cycles,
                &serial_bytes,
            ));
        }
        write_probe_json_lines(probe_path, &probes)?;
        Ok(probes)
    }
}

fn capture_probe_snapshot(
    machine: &RunnerMachine,
    t_cycles: u64,
    serial_bytes: &[u8],
) -> DifferentialProbeSnapshot {
    let cpu = machine.cpu_snapshot();
    let registers = cpu.registers;
    let ppu = match machine {
        RunnerMachine::Buffered(machine) => machine.ppu().snapshot(),
        RunnerMachine::Summary(machine) => machine.ppu().snapshot(),
    };

    DifferentialProbeSnapshot {
        t_cycles,
        pc: registers.pc,
        sp: registers.sp,
        af: u16::from_be_bytes([registers.a, registers.f]),
        bc: u16::from_be_bytes([registers.b, registers.c]),
        de: u16::from_be_bytes([registers.d, registers.e]),
        hl: u16::from_be_bytes([registers.h, registers.l]),
        ime: cpu.ime,
        div: machine.read_bus_for_probe(0xFF04),
        tima: machine.read_bus_for_probe(0xFF05),
        tma: machine.read_bus_for_probe(0xFF06),
        tac: machine.read_bus_for_probe(0xFF07),
        interrupt_flags: machine.read_bus_for_probe(0xFF0F),
        interrupt_enable: machine.read_bus_for_probe(0xFFFF),
        lcdc: machine.read_bus_for_probe(0xFF40),
        stat: machine.read_bus_for_probe(0xFF41),
        ly: machine.read_bus_for_probe(0xFF44),
        line_dot: ppu.line_dot,
        scy: machine.read_bus_for_probe(0xFF42),
        scx: machine.read_bus_for_probe(0xFF43),
        lyc: machine.read_bus_for_probe(0xFF45),
        bgp: machine.read_bus_for_probe(0xFF47),
        obp0: machine.read_bus_for_probe(0xFF48),
        obp1: machine.read_bus_for_probe(0xFF49),
        wy: machine.read_bus_for_probe(0xFF4A),
        wx: machine.read_bus_for_probe(0xFF4B),
        vram_hash: fnv1a64_hex(machine.debug_vram_bytes()),
        oam_hash: fnv1a64_hex(machine.debug_oam_bytes()),
        wram_hash: fnv1a64_hex(machine.debug_wram_bytes()),
        hram_hash: fnv1a64_hex(machine.debug_hram_bytes()),
        framebuffer_hash: local_framebuffer_rank_hash(machine.framebuffer()),
        serial_hex: crate::encode_bytes_as_upper_hex(serial_bytes),
    }
}

fn compare_probe_sequences(
    local: &[DifferentialProbeSnapshot],
    oracle: &[DifferentialProbeSnapshot],
    compare_mode: FirstDivergenceCompareMode,
) -> FirstDivergenceCaseOutcome {
    let max_len = local.len().max(oracle.len());
    for index in 0..max_len {
        let Some(local_probe) = local.get(index) else {
            return FirstDivergenceCaseOutcome::Diverged {
                first_probe_index: index,
                window_start_tcycles: local
                    .get(index.saturating_sub(1))
                    .map_or(0, |probe| probe.t_cycles),
                local_tcycles: None,
                oracle_tcycles: oracle.get(index).map(|probe| probe.t_cycles),
                mismatches: vec![ProbeFieldMismatch {
                    field: "probe_count".to_string(),
                    local: local.len().to_string(),
                    oracle: oracle.len().to_string(),
                }],
            };
        };
        let Some(oracle_probe) = oracle.get(index) else {
            return FirstDivergenceCaseOutcome::Diverged {
                first_probe_index: index,
                window_start_tcycles: local
                    .get(index.saturating_sub(1))
                    .map_or(0, |probe| probe.t_cycles),
                local_tcycles: Some(local_probe.t_cycles),
                oracle_tcycles: None,
                mismatches: vec![ProbeFieldMismatch {
                    field: "probe_count".to_string(),
                    local: local.len().to_string(),
                    oracle: oracle.len().to_string(),
                }],
            };
        };
        let mismatches = probe_mismatches(local_probe, oracle_probe, compare_mode);
        if !mismatches.is_empty() {
            return FirstDivergenceCaseOutcome::Diverged {
                first_probe_index: index,
                window_start_tcycles: local
                    .get(index.saturating_sub(1))
                    .map_or(0, |probe| probe.t_cycles),
                local_tcycles: Some(local_probe.t_cycles),
                oracle_tcycles: Some(oracle_probe.t_cycles),
                mismatches,
            };
        }
    }

    FirstDivergenceCaseOutcome::Matched
}

fn probe_mismatches(
    local: &DifferentialProbeSnapshot,
    oracle: &DifferentialProbeSnapshot,
    compare_mode: FirstDivergenceCompareMode,
) -> Vec<ProbeFieldMismatch> {
    match compare_mode {
        FirstDivergenceCompareMode::Framebuffer => {
            let mut mismatches = Vec::new();
            if local.t_cycles != oracle.t_cycles {
                mismatches.push(ProbeFieldMismatch {
                    field: "t_cycles".to_string(),
                    local: local.t_cycles.to_string(),
                    oracle: oracle.t_cycles.to_string(),
                });
            }
            if let Some(mismatch) = compare_field(
                "framebuffer_hash",
                &local.framebuffer_hash,
                &oracle.framebuffer_hash,
            ) {
                mismatches.push(mismatch);
            }
            mismatches
        }
        FirstDivergenceCompareMode::State => compare_state_fields(local, oracle),
    }
}

fn compare_state_fields(
    local: &DifferentialProbeSnapshot,
    oracle: &DifferentialProbeSnapshot,
) -> Vec<ProbeFieldMismatch> {
    let local_value = serde_json::to_value(local).expect("probe snapshot should serialize");
    let oracle_value = serde_json::to_value(oracle).expect("probe snapshot should serialize");
    let local_object = local_value
        .as_object()
        .expect("probe snapshot is an object");
    let oracle_object = oracle_value
        .as_object()
        .expect("probe snapshot is an object");
    let mut mismatches = Vec::new();
    for (field, local_field) in local_object {
        if field == "t_cycles" {
            continue;
        }
        let oracle_field = oracle_object
            .get(field)
            .expect("oracle probe should have matching schema");
        if local_field != oracle_field {
            mismatches.push(ProbeFieldMismatch {
                field: field.clone(),
                local: json_value_to_string(local_field),
                oracle: json_value_to_string(oracle_field),
            });
        }
    }
    mismatches
}

fn compare_field(field: &str, local: &str, oracle: &str) -> Option<ProbeFieldMismatch> {
    (local != oracle).then(|| ProbeFieldMismatch {
        field: field.to_string(),
        local: local.to_string(),
        oracle: oracle.to_string(),
    })
}

fn json_value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(value) => value.clone(),
        _ => value.to_string(),
    }
}

fn write_probe_json_lines(
    path: &Path,
    probes: &[DifferentialProbeSnapshot],
) -> Result<(), FirstDivergenceExecutionError> {
    let mut file =
        File::create(path).map_err(|source| FirstDivergenceExecutionError::CreateProbeFile {
            path: path.to_path_buf(),
            source,
        })?;
    for probe in probes {
        serde_json::to_writer(&mut file, probe).map_err(|source| {
            FirstDivergenceExecutionError::DecodeProbeJson {
                path: path.to_path_buf(),
                line: 0,
                source,
            }
        })?;
        file.write_all(b"\n")
            .map_err(|source| FirstDivergenceExecutionError::WriteProbeFile {
                path: path.to_path_buf(),
                source,
            })?;
    }
    Ok(())
}

fn read_probe_json_lines(
    path: &Path,
) -> Result<Vec<DifferentialProbeSnapshot>, FirstDivergenceExecutionError> {
    let file = File::open(path).map_err(|source| FirstDivergenceExecutionError::ReadProbeFile {
        path: path.to_path_buf(),
        source,
    })?;
    let reader = BufReader::new(file);
    let mut probes = Vec::new();
    for (index, line) in reader.lines().enumerate() {
        let line = line.map_err(|source| FirstDivergenceExecutionError::ReadProbeFile {
            path: path.to_path_buf(),
            source,
        })?;
        if line.trim().is_empty() {
            continue;
        }
        probes.push(serde_json::from_str(&line).map_err(|source| {
            FirstDivergenceExecutionError::DecodeProbeJson {
                path: path.to_path_buf(),
                line: index + 1,
                source,
            }
        })?);
    }
    Ok(probes)
}

fn fnv1a64_hex(bytes: &[u8]) -> String {
    let mut hash = FNV1A64_OFFSET;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV1A64_PRIME);
    }
    format!("{hash:016x}")
}

fn local_framebuffer_rank_hash(framebuffer: &[u8]) -> String {
    let mut ranks = framebuffer.to_vec();
    ranks.sort_unstable();
    ranks.dedup();
    let compacted = framebuffer
        .iter()
        .map(|pixel| {
            ranks
                .binary_search(pixel)
                .expect("rank source contains every framebuffer pixel") as u8
        })
        .collect::<Vec<_>>();
    fnv1a64_hex(&compacted)
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    use gb_core::ExecutionMode;

    use crate::{
        ExternalStimulus, ExternalStimulusAction, RomSuite, SameBoyCaseBundleRunner, TestSubsystem,
        Timeout,
    };

    use super::{
        DifferentialProbeSnapshot, FirstDivergenceCaseOutcome, FirstDivergenceCompareMode,
        FirstDivergenceExecutionError, FirstDivergenceRunner, compare_probe_sequences, fnv1a64_hex,
        local_framebuffer_rank_hash, read_probe_json_lines, write_probe_json_lines,
    };

    fn unique_temp_dir(label: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "gb-cycle-first-divergence-{}-{}-{}",
            label,
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos()
        ))
    }

    #[cfg(unix)]
    fn write_fake_sameboy_probe_runner(path: &Path) {
        fs::write(
            path,
            concat!(
                "#!/bin/sh\n",
                "set -eu\n",
                "probe_json_out=''\n",
                "while [ \"$#\" -gt 0 ]; do\n",
                "  case \"$1\" in\n",
                "    --probe-json-out)\n",
                "      shift\n",
                "      probe_json_out=\"$1\"\n",
                "      ;;\n",
                "    --probe-interval-tcycles|--timeout-tcycles|--timeout-frames|--model|--rom|--startup-cartridge-rtc-seconds)\n",
                "      shift\n",
                "      ;;\n",
                "    --write-memory)\n",
                "      shift\n",
                "      shift\n",
                "      ;;\n",
                "  esac\n",
                "  shift\n",
                "done\n",
                "if [ -n \"$probe_json_out\" ]; then\n",
                "  mkdir -p \"$(dirname \"$probe_json_out\")\"\n",
                "  cat > \"$probe_json_out\" <<'JSON'\n",
                "{\"t_cycles\":0,\"pc\":256,\"sp\":65534,\"af\":432,\"bc\":19,\"de\":216,\"hl\":333,\"ime\":false,\"div\":171,\"tima\":0,\"tma\":0,\"tac\":248,\"interrupt_flags\":225,\"interrupt_enable\":0,\"lcdc\":145,\"stat\":133,\"ly\":0,\"line_dot\":0,\"scy\":0,\"scx\":0,\"lyc\":0,\"bgp\":252,\"obp0\":255,\"obp1\":255,\"wy\":0,\"wx\":0,\"vram_hash\":\"a\",\"oam_hash\":\"b\",\"wram_hash\":\"c\",\"hram_hash\":\"d\",\"framebuffer_hash\":\"e\",\"serial_hex\":\"\"}\n",
                "JSON\n",
                "fi\n",
            ),
        )
        .expect("fake runner should be writable");
        let mut permissions = fs::metadata(path)
            .expect("fake runner metadata should exist")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("fake runner should be executable");
    }

    fn probe(t_cycles: u64, framebuffer_hash: &str) -> DifferentialProbeSnapshot {
        DifferentialProbeSnapshot {
            t_cycles,
            pc: 0x100,
            sp: 0xFFFE,
            af: 0x01B0,
            bc: 0x0013,
            de: 0x00D8,
            hl: 0x014D,
            ime: false,
            div: 0xAB,
            tima: 0,
            tma: 0,
            tac: 0xF8,
            interrupt_flags: 0xE1,
            interrupt_enable: 0,
            lcdc: 0x91,
            stat: 0x85,
            ly: 0,
            line_dot: 0,
            scy: 0,
            scx: 0,
            lyc: 0,
            bgp: 0xFC,
            obp0: 0xFF,
            obp1: 0xFF,
            wy: 0,
            wx: 0,
            vram_hash: "a".repeat(16),
            oam_hash: "b".repeat(16),
            wram_hash: "c".repeat(16),
            hram_hash: "d".repeat(16),
            framebuffer_hash: framebuffer_hash.to_string(),
            serial_hex: String::new(),
        }
    }

    #[test]
    fn framebuffer_compare_reports_first_mismatch_window() {
        let local = [probe(0, "same"), probe(10, "local")];
        let oracle = [probe(0, "same"), probe(10, "oracle")];
        let outcome =
            compare_probe_sequences(&local, &oracle, FirstDivergenceCompareMode::Framebuffer);
        let FirstDivergenceCaseOutcome::Diverged {
            first_probe_index,
            window_start_tcycles,
            mismatches,
            ..
        } = outcome
        else {
            panic!("expected divergence");
        };
        assert_eq!(first_probe_index, 1);
        assert_eq!(window_start_tcycles, 0);
        assert_eq!(mismatches[0].field, "framebuffer_hash");
    }

    #[test]
    fn framebuffer_compare_reports_probe_tcycle_drift_before_framebuffer_state() {
        let local = [probe(10, "same")];
        let oracle = [probe(12, "same")];
        let outcome =
            compare_probe_sequences(&local, &oracle, FirstDivergenceCompareMode::Framebuffer);
        let FirstDivergenceCaseOutcome::Diverged {
            local_tcycles,
            oracle_tcycles,
            mismatches,
            ..
        } = outcome
        else {
            panic!("expected cadence divergence");
        };
        assert_eq!(local_tcycles, Some(10));
        assert_eq!(oracle_tcycles, Some(12));
        assert_eq!(mismatches[0].field, "t_cycles");
    }

    #[test]
    fn compare_reports_probe_count_drift_and_full_match() {
        let local = [probe(0, "same"), probe(10, "same")];
        let oracle = [probe(0, "same")];
        let outcome =
            compare_probe_sequences(&local, &oracle, FirstDivergenceCompareMode::Framebuffer);
        let FirstDivergenceCaseOutcome::Diverged {
            first_probe_index,
            window_start_tcycles,
            local_tcycles,
            oracle_tcycles,
            mismatches,
        } = outcome
        else {
            panic!("expected missing oracle probe divergence");
        };
        assert_eq!(first_probe_index, 1);
        assert_eq!(window_start_tcycles, 0);
        assert_eq!(local_tcycles, Some(10));
        assert_eq!(oracle_tcycles, None);
        assert_eq!(mismatches[0].field, "probe_count");

        let outcome =
            compare_probe_sequences(&oracle, &local, FirstDivergenceCompareMode::Framebuffer);
        let FirstDivergenceCaseOutcome::Diverged {
            local_tcycles,
            oracle_tcycles,
            ..
        } = outcome
        else {
            panic!("expected missing local probe divergence");
        };
        assert_eq!(local_tcycles, None);
        assert_eq!(oracle_tcycles, Some(10));

        assert!(matches!(
            compare_probe_sequences(&local, &local, FirstDivergenceCompareMode::Framebuffer),
            FirstDivergenceCaseOutcome::Matched
        ));
    }

    #[test]
    fn state_compare_ignores_probe_tcycle_drift_but_reports_state_fields() {
        let local = [probe(10, "same")];
        let mut oracle = probe(12, "same");
        oracle.div = 0xAC;
        let outcome = compare_probe_sequences(&local, &[oracle], FirstDivergenceCompareMode::State);
        let FirstDivergenceCaseOutcome::Diverged { mismatches, .. } = outcome else {
            panic!("expected divergence");
        };
        assert!(mismatches.iter().any(|mismatch| mismatch.field == "div"));
        assert!(
            !mismatches
                .iter()
                .any(|mismatch| mismatch.field == "t_cycles")
        );
    }

    #[test]
    fn state_compare_formats_string_field_mismatches_without_json_quotes() {
        let local = [probe(10, "local-hash")];
        let oracle = [probe(10, "oracle-hash")];
        let outcome = compare_probe_sequences(&local, &oracle, FirstDivergenceCompareMode::State);
        let FirstDivergenceCaseOutcome::Diverged { mismatches, .. } = outcome else {
            panic!("expected state divergence");
        };
        let framebuffer = mismatches
            .iter()
            .find(|mismatch| mismatch.field == "framebuffer_hash")
            .expect("state comparison should include framebuffer hash");
        assert_eq!(framebuffer.local, "local-hash");
        assert_eq!(framebuffer.oracle, "oracle-hash");
    }

    #[test]
    fn probe_json_lines_roundtrip_ignore_blank_lines_and_report_decode_line() {
        let temp_dir = unique_temp_dir("json");
        fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
        let path = temp_dir.join("probes.jsonl");
        let probes = vec![probe(0, "first"), probe(1, "second")];
        write_probe_json_lines(&path, &probes).expect("probe JSONL should be writable");
        assert_eq!(
            read_probe_json_lines(&path).expect("probe JSONL should be readable"),
            probes
        );

        let blank_path = temp_dir.join("blank-probes.jsonl");
        let encoded_probe = serde_json::to_string(&probes[0]).expect("probe should serialize");
        fs::write(&blank_path, format!("\n{encoded_probe}\n\n"))
            .expect("blank probe fixture should be writable");
        assert_eq!(
            read_probe_json_lines(&blank_path).expect("blank lines should be skipped"),
            vec![probes[0].clone()]
        );

        let invalid_path = temp_dir.join("bad-probes.jsonl");
        fs::write(&invalid_path, "{not-json}\n").expect("bad probe fixture should be writable");
        let error = read_probe_json_lines(&invalid_path).expect_err("invalid JSON should fail");
        assert!(matches!(
            error,
            FirstDivergenceExecutionError::DecodeProbeJson { line: 1, .. }
        ));
    }

    #[test]
    fn runner_rejects_invalid_suite_non_strict_and_external_stimuli_without_io() {
        let runner = FirstDivergenceRunner::new(unique_temp_dir("reject"));
        let invalid_suite = RomSuite::new("", TestSubsystem::Cpu);
        assert!(matches!(
            runner.run_suite(&invalid_suite),
            Err(FirstDivergenceExecutionError::InvalidSuite(_))
        ));

        let mut non_strict = crate::phase_2_cpu_timing_suite();
        non_strict.cases.truncate(1);
        non_strict.cases[0].execution_mode = ExecutionMode::Permissive;
        assert!(matches!(
            runner.run_suite(&non_strict),
            Err(FirstDivergenceExecutionError::NonStrictCase { .. })
        ));

        let mut external = crate::phase_2_cpu_timing_suite();
        external.cases.truncate(1);
        external.cases[0] =
            external.cases[0]
                .clone()
                .with_external_stimulus(ExternalStimulus::at_t_cycle(
                    1,
                    ExternalStimulusAction::WriteMemory {
                        address: 0xC000,
                        value: 0x42,
                    },
                ));
        assert!(matches!(
            runner.run_suite(&external),
            Err(FirstDivergenceExecutionError::UnsupportedExternalStimuli { .. })
        ));
    }

    #[test]
    #[should_panic(expected = "probe interval T-cycle cadence must be greater than zero")]
    fn runner_builder_rejects_zero_probe_interval() {
        let _runner = FirstDivergenceRunner::new(unique_temp_dir("zero-interval-builder"))
            .with_probe_interval_tcycles(0);
    }

    #[test]
    fn runner_rejects_zero_probe_interval_before_suite_execution() {
        let mut runner = FirstDivergenceRunner::new(unique_temp_dir("zero-interval"));
        runner.probe_interval_tcycles = 0;
        let mut suite = crate::phase_2_cpu_timing_suite();
        suite.cases.truncate(1);

        assert!(matches!(
            runner.run_suite(&suite),
            Err(FirstDivergenceExecutionError::InvalidProbeInterval {
                probe_interval_tcycles: 0,
            })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn runner_captures_local_and_sameboy_probe_jsonl_files() {
        let temp_dir = unique_temp_dir("runner");
        fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
        let fake_runner = temp_dir.join("fake-sameboy-case-bundle.sh");
        write_fake_sameboy_probe_runner(&fake_runner);

        let mut suite = crate::phase_2_cpu_timing_suite();
        suite.cases.truncate(1);
        suite.cases[0].timeout = Timeout::TCycles(1);
        let probe_root = temp_dir.join("probes");
        let sameboy_runner =
            SameBoyCaseBundleRunner::new(&probe_root).with_runner_binary(&fake_runner);

        let report = FirstDivergenceRunner::new(&probe_root)
            .with_sameboy_runner(sameboy_runner)
            .with_probe_interval_tcycles(1)
            .run_suite(&suite)
            .expect("first-divergence runner should execute one probe case");

        assert_eq!(report.suite_name, "phase-2-cpu-timing");
        assert_eq!(report.compare_mode.name(), "framebuffer");
        assert_eq!(report.probe_interval_tcycles, 1);
        assert_eq!(report.cases.len(), 1);
        let case = &report.cases[0];
        assert_eq!(case.case_id, "phase2-fetch-immediate-order");
        assert!(case.local_probe_path.is_file());
        assert!(case.oracle_probe_path.is_file());
        assert!(case.local_probe_count >= 2);
        assert_eq!(case.oracle_probe_count, 1);
        assert!(!case.matched());
        assert!(matches!(
            case.outcome,
            FirstDivergenceCaseOutcome::Diverged { .. }
        ));
        assert!(
            fs::read_to_string(&case.local_probe_path)
                .expect("local probe JSONL should be readable")
                .contains("\"t_cycles\":0")
        );
    }

    #[test]
    fn fnv_hash_is_stable_for_probe_files() {
        assert_eq!(fnv1a64_hex(&[]), "cbf29ce484222325");
        assert_eq!(fnv1a64_hex(&[0, 1, 2, 3]), "4475327f98e05411");
    }

    #[test]
    fn local_framebuffer_hash_compacts_present_palette_ranks() {
        assert_eq!(
            local_framebuffer_rank_hash(&[0, 3, 3, 0]),
            fnv1a64_hex(&[0, 1, 1, 0])
        );
    }
}
