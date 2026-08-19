use serde::{Deserialize, Serialize};

/// Endpoint identity lifecycle (persistent record), per
/// `docs/specifications/m0-endpoint-identity-lifecycle.md` "Endpoint identity
/// lifecycle (persistent record)".
///
/// `(no record)` is not represented as a variant: absence of a record is the
/// absence of an `IdentityState` value (`Option<Endpoint>` at the repository
/// boundary), not a state this type can hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IdentityState {
    PendingEnrollment,
    Enrolled,
    Retired,
}

/// A transition attempted from a state that does not permit it.
///
/// `m0-endpoint-identity-lifecycle.md` "Transitions" defines the only legal
/// moves: `PendingEnrollment -> Enrolled` (explicit operator approval) and
/// `Enrolled -> Retired` (explicit operator action). Every other combination
/// is rejected, never silently coerced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("invalid identity transition: {from:?} -> {attempted:?}")]
pub struct InvalidIdentityTransition {
    pub from: IdentityState,
    pub attempted: &'static str,
}

/// `PendingEnrollment -> Enrolled`: explicit operator approval action only
/// (`docs/decisions/0004-endpoint-identity-and-enrollment-bootstrap.md`
/// "Decision: operator-approval-gated first enrollment").
pub fn approve(state: IdentityState) -> Result<IdentityState, InvalidIdentityTransition> {
    match state {
        IdentityState::PendingEnrollment => Ok(IdentityState::Enrolled),
        other => Err(InvalidIdentityTransition {
            from: other,
            attempted: "Enrolled",
        }),
    }
}

/// `Enrolled -> Retired`: explicit operator action only.
pub fn retire(state: IdentityState) -> Result<IdentityState, InvalidIdentityTransition> {
    match state {
        IdentityState::Enrolled => Ok(IdentityState::Retired),
        other => Err(InvalidIdentityTransition {
            from: other,
            attempted: "Retired",
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_enrollment_approves_to_enrolled() {
        assert_eq!(
            approve(IdentityState::PendingEnrollment),
            Ok(IdentityState::Enrolled)
        );
    }

    #[test]
    fn enrolled_cannot_be_approved_again() {
        assert!(approve(IdentityState::Enrolled).is_err());
    }

    #[test]
    fn retired_cannot_be_approved() {
        assert!(approve(IdentityState::Retired).is_err());
    }

    #[test]
    fn enrolled_retires_to_retired() {
        assert_eq!(retire(IdentityState::Enrolled), Ok(IdentityState::Retired));
    }

    #[test]
    fn pending_enrollment_cannot_be_retired_directly() {
        assert!(retire(IdentityState::PendingEnrollment).is_err());
    }

    #[test]
    fn retired_cannot_be_retired_again() {
        assert!(retire(IdentityState::Retired).is_err());
    }
}
