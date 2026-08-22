use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::credential::CredentialChain;
use crate::current_boot::CurrentBoot;
use crate::identity::IdentityState;

/// Durable Endpoint identity: a Server-assigned identifier, independent of
/// any hardware attribute (`m0-endpoint-identity-lifecycle.md` "Identity
/// model").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EndpointId(pub Uuid);

impl EndpointId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for EndpointId {
    fn default() -> Self {
        Self::new()
    }
}

/// The aggregate this Work Package operates on: identity lifecycle,
/// credential/session lifecycle, and the authoritative current-boot/
/// trusted-bootstrap dimension (`current_boot`), kept as the independent
/// dimensions `m0-endpoint-identity-lifecycle.md` and
/// `m0-trusted-bootstrap-and-server-fingerprint-contract.md` require them to
/// be. Only the dimensions WP1 actually implements are represented here;
/// hardware-confidence state is a separate, not-yet-implemented dimension
/// and is out of WP1's scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EndpointAggregate {
    pub id: EndpointId,
    /// The inventory signal used to match a reconnecting known Endpoint
    /// (ADR-0004 "Reconnect handling"). A simulated stand-in for the real
    /// MAC/disk-fingerprint matching algorithm, which remains
    /// implementation-time and out of WP1's scope.
    pub inventory_signal: String,
    pub identity: IdentityState,
    pub credential: CredentialChain,
    /// The Endpoint's authoritative current-boot projection
    /// (`m0-trusted-bootstrap-and-server-fingerprint-contract.md`
    /// "Authoritative current boot and durable Server state"): the fourth
    /// independent aggregate dimension. `None` means legacy/unknown current
    /// boot — trusted bootstrap can never be considered established for such
    /// a row; this is a fail-closed absence, never inferred from identity,
    /// credential state, or reconnect.
    pub current_boot: Option<CurrentBoot>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
