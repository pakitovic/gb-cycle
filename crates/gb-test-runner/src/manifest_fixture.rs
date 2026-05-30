use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub(crate) enum ManifestFixtureField {
    Single(PathBuf),
    Multiple(Vec<PathBuf>),
}

impl ManifestFixtureField {
    pub(crate) fn into_single_path(self, case_id: &str, oracle: &str) -> Result<PathBuf, String> {
        match self {
            Self::Single(path) => Ok(path),
            Self::Multiple(paths) => match paths.as_slice() {
                [path] => Ok(path.clone()),
                [] => Err(format!(
                    "case {case_id} has empty fixture array for {oracle}"
                )),
                _ => Err(format!(
                    "case {case_id} fixture must contain exactly one path for {oracle}"
                )),
            },
        }
    }

    pub(crate) fn into_non_empty_paths(
        self,
        case_id: &str,
        oracle: &str,
    ) -> Result<Vec<PathBuf>, String> {
        let paths = match self {
            Self::Single(path) => vec![path],
            Self::Multiple(paths) => paths,
        };
        if paths.is_empty() {
            return Err(format!("case {case_id} is missing fixture for {oracle}"));
        }
        Ok(paths)
    }

    #[cfg(test)]
    pub(crate) fn into_paths(self) -> Vec<PathBuf> {
        match self {
            Self::Single(path) => vec![path],
            Self::Multiple(paths) => paths,
        }
    }
}
