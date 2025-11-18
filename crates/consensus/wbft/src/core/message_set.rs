//! Message storage for consensus rounds
//!
//! Manages storage and retrieval of consensus messages during a round.

use crate::messages::{Commit, Prepare};
use alloy_primitives::Address;
use std::collections::HashMap;

/// Storage for consensus messages in current round
///
/// Tracks PREPARE and COMMIT messages received from validators.
#[derive(Debug, Clone)]
pub struct MessageSet {
    /// PREPARE messages indexed by sender address
    prepares: HashMap<Address, Prepare>,

    /// COMMIT messages indexed by sender address
    commits: HashMap<Address, Commit>,
}

impl MessageSet {
    /// Create a new empty message set
    pub fn new() -> Self {
        Self { prepares: HashMap::new(), commits: HashMap::new() }
    }

    /// Clear all stored messages
    pub fn clear(&mut self) {
        self.prepares.clear();
        self.commits.clear();
    }

    /// Add a PREPARE message
    ///
    /// # Arguments
    ///
    /// * `msg` - PREPARE message to add
    pub fn add_prepare(&mut self, msg: Prepare) {
        self.prepares.insert(msg.sender, msg);
    }

    /// Add a COMMIT message
    ///
    /// # Arguments
    ///
    /// * `msg` - COMMIT message to add
    pub fn add_commit(&mut self, msg: Commit) {
        self.commits.insert(msg.sender, msg);
    }

    /// Check if we have a PREPARE from given address
    pub fn has_prepare(&self, addr: &Address) -> bool {
        self.prepares.contains_key(addr)
    }

    /// Check if we have a COMMIT from given address
    pub fn has_commit(&self, addr: &Address) -> bool {
        self.commits.contains_key(addr)
    }

    /// Get PREPARE message from address
    pub fn get_prepare(&self, addr: &Address) -> Option<&Prepare> {
        self.prepares.get(addr)
    }

    /// Get COMMIT message from address
    pub fn get_commit(&self, addr: &Address) -> Option<&Commit> {
        self.commits.get(addr)
    }

    /// Get number of PREPARE messages
    pub fn prepare_count(&self) -> usize {
        self.prepares.len()
    }

    /// Get number of COMMIT messages
    pub fn commit_count(&self) -> usize {
        self.commits.len()
    }

    /// Get all PREPARE messages
    pub fn prepares(&self) -> impl Iterator<Item = &Prepare> {
        self.prepares.values()
    }

    /// Get all COMMIT messages
    pub fn commits(&self) -> impl Iterator<Item = &Commit> {
        self.commits.values()
    }

    /// Extract commit seals from all COMMIT messages
    ///
    /// Returns vector of commit seals for aggregation.
    pub fn commit_seals(&self) -> Vec<[u8; 96]> {
        self.commits.values().map(|commit| commit.commit_seal).collect()
    }
}

impl Default for MessageSet {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{bls::SecretKey, messages::WbftMessage, types::View};
    use alloy_primitives::{B256, U256};

    fn create_test_prepare(sender: Address) -> Prepare {
        let sk = SecretKey::random();
        let view = View { sequence: U256::from(1), round: U256::from(0) };
        let digest = B256::from([0x42; 32]);

        let mut prepare = Prepare::new(view, digest, sender, [0u8; 96]);
        let message = prepare.encode_for_signing();
        let signature = sk.sign(&message);
        prepare.signature = signature.to_bytes();

        prepare
    }

    fn create_test_commit(sender: Address) -> Commit {
        let sk = SecretKey::random();
        let view = View { sequence: U256::from(1), round: U256::from(0) };
        let digest = B256::from([0x42; 32]);
        let commit_seal = sk.sign(b"prepare seal").to_bytes();

        let mut commit = Commit::new(view, digest, sender, [0u8; 96], commit_seal);
        let message = commit.encode_for_signing();
        let signature = sk.sign(&message);
        commit.signature = signature.to_bytes();

        commit
    }

    #[test]
    fn test_message_set_creation() {
        let msg_set = MessageSet::new();

        assert_eq!(msg_set.prepare_count(), 0);
        assert_eq!(msg_set.commit_count(), 0);
    }

    #[test]
    fn test_add_prepare() {
        let mut msg_set = MessageSet::new();
        let addr1 = Address::from([0x01; 20]);
        let addr2 = Address::from([0x02; 20]);

        let prepare1 = create_test_prepare(addr1);
        let prepare2 = create_test_prepare(addr2);

        msg_set.add_prepare(prepare1);
        msg_set.add_prepare(prepare2);

        assert_eq!(msg_set.prepare_count(), 2);
        assert!(msg_set.has_prepare(&addr1));
        assert!(msg_set.has_prepare(&addr2));
    }

    #[test]
    fn test_add_commit() {
        let mut msg_set = MessageSet::new();
        let addr1 = Address::from([0x01; 20]);
        let addr2 = Address::from([0x02; 20]);

        let commit1 = create_test_commit(addr1);
        let commit2 = create_test_commit(addr2);

        msg_set.add_commit(commit1);
        msg_set.add_commit(commit2);

        assert_eq!(msg_set.commit_count(), 2);
        assert!(msg_set.has_commit(&addr1));
        assert!(msg_set.has_commit(&addr2));
    }

    #[test]
    fn test_get_prepare() {
        let mut msg_set = MessageSet::new();
        let addr = Address::from([0x01; 20]);
        let prepare = create_test_prepare(addr);
        let digest = prepare.digest;

        msg_set.add_prepare(prepare);

        let retrieved = msg_set.get_prepare(&addr).unwrap();
        assert_eq!(retrieved.sender, addr);
        assert_eq!(retrieved.digest, digest);
    }

    #[test]
    fn test_get_commit() {
        let mut msg_set = MessageSet::new();
        let addr = Address::from([0x01; 20]);
        let commit = create_test_commit(addr);
        let digest = commit.digest;

        msg_set.add_commit(commit);

        let retrieved = msg_set.get_commit(&addr).unwrap();
        assert_eq!(retrieved.sender, addr);
        assert_eq!(retrieved.digest, digest);
    }

    #[test]
    fn test_clear() {
        let mut msg_set = MessageSet::new();
        let addr = Address::from([0x01; 20]);

        msg_set.add_prepare(create_test_prepare(addr));
        msg_set.add_commit(create_test_commit(addr));

        assert_eq!(msg_set.prepare_count(), 1);
        assert_eq!(msg_set.commit_count(), 1);

        msg_set.clear();

        assert_eq!(msg_set.prepare_count(), 0);
        assert_eq!(msg_set.commit_count(), 0);
    }

    #[test]
    fn test_commit_seals() {
        let mut msg_set = MessageSet::new();
        let addr1 = Address::from([0x01; 20]);
        let addr2 = Address::from([0x02; 20]);

        let commit1 = create_test_commit(addr1);
        let commit2 = create_test_commit(addr2);

        let seal1 = commit1.commit_seal;
        let seal2 = commit2.commit_seal;

        msg_set.add_commit(commit1);
        msg_set.add_commit(commit2);

        let seals = msg_set.commit_seals();
        assert_eq!(seals.len(), 2);
        assert!(seals.contains(&seal1) || seals.contains(&seal2));
    }

    #[test]
    fn test_duplicate_message_replacement() {
        let mut msg_set = MessageSet::new();
        let addr = Address::from([0x01; 20]);

        let prepare1 = create_test_prepare(addr);
        let prepare2 = create_test_prepare(addr);

        msg_set.add_prepare(prepare1);
        msg_set.add_prepare(prepare2.clone());

        // Should replace, not add
        assert_eq!(msg_set.prepare_count(), 1);

        // Should have the second message
        let retrieved = msg_set.get_prepare(&addr).unwrap();
        assert_eq!(retrieved.signature, prepare2.signature);
    }

    #[test]
    fn test_iterators() {
        let mut msg_set = MessageSet::new();
        let addr1 = Address::from([0x01; 20]);
        let addr2 = Address::from([0x02; 20]);
        let addr3 = Address::from([0x03; 20]);

        msg_set.add_prepare(create_test_prepare(addr1));
        msg_set.add_prepare(create_test_prepare(addr2));
        msg_set.add_commit(create_test_commit(addr3));

        let prepare_addrs: Vec<Address> = msg_set.prepares().map(|p| p.sender).collect();
        assert_eq!(prepare_addrs.len(), 2);
        assert!(prepare_addrs.contains(&addr1));
        assert!(prepare_addrs.contains(&addr2));

        let commit_addrs: Vec<Address> = msg_set.commits().map(|c| c.sender).collect();
        assert_eq!(commit_addrs.len(), 1);
        assert!(commit_addrs.contains(&addr3));
    }
}
