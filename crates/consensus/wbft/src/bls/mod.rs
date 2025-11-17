//! BLS signature implementation for WBFT consensus
//!
//! This module provides BLS12-381 signature functionality including:
//! - Key generation and management
//! - Individual signature creation and verification
//! - Signature aggregation for efficient multi-signature schemes
//!
//! BLS signatures enable efficient signature aggregation, crucial for
//! WBFT consensus where multiple validators sign the same message.

use alloy_primitives::hex;
use blst::{
    min_pk::{PublicKey as BlstPublicKey, SecretKey as BlstSecretKey, Signature as BlstSignature},
    BLST_ERROR,
};
use thiserror::Error;

pub mod aggregation;
pub mod sealer_set;

pub use aggregation::{aggregate_signatures, verify_aggregated, WbftAggregatedSeal};
pub use sealer_set::SealerSet;

/// BLS signature errors
#[derive(Debug, Error)]
pub enum BlsError {
    /// Invalid secret key bytes
    #[error("invalid secret key")]
    InvalidSecretKey,

    /// Invalid public key bytes
    #[error("invalid public key")]
    InvalidPublicKey,

    /// Invalid signature bytes
    #[error("invalid signature")]
    InvalidSignature,

    /// Signature verification failed
    #[error("signature verification failed")]
    VerificationFailed,

    /// Aggregation failed
    #[error("aggregation failed: {0}")]
    AggregationFailed(String),

    /// Internal BLST error
    #[error("BLST error: {0:?}")]
    BlstError(BLST_ERROR),
}

impl From<BLST_ERROR> for BlsError {
    fn from(err: BLST_ERROR) -> Self {
        Self::BlstError(err)
    }
}

/// BLS secret key wrapper
#[derive(Clone)]
pub struct SecretKey(BlstSecretKey);

impl SecretKey {
    /// Generate a random secret key
    ///
    /// Uses cryptographically secure randomness from the OS.
    pub fn random() -> Self {
        let mut ikm = [0u8; 32];
        getrandom::getrandom(&mut ikm).expect("failed to get random bytes");
        let sk = BlstSecretKey::key_gen(&ikm, &[]).expect("key generation failed");
        Self(sk)
    }

    /// Create secret key from bytes
    ///
    /// # Errors
    ///
    /// Returns error if bytes are invalid for BLS secret key.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, BlsError> {
        if bytes.len() != 32 {
            return Err(BlsError::InvalidSecretKey);
        }

        let mut array = [0u8; 32];
        array.copy_from_slice(bytes);

        BlstSecretKey::from_bytes(&array).map(Self).map_err(|_| BlsError::InvalidSecretKey)
    }

    /// Convert secret key to bytes
    pub fn to_bytes(&self) -> [u8; 32] {
        self.0.to_bytes()
    }

    /// Derive public key from secret key
    pub fn public_key(&self) -> PublicKey {
        PublicKey(self.0.sk_to_pk())
    }

    /// Sign a message
    ///
    /// Uses BLS signature scheme with domain separation tag (DST).
    pub fn sign(&self, message: &[u8]) -> Signature {
        const DST: &[u8] = b"WBFT_BLS_SIG";
        Signature(self.0.sign(message, DST, &[]))
    }
}

impl std::fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecretKey").finish_non_exhaustive()
    }
}

/// BLS public key wrapper
#[derive(Clone, PartialEq, Eq)]
pub struct PublicKey(BlstPublicKey);

impl PublicKey {
    /// Create public key from compressed bytes
    ///
    /// # Errors
    ///
    /// Returns error if bytes are invalid for BLS public key.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, BlsError> {
        if bytes.len() != 48 {
            return Err(BlsError::InvalidPublicKey);
        }

        BlstPublicKey::from_bytes(bytes).map(Self).map_err(|_| BlsError::InvalidPublicKey)
    }

    /// Convert public key to compressed bytes
    pub fn to_bytes(&self) -> [u8; 48] {
        self.0.to_bytes()
    }

    /// Verify a signature on a message
    ///
    /// Returns `true` if signature is valid for this public key and message.
    pub fn verify(&self, message: &[u8], signature: &Signature) -> bool {
        const DST: &[u8] = b"WBFT_BLS_SIG";
        let result = signature.0.verify(true, message, DST, &[], &self.0, true);
        result == BLST_ERROR::BLST_SUCCESS
    }
}

impl std::fmt::Debug for PublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PublicKey({})", hex::encode(self.to_bytes()))
    }
}

/// BLS signature wrapper
#[derive(Clone, PartialEq, Eq)]
pub struct Signature(BlstSignature);

impl Signature {
    /// Create signature from bytes
    ///
    /// # Errors
    ///
    /// Returns error if bytes are invalid for BLS signature.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, BlsError> {
        if bytes.len() != 96 {
            return Err(BlsError::InvalidSignature);
        }

        BlstSignature::from_bytes(bytes).map(Self).map_err(|_| BlsError::InvalidSignature)
    }

    /// Convert signature to bytes
    pub fn to_bytes(&self) -> [u8; 96] {
        self.0.to_bytes()
    }
}

impl std::fmt::Debug for Signature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Signature({})", hex::encode(self.to_bytes()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_generation() {
        let sk = SecretKey::random();
        let pk = sk.public_key();

        let sk_bytes = sk.to_bytes();
        let pk_bytes = pk.to_bytes();

        assert_eq!(sk_bytes.len(), 32);
        assert_eq!(pk_bytes.len(), 48);

        let sk2 = SecretKey::from_bytes(&sk_bytes).unwrap();
        let pk2 = sk2.public_key();

        assert_eq!(pk.to_bytes(), pk2.to_bytes());
    }

    #[test]
    fn test_sign_verify() {
        let sk = SecretKey::random();
        let pk = sk.public_key();
        let message = b"test message";

        let signature = sk.sign(message);
        assert!(pk.verify(message, &signature));

        let wrong_message = b"wrong message";
        assert!(!pk.verify(wrong_message, &signature));
    }

    #[test]
    fn test_invalid_signature() {
        let sk = SecretKey::random();
        let pk = sk.public_key();
        let message = b"test message";

        let signature = sk.sign(message);

        let mut invalid_sig_bytes = signature.to_bytes();
        invalid_sig_bytes[0] ^= 0xFF;

        if let Ok(invalid_sig) = Signature::from_bytes(&invalid_sig_bytes) {
            assert!(!pk.verify(message, &invalid_sig));
        }
    }

    #[test]
    fn test_signature_serialization() {
        let sk = SecretKey::random();
        let message = b"test message";

        let signature = sk.sign(message);
        let sig_bytes = signature.to_bytes();

        assert_eq!(sig_bytes.len(), 96);

        let signature2 = Signature::from_bytes(&sig_bytes).unwrap();
        assert_eq!(signature.to_bytes(), signature2.to_bytes());
    }

    #[test]
    fn test_public_key_serialization() {
        let sk = SecretKey::random();
        let pk = sk.public_key();

        let pk_bytes = pk.to_bytes();
        assert_eq!(pk_bytes.len(), 48);

        let pk2 = PublicKey::from_bytes(&pk_bytes).unwrap();
        assert_eq!(pk.to_bytes(), pk2.to_bytes());
    }

    #[test]
    fn test_invalid_key_sizes() {
        assert!(SecretKey::from_bytes(&[0u8; 31]).is_err());
        assert!(SecretKey::from_bytes(&[0u8; 33]).is_err());

        assert!(PublicKey::from_bytes(&[0u8; 47]).is_err());
        assert!(PublicKey::from_bytes(&[0u8; 49]).is_err());

        assert!(Signature::from_bytes(&[0u8; 95]).is_err());
        assert!(Signature::from_bytes(&[0u8; 97]).is_err());
    }
}
