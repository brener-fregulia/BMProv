//! `CurrentBoot`: the Endpoint's authoritative durable current-boot
//! projection
//! (`docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md`
//! "Authoritative current boot and durable Server state"; ADR-0014
//! "Amendment (owner-approved, WP1 trusted-bootstrap checkpoint)").
//!
//! This is the fourth independent aggregate dimension the Specification
//! already defines, alongside identity ([`crate::identity::IdentityState`])
//! and credential/session lifecycle ([`crate::credential::CredentialChain`]):
//! it is not Agent presence, not credential validity, not identity
//! enrollment, and not hardware confidence. Absence of a `CurrentBoot`
//! (`EndpointAggregate::current_boot == None`) means no trusted current boot
//! is known and fails closed — legacy/unknown Endpoint rows never get an
//! inferred one.
//!
//! This module is pure, like every other Domain module: it never touches the
//! clock, randomness, or persistence directly.

use bamep_trusted_bootstrap::BootNonce;

use crate::presented_credential::CredentialLookupId;

/// Whether the Server has independently verified authenticated
/// `BootstrapEvidence` for this exact current boot
/// (`m0-trusted-bootstrap-and-server-fingerprint-contract.md` "(D)
/// Server-side bootstrap evidence"). This checkpoint defines the value only;
/// the production transition that promotes `NotEstablished` to `Established`
/// by accepting verified evidence is a later checkpoint's responsibility —
/// no Port, Gateway API, or Domain bypass constructor for that is introduced
/// here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TrustedBootstrapState {
    NotEstablished,
    Established,
}

/// The Endpoint's authoritative current-boot projection: which durable
/// `BootContext` (by its non-secret lookup identifier) and exact `BootNonce`
/// are current, and whether trusted bootstrap has been established for that
/// exact boot. Deliberately carries both `boot_context_id` and `boot_nonce`
/// together — never `BootNonce` alone without the `BootContext` identity it
/// belongs to.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CurrentBoot {
    boot_context_id: CredentialLookupId,
    boot_nonce: BootNonce,
    trusted_bootstrap: TrustedBootstrapState,
}

impl CurrentBoot {
    pub fn new(
        boot_context_id: CredentialLookupId,
        boot_nonce: BootNonce,
        trusted_bootstrap: TrustedBootstrapState,
    ) -> Self {
        Self {
            boot_context_id,
            boot_nonce,
            trusted_bootstrap,
        }
    }

    pub fn boot_context_id(&self) -> &CredentialLookupId {
        &self.boot_context_id
    }

    pub fn boot_nonce(&self) -> BootNonce {
        self.boot_nonce
    }

    pub fn trusted_bootstrap(&self) -> TrustedBootstrapState {
        self.trusted_bootstrap
    }
}
