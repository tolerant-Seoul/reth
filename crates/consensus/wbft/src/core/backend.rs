//! Backend trait for blockchain integration
//!
//! Defines the interface between consensus engine and blockchain state.

use alloy_primitives::{Bytes, B256};

/// Backend trait for blockchain operations
///
/// This trait provides the interface for the consensus engine to interact
/// with the underlying blockchain state, including block validation and
/// finalization.
pub trait Backend: Send + Sync {
    /// Verify a block proposal
    ///
    /// Validates the proposed block against blockchain rules without
    /// executing it. This includes checks for:
    /// - Valid block structure
    /// - Valid transactions
    /// - State transition validity
    ///
    /// # Arguments
    ///
    /// * `proposal` - RLP-encoded block data
    ///
    /// # Errors
    ///
    /// Returns error if proposal is invalid
    fn verify_proposal(&self, proposal: &Bytes) -> Result<(), Box<dyn std::error::Error>>;

    /// Commit a finalized block
    ///
    /// Finalizes and executes the block, updating blockchain state.
    /// The block has received quorum of COMMIT messages and is considered
    /// final.
    ///
    /// # Arguments
    ///
    /// * `hash` - Block hash to commit
    /// * `seals` - Aggregated commit seals from validators
    ///
    /// # Errors
    ///
    /// Returns error if commit fails
    fn commit(&self, hash: B256, seals: Vec<[u8; 96]>) -> Result<(), Box<dyn std::error::Error>>;

    /// Get current block number
    ///
    /// Returns the height of the latest finalized block.
    fn current_block(&self) -> u64 {
        0
    }

    /// Check if this node should propose for current view
    ///
    /// Determines if this node is the designated proposer based on
    /// validator set and round-robin or other proposer selection policy.
    fn is_proposer(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestBackend {
        block_number: u64,
    }

    impl Backend for TestBackend {
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

        fn current_block(&self) -> u64 {
            self.block_number
        }
    }

    #[test]
    fn test_backend_trait() {
        let backend = TestBackend { block_number: 42 };

        assert_eq!(backend.current_block(), 42);
        assert!(backend.verify_proposal(&Bytes::new()).is_ok());
        assert!(backend.commit(B256::ZERO, vec![]).is_ok());
    }
}
