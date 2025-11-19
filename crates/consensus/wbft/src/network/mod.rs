//! WBFT Network Layer
//!
//! This module implements the P2P networking layer for WBFT consensus,
//! including message handling, broadcasting, and peer management.

pub mod protocol;

pub use protocol::{
    is_valid_message_code, supported_message_codes, WbftCapability, WbftMessageCode,
    WbftProtocolMessage, WBFT_PROTOCOL_ID, WBFT_VERSION,
};
