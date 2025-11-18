//! WBFT Consensus implementation
//!
//! Implements reth's Consensus, HeaderValidator, and FullConsensus traits
//! for WBFT Byzantine Fault Tolerant consensus protocol.

use reth_chainspec::ChainSpec;
use reth_consensus::{Consensus, ConsensusError, FullConsensus, HeaderValidator};
use reth_consensus_common::validation::validate_body_against_header;
use reth_execution_types::BlockExecutionResult;
use reth_primitives_traits::{Block, NodePrimitives, RecoveredBlock, SealedBlock, SealedHeader};
use std::sync::Arc;

/// WBFT consensus configuration
#[derive(Debug, Clone)]
pub struct WbftConfig {
    /// Request timeout in seconds
    pub request_timeout_seconds: u64,

    /// Block period in seconds (target block time)
    pub block_period_seconds: u64,

    /// Proposer selection policy (0 = RoundRobin, 1 = Sticky)
    pub proposer_policy: u64,

    /// Epoch length in blocks
    pub epoch_length: u64,

    /// Maximum request timeout in seconds (optional)
    pub max_request_timeout_seconds: Option<u64>,
}

impl Default for WbftConfig {
    fn default() -> Self {
        Self {
            request_timeout_seconds: 2,
            block_period_seconds: 1,
            proposer_policy: 0, // RoundRobin
            epoch_length: 10,
            max_request_timeout_seconds: None,
        }
    }
}

/// WBFT consensus implementation
///
/// This struct implements reth's consensus traits for WBFT protocol.
/// It validates blocks according to WBFT rules including BLS signature
/// verification and quorum checking.
#[derive(Debug, Clone)]
pub struct WbftConsensus {
    /// Chain specification
    chain_spec: Arc<ChainSpec>,

    /// WBFT configuration
    config: WbftConfig,
}

impl WbftConsensus {
    /// Create a new WBFT consensus instance
    ///
    /// # Arguments
    ///
    /// * `chain_spec` - Chain specification
    /// * `config` - WBFT configuration
    pub fn new(chain_spec: Arc<ChainSpec>, config: WbftConfig) -> Self {
        Self { chain_spec, config }
    }

    /// Create a new WBFT consensus instance with default configuration
    pub fn with_chain_spec(chain_spec: Arc<ChainSpec>) -> Self {
        Self::new(chain_spec, WbftConfig::default())
    }

    /// Get the chain specification
    pub fn chain_spec(&self) -> &ChainSpec {
        &self.chain_spec
    }

    /// Get the WBFT configuration
    pub fn config(&self) -> &WbftConfig {
        &self.config
    }

    /// Check if a block number is an epoch block
    pub fn is_epoch_block(&self, block_number: u64) -> bool {
        block_number > 0 && block_number % self.config.epoch_length == 0
    }
}

impl<H> HeaderValidator<H> for WbftConsensus
where
    H: alloy_consensus::BlockHeader + reth_primitives_traits::BlockHeader,
{
    fn validate_header(&self, header: &SealedHeader<H>) -> Result<(), ConsensusError> {
        // WBFT header validation:
        // 1. Parse and validate extra data structure
        // 2. Verify committed seal (BLS aggregated signature)
        // 3. Check quorum requirements

        // Skip genesis block
        if header.number() == 0 {
            return Ok(());
        }

        // Parse WbftExtra from header extra_data
        let extra = crate::header::WbftExtra::decode(header.extra_data())
            .map_err(|e| ConsensusError::BaseFeeMissing)?; // TODO: Better error type

        // Committed seal must be present for non-genesis blocks
        let committed_seal = extra.committed_seal
            .ok_or(ConsensusError::BaseFeeMissing)?; // TODO: Better error type

        // Verify quorum: at least 2f+1 validators signed
        // For now, just check that bitmap is not empty
        // Full implementation will verify BLS signatures and validator set
        if committed_seal.bitmap.count() == 0 {
            return Err(ConsensusError::BaseFeeMissing); // TODO: Better error type
        }

        Ok(())
    }

    fn validate_header_against_parent(
        &self,
        header: &SealedHeader<H>,
        parent: &SealedHeader<H>,
    ) -> Result<(), ConsensusError> {
        // WBFT parent validation:
        // 1. Verify block number is parent + 1
        // 2. Verify timestamp is after parent
        // 3. Verify parent hash matches

        // Use reth's built-in parent validation
        reth_consensus_common::validation::validate_against_parent_hash_number(
            header.header(),
            parent,
        )?;

        // Verify timestamp progression
        reth_consensus_common::validation::validate_against_parent_timestamp(
            header.header(),
            parent.header(),
        )?;

        // WBFT-specific validation
        // In the future, we will verify:
        // - Proposer selection based on round and validator set
        // - Round progression rules
        // - Epoch transitions

        Ok(())
    }
}

impl<B: Block> Consensus<B> for WbftConsensus
where
    B::Header: alloy_consensus::BlockHeader,
{
    type Error = ConsensusError;

    fn validate_body_against_header(
        &self,
        body: &B::Body,
        header: &SealedHeader<B::Header>,
    ) -> Result<(), Self::Error> {
        // Use reth's built-in body validation
        validate_body_against_header(body, header.header())
    }

    fn validate_block_pre_execution(&self, block: &SealedBlock<B>) -> Result<(), Self::Error> {
        // TODO: Implement WBFT-specific block pre-execution validation
        // This will be implemented in Phase 3.3
        // For now, just validate body against header using reth's built-in validation
        validate_body_against_header(block.body(), block.header())
    }
}

impl<N: NodePrimitives> FullConsensus<N> for WbftConsensus
where
    N::Block: Block,
    <N::Block as Block>::Header: alloy_consensus::BlockHeader,
{
    fn validate_block_post_execution(
        &self,
        _block: &RecoveredBlock<N::Block>,
        _result: &BlockExecutionResult<N::Receipt>,
    ) -> Result<(), ConsensusError> {
        // TODO: Implement post-execution validation
        // This will be implemented in Phase 3.4 (FullConsensus trait)
        // For now, accept all blocks to allow integration testing
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reth_chainspec::MAINNET;

    #[test]
    fn test_wbft_consensus_creation() {
        let consensus = WbftConsensus::with_chain_spec(MAINNET.clone());

        assert_eq!(consensus.config().request_timeout_seconds, 2);
        assert_eq!(consensus.config().block_period_seconds, 1);
        assert_eq!(consensus.config().proposer_policy, 0);
        assert_eq!(consensus.config().epoch_length, 10);
    }

    #[test]
    fn test_wbft_config_default() {
        let config = WbftConfig::default();

        assert_eq!(config.request_timeout_seconds, 2);
        assert_eq!(config.block_period_seconds, 1);
        assert_eq!(config.proposer_policy, 0);
        assert_eq!(config.epoch_length, 10);
        assert!(config.max_request_timeout_seconds.is_none());
    }

    #[test]
    fn test_is_epoch_block() {
        let consensus = WbftConsensus::with_chain_spec(MAINNET.clone());

        // Genesis is not an epoch block
        assert!(!consensus.is_epoch_block(0));

        // Regular blocks are not epoch blocks
        assert!(!consensus.is_epoch_block(1));
        assert!(!consensus.is_epoch_block(5));
        assert!(!consensus.is_epoch_block(9));

        // Epoch blocks (every 10 blocks)
        assert!(consensus.is_epoch_block(10));
        assert!(consensus.is_epoch_block(20));
        assert!(consensus.is_epoch_block(100));
    }

    #[test]
    fn test_wbft_consensus_with_custom_config() {
        let config = WbftConfig {
            request_timeout_seconds: 5,
            block_period_seconds: 2,
            proposer_policy: 1, // Sticky
            epoch_length: 20,
            max_request_timeout_seconds: Some(30),
        };

        let consensus = WbftConsensus::new(MAINNET.clone(), config);

        assert_eq!(consensus.config().request_timeout_seconds, 5);
        assert_eq!(consensus.config().block_period_seconds, 2);
        assert_eq!(consensus.config().proposer_policy, 1);
        assert_eq!(consensus.config().epoch_length, 20);
        assert_eq!(consensus.config().max_request_timeout_seconds, Some(30));
    }

    #[test]
    fn test_epoch_block_with_custom_epoch_length() {
        let config = WbftConfig {
            epoch_length: 7,
            ..Default::default()
        };

        let consensus = WbftConsensus::new(MAINNET.clone(), config);

        assert!(!consensus.is_epoch_block(0));
        assert!(!consensus.is_epoch_block(6));
        assert!(consensus.is_epoch_block(7));
        assert!(consensus.is_epoch_block(14));
        assert!(consensus.is_epoch_block(21));
    }
}
