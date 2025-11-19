//! WBFT network protocol definition
//!
//! This module defines the WBFT consensus protocol for P2P communication,
//! including message types and protocol identifiers.

use alloy_primitives::bytes::Bytes;
use alloy_rlp::{Decodable, Encodable, RlpDecodable, RlpEncodable};

/// WBFT protocol identifier
pub const WBFT_PROTOCOL_ID: &str = "wbft";

/// WBFT protocol version
pub const WBFT_VERSION: u8 = 1;

/// Message codes for WBFT consensus protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WbftMessageCode {
    /// PRE-PREPARE message from proposer
    PrePrepare = 0x12,
    /// PREPARE message from validators
    Prepare = 0x13,
    /// COMMIT message from validators
    Commit = 0x14,
    /// ROUND-CHANGE message for timeout recovery
    RoundChange = 0x15,
}

impl WbftMessageCode {
    /// Convert from u8 code to message type
    ///
    /// # Returns
    ///
    /// Some(WbftMessageCode) if valid, None if unknown code
    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            0x12 => Some(Self::PrePrepare),
            0x13 => Some(Self::Prepare),
            0x14 => Some(Self::Commit),
            0x15 => Some(Self::RoundChange),
            _ => None,
        }
    }

    /// Get the u8 code for this message type
    pub fn code(&self) -> u8 {
        *self as u8
    }

    /// Get human-readable name for this message type
    pub fn name(&self) -> &'static str {
        match self {
            Self::PrePrepare => "PRE-PREPARE",
            Self::Prepare => "PREPARE",
            Self::Commit => "COMMIT",
            Self::RoundChange => "ROUND-CHANGE",
        }
    }
}

impl std::fmt::Display for WbftMessageCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// WBFT protocol message wrapper
///
/// Wraps consensus messages for network transmission with
/// message type identification.
#[derive(Debug, Clone, PartialEq, Eq, RlpEncodable, RlpDecodable)]
pub struct WbftProtocolMessage {
    /// Message code identifying the type
    pub code: u8,
    /// RLP-encoded message payload
    pub payload: Bytes,
}

impl WbftProtocolMessage {
    /// Create a new protocol message
    ///
    /// # Arguments
    ///
    /// * `code` - Message type code
    /// * `payload` - RLP-encoded message data
    pub fn new(code: WbftMessageCode, payload: Bytes) -> Self {
        Self {
            code: code.code(),
            payload,
        }
    }

    /// Get the message code enum
    ///
    /// # Returns
    ///
    /// Some(WbftMessageCode) if valid, None if unknown
    pub fn message_code(&self) -> Option<WbftMessageCode> {
        WbftMessageCode::from_code(self.code)
    }

    /// Encode the message for network transmission
    pub fn encode_message(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        self.encode(&mut buf);
        buf
    }

    /// Decode a message from network bytes
    ///
    /// # Errors
    ///
    /// Returns error if RLP decoding fails
    pub fn decode_message(data: &[u8]) -> Result<Self, alloy_rlp::Error> {
        Self::decode(&mut &data[..])
    }
}

/// Protocol capability information
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WbftCapability {
    /// Protocol name
    pub name: String,
    /// Protocol version
    pub version: u8,
}

impl WbftCapability {
    /// Create default WBFT capability
    pub fn new() -> Self {
        Self {
            name: WBFT_PROTOCOL_ID.to_string(),
            version: WBFT_VERSION,
        }
    }
}

impl Default for WbftCapability {
    fn default() -> Self {
        Self::new()
    }
}

/// List of supported WBFT message codes
pub fn supported_message_codes() -> Vec<u8> {
    vec![
        WbftMessageCode::PrePrepare.code(),
        WbftMessageCode::Prepare.code(),
        WbftMessageCode::Commit.code(),
        WbftMessageCode::RoundChange.code(),
    ]
}

/// Check if a message code is a valid WBFT message
pub fn is_valid_message_code(code: u8) -> bool {
    WbftMessageCode::from_code(code).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_code_values() {
        assert_eq!(WbftMessageCode::PrePrepare.code(), 0x12);
        assert_eq!(WbftMessageCode::Prepare.code(), 0x13);
        assert_eq!(WbftMessageCode::Commit.code(), 0x14);
        assert_eq!(WbftMessageCode::RoundChange.code(), 0x15);
    }

    #[test]
    fn test_message_code_from_code() {
        assert_eq!(
            WbftMessageCode::from_code(0x12),
            Some(WbftMessageCode::PrePrepare)
        );
        assert_eq!(
            WbftMessageCode::from_code(0x13),
            Some(WbftMessageCode::Prepare)
        );
        assert_eq!(
            WbftMessageCode::from_code(0x14),
            Some(WbftMessageCode::Commit)
        );
        assert_eq!(
            WbftMessageCode::from_code(0x15),
            Some(WbftMessageCode::RoundChange)
        );
        assert_eq!(WbftMessageCode::from_code(0x00), None);
        assert_eq!(WbftMessageCode::from_code(0xFF), None);
    }

    #[test]
    fn test_message_code_names() {
        assert_eq!(WbftMessageCode::PrePrepare.name(), "PRE-PREPARE");
        assert_eq!(WbftMessageCode::Prepare.name(), "PREPARE");
        assert_eq!(WbftMessageCode::Commit.name(), "COMMIT");
        assert_eq!(WbftMessageCode::RoundChange.name(), "ROUND-CHANGE");
    }

    #[test]
    fn test_message_code_display() {
        assert_eq!(format!("{}", WbftMessageCode::PrePrepare), "PRE-PREPARE");
        assert_eq!(format!("{}", WbftMessageCode::Commit), "COMMIT");
    }

    #[test]
    fn test_protocol_message_new() {
        let payload = Bytes::from(vec![1, 2, 3]);
        let msg = WbftProtocolMessage::new(WbftMessageCode::Prepare, payload.clone());

        assert_eq!(msg.code, 0x13);
        assert_eq!(msg.payload, payload);
        assert_eq!(msg.message_code(), Some(WbftMessageCode::Prepare));
    }

    #[test]
    fn test_protocol_message_encode_decode() {
        let payload = Bytes::from(vec![0x42, 0x43, 0x44]);
        let msg = WbftProtocolMessage::new(WbftMessageCode::Commit, payload);

        let encoded = msg.encode_message();
        let decoded = WbftProtocolMessage::decode_message(&encoded).unwrap();

        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_protocol_message_unknown_code() {
        let msg = WbftProtocolMessage {
            code: 0xFF,
            payload: Bytes::new(),
        };

        assert_eq!(msg.message_code(), None);
    }

    #[test]
    fn test_wbft_capability() {
        let cap = WbftCapability::new();

        assert_eq!(cap.name, WBFT_PROTOCOL_ID);
        assert_eq!(cap.version, WBFT_VERSION);
    }

    #[test]
    fn test_wbft_capability_default() {
        let cap = WbftCapability::default();

        assert_eq!(cap.name, "wbft");
        assert_eq!(cap.version, 1);
    }

    #[test]
    fn test_supported_message_codes() {
        let codes = supported_message_codes();

        assert_eq!(codes.len(), 4);
        assert!(codes.contains(&0x12));
        assert!(codes.contains(&0x13));
        assert!(codes.contains(&0x14));
        assert!(codes.contains(&0x15));
    }

    #[test]
    fn test_is_valid_message_code() {
        assert!(is_valid_message_code(0x12));
        assert!(is_valid_message_code(0x13));
        assert!(is_valid_message_code(0x14));
        assert!(is_valid_message_code(0x15));
        assert!(!is_valid_message_code(0x00));
        assert!(!is_valid_message_code(0x11));
        assert!(!is_valid_message_code(0x16));
    }

    #[test]
    fn test_protocol_constants() {
        assert_eq!(WBFT_PROTOCOL_ID, "wbft");
        assert_eq!(WBFT_VERSION, 1);
    }

    #[test]
    fn test_message_roundtrip_all_types() {
        let message_codes = vec![
            WbftMessageCode::PrePrepare,
            WbftMessageCode::Prepare,
            WbftMessageCode::Commit,
            WbftMessageCode::RoundChange,
        ];

        for code in message_codes {
            let payload = Bytes::from(vec![code.code(), 0xAB, 0xCD]);
            let msg = WbftProtocolMessage::new(code, payload);

            let encoded = msg.encode_message();
            let decoded = WbftProtocolMessage::decode_message(&encoded).unwrap();

            assert_eq!(msg, decoded);
            assert_eq!(decoded.message_code(), Some(code));
        }
    }

    #[test]
    fn test_empty_payload() {
        let msg = WbftProtocolMessage::new(WbftMessageCode::Prepare, Bytes::new());

        let encoded = msg.encode_message();
        let decoded = WbftProtocolMessage::decode_message(&encoded).unwrap();

        assert_eq!(decoded.payload, Bytes::new());
    }

    #[test]
    fn test_large_payload() {
        let large_payload = Bytes::from(vec![0x42; 10000]);
        let msg = WbftProtocolMessage::new(WbftMessageCode::PrePrepare, large_payload.clone());

        let encoded = msg.encode_message();
        let decoded = WbftProtocolMessage::decode_message(&encoded).unwrap();

        assert_eq!(decoded.payload, large_payload);
    }
}
