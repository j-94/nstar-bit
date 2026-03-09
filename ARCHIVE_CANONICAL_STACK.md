# Archive Canonical Stack

This file freezes the current `canonical` stack as a reference artifact while new work moves to the smaller `nstar-autogenesis` first loop.

The active reference surfaces are:

- `src/canonical/core.rs`
- `src/canonical/promotion.rs`
- `src/canonical/schema.rs`
- `src/canonical/types.rs`
- `src/bin/canonical.rs`
- `nstar_canonical_state_derived/war_room.json`
- `nstar_canonical_state_derived/capability_ledger.json`
- `nstar_canonical_state_derived/requirements_lock.json`

Why this is archived instead of extended further:

- it already contains a larger benchmark/governance stack than is needed for the next bootstrap step
- it is useful as a design reference for receipts, control state, and derived views
- it should not be the iteration surface for the smallest graph-memory loop

What to keep from it conceptually:

- receipted state transitions
- live control state separated from derived views
- benchmark/promotion as a later-stage authority layer

What not to keep in the bootstrap loop:

- the full canonical promotion stack
- war-room generation as a required runtime dependency
- extra control-plane layers before the graph loop itself compounds

The next active system is `nstar-autogenesis`, which is intentionally smaller:

- ingest free-form turns
- update graph hypotheses
- compute utility
- select active core
- generate probe seeds
- monitor output after each turn
