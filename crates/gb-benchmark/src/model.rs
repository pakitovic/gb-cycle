use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BenchmarkModel {
    #[serde(rename = "DMG", alias = "dmg")]
    Dmg,
    #[serde(rename = "MGB", alias = "mgb")]
    Mgb,
    #[serde(rename = "LGB", alias = "lgb")]
    Lgb,
    #[serde(rename = "CGB", alias = "cgb")]
    Cgb,
    #[serde(rename = "AGB", alias = "agb")]
    Agb,
}

impl BenchmarkModel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dmg => "DMG",
            Self::Mgb => "MGB",
            Self::Lgb => "LGB",
            Self::Cgb => "CGB",
            Self::Agb => "AGB",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BenchmarkStartup {
    #[serde(rename = "skip-boot")]
    SkipBoot,
    #[serde(rename = "custom-boot")]
    CustomBoot,
    #[serde(rename = "real-boot")]
    RealBoot,
}

impl BenchmarkStartup {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SkipBoot => "skip-boot",
            Self::CustomBoot => "custom-boot",
            Self::RealBoot => "real-boot",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BenchmarkMode {
    #[serde(rename = "strict")]
    Strict,
    #[serde(rename = "permissive")]
    Permissive,
    #[serde(rename = "experimental")]
    Experimental,
}

impl BenchmarkMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Strict => "strict",
            Self::Permissive => "permissive",
            Self::Experimental => "experimental",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BenchmarkPalette {
    #[serde(rename = "grey")]
    Grey,
}

impl BenchmarkPalette {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Grey => "grey",
        }
    }
}
