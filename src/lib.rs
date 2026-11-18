#![no_std]

pub const fn saturating_increment(value: u8) -> u8 {
    value.saturating_add(1)
}

#[cfg(test)]
mod tests {
    use super::saturating_increment;

    #[test]
    fn increments_values() {
        assert_eq!(saturating_increment(0), 1);
        assert_eq!(saturating_increment(254), 255);
    }

    #[test]
    fn saturates_at_maximum() {
        assert_eq!(saturating_increment(u8::MAX), u8::MAX);
    }
}
