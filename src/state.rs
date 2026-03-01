//! Persistent state — the accumulated predicates and collapse history.
//!
//! This is the "memory" of the nstar-bit system. It persists between turns
//! and across sessions. The state grows as new predicates are discovered
//! and collapses accumulate.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::collapse::Collapse;
use crate::predicate::Predicate;

/// The full nstar-bit state — persisted to disk as JSON.
///
/// Starts empty: zero predicates, zero collapses.
/// Grows as the system discovers its own metacognitive structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NstarState {
    /// All discovered predicates (dynamic n)
    pub predicates: Vec<Predicate>,

    /// Collapse history (bounded — keeps last N)
    pub collapses: Vec<Collapse>,

    /// Total turns processed
    pub total_turns: u64,

    /// Session ID
    pub session: String,

    /// Max collapses to retain in history
    #[serde(default = "default_max_history")]
    pub max_history: usize,
}

fn default_max_history() -> usize {
    100
}

impl NstarState {
    /// Create a fresh state with zero predicates.
    pub fn new() -> Self {
        Self {
            predicates: Vec::new(),
            collapses: Vec::new(),
            total_turns: 0,
            session: uuid::Uuid::new_v4().to_string(),
            max_history: default_max_history(),
        }
    }

    /// Load state from a JSON file, or create new if file doesn't exist.
    pub fn load_or_create(path: &Path) -> Result<Self> {
        if path.exists() {
            let data = std::fs::read_to_string(path)?;
            let state: NstarState = serde_json::from_str(&data)?;
            Ok(state)
        } else {
            Ok(Self::new())
        }
    }

    /// Save state to a JSON file.
    pub fn save(&self, path: &Path) -> Result<()> {
        let data = serde_json::to_string_pretty(self)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, data)?;
        Ok(())
    }

    /// Add a collapse and manage history bounds.
    pub fn add_collapse(&mut self, collapse: Collapse) {
        self.total_turns += 1;
        self.collapses.push(collapse);

        // Trim history if needed
        if self.collapses.len() > self.max_history {
            let drain_count = self.collapses.len() - self.max_history;
            self.collapses.drain(..drain_count);
        }
    }

    /// Add a newly discovered predicate.
    pub fn add_predicate(&mut self, mut predicate: Predicate) {
        predicate.discovered_at = self.total_turns;
        self.predicates.push(predicate);
    }

    /// Find predicates that always co-activate (merge candidates).
    ///
    /// If two predicates have activation > 0 in the same collapses
    /// more than `threshold` fraction of the time, they may be redundant.
    pub fn find_merge_candidates(&self, threshold: f32) -> Vec<(String, String)> {
        let mut candidates = Vec::new();
        let n_collapses = self.collapses.len();
        if n_collapses < 5 {
            return candidates;
        }

        for i in 0..self.predicates.len() {
            for j in (i + 1)..self.predicates.len() {
                let name_i = &self.predicates[i].name;
                let name_j = &self.predicates[j].name;

                let mut both_active = 0u64;
                let mut either_active = 0u64;

                for collapse in &self.collapses {
                    let a_i = collapse
                        .activations
                        .iter()
                        .find(|(n, _)| n == name_i)
                        .map_or(0.0, |(_, v)| *v);
                    let a_j = collapse
                        .activations
                        .iter()
                        .find(|(n, _)| n == name_j)
                        .map_or(0.0, |(_, v)| *v);

                    if a_i > 0.0 || a_j > 0.0 {
                        either_active += 1;
                    }
                    if a_i > 0.0 && a_j > 0.0 {
                        both_active += 1;
                    }
                }

                if either_active > 0 {
                    let jaccard = both_active as f32 / either_active as f32;
                    if jaccard >= threshold {
                        candidates.push((name_i.clone(), name_j.clone()));
                    }
                }
            }
        }

        candidates
    }

    /// Summary for display.
    pub fn summary(&self) -> String {
        format!(
            "N★ State: {} predicates, {} collapses, {} turns\nPredicates: {}",
            self.predicates.len(),
            self.collapses.len(),
            self.total_turns,
            self.predicates
                .iter()
                .map(|p| format!("{}({}×)", p.name, p.reinforcements))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

impl Default for NstarState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::predicate::GateType;

    #[test]
    fn new_state_is_empty() {
        let state = NstarState::new();
        assert_eq!(state.predicates.len(), 0);
        assert_eq!(state.collapses.len(), 0);
        assert_eq!(state.total_turns, 0);
    }

    #[test]
    fn add_predicate_sets_discovery_turn() {
        let mut state = NstarState::new();
        state.total_turns = 5;
        let pred = Predicate::new("Test", "test condition", GateType::None, 0.5);
        state.add_predicate(pred);
        assert_eq!(state.predicates[0].discovered_at, 5);
    }
}
