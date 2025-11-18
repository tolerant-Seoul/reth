//! WBFT consensus core state machine
//!
//! This module implements the core consensus state machine that orchestrates
//! the 3-phase commit protocol (PRE-PREPARE → PREPARE → COMMIT).

use crate::{
    messages::{Commit, MessageError, PrePrepare, Prepare, RoundChange, WbftMessage},
    types::{State, Subject, View},
};
use alloy_primitives::{Address, B256};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use thiserror::Error;

pub mod backend;
pub mod message_set;

pub use backend::Backend;
pub use message_set::MessageSet;

/// Errors that can occur during consensus operations
#[derive(Debug, Error)]
pub enum CoreError {
    /// Invalid state transition
    #[error("invalid state transition from {from:?} to {to:?}")]
    InvalidStateTransition {
        /// Current state
        from: State,
        /// Attempted new state
        to: State,
    },

    /// Message from wrong view
    #[error("message from wrong view: expected {expected:?}, got {actual:?}")]
    WrongView {
        /// Expected view
        expected: View,
        /// Actual view in message
        actual: View,
    },

    /// Not enough messages for quorum
    #[error("insufficient messages: need {needed}, have {actual}")]
    InsufficientMessages {
        /// Number of messages needed
        needed: usize,
        /// Number of messages received
        actual: usize,
    },

    /// Message validation failed
    #[error("message validation failed: {0}")]
    MessageValidation(#[from] MessageError),

    /// Backend operation failed
    #[error("backend error: {0}")]
    Backend(String),

    /// Invalid proposer
    #[error("invalid proposer: expected {expected}, got {actual}")]
    InvalidProposer {
        /// Expected proposer address
        expected: Address,
        /// Actual message sender
        actual: Address,
    },

    /// Duplicate message
    #[error("duplicate message from {0}")]
    DuplicateMessage(Address),
}

/// Core consensus state machine
///
/// Manages the consensus protocol state transitions and message processing.
pub struct Core<B: Backend> {
    /// Current consensus state
    state: State,

    /// Current view (sequence + round)
    current_view: View,

    /// Blockchain backend for block validation and finalization
    backend: Arc<B>,

    /// Current block proposal (if any)
    current_proposal: Option<B256>,

    /// Message storage for current consensus round
    message_set: MessageSet,

    /// Round change message storage
    round_change_set: HashMap<Address, RoundChange>,

    /// Validator addresses in current set
    validators: Vec<Address>,

    /// Current proposer address
    proposer: Address,
}

impl<B: Backend> Core<B> {
    /// Create a new Core instance
    ///
    /// # Arguments
    ///
    /// * `backend` - Blockchain backend
    /// * `validators` - List of validator addresses
    /// * `initial_proposer` - Address of initial proposer
    pub fn new(backend: Arc<B>, validators: Vec<Address>, initial_proposer: Address) -> Self {
        let initial_view = View::genesis();

        Self {
            state: State::AcceptRequest,
            current_view: initial_view,
            backend,
            current_proposal: None,
            message_set: MessageSet::new(),
            round_change_set: HashMap::new(),
            validators,
            proposer: initial_proposer,
        }
    }

    /// Get current consensus state
    pub fn state(&self) -> State {
        self.state
    }

    /// Get current view
    pub fn view(&self) -> &View {
        &self.current_view
    }

    /// Get current proposal hash
    pub fn proposal(&self) -> Option<B256> {
        self.current_proposal
    }

    /// Calculate quorum size (2f + 1)
    pub fn quorum_size(&self) -> usize {
        let f = (self.validators.len() - 1) / 3;
        2 * f + 1
    }

    /// Check if address is current proposer
    pub fn is_proposer(&self, addr: &Address) -> bool {
        &self.proposer == addr
    }

    /// Transition to new state
    ///
    /// # Errors
    ///
    /// Returns error if transition is invalid
    fn transition_state(&mut self, new_state: State) -> Result<(), CoreError> {
        // Validate state transition
        let valid = match (self.state, new_state) {
            (State::AcceptRequest, State::Preprepared) => true,
            (State::Preprepared, State::Prepared) => true,
            (State::Prepared, State::Committed) => true,
            (_, State::AcceptRequest) => true, // Can always reset to AcceptRequest
            _ => false,
        };

        if !valid {
            return Err(CoreError::InvalidStateTransition { from: self.state, to: new_state });
        }

        self.state = new_state;
        Ok(())
    }

    /// Handle PRE-PREPARE message
    ///
    /// Validates and processes PRE-PREPARE message from proposer.
    ///
    /// # Errors
    ///
    /// Returns error if message is invalid or from wrong proposer
    pub fn handle_preprepare(&mut self, msg: &PrePrepare) -> Result<(), CoreError> {
        // Validate view matches
        if msg.view != self.current_view {
            return Err(CoreError::WrongView {
                expected: self.current_view.clone(),
                actual: msg.view.clone(),
            });
        }

        // Validate proposer
        if &msg.sender != &self.proposer {
            return Err(CoreError::InvalidProposer { expected: self.proposer, actual: msg.sender });
        }

        // Validate message
        msg.validate()?;

        // Validate proposal through backend
        self.backend
            .verify_proposal(&msg.proposal_block)
            .map_err(|e| CoreError::Backend(e.to_string()))?;

        // Store proposal and transition state
        self.current_proposal = Some(msg.proposal);
        self.transition_state(State::Preprepared)?;

        Ok(())
    }

    /// Handle PREPARE message
    ///
    /// Collects PREPARE messages and transitions to Prepared when quorum reached.
    ///
    /// # Errors
    ///
    /// Returns error if message is invalid
    pub fn handle_prepare(&mut self, msg: &Prepare) -> Result<(), CoreError> {
        // Validate view matches
        if msg.view != self.current_view {
            return Err(CoreError::WrongView {
                expected: self.current_view.clone(),
                actual: msg.view.clone(),
            });
        }

        // Validate message
        msg.validate()?;

        // Check for duplicate
        if self.message_set.has_prepare(&msg.sender) {
            return Err(CoreError::DuplicateMessage(msg.sender));
        }

        // Add to message set
        self.message_set.add_prepare(msg.clone());

        // Check if we have quorum
        if self.message_set.prepare_count() >= self.quorum_size() {
            self.transition_state(State::Prepared)?;
        }

        Ok(())
    }

    /// Handle COMMIT message
    ///
    /// Collects COMMIT messages and finalizes block when quorum reached.
    ///
    /// # Errors
    ///
    /// Returns error if message is invalid
    pub fn handle_commit(&mut self, msg: &Commit) -> Result<(), CoreError> {
        // Validate view matches
        if msg.view != self.current_view {
            return Err(CoreError::WrongView {
                expected: self.current_view.clone(),
                actual: msg.view.clone(),
            });
        }

        // Validate message
        msg.validate()?;

        // Check for duplicate
        if self.message_set.has_commit(&msg.sender) {
            return Err(CoreError::DuplicateMessage(msg.sender));
        }

        // Add to message set
        self.message_set.add_commit(msg.clone());

        // Check if we have quorum
        if self.message_set.commit_count() >= self.quorum_size() {
            self.transition_state(State::Committed)?;

            // Finalize block through backend
            if let Some(proposal) = self.current_proposal {
                self.backend
                    .commit(proposal, self.message_set.commit_seals())
                    .map_err(|e| CoreError::Backend(e.to_string()))?;
            }
        }

        Ok(())
    }

    /// Handle ROUND-CHANGE message
    ///
    /// Collects round change messages and triggers view change when quorum reached.
    ///
    /// # Errors
    ///
    /// Returns error if message is invalid
    pub fn handle_round_change(&mut self, msg: &RoundChange) -> Result<(), CoreError> {
        // Validate message
        msg.validate()?;

        // Check for duplicate
        if self.round_change_set.contains_key(&msg.sender) {
            return Err(CoreError::DuplicateMessage(msg.sender));
        }

        // Add to round change set
        self.round_change_set.insert(msg.sender, msg.clone());

        // Check if we have quorum for this view
        let round_change_count =
            self.round_change_set.values().filter(|rc| rc.view == msg.view).count();

        if round_change_count >= self.quorum_size() {
            // Trigger view change
            self.start_new_round(msg.view.clone())?;
        }

        Ok(())
    }

    /// Start a new consensus round
    ///
    /// Resets state and message sets for new round.
    ///
    /// # Errors
    ///
    /// Returns error if view transition is invalid
    pub fn start_new_round(&mut self, new_view: View) -> Result<(), CoreError> {
        // Clear message sets
        self.message_set.clear();
        self.round_change_set.clear();
        self.current_proposal = None;

        // Update view and state
        self.current_view = new_view;
        self.transition_state(State::AcceptRequest)?;

        // Update proposer for new round
        self.update_proposer();

        Ok(())
    }

    /// Update proposer for current round (round-robin)
    fn update_proposer(&mut self) {
        if self.validators.is_empty() {
            return;
        }

        let round = self.current_view.round.to::<u64>();
        let index = (round as usize) % self.validators.len();
        self.proposer = self.validators[index];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bls::SecretKey;
    use alloy_primitives::{Bytes, U256};

    /// Mock backend for testing
    struct MockBackend;

    impl Backend for MockBackend {
        fn verify_proposal(&self, _proposal: &Bytes) -> Result<(), Box<dyn std::error::Error>> {
            Ok(())
        }

        fn commit(
            &self,
            _hash: B256,
            _seals: Vec<[u8; 96]>,
        ) -> Result<(), Box<dyn std::error::Error>> {
            Ok(())
        }
    }

    fn create_test_core() -> Core<MockBackend> {
        let backend = Arc::new(MockBackend);
        let validators = vec![
            Address::from([0x01; 20]),
            Address::from([0x02; 20]),
            Address::from([0x03; 20]),
            Address::from([0x04; 20]),
        ];
        let proposer = validators[0];

        Core::new(backend, validators, proposer)
    }

    #[test]
    fn test_core_creation() {
        let core = create_test_core();

        assert_eq!(core.state(), State::AcceptRequest);
        assert_eq!(core.view(), &View::genesis());
        assert_eq!(core.proposal(), None);
        assert_eq!(core.quorum_size(), 3); // 4 validators: 2*1+1 = 3
    }

    #[test]
    fn test_quorum_size_calculation() {
        let core = create_test_core();
        assert_eq!(core.quorum_size(), 3); // 4 validators: f=1, quorum=3

        // Test with 7 validators
        let backend = Arc::new(MockBackend);
        let validators = (0..7).map(|i| Address::from([i as u8; 20])).collect();
        let core = Core::new(backend, validators, Address::from([0x00; 20]));
        assert_eq!(core.quorum_size(), 5); // 7 validators: f=2, quorum=5
    }

    #[test]
    fn test_state_transition() {
        let mut core = create_test_core();

        // Valid transitions
        assert!(core.transition_state(State::Preprepared).is_ok());
        assert_eq!(core.state(), State::Preprepared);

        assert!(core.transition_state(State::Prepared).is_ok());
        assert_eq!(core.state(), State::Prepared);

        assert!(core.transition_state(State::Committed).is_ok());
        assert_eq!(core.state(), State::Committed);

        // Can always reset to AcceptRequest
        assert!(core.transition_state(State::AcceptRequest).is_ok());
        assert_eq!(core.state(), State::AcceptRequest);
    }

    #[test]
    fn test_invalid_state_transition() {
        let mut core = create_test_core();

        // Cannot go directly from AcceptRequest to Prepared
        let result = core.transition_state(State::Prepared);
        assert!(result.is_err());
        assert_eq!(core.state(), State::AcceptRequest);
    }

    #[test]
    fn test_proposer_rotation() {
        let mut core = create_test_core();

        // Round 0: proposer is validators[0]
        assert_eq!(core.proposer, Address::from([0x01; 20]));

        // Move to round 1
        let new_view = View { sequence: U256::from(1), round: U256::from(1) };
        core.start_new_round(new_view).unwrap();
        assert_eq!(core.proposer, Address::from([0x02; 20]));

        // Move to round 2
        let new_view = View { sequence: U256::from(1), round: U256::from(2) };
        core.start_new_round(new_view).unwrap();
        assert_eq!(core.proposer, Address::from([0x03; 20]));
    }
}
