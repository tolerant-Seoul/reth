//! BLS signature aggregation for WBFT consensus
//!
//! Provides efficient multi-signature aggregation using BLS12-381.
//! Multiple validators can sign the same message, and their signatures
//! can be aggregated into a single compact signature.

use super::{BlsError, PublicKey, Signature};
use crate::bls::SealerSet;
use alloy_rlp::{RlpDecodable, RlpEncodable};
use blst::{min_pk::AggregateSignature, BLST_ERROR};

/// Aggregated BLS signature with sealer bitmap
///
/// Combines multiple validator signatures into one signature plus a bitmap
/// indicating which validators signed.
#[derive(Debug, Clone, PartialEq, Eq, RlpEncodable, RlpDecodable)]
pub struct WbftAggregatedSeal {
    /// Bitmap indicating which validators signed
    pub bitmap: SealerSet,
    /// Aggregated BLS signature (96 bytes)
    pub signature: [u8; 96],
}

impl WbftAggregatedSeal {
    /// Create a new aggregated seal
    pub fn new(bitmap: SealerSet, signature: [u8; 96]) -> Self {
        Self { bitmap, signature }
    }

    /// Verify aggregated signature against multiple public keys
    ///
    /// # Arguments
    ///
    /// * `public_keys` - All validator public keys (indexed by validator index)
    /// * `message` - The message that was signed
    ///
    /// # Returns
    ///
    /// `true` if the aggregated signature is valid for the public keys indicated
    /// by the bitmap signing the message.
    ///
    /// # Errors
    ///
    /// Returns error if signature bytes are invalid or verification fails.
    pub fn verify(&self, public_keys: &[PublicKey], message: &[u8]) -> Result<bool, BlsError> {
        let sig = Signature::from_bytes(&self.signature)?;
        let sealer_indices = self.bitmap.get_sealers();

        if sealer_indices.is_empty() {
            return Err(BlsError::AggregationFailed("no sealers in bitmap".into()));
        }

        let sealer_pubkeys: Vec<&PublicKey> =
            sealer_indices.iter().filter_map(|&idx| public_keys.get(idx as usize)).collect();

        if sealer_pubkeys.len() != sealer_indices.len() {
            return Err(BlsError::AggregationFailed("missing public keys for some sealers".into()));
        }

        Ok(verify_aggregated(&sealer_pubkeys, message, &sig)?)
    }

    /// Get count of signers
    pub fn signer_count(&self) -> usize {
        self.bitmap.count()
    }
}

/// Aggregate multiple BLS signatures into one
///
/// All signatures must be on the same message. The aggregated signature
/// can be verified against all public keys at once.
///
/// # Errors
///
/// Returns error if signature aggregation fails.
pub fn aggregate_signatures(signatures: &[Signature]) -> Result<Signature, BlsError> {
    if signatures.is_empty() {
        return Err(BlsError::AggregationFailed("no signatures to aggregate".into()));
    }

    if signatures.len() == 1 {
        return Ok(signatures[0].clone());
    }

    // Use from_signature to create initial aggregate
    let mut agg = AggregateSignature::from_signature(&signatures[0].0);

    // Add remaining signatures
    for sig in &signatures[1..] {
        if let Err(err) = agg.add_signature(&sig.0, true) {
            return Err(BlsError::from(err));
        }
    }

    let agg_sig = agg.to_signature();
    Ok(Signature(agg_sig))
}

/// Verify an aggregated signature against multiple public keys
///
/// Fast aggregate verification: all public keys signed the same message.
///
/// # Arguments
///
/// * `public_keys` - Public keys of all signers
/// * `message` - The message that was signed
/// * `signature` - The aggregated signature
///
/// # Returns
///
/// `true` if the signature is valid for all public keys on the message.
///
/// # Errors
///
/// Returns error if verification check fails (not the same as invalid signature).
pub fn verify_aggregated(
    public_keys: &[&PublicKey],
    message: &[u8],
    signature: &Signature,
) -> Result<bool, BlsError> {
    if public_keys.is_empty() {
        return Err(BlsError::AggregationFailed("no public keys provided".into()));
    }

    const DST: &[u8] = b"WBFT_BLS_SIG";

    let pk_refs: Vec<&blst::min_pk::PublicKey> = public_keys.iter().map(|pk| &pk.0).collect();

    let result = signature.0.fast_aggregate_verify(true, message, DST, &pk_refs);

    Ok(result == BLST_ERROR::BLST_SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bls::SecretKey;

    #[test]
    fn test_aggregate_two_signatures() {
        let sk1 = SecretKey::random();
        let sk2 = SecretKey::random();

        let message = b"test message";

        let sig1 = sk1.sign(message);
        let sig2 = sk2.sign(message);

        let agg_sig = aggregate_signatures(&[sig1, sig2]).unwrap();

        let pk1 = sk1.public_key();
        let pk2 = sk2.public_key();

        assert!(verify_aggregated(&[&pk1, &pk2], message, &agg_sig).unwrap());
    }

    #[test]
    fn test_aggregate_three_signatures() {
        let sk1 = SecretKey::random();
        let sk2 = SecretKey::random();
        let sk3 = SecretKey::random();

        let message = b"consensus message";

        let sig1 = sk1.sign(message);
        let sig2 = sk2.sign(message);
        let sig3 = sk3.sign(message);

        let agg_sig = aggregate_signatures(&[sig1, sig2, sig3]).unwrap();

        let pk1 = sk1.public_key();
        let pk2 = sk2.public_key();
        let pk3 = sk3.public_key();

        assert!(verify_aggregated(&[&pk1, &pk2, &pk3], message, &agg_sig).unwrap());
    }

    #[test]
    fn test_aggregate_seven_signatures() {
        // Create 7 validators (typical BFT setup: 3f+1 = 7 for f=2)
        let validators: Vec<_> = (0..7).map(|_| SecretKey::random()).collect();
        let public_keys: Vec<_> = validators.iter().map(|sk| sk.public_key()).collect();

        let message = b"block hash for 7 validators";

        // All 7 validators sign
        let signatures: Vec<_> = validators.iter().map(|sk| sk.sign(message)).collect();

        let agg_sig = aggregate_signatures(&signatures).unwrap();

        // Verify with all 7 public keys
        let pk_refs: Vec<_> = public_keys.iter().collect();
        assert!(verify_aggregated(&pk_refs, message, &agg_sig).unwrap());
    }

    #[test]
    fn test_aggregate_quorum_of_seven() {
        // 7 validators, quorum is 5 (2f+1 where f=2)
        let validators: Vec<_> = (0..7).map(|_| SecretKey::random()).collect();
        let public_keys: Vec<_> = validators.iter().map(|sk| sk.public_key()).collect();

        let message = b"quorum test";

        // Only 5 validators sign (quorum)
        let signatures: Vec<_> = validators.iter().take(5).map(|sk| sk.sign(message)).collect();

        let agg_sig = aggregate_signatures(&signatures).unwrap();

        // Verify with the 5 public keys that signed
        let pk_refs: Vec<_> = public_keys.iter().take(5).collect();
        assert!(verify_aggregated(&pk_refs, message, &agg_sig).unwrap());
    }

    #[test]
    fn test_aggregate_verification_fails_wrong_message() {
        let sk1 = SecretKey::random();
        let sk2 = SecretKey::random();

        let message = b"correct message";
        let wrong_message = b"wrong message";

        let sig1 = sk1.sign(message);
        let sig2 = sk2.sign(message);

        let agg_sig = aggregate_signatures(&[sig1, sig2]).unwrap();

        let pk1 = sk1.public_key();
        let pk2 = sk2.public_key();

        assert!(!verify_aggregated(&[&pk1, &pk2], wrong_message, &agg_sig).unwrap());
    }

    #[test]
    fn test_aggregate_verification_fails_wrong_pubkey() {
        let sk1 = SecretKey::random();
        let sk2 = SecretKey::random();
        let sk3 = SecretKey::random();

        let message = b"test message";

        let sig1 = sk1.sign(message);
        let sig2 = sk2.sign(message);

        let agg_sig = aggregate_signatures(&[sig1, sig2]).unwrap();

        let pk1 = sk1.public_key();
        let pk3 = sk3.public_key();

        assert!(!verify_aggregated(&[&pk1, &pk3], message, &agg_sig).unwrap());
    }

    #[test]
    fn test_wbft_aggregated_seal() {
        let validators: Vec<_> = (0..4).map(|_| SecretKey::random()).collect();
        let public_keys: Vec<_> = validators.iter().map(|sk| sk.public_key()).collect();

        let message = b"block hash";

        let mut signatures = Vec::new();
        let mut bitmap = SealerSet::new(4);

        for (i, sk) in validators.iter().enumerate().take(3) {
            signatures.push(sk.sign(message));
            bitmap.set_sealer(i as u32);
        }

        let agg_sig = aggregate_signatures(&signatures).unwrap();
        let seal = WbftAggregatedSeal::new(bitmap, agg_sig.to_bytes());

        assert_eq!(seal.signer_count(), 3);
        assert!(seal.verify(&public_keys, message).unwrap());
    }

    #[test]
    fn test_wbft_aggregated_seal_rlp_roundtrip() {
        use alloy_rlp::{Decodable, Encodable};

        let validators: Vec<_> = (0..3).map(|_| SecretKey::random()).collect();
        let message = b"test";

        let mut signatures = Vec::new();
        let mut bitmap = SealerSet::new(3);

        for (i, sk) in validators.iter().enumerate() {
            signatures.push(sk.sign(message));
            bitmap.set_sealer(i as u32);
        }

        let agg_sig = aggregate_signatures(&signatures).unwrap();
        let seal = WbftAggregatedSeal::new(bitmap, agg_sig.to_bytes());

        let mut encoded = Vec::new();
        seal.encode(&mut encoded);
        let decoded = WbftAggregatedSeal::decode(&mut &encoded[..]).expect("decode failed");

        assert_eq!(seal, decoded);
        assert_eq!(seal.signer_count(), decoded.signer_count());
    }

    #[test]
    fn test_aggregate_empty_signatures() {
        let result = aggregate_signatures(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_aggregate_single_signature() {
        let sk = SecretKey::random();
        let message = b"test";
        let sig = sk.sign(message);

        let agg_sig = aggregate_signatures(&[sig.clone()]).unwrap();

        assert_eq!(sig.to_bytes(), agg_sig.to_bytes());
    }
}
