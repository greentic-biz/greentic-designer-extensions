//! Minimal hex encoding utility shared across the crate and integration tests.

/// Encode a byte slice as a lowercase hexadecimal string.
#[must_use]
pub fn encode(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut acc, b| {
            let _ = write!(acc, "{b:02x}");
            acc
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_known_value() {
        let input = b"\x00\xff\x0f\xf0";
        assert_eq!(encode(input), "00ff0ff0");
    }
}
