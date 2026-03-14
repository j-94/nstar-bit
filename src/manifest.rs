// Manifest-driven dispatch.
// Deterministic routing layer: if a signal matches a manifest edge, the LM is bypassed.
// If no edge matches, the OMNI (LM) path fires.
//
// Current state: minimal implementation — find_and_load always returns None,
// meaning all turns go through OMNI. Full manifest.yaml dispatch is a future milestone.

use crate::autogenesis::State;
use std::path::Path;

/// A loaded manifest with its dispatch rules.
pub struct ManifestDispatch;

/// Result of a deterministic dispatch — operations to apply without the LM.
pub struct DispatchResult;

impl ManifestDispatch {
    /// Try to match a signal against manifest edges.
    /// Returns Some if a deterministic edge matched, None if OMNI should fire.
    pub fn try_dispatch(&self, _signal: &str) -> Option<DispatchResult> {
        None
    }

    /// Apply deterministic operations from a dispatch result to the state.
    pub fn apply_ops(&self, _state: &mut State, _result: &DispatchResult) {}

    /// Seed universal primitives into the graph at startup.
    pub fn seed_primitives(&self, _state: &mut State) {}
}

/// Find and load a manifest.yaml adjacent to the state file.
/// Returns None if no manifest exists (OMNI-only mode).
pub fn find_and_load(_state_path: &Path) -> Option<ManifestDispatch> {
    None
}
