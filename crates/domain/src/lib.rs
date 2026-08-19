//! Bamep Domain: Endpoint identity lifecycle and runtime-credential chain.
//!
//! Pure business logic only — no I/O, no SQLite, no WebSocket, no clock or
//! RNG access hidden inside a transition (`AGENTS.md` "Architecture and
//! dependencies"; `docs/specifications/m0-stack-and-boundaries-baseline.md`
//! "Component responsibilities and boundaries"). Every state transition
//! takes `now` and any needed secrets as explicit parameters and returns new
//! state plus the domain events/audit record it requires, leaving
//! persistence entirely to the `server` crate's Adapters.

pub mod credential;
pub mod endpoint;
pub mod events;
pub mod identity;
pub mod transitions;

pub use credential::{
    AuthOutcome, CredentialChain, CredentialDimension, CredentialSecret, DEFAULT_CREDENTIAL_TTL,
};
pub use endpoint::{EndpointAggregate, EndpointId};
pub use events::{Actor, AuditRecord, DomainEvent, TransitionOutcome};
pub use identity::{IdentityState, InvalidIdentityTransition};
pub use transitions::RedeemOutcome;
