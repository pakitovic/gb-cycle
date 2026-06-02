use super::catalog::{OracleConfig, OracleObservations, OracleOutcome, OracleStep};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MemoryByteEqualsOracle {
    address: u16,
    expected: u8,
    fail_value: Option<u8>,
    matched: Option<MemoryByteEqualsMatch>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MemoryByteEqualsMatch {
    Passed { actual: u8 },
    Failed { actual: u8 },
}

impl MemoryByteEqualsOracle {
    pub(super) fn from_manifest(config: &OracleConfig) -> Result<Self, String> {
        config.reject_unknown_parameters(&["address", "value", "fail_value"])?;
        Ok(Self::new(
            config.required_u16("address")?,
            config.required_u8("value")?,
            config.optional_u8("fail_value")?,
        ))
    }

    pub(crate) const fn new(address: u16, expected: u8, fail_value: Option<u8>) -> Self {
        Self {
            address,
            expected,
            fail_value,
            matched: None,
        }
    }

    pub(crate) const fn address(&self) -> u16 {
        self.address
    }

    pub(crate) fn observe(
        &mut self,
        observations: OracleObservations<'_>,
    ) -> Result<OracleStep, String> {
        if let Some(matched) = self.match_observations(observations)? {
            self.matched = Some(matched);
            Ok(OracleStep::Stop)
        } else {
            Ok(OracleStep::Continue)
        }
    }

    pub(crate) fn finish(
        &mut self,
        observations: OracleObservations<'_>,
    ) -> Result<OracleOutcome, String> {
        let matched = match self.matched {
            Some(matched) => Some(matched),
            None => self.match_observations(observations)?,
        };
        Ok(match matched {
            Some(MemoryByteEqualsMatch::Passed { .. }) => OracleOutcome::Passed,
            Some(MemoryByteEqualsMatch::Failed { actual }) => {
                OracleOutcome::Failed(self.failure_message(actual))
            }
            None => OracleOutcome::Failed(self.failure_message(self.observed_value(observations)?)),
        })
    }

    fn match_observations(
        &self,
        observations: OracleObservations<'_>,
    ) -> Result<Option<MemoryByteEqualsMatch>, String> {
        let actual = self.observed_value(observations)?;
        if actual == self.expected {
            Ok(Some(MemoryByteEqualsMatch::Passed { actual }))
        } else if self.fail_value == Some(actual) {
            Ok(Some(MemoryByteEqualsMatch::Failed { actual }))
        } else {
            Ok(None)
        }
    }

    fn observed_value(&self, observations: OracleObservations<'_>) -> Result<u8, String> {
        observations
            .memory
            .iter()
            .find(|byte| byte.address == self.address)
            .map(|byte| byte.value)
            .ok_or_else(|| {
                format!(
                    "memory-byte-equals oracle requires memory observation for address {:#06X}",
                    self.address
                )
            })
    }

    fn failure_message(&self, actual: u8) -> String {
        match self.fail_value {
            Some(fail_value) => format!(
                "memory byte mismatch at {:#06X}: expected {:#04X}, fail_value {:#04X}, actual {:#04X}",
                self.address, self.expected, fail_value, actual
            ),
            None => format!(
                "memory byte mismatch at {:#06X}: expected {:#04X}, actual {:#04X}",
                self.address, self.expected, actual
            ),
        }
    }
}
