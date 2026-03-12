pub fn version() -> &'static str {
    "gb-cycle"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_version() {
        assert_eq!(version(), "gb-cycle");
    }
}
