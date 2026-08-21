//! Common Agent Protocol v1 message envelope fields and their wire-encoding
//! primitives (`docs/specifications/m0-agent-protocol-contract.md` "Message
//! envelope", "Wire encoding").
//!
//! Every Agent Protocol message is one flat JSON object carrying these
//! envelope fields alongside its message-specific fields at the same level —
//! there is no nested "envelope" or "payload" object on the wire. The
//! [`Envelope`] struct exists only as a Rust-side grouping, flattened into
//! each concrete message struct via `#[serde(flatten)]`.

use std::fmt;

use chrono::{DateTime, SecondsFormat, Utc};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

/// Wire textual value of the currently supported Agent Protocol version.
pub const PROTOCOL_VERSION_V1: &str = "1";

/// `protocol_version` as carried on the wire: a string, not a closed enum.
///
/// The Specification requires a future Server handshake to be able to
/// *receive* an incompatible value (e.g. `"2"`) and explicitly reject it with
/// `AuthError`, rather than have deserialization itself fail. This type
/// therefore preserves whatever textual value was received; comparing
/// against [`ProtocolVersion::v1`] (or [`ProtocolVersion::is_v1`]) is a
/// caller-level decision, not a parse-time one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolVersion(String);

impl ProtocolVersion {
    /// The currently supported Agent Protocol v1 value.
    pub fn v1() -> Self {
        Self(PROTOCOL_VERSION_V1.to_string())
    }

    /// Wraps an arbitrary received textual value without validating it.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether this value equals the currently supported v1 textual value.
    pub fn is_v1(&self) -> bool {
        self.0 == PROTOCOL_VERSION_V1
    }
}

impl Default for ProtocolVersion {
    fn default() -> Self {
        Self::v1()
    }
}

impl Serialize for ProtocolVersion {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ProtocolVersion {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(Self)
    }
}

/// A syntactically valid UUID that is not version 4, rejected by
/// [`ProtocolId`]'s `Deserialize` implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("protocol identifier is not a version-4 UUID")]
pub struct NotUuidV4;

/// A `message_id` / `session_id` / `correlation_id` (and, in future
/// checkpoints, `action_id`) wire identifier: UUID version 4, encoded as a
/// lowercase hyphenated string
/// (`docs/specifications/m0-agent-protocol-contract.md` "Wire encoding").
///
/// Generation always produces a v4 UUID; deserialization rejects a
/// syntactically valid UUID of any other version, rather than silently
/// accepting it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProtocolId(Uuid);

impl ProtocolId {
    pub fn generate() -> Self {
        Self(Uuid::new_v4())
    }

    /// Wraps an already-known UUID, rejecting anything that is not version 4.
    pub fn from_uuid(uuid: Uuid) -> Result<Self, NotUuidV4> {
        if uuid.get_version_num() != 4 {
            return Err(NotUuidV4);
        }
        Ok(Self(uuid))
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl fmt::Display for ProtocolId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Serialize for ProtocolId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for ProtocolId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        let uuid = Uuid::parse_str(&raw).map_err(D::Error::custom)?;
        ProtocolId::from_uuid(uuid).map_err(D::Error::custom)
    }
}

/// Wire timestamp: RFC 3339 / ISO 8601, always UTC, always a JSON string —
/// never an epoch integer
/// (`docs/specifications/m0-agent-protocol-contract.md` "Wire encoding").
///
/// Serializes with a `Z` UTC designator (e.g. `2026-08-14T21:00:00Z`), not
/// `+00:00`, matching the Specification's own example. Deserialization
/// accepts any valid RFC 3339 string and normalizes it to UTC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageTimestamp(DateTime<Utc>);

impl MessageTimestamp {
    pub fn now() -> Self {
        Self(Utc::now())
    }

    pub fn from_datetime(value: DateTime<Utc>) -> Self {
        Self(value)
    }

    pub fn as_datetime(&self) -> DateTime<Utc> {
        self.0
    }
}

impl From<DateTime<Utc>> for MessageTimestamp {
    fn from(value: DateTime<Utc>) -> Self {
        Self(value)
    }
}

impl Serialize for MessageTimestamp {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_rfc3339_opts(SecondsFormat::Millis, true))
    }
}

impl<'de> Deserialize<'de> for MessageTimestamp {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        let parsed = DateTime::parse_from_rfc3339(&raw).map_err(D::Error::custom)?;
        Ok(Self(parsed.with_timezone(&Utc)))
    }
}

/// Fields common to every Agent Protocol v1 message, flattened into each
/// concrete message struct rather than nested under a `"envelope"` key on
/// the wire (`m0-agent-protocol-contract.md` "Message envelope").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    pub message_id: ProtocolId,
    pub protocol_version: ProtocolVersion,
    pub timestamp: MessageTimestamp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<ProtocolId>,
}

impl Envelope {
    /// A fresh v1 envelope stamped with a new `message_id` and the current
    /// time, no `correlation_id`.
    pub fn new() -> Self {
        Self {
            message_id: ProtocolId::generate(),
            protocol_version: ProtocolVersion::v1(),
            timestamp: MessageTimestamp::now(),
            correlation_id: None,
        }
    }

    pub fn with_correlation_id(mut self, correlation_id: ProtocolId) -> Self {
        self.correlation_id = Some(correlation_id);
        self
    }
}

impl Default for Envelope {
    fn default() -> Self {
        Self::new()
    }
}
