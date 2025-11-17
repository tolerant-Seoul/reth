//! Core WBFT data structures
//!
//! This module defines the fundamental types used throughout the WBFT consensus protocol.

use alloy_primitives::{B256, U256};
use alloy_rlp::{RlpDecodable, RlpEncodable};
use std::cmp::Ordering;

/// WBFT consensus state
///
/// Represents the current phase of the consensus protocol for a given view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum State {
    /// Initial state, ready to accept new proposals
    AcceptRequest,
    /// PRE-PREPARE received and validated
    Preprepared,
    /// Quorum of PREPARE messages received
    Prepared,
    /// Quorum of COMMIT messages received, block finalized
    Committed,
}

impl State {
    /// Check if state allows processing PRE-PREPARE messages
    pub const fn can_accept_preprepare(&self) -> bool {
        matches!(self, Self::AcceptRequest)
    }

    /// Check if state allows processing PREPARE messages
    pub const fn can_accept_prepare(&self) -> bool {
        matches!(self, Self::Preprepared | Self::Prepared | Self::Committed)
    }

    /// Check if state allows processing COMMIT messages
    pub const fn can_accept_commit(&self) -> bool {
        matches!(self, Self::Prepared | Self::Committed)
    }
}

/// View represents the current consensus round
///
/// Combines block sequence number with round number to handle view changes.
/// Round number increases when timeouts occur and consensus needs to restart.
#[derive(Debug, Clone, PartialEq, Eq, Hash, RlpEncodable, RlpDecodable)]
pub struct View {
    /// Block sequence number (height)
    pub sequence: U256,
    /// Round number (starts at 0, increments on timeout)
    pub round: U256,
}

impl View {
    /// Create a new view
    pub const fn new(sequence: U256, round: U256) -> Self {
        Self { sequence, round }
    }

    /// Create view for genesis block
    pub fn genesis() -> Self {
        Self::new(U256::ZERO, U256::ZERO)
    }

    /// Compare two views for ordering
    ///
    /// Views are ordered first by sequence, then by round.
    /// Returns:
    /// - `Ordering::Less` if self < other
    /// - `Ordering::Equal` if self == other
    /// - `Ordering::Greater` if self > other
    pub fn cmp(&self, other: &Self) -> Ordering {
        match self.sequence.cmp(&other.sequence) {
            Ordering::Equal => self.round.cmp(&other.round),
            ord => ord,
        }
    }

    /// Check if this view is for the same sequence but later round
    pub fn is_later_round(&self, other: &Self) -> bool {
        self.sequence == other.sequence && self.round > other.round
    }

    /// Increment round number (for round change)
    pub fn next_round(&self) -> Self {
        Self::new(self.sequence, self.round + U256::from(1))
    }

    /// Next sequence (new block)
    pub fn next_sequence(&self) -> Self {
        Self::new(self.sequence + U256::from(1), U256::ZERO)
    }
}

impl PartialOrd for View {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for View {
    fn cmp(&self, other: &Self) -> Ordering {
        View::cmp(self, other)
    }
}

/// Subject identifies what is being agreed upon in consensus
///
/// Combines the view (which block/round) with the digest (what block content).
#[derive(Debug, Clone, PartialEq, Eq, Hash, RlpEncodable, RlpDecodable)]
pub struct Subject {
    /// The consensus view (sequence + round)
    pub view: View,
    /// Hash digest of the proposed block
    pub digest: B256,
}

impl Subject {
    /// Create a new subject
    pub const fn new(view: View, digest: B256) -> Self {
        Self { view, digest }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_transitions() {
        let state = State::AcceptRequest;
        assert!(state.can_accept_preprepare());
        assert!(!state.can_accept_prepare());
        assert!(!state.can_accept_commit());

        let state = State::Preprepared;
        assert!(!state.can_accept_preprepare());
        assert!(state.can_accept_prepare());
        assert!(!state.can_accept_commit());

        let state = State::Prepared;
        assert!(!state.can_accept_preprepare());
        assert!(state.can_accept_prepare());
        assert!(state.can_accept_commit());

        let state = State::Committed;
        assert!(!state.can_accept_preprepare());
        assert!(state.can_accept_prepare());
        assert!(state.can_accept_commit());
    }

    #[test]
    fn test_view_ordering() {
        let view1 = View::new(U256::from(1), U256::from(0));
        let view2 = View::new(U256::from(1), U256::from(1));
        let view3 = View::new(U256::from(2), U256::from(0));

        assert!(view1 < view2);
        assert!(view2 < view3);
        assert!(view1 < view3);
        assert_eq!(view1.cmp(&view1), Ordering::Equal);
    }

    #[test]
    fn test_view_comparison() {
        let view1 = View::new(U256::from(1), U256::from(0));
        let view2 = View::new(U256::from(1), U256::from(1));
        let view3 = View::new(U256::from(2), U256::from(0));

        assert!(view2.is_later_round(&view1));
        assert!(!view3.is_later_round(&view1));
        assert!(!view1.is_later_round(&view2));
    }

    #[test]
    fn test_view_next_round() {
        let view = View::new(U256::from(5), U256::from(2));
        let next = view.next_round();

        assert_eq!(next.sequence, U256::from(5));
        assert_eq!(next.round, U256::from(3));
    }

    #[test]
    fn test_view_next_sequence() {
        let view = View::new(U256::from(5), U256::from(2));
        let next = view.next_sequence();

        assert_eq!(next.sequence, U256::from(6));
        assert_eq!(next.round, U256::ZERO);
    }

    #[test]
    fn test_view_rlp_roundtrip() {
        use alloy_rlp::{Decodable, Encodable};

        let view = View::new(U256::from(12345), U256::from(67));
        let mut encoded = Vec::new();
        view.encode(&mut encoded);
        let decoded = View::decode(&mut &encoded[..]).expect("decode failed");

        assert_eq!(view, decoded);
    }

    #[test]
    fn test_subject_rlp_roundtrip() {
        use alloy_rlp::{Decodable, Encodable};

        let view = View::new(U256::from(100), U256::from(5));
        let digest = B256::from([0x42; 32]);
        let subject = Subject::new(view, digest);

        let mut encoded = Vec::new();
        subject.encode(&mut encoded);
        let decoded = Subject::decode(&mut &encoded[..]).expect("decode failed");

        assert_eq!(subject, decoded);
    }

    #[test]
    fn test_genesis_view() {
        let genesis = View::genesis();
        assert_eq!(genesis.sequence, U256::ZERO);
        assert_eq!(genesis.round, U256::ZERO);
    }
}
