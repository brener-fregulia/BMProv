//! Small explicit UTF-8 JSON codec for [`AgentProtocolMessage`]
//! (`m0-agent-protocol-contract.md` "Wire encoding"). No socket or
//! WebSocket-frame types, and no protocol state machine — encoding/decoding
//! only.

use crate::messages::AgentProtocolMessage;

/// `Display`/`Debug` intentionally carry only `serde_json`'s own error
/// description (position, expected-type/variant information) — never the
/// raw input JSON, so a malformed message containing a credential value
/// cannot leak through this error's textual representation.
#[derive(Debug, thiserror::Error)]
#[error("failed to encode Agent Protocol message: {0}")]
pub struct EncodeError(#[source] serde_json::Error);

#[derive(Debug, thiserror::Error)]
#[error("failed to decode Agent Protocol message: {0}")]
pub struct DecodeError(#[source] serde_json::Error);

/// Serializes a message to its UTF-8 JSON wire representation.
pub fn encode(message: &AgentProtocolMessage) -> Result<String, EncodeError> {
    serde_json::to_string(message).map_err(EncodeError)
}

/// Parses a UTF-8 JSON wire value into a known message.
///
/// An unrecognized top-level `type` (or any other structural violation)
/// fails explicitly via [`DecodeError`] — it is never silently interpreted
/// as some fallback variant. Unknown fields inside an otherwise valid, known
/// message are ignored by the underlying message-body deserializers, per
/// the Specification's forward-compatibility requirement.
pub fn decode(json: &str) -> Result<AgentProtocolMessage, DecodeError> {
    serde_json::from_str(json).map_err(DecodeError)
}
