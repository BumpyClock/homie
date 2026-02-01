use serde::{Deserialize, Serialize};

/// Current protocol version.
pub const PROTOCOL_VERSION: u16 = 1;

/// Version range advertised during handshake.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionRange {
    pub min: u16,
    pub max: u16,
}

impl VersionRange {
    pub fn new(min: u16, max: u16) -> Self {
        Self { min, max }
    }

    /// Returns the highest version both ranges support, or `None`.
    pub fn negotiate(&self, other: &VersionRange) -> Option<u16> {
        let lo = self.min.max(other.min);
        let hi = self.max.min(other.max);
        if lo <= hi {
            Some(hi)
        } else {
            None
        }
    }
}

impl Default for VersionRange {
    fn default() -> Self {
        Self {
            min: PROTOCOL_VERSION,
            max: PROTOCOL_VERSION,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negotiate_overlap() {
        let a = VersionRange::new(1, 3);
        let b = VersionRange::new(2, 5);
        assert_eq!(a.negotiate(&b), Some(3));
    }

    #[test]
    fn negotiate_exact() {
        let a = VersionRange::new(1, 1);
        let b = VersionRange::new(1, 1);
        assert_eq!(a.negotiate(&b), Some(1));
    }

    #[test]
    fn negotiate_no_overlap() {
        let a = VersionRange::new(1, 2);
        let b = VersionRange::new(3, 4);
        assert_eq!(a.negotiate(&b), None);
    }
}
