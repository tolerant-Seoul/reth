//! Proposer selection policies
//!
//! Implements different strategies for selecting block proposers.

use super::{Validator, ValidatorError, ValidatorSet};
use alloy_primitives::Address;

/// Proposer selection policy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposerPolicy {
    /// Round-robin: proposer rotates each round
    ///
    /// Proposer index = round % validator_count
    RoundRobin,

    /// Sticky: proposer stays same until view change
    ///
    /// Proposer only changes on view change (sequence increment),
    /// not on round change within same sequence.
    Sticky,
}

impl Default for ProposerPolicy {
    fn default() -> Self {
        Self::RoundRobin
    }
}

/// Calculate proposer based on policy
///
/// # Arguments
///
/// * `policy` - Proposer selection policy
/// * `validator_set` - Mutable validator set to update
/// * `last_proposer` - Address of previous proposer
/// * `round` - Current round number
///
/// # Returns
///
/// Address of selected proposer
///
/// # Errors
///
/// Returns error if validator set is empty
pub fn calc_proposer<V: ValidatorSet + ?Sized>(
    policy: ProposerPolicy,
    validator_set: &mut V,
    last_proposer: Address,
    round: u64,
) -> Result<Address, ValidatorError> {
    if validator_set.size() == 0 {
        return Err(ValidatorError::EmptySet);
    }

    let index = match policy {
        ProposerPolicy::RoundRobin => {
            // Simple round-robin based on round number
            (round as usize) % validator_set.size()
        }
        ProposerPolicy::Sticky => {
            // Find last proposer index and use it
            // Only changes when explicitly set (e.g., on view change)
            validator_set.get_index(&last_proposer).unwrap_or(0)
        }
    };

    let proposer = validator_set.get_by_index(index)?;
    Ok(proposer.address())
}

/// Calculate proposer using round-robin policy
///
/// # Arguments
///
/// * `validator_set` - Validator set
/// * `round` - Current round number
///
/// # Returns
///
/// Index of selected proposer
pub fn round_robin_proposer<V: ValidatorSet + ?Sized>(
    validator_set: &V,
    round: u64,
) -> Result<usize, ValidatorError> {
    if validator_set.size() == 0 {
        return Err(ValidatorError::EmptySet);
    }

    Ok((round as usize) % validator_set.size())
}

/// Calculate proposer using sticky policy
///
/// Sticky policy keeps the same proposer until view change.
/// This is useful for maintaining leader stability within a sequence.
///
/// # Arguments
///
/// * `validator_set` - Validator set
/// * `last_proposer` - Address of current/last proposer
///
/// # Returns
///
/// Index of proposer (same as last proposer if valid)
pub fn sticky_proposer<V: ValidatorSet + ?Sized>(
    validator_set: &V,
    last_proposer: &Address,
) -> Result<usize, ValidatorError> {
    if validator_set.size() == 0 {
        return Err(ValidatorError::EmptySet);
    }

    // Try to keep same proposer, fallback to index 0
    validator_set.get_index(last_proposer).or(Ok(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validator::{DefaultValidator, DefaultValidatorSet};

    fn create_test_set(count: usize) -> DefaultValidatorSet {
        let validators: Vec<_> = (0..count)
            .map(|i| {
                let mut addr_bytes = [0u8; 20];
                addr_bytes[0] = (i + 1) as u8;
                DefaultValidator::new(Address::from(addr_bytes), [i as u8; 48])
            })
            .collect();

        DefaultValidatorSet::new(validators).unwrap()
    }

    #[test]
    fn test_round_robin_policy() {
        let mut set = create_test_set(4);
        let addrs: Vec<_> = set.validators().iter().map(|v| v.address()).collect();

        // Round 0: validator 0
        let proposer = calc_proposer(
            ProposerPolicy::RoundRobin,
            &mut set,
            Address::ZERO,
            0,
        )
        .unwrap();
        assert_eq!(proposer, addrs[0]);

        // Round 1: validator 1
        let proposer = calc_proposer(
            ProposerPolicy::RoundRobin,
            &mut set,
            Address::ZERO,
            1,
        )
        .unwrap();
        assert_eq!(proposer, addrs[1]);

        // Round 4: wraps to validator 0
        let proposer = calc_proposer(
            ProposerPolicy::RoundRobin,
            &mut set,
            Address::ZERO,
            4,
        )
        .unwrap();
        assert_eq!(proposer, addrs[0]);

        // Round 7: wraps to validator 3
        let proposer = calc_proposer(
            ProposerPolicy::RoundRobin,
            &mut set,
            Address::ZERO,
            7,
        )
        .unwrap();
        assert_eq!(proposer, addrs[3]);
    }

    #[test]
    fn test_sticky_policy() {
        let mut set = create_test_set(4);
        let addrs: Vec<_> = set.validators().iter().map(|v| v.address()).collect();

        // Set initial proposer to validator 2
        let last_proposer = addrs[2];

        // Round 0: stays at validator 2
        let proposer =
            calc_proposer(ProposerPolicy::Sticky, &mut set, last_proposer, 0).unwrap();
        assert_eq!(proposer, addrs[2]);

        // Round 1: still validator 2 (sticky)
        let proposer =
            calc_proposer(ProposerPolicy::Sticky, &mut set, last_proposer, 1).unwrap();
        assert_eq!(proposer, addrs[2]);

        // Round 5: still validator 2 (sticky)
        let proposer =
            calc_proposer(ProposerPolicy::Sticky, &mut set, last_proposer, 5).unwrap();
        assert_eq!(proposer, addrs[2]);
    }

    #[test]
    fn test_sticky_policy_unknown_proposer() {
        let mut set = create_test_set(4);
        let addrs: Vec<_> = set.validators().iter().map(|v| v.address()).collect();

        // Unknown last proposer: falls back to index 0
        let unknown_proposer = Address::from([0xFF; 20]);
        let proposer =
            calc_proposer(ProposerPolicy::Sticky, &mut set, unknown_proposer, 0).unwrap();
        assert_eq!(proposer, addrs[0]);
    }

    #[test]
    fn test_round_robin_proposer_function() {
        let set = create_test_set(4);

        assert_eq!(round_robin_proposer(&set, 0).unwrap(), 0);
        assert_eq!(round_robin_proposer(&set, 1).unwrap(), 1);
        assert_eq!(round_robin_proposer(&set, 2).unwrap(), 2);
        assert_eq!(round_robin_proposer(&set, 3).unwrap(), 3);
        assert_eq!(round_robin_proposer(&set, 4).unwrap(), 0); // Wraps
        assert_eq!(round_robin_proposer(&set, 7).unwrap(), 3);
    }

    #[test]
    fn test_sticky_proposer_function() {
        let set = create_test_set(4);
        let addrs: Vec<_> = set.validators().iter().map(|v| v.address()).collect();

        // Valid proposer
        assert_eq!(sticky_proposer(&set, &addrs[2]).unwrap(), 2);
        assert_eq!(sticky_proposer(&set, &addrs[0]).unwrap(), 0);

        // Unknown proposer: fallback to 0
        let unknown = Address::from([0xFF; 20]);
        assert_eq!(sticky_proposer(&set, &unknown).unwrap(), 0);
    }

    #[test]
    fn test_proposer_policy_default() {
        let policy = ProposerPolicy::default();
        assert_eq!(policy, ProposerPolicy::RoundRobin);
    }

    #[test]
    fn test_calc_proposer_empty_set() {
        let validators = vec![];
        let set = DefaultValidatorSet::new(validators);
        assert!(set.is_err());
    }

    #[test]
    fn test_policy_comparison() {
        assert_eq!(ProposerPolicy::RoundRobin, ProposerPolicy::RoundRobin);
        assert_eq!(ProposerPolicy::Sticky, ProposerPolicy::Sticky);
        assert_ne!(ProposerPolicy::RoundRobin, ProposerPolicy::Sticky);
    }
}
