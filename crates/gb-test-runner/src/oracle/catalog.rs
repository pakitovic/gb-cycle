use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use super::fibonacci_result::FibonacciResultOracle;
use super::framebuffer::FramebufferOracle;
use super::memory_byte_equals::MemoryByteEqualsOracle;
use super::serial_contains::SerialContainsOracle;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct OracleConfig {
    #[serde(rename = "type")]
    kind: Option<String>,
    #[serde(flatten)]
    parameters: BTreeMap<String, toml::Value>,
}

impl OracleConfig {
    pub(crate) fn has_kind(&self) -> bool {
        self.kind.is_some()
    }

    pub(crate) fn with_defaults(mut self, defaults: &Self) -> Result<Self, String> {
        let kind = defaults
            .kind
            .clone()
            .ok_or_else(|| "oracle override requires a default oracle with type".to_string())?;
        let mut parameters = defaults.parameters.clone();
        parameters.extend(self.parameters);
        self.kind = Some(kind);
        self.parameters = parameters;
        Ok(self)
    }

    fn kind(&self) -> Result<&str, String> {
        self.kind
            .as_deref()
            .ok_or_else(|| "oracle must define type".to_string())
    }

    fn kind_label(&self) -> &str {
        self.kind.as_deref().unwrap_or("<missing>")
    }

    pub(super) fn required_string(&self, field: &str) -> Result<String, String> {
        match self.parameters.get(field) {
            Some(toml::Value::String(value)) => Ok(value.clone()),
            Some(_) => Err(format!(
                "oracle {:?} field {field} must be a string",
                self.kind_label()
            )),
            None => Err(format!("oracle {:?} requires {field}", self.kind_label())),
        }
    }

    pub(super) fn optional_string(&self, field: &str) -> Result<Option<String>, String> {
        match self.parameters.get(field) {
            Some(toml::Value::String(value)) => Ok(Some(value.clone())),
            Some(_) => Err(format!(
                "oracle {:?} field {field} must be a string",
                self.kind_label()
            )),
            None => Ok(None),
        }
    }

    pub(super) fn optional_u64(&self, field: &str) -> Result<Option<u64>, String> {
        match self.parameters.get(field) {
            Some(toml::Value::Integer(value)) => u64::try_from(*value).map(Some).map_err(|_| {
                format!(
                    "oracle {:?} field {field} must be a non-negative integer",
                    self.kind_label()
                )
            }),
            Some(_) => Err(format!(
                "oracle {:?} field {field} must be an integer",
                self.kind_label()
            )),
            None => Ok(None),
        }
    }

    pub(super) fn required_u16(&self, field: &str) -> Result<u16, String> {
        self.optional_u16(field)?
            .ok_or_else(|| format!("oracle {:?} requires {field}", self.kind_label()))
    }

    pub(super) fn required_u8(&self, field: &str) -> Result<u8, String> {
        self.optional_u8(field)?
            .ok_or_else(|| format!("oracle {:?} requires {field}", self.kind_label()))
    }

    pub(super) fn optional_u16(&self, field: &str) -> Result<Option<u16>, String> {
        match self.optional_u64(field)? {
            Some(value) => u16::try_from(value).map(Some).map_err(|_| {
                format!(
                    "oracle {:?} field {field} must be between 0 and 65535",
                    self.kind_label()
                )
            }),
            None => Ok(None),
        }
    }

    pub(super) fn optional_u8(&self, field: &str) -> Result<Option<u8>, String> {
        match self.optional_u64(field)? {
            Some(value) => u8::try_from(value).map(Some).map_err(|_| {
                format!(
                    "oracle {:?} field {field} must be between 0 and 255",
                    self.kind_label()
                )
            }),
            None => Ok(None),
        }
    }

    pub(super) fn has_parameter(&self, field: &str) -> bool {
        self.parameters.contains_key(field)
    }

    pub(super) fn string_or_string_array(
        &self,
        field: &str,
    ) -> Result<Option<Vec<String>>, String> {
        match self.parameters.get(field) {
            Some(toml::Value::String(value)) => Ok(Some(vec![value.clone()])),
            Some(toml::Value::Array(values)) => {
                let mut strings = Vec::with_capacity(values.len());
                for value in values {
                    let toml::Value::String(value) = value else {
                        return Err(format!(
                            "oracle {:?} field {field} array entries must be strings",
                            self.kind_label()
                        ));
                    };
                    strings.push(value.clone());
                }
                Ok(Some(strings))
            }
            Some(_) => Err(format!(
                "oracle {:?} field {field} must be a string or an array of strings",
                self.kind_label()
            )),
            None => Ok(None),
        }
    }

    pub(super) fn reject_unknown_parameters(&self, allowed: &[&str]) -> Result<(), String> {
        for parameter in self.parameters.keys() {
            if !allowed.contains(&parameter.as_str()) {
                return Err(format!(
                    "oracle {:?} does not support parameter {parameter:?}",
                    self.kind_label()
                ));
            }
        }
        Ok(())
    }
}

pub(crate) const CPU_OBSERVATION_WINDOW_BACKTRACK: usize = 4;
pub(crate) const CPU_OBSERVATION_WINDOW_BYTES: usize = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CpuObservation {
    pub(crate) b: u8,
    pub(crate) c: u8,
    pub(crate) d: u8,
    pub(crate) e: u8,
    pub(crate) h: u8,
    pub(crate) l: u8,
    pub(crate) pc: u16,
    pub(crate) current_opcode: Option<u8>,
    pub(crate) pc_window: [u8; CPU_OBSERVATION_WINDOW_BYTES],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MemoryObservation {
    pub(crate) address: u16,
    pub(crate) value: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FramebufferObservation<'a> {
    pub(crate) dmg: Option<&'a [u8]>,
    pub(crate) cgb_rgb555: Option<&'a [u16]>,
    pub(crate) in_vblank: bool,
}

impl<'a> FramebufferObservation<'a> {
    #[cfg(test)]
    pub(crate) fn empty() -> Self {
        Self {
            dmg: None,
            cgb_rgb555: None,
            in_vblank: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ParticipantFramebufferObservation<'a> {
    pub(crate) id: &'a str,
    pub(crate) framebuffer: FramebufferObservation<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OracleObservations<'a> {
    pub(crate) serial: &'a [u8],
    pub(crate) cpu: Option<CpuObservation>,
    pub(crate) memory: &'a [MemoryObservation],
    pub(crate) executed_tcycles: u64,
    pub(crate) framebuffer: FramebufferObservation<'a>,
    pub(crate) participants: &'a [ParticipantFramebufferObservation<'a>],
}

impl<'a> OracleObservations<'a> {
    #[cfg(test)]
    pub(crate) fn serial(serial: &'a [u8]) -> Self {
        Self {
            serial,
            cpu: None,
            memory: &[],
            executed_tcycles: 0,
            framebuffer: FramebufferObservation::empty(),
            participants: &[],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OracleStep {
    Continue,
    Stop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OracleOutcome {
    Passed,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Oracle {
    FibonacciResult(FibonacciResultOracle),
    Framebuffer(FramebufferOracle),
    MemoryByteEquals(MemoryByteEqualsOracle),
    SerialContains(SerialContainsOracle),
}

impl Oracle {
    #[cfg(test)]
    pub(crate) fn from_manifest(config: &OracleConfig) -> Result<Self, String> {
        Self::from_manifest_with_fixture_root(config, Path::new(""))
    }

    pub(crate) fn from_manifest_with_fixture_root(
        config: &OracleConfig,
        fixture_root: &Path,
    ) -> Result<Self, String> {
        match config.kind()? {
            "fibonacci-result" => Ok(Self::FibonacciResult(FibonacciResultOracle::from_manifest(
                config,
            )?)),
            "framebuffer" => Ok(Self::Framebuffer(FramebufferOracle::from_manifest(
                config,
                fixture_root,
            )?)),
            "memory-byte-equals" => Ok(Self::MemoryByteEquals(
                MemoryByteEqualsOracle::from_manifest(config)?,
            )),
            "serial-contains" => Ok(Self::SerialContains(SerialContainsOracle::from_manifest(
                config,
            )?)),
            other => Err(format!("unsupported suite oracle {other:?}")),
        }
    }

    pub(crate) fn observe(
        &mut self,
        observations: OracleObservations<'_>,
    ) -> Result<OracleStep, String> {
        match self {
            Self::FibonacciResult(oracle) => oracle.observe(observations),
            Self::Framebuffer(oracle) => oracle.observe(observations),
            Self::MemoryByteEquals(oracle) => oracle.observe(observations),
            Self::SerialContains(oracle) => Ok(oracle.observe(observations)),
        }
    }

    pub(crate) fn finish(
        &mut self,
        observations: OracleObservations<'_>,
    ) -> Result<OracleOutcome, String> {
        match self {
            Self::FibonacciResult(oracle) => oracle.finish(observations),
            Self::Framebuffer(oracle) => oracle.finish(observations),
            Self::MemoryByteEquals(oracle) => oracle.finish(observations),
            Self::SerialContains(oracle) => Ok(oracle.finish(observations)),
        }
    }

    pub(crate) fn needs_cpu_observation(&self) -> bool {
        matches!(self, Self::FibonacciResult(_))
    }

    pub(crate) fn memory_addresses(&self) -> Vec<u16> {
        match self {
            Self::MemoryByteEquals(oracle) => vec![oracle.address()],
            _ => Vec::new(),
        }
    }

    pub(crate) fn framebuffer_artifact_descriptor(
        &self,
    ) -> Option<super::framebuffer::FramebufferArtifactDescriptor> {
        match self {
            Self::Framebuffer(oracle) => oracle.artifact_descriptor(),
            _ => None,
        }
    }
}
