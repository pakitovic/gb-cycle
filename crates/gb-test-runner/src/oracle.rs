mod catalog;
pub(crate) mod serial_contains;

#[cfg(test)]
mod test;

pub(crate) use catalog::{Oracle, OracleConfig, OracleObservations};
