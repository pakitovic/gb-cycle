use super::catalog::{CpuObservation, OracleConfig, OracleObservations, OracleOutcome, OracleStep};

const MAGIC_BREAKPOINT_OPCODE: u8 = 0x40;
const PASS_SIGNATURE: [u8; 6] = [3, 5, 8, 13, 21, 34];
const FAIL_SIGNATURE: [u8; 6] = [0x42; 6];
const HALT_LOOP_BYTES: [u8; 4] = [0x40, 0x00, 0x18, 0xFD];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FibonacciResultOracle {
    result: Option<FibonacciResult>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FibonacciResult {
    Passed,
    Failed,
}

impl FibonacciResultOracle {
    pub(super) fn from_manifest(config: &OracleConfig) -> Result<Self, String> {
        config.reject_unknown_parameters(&[])?;
        Ok(Self::new())
    }

    pub(crate) const fn new() -> Self {
        Self { result: None }
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
            Some(FibonacciResult::Failed) => {
                OracleOutcome::Failed("fibonacci result reported failure signature".to_string())
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
        let Some(result) = result_for_signature(cpu) else {
            return Ok(None);
        };
        if terminal_signal_reached(cpu) {
            Ok(Some(result))
        } else {
            Ok(None)
        }
    }

    fn failure_message(&self, observations: OracleObservations<'_>) -> String {
        match observations.cpu {
            Some(cpu) if result_for_signature(cpu).is_some() && !terminal_signal_reached(cpu) => {
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
        FAIL_SIGNATURE => Some(FibonacciResult::Failed),
        _ => None,
    }
}

fn terminal_signal_reached(cpu: CpuObservation) -> bool {
    cpu.current_opcode == Some(MAGIC_BREAKPOINT_OPCODE)
        || cpu
            .pc_window
            .windows(HALT_LOOP_BYTES.len())
            .any(|window| window == HALT_LOOP_BYTES)
}
