use super::catalog::{CpuObservation, OracleConfig, OracleObservations, OracleOutcome, OracleStep};

const MAGIC_BREAKPOINT_OPCODE: u8 = 0x40;
const LEGACY_MAGIC_BREAKPOINT_OPCODE: u8 = 0xED;
const PASS_SIGNATURE: [u8; 6] = [3, 5, 8, 13, 21, 34];
const FAIL_SIGNATURE: [u8; 6] = [0x42; 6];
const NOP_PADDED_TERMINAL_LOOP_BYTES: [u8; 4] = [0x40, 0x00, 0x18, 0xFD];
const COMPACT_TERMINAL_LOOP_BYTES: [u8; 3] = [0x40, 0x18, 0xFE];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FibonacciResultOracle {
    result: Option<FibonacciResult>,
    legacy: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FibonacciResult {
    Passed,
    Failed(FibonacciFailure),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FibonacciFailure {
    FailureSignature,
    LegacyTerminalWithoutPassSignature,
}

impl FibonacciResultOracle {
    pub(super) fn from_manifest(config: &OracleConfig) -> Result<Self, String> {
        config.reject_unknown_parameters(&["legacy"])?;
        Ok(Self {
            result: None,
            legacy: config.optional_bool("legacy")?.unwrap_or(false),
        })
    }

    pub(crate) fn observe(
        &mut self,
        observations: OracleObservations<'_>,
    ) -> Result<OracleStep, String> {
        if let Some(result) = self.detect_result(observations)? {
            self.result = Some(result);
            Ok(OracleStep::Stop)
        } else {
            Ok(OracleStep::Continue)
        }
    }

    pub(crate) fn finish(
        &mut self,
        observations: OracleObservations<'_>,
    ) -> Result<OracleOutcome, String> {
        let result = match self.result {
            Some(result) => Some(result),
            None => self.detect_result(observations)?,
        };
        Ok(match result {
            Some(FibonacciResult::Passed) => OracleOutcome::Passed,
            Some(FibonacciResult::Failed(failure)) => {
                OracleOutcome::Failed(failure.message(observations))
            }
            None => OracleOutcome::Failed(self.failure_message(observations)),
        })
    }

    fn detect_result(
        &self,
        observations: OracleObservations<'_>,
    ) -> Result<Option<FibonacciResult>, String> {
        let cpu = observations
            .cpu
            .ok_or_else(|| "fibonacci-result oracle requires CPU observation".to_string())?;
        let result = result_for_signature(cpu);
        if self.legacy && legacy_terminal_signal_reached(cpu) {
            return Ok(Some(result.unwrap_or(FibonacciResult::Failed(
                FibonacciFailure::LegacyTerminalWithoutPassSignature,
            ))));
        }
        let Some(result) = result else {
            return Ok(None);
        };
        if terminal_signal_reached(cpu, self.legacy) {
            Ok(Some(result))
        } else {
            Ok(None)
        }
    }

    fn failure_message(&self, observations: OracleObservations<'_>) -> String {
        match observations.cpu {
            Some(cpu)
                if result_for_signature(cpu).is_some()
                    && !terminal_signal_reached(cpu, self.legacy) =>
            {
                format!(
                    "fibonacci result signature reached without terminal signal at PC {:#06X}",
                    cpu.pc
                )
            }
            Some(cpu) => format!("fibonacci result was not reached at PC {:#06X}", cpu.pc),
            None => "fibonacci result was not reached".to_string(),
        }
    }
}

fn result_for_signature(cpu: CpuObservation) -> Option<FibonacciResult> {
    let signature = [cpu.b, cpu.c, cpu.d, cpu.e, cpu.h, cpu.l];
    match signature {
        PASS_SIGNATURE => Some(FibonacciResult::Passed),
        FAIL_SIGNATURE => Some(FibonacciResult::Failed(FibonacciFailure::FailureSignature)),
        _ => None,
    }
}

impl FibonacciFailure {
    fn message(self, observations: OracleObservations<'_>) -> String {
        match self {
            Self::FailureSignature => "fibonacci result reported failure signature".to_string(),
            Self::LegacyTerminalWithoutPassSignature => match observations.cpu {
                Some(cpu) => format!(
                    "legacy fibonacci terminal reached without pass signature at PC {:#06X}",
                    cpu.pc
                ),
                None => "legacy fibonacci terminal reached without pass signature".to_string(),
            },
        }
    }
}

fn terminal_signal_reached(cpu: CpuObservation, legacy: bool) -> bool {
    cpu.current_opcode == Some(MAGIC_BREAKPOINT_OPCODE)
        || legacy && legacy_terminal_signal_reached(cpu)
        || terminal_loop_reached(cpu)
}

fn legacy_terminal_signal_reached(cpu: CpuObservation) -> bool {
    cpu.current_opcode == Some(LEGACY_MAGIC_BREAKPOINT_OPCODE)
}

fn terminal_loop_reached(cpu: CpuObservation) -> bool {
    cpu.pc_window
        .windows(NOP_PADDED_TERMINAL_LOOP_BYTES.len())
        .any(|window| window == NOP_PADDED_TERMINAL_LOOP_BYTES)
        || cpu
            .pc_window
            .windows(COMPACT_TERMINAL_LOOP_BYTES.len())
            .any(|window| window == COMPACT_TERMINAL_LOOP_BYTES)
}
