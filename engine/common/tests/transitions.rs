//! Exhaustive lifecycle-transition tests (SPEC §5.2). The state machine is the
//! backbone of the workspace; every one of the 64 (from, to) pairs is checked
//! against the spec's permitted set, so no transition can silently drift.

use wse_common::WorkspaceState::{self, *};

const ALL: [WorkspaceState; 8] = [
    Created, Running, Idle, Paused, Resuming, Saved, Archived, Deleted,
];

/// The spec's permitted transitions (SPEC §5.2), written independently of the
/// implementation so the two must agree.
fn spec_allows(from: WorkspaceState, to: WorkspaceState) -> bool {
    matches!(
        (from, to),
        (Created, Running)
            | (Created, Deleted)
            | (Running, Idle)
            | (Running, Paused)
            | (Running, Saved)
            | (Running, Deleted)
            | (Idle, Running)
            | (Idle, Paused)
            | (Idle, Saved)
            | (Idle, Deleted)
            | (Paused, Resuming)
            | (Paused, Saved)
            | (Paused, Deleted)
            | (Resuming, Running)
            | (Saved, Resuming)
            | (Saved, Archived)
            | (Saved, Deleted)
            | (Archived, Saved)
            | (Archived, Deleted)
    )
}

#[test]
fn full_transition_matrix_matches_spec() {
    for &from in &ALL {
        for &to in &ALL {
            assert_eq!(
                from.can_transition(to),
                spec_allows(from, to),
                "transition {from:?} -> {to:?} disagrees with the spec"
            );
        }
    }
}

#[test]
fn deleted_is_terminal() {
    for &to in &ALL {
        assert!(
            !Deleted.can_transition(to),
            "Deleted must be terminal, but allowed -> {to:?}"
        );
    }
}

#[test]
fn resuming_only_goes_to_running() {
    for &to in &ALL {
        assert_eq!(Resuming.can_transition(to), to == Running);
    }
}

#[test]
fn no_self_transitions() {
    for &s in &ALL {
        assert!(!s.can_transition(s), "{s:?} -> {s:?} must not be allowed");
    }
}

#[test]
fn stable_states_can_be_deleted() {
    // Every settled state can be torn down directly. Resuming is the exception:
    // it is a transient state that only completes to Running (SPEC §5.2).
    for &s in &ALL {
        if s != Deleted && s != Resuming {
            assert!(s.can_transition(Deleted), "{s:?} must be deletable");
        }
    }
    assert!(!Resuming.can_transition(Deleted));
}
