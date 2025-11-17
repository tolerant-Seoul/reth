//! Sealer set bitmap for tracking validator signatures
//!
//! Efficiently tracks which validators have signed using a bitmap representation.

use alloy_rlp::{RlpDecodable, RlpEncodable};

/// Sealer set represented as a bitmap
///
/// Each bit indicates whether a validator at that index has signed.
/// Supports up to 8 * byte_count validators.
#[derive(Debug, Clone, PartialEq, Eq, RlpEncodable, RlpDecodable)]
pub struct SealerSet(Vec<u8>);

impl SealerSet {
    /// Create a new sealer set with capacity for `size` validators
    ///
    /// Initializes all bits to 0 (no sealers).
    pub fn new(size: usize) -> Self {
        let byte_count = (size + 7) / 8;
        Self(vec![0u8; byte_count])
    }

    /// Mark a validator as having signed
    ///
    /// # Panics
    ///
    /// Panics if index is out of bounds for the bitmap.
    pub fn set_sealer(&mut self, index: u32) {
        let byte_idx = (index / 8) as usize;
        let bit_idx = index % 8;

        if byte_idx >= self.0.len() {
            panic!("sealer index {} out of bounds", index);
        }

        self.0[byte_idx] |= 1 << bit_idx;
    }

    /// Check if a validator has signed
    ///
    /// Returns `false` if index is out of bounds.
    pub fn is_sealer(&self, index: u32) -> bool {
        let byte_idx = (index / 8) as usize;
        let bit_idx = index % 8;

        if byte_idx >= self.0.len() {
            return false;
        }

        (self.0[byte_idx] & (1 << bit_idx)) != 0
    }

    /// Get list of all validator indices that have signed
    pub fn get_sealers(&self) -> Vec<u32> {
        let mut sealers = Vec::new();
        let max_index = self.0.len() * 8;

        for index in 0..max_index {
            if self.is_sealer(index as u32) {
                sealers.push(index as u32);
            }
        }

        sealers
    }

    /// Count how many validators have signed
    pub fn count(&self) -> usize {
        self.0.iter().map(|byte| byte.count_ones() as usize).sum()
    }

    /// Get the underlying byte array
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Create from byte array
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sealer_set_basic() {
        let mut set = SealerSet::new(10);

        assert_eq!(set.count(), 0);
        assert!(!set.is_sealer(0));
        assert!(!set.is_sealer(5));

        set.set_sealer(0);
        assert!(set.is_sealer(0));
        assert_eq!(set.count(), 1);

        set.set_sealer(5);
        assert!(set.is_sealer(5));
        assert_eq!(set.count(), 2);
    }

    #[test]
    fn test_sealer_set_multiple() {
        let mut set = SealerSet::new(16);

        set.set_sealer(0);
        set.set_sealer(3);
        set.set_sealer(7);
        set.set_sealer(15);

        let sealers = set.get_sealers();
        assert_eq!(sealers, vec![0, 3, 7, 15]);
        assert_eq!(set.count(), 4);
    }

    #[test]
    fn test_sealer_set_cross_byte() {
        let mut set = SealerSet::new(16);

        set.set_sealer(7);
        set.set_sealer(8);

        assert!(set.is_sealer(7));
        assert!(set.is_sealer(8));
        assert_eq!(set.count(), 2);
    }

    #[test]
    fn test_sealer_set_idempotent() {
        let mut set = SealerSet::new(10);

        set.set_sealer(5);
        set.set_sealer(5);
        set.set_sealer(5);

        assert_eq!(set.count(), 1);
    }

    #[test]
    fn test_sealer_set_rlp_roundtrip() {
        use alloy_rlp::{Decodable, Encodable};

        let mut set = SealerSet::new(24);
        set.set_sealer(0);
        set.set_sealer(10);
        set.set_sealer(20);

        let mut encoded = Vec::new();
        set.encode(&mut encoded);
        let decoded = SealerSet::decode(&mut &encoded[..]).expect("decode failed");

        assert_eq!(set, decoded);
        assert_eq!(set.get_sealers(), decoded.get_sealers());
    }

    #[test]
    #[should_panic(expected = "out of bounds")]
    fn test_sealer_set_out_of_bounds() {
        let mut set = SealerSet::new(8);
        set.set_sealer(10);
    }

    #[test]
    fn test_is_sealer_out_of_bounds() {
        let set = SealerSet::new(8);
        assert!(!set.is_sealer(10));
    }
}
