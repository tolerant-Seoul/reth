//! Validator set management
//!
//! Manages the collection of validators and proposer selection.

use super::{DefaultValidator, Validator, ValidatorError};
use alloy_primitives::Address;

/// Trait for managing validator sets
///
/// Provides interface for validator set operations including proposer
/// selection, quorum calculation, and validator queries.
pub trait ValidatorSet: Send + Sync {
    /// Get the current proposer
    fn proposer(&self) -> &dyn Validator;

    /// Get validator by address
    ///
    /// # Errors
    ///
    /// Returns error if validator not found
    fn get_by_address(&self, addr: &Address) -> Result<&dyn Validator, ValidatorError>;

    /// Get validator by index
    ///
    /// # Errors
    ///
    /// Returns error if index out of bounds
    fn get_by_index(&self, index: usize) -> Result<&dyn Validator, ValidatorError>;

    /// Get number of validators
    fn size(&self) -> usize;

    /// Get all validators
    fn list(&self) -> Vec<&dyn Validator>;

    /// Calculate Byzantine fault tolerance threshold (f)
    ///
    /// For n validators, f = floor((n-1)/3)
    fn f(&self) -> usize {
        if self.size() == 0 {
            0
        } else {
            (self.size() - 1) / 3
        }
    }

    /// Calculate quorum size (2f + 1)
    ///
    /// Minimum number of validators needed for consensus
    fn quorum_size(&self) -> usize {
        2 * self.f() + 1
    }

    /// Check if address is a validator
    fn is_validator(&self, addr: &Address) -> bool {
        self.get_by_address(addr).is_ok()
    }

    /// Get index of validator by address
    ///
    /// # Errors
    ///
    /// Returns error if validator not found
    fn get_index(&self, addr: &Address) -> Result<usize, ValidatorError>;
}

/// Default implementation of ValidatorSet
#[derive(Debug, Clone)]
pub struct DefaultValidatorSet {
    /// List of validators
    validators: Vec<DefaultValidator>,

    /// Current proposer index
    proposer_index: usize,
}

impl DefaultValidatorSet {
    /// Create a new validator set
    ///
    /// # Arguments
    ///
    /// * `validators` - List of validators (must not be empty)
    ///
    /// # Errors
    ///
    /// Returns error if validator list is empty
    pub fn new(validators: Vec<DefaultValidator>) -> Result<Self, ValidatorError> {
        if validators.is_empty() {
            return Err(ValidatorError::EmptySet);
        }

        Ok(Self { validators, proposer_index: 0 })
    }

    /// Set proposer by index
    ///
    /// # Errors
    ///
    /// Returns error if index out of bounds
    pub fn set_proposer(&mut self, index: usize) -> Result<(), ValidatorError> {
        if index >= self.validators.len() {
            return Err(ValidatorError::InvalidIndex { index, size: self.validators.len() });
        }

        self.proposer_index = index;
        Ok(())
    }

    /// Set proposer by address
    ///
    /// # Errors
    ///
    /// Returns error if validator not found
    pub fn set_proposer_by_address(&mut self, addr: &Address) -> Result<(), ValidatorError> {
        let index = self.get_index(addr)?;
        self.proposer_index = index;
        Ok(())
    }

    /// Calculate next proposer based on round (round-robin)
    ///
    /// # Arguments
    ///
    /// * `round` - Current round number
    pub fn calc_proposer(&mut self, round: u64) {
        self.proposer_index = (round as usize) % self.validators.len();
    }

    /// Get validators as slice
    pub fn validators(&self) -> &[DefaultValidator] {
        &self.validators
    }

    /// Get proposer index
    pub fn proposer_index(&self) -> usize {
        self.proposer_index
    }
}

impl ValidatorSet for DefaultValidatorSet {
    fn proposer(&self) -> &dyn Validator {
        &self.validators[self.proposer_index]
    }

    fn get_by_address(&self, addr: &Address) -> Result<&dyn Validator, ValidatorError> {
        self.validators
            .iter()
            .find(|v| v.address() == *addr)
            .map(|v| v as &dyn Validator)
            .ok_or_else(|| ValidatorError::NotFound(*addr))
    }

    fn get_by_index(&self, index: usize) -> Result<&dyn Validator, ValidatorError> {
        self.validators
            .get(index)
            .map(|v| v as &dyn Validator)
            .ok_or_else(|| ValidatorError::InvalidIndex { index, size: self.validators.len() })
    }

    fn size(&self) -> usize {
        self.validators.len()
    }

    fn list(&self) -> Vec<&dyn Validator> {
        self.validators.iter().map(|v| v as &dyn Validator).collect()
    }

    fn get_index(&self, addr: &Address) -> Result<usize, ValidatorError> {
        self.validators
            .iter()
            .position(|v| v.address() == *addr)
            .ok_or_else(|| ValidatorError::NotFound(*addr))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_validators(count: usize) -> Vec<DefaultValidator> {
        (0..count)
            .map(|i| {
                let mut addr_bytes = [0u8; 20];
                addr_bytes[0] = i as u8;
                let addr = Address::from(addr_bytes);

                let mut key = [0u8; 48];
                key[0] = i as u8;

                DefaultValidator::new(addr, key)
            })
            .collect()
    }

    #[test]
    fn test_validator_set_creation() {
        let validators = create_test_validators(4);
        let set = DefaultValidatorSet::new(validators).unwrap();

        assert_eq!(set.size(), 4);
        assert_eq!(set.proposer_index(), 0);
    }

    #[test]
    fn test_empty_validator_set() {
        let result = DefaultValidatorSet::new(vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn test_quorum_size_4_validators() {
        let validators = create_test_validators(4);
        let set = DefaultValidatorSet::new(validators).unwrap();

        assert_eq!(set.f(), 1); // (4-1)/3 = 1
        assert_eq!(set.quorum_size(), 3); // 2*1+1 = 3
    }

    #[test]
    fn test_quorum_size_7_validators() {
        let validators = create_test_validators(7);
        let set = DefaultValidatorSet::new(validators).unwrap();

        assert_eq!(set.f(), 2); // (7-1)/3 = 2
        assert_eq!(set.quorum_size(), 5); // 2*2+1 = 5
    }

    #[test]
    fn test_quorum_size_10_validators() {
        let validators = create_test_validators(10);
        let set = DefaultValidatorSet::new(validators).unwrap();

        assert_eq!(set.f(), 3); // (10-1)/3 = 3
        assert_eq!(set.quorum_size(), 7); // 2*3+1 = 7
    }

    #[test]
    fn test_get_by_address() {
        let validators = create_test_validators(4);
        let addr = validators[2].address();
        let set = DefaultValidatorSet::new(validators).unwrap();

        let validator = set.get_by_address(&addr).unwrap();
        assert_eq!(validator.address(), addr);
    }

    #[test]
    fn test_get_by_address_not_found() {
        let validators = create_test_validators(4);
        let set = DefaultValidatorSet::new(validators).unwrap();

        let unknown_addr = Address::from([0xFF; 20]);
        let result = set.get_by_address(&unknown_addr);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_by_index() {
        let validators = create_test_validators(4);
        let expected_addr = validators[2].address();
        let set = DefaultValidatorSet::new(validators).unwrap();

        let validator = set.get_by_index(2).unwrap();
        assert_eq!(validator.address(), expected_addr);
    }

    #[test]
    fn test_get_by_index_out_of_bounds() {
        let validators = create_test_validators(4);
        let set = DefaultValidatorSet::new(validators).unwrap();

        let result = set.get_by_index(10);
        assert!(result.is_err());
    }

    #[test]
    fn test_is_validator() {
        let validators = create_test_validators(4);
        let addr = validators[1].address();
        let set = DefaultValidatorSet::new(validators).unwrap();

        assert!(set.is_validator(&addr));
        assert!(!set.is_validator(&Address::from([0xFF; 20])));
    }

    #[test]
    fn test_get_index() {
        let validators = create_test_validators(4);
        let addr = validators[2].address();
        let set = DefaultValidatorSet::new(validators).unwrap();

        let index = set.get_index(&addr).unwrap();
        assert_eq!(index, 2);
    }

    #[test]
    fn test_proposer() {
        let validators = create_test_validators(4);
        let expected_addr = validators[0].address();
        let set = DefaultValidatorSet::new(validators).unwrap();

        assert_eq!(set.proposer().address(), expected_addr);
    }

    #[test]
    fn test_set_proposer() {
        let validators = create_test_validators(4);
        let expected_addr = validators[2].address();
        let mut set = DefaultValidatorSet::new(validators).unwrap();

        set.set_proposer(2).unwrap();
        assert_eq!(set.proposer().address(), expected_addr);
        assert_eq!(set.proposer_index(), 2);
    }

    #[test]
    fn test_set_proposer_by_address() {
        let validators = create_test_validators(4);
        let addr = validators[3].address();
        let mut set = DefaultValidatorSet::new(validators).unwrap();

        set.set_proposer_by_address(&addr).unwrap();
        assert_eq!(set.proposer().address(), addr);
        assert_eq!(set.proposer_index(), 3);
    }

    #[test]
    fn test_calc_proposer_round_robin() {
        let validators = create_test_validators(4);
        let mut set = DefaultValidatorSet::new(validators.clone()).unwrap();

        // Round 0: proposer index 0
        set.calc_proposer(0);
        assert_eq!(set.proposer().address(), validators[0].address());

        // Round 1: proposer index 1
        set.calc_proposer(1);
        assert_eq!(set.proposer().address(), validators[1].address());

        // Round 4: wraps around to index 0
        set.calc_proposer(4);
        assert_eq!(set.proposer().address(), validators[0].address());

        // Round 7: wraps to index 3
        set.calc_proposer(7);
        assert_eq!(set.proposer().address(), validators[3].address());
    }

    #[test]
    fn test_list() {
        let validators = create_test_validators(4);
        let set = DefaultValidatorSet::new(validators.clone()).unwrap();

        let list = set.list();
        assert_eq!(list.len(), 4);

        for (i, validator) in list.iter().enumerate() {
            assert_eq!(validator.address(), validators[i].address());
        }
    }
}
