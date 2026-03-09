# Kernel Comparison — Minimal Stealable Primitives

**Task:** Inspect smallest kernel-like repos adjacent to nstar-bit. Prioritize mia-kernel, dreaming-kernel, tmp-meta3-engine-test. Goal: identify the smallest kernel worth stealing for bootstrap graph-memory loop.

**Subagent:** `0183ac98-2730-4672-8af4-e14a83ae3d89`

---

## Comparison Table

| Path | What it does | Minimal primitive | Smaller/better than nstar-autogenesis? |
|------|--------------|-------------------|----------------------------------------|
| `/Users/jobs/Desktop/mia-kernel` | Tiny Rust execution kernel: reads intent, scores against constitution themes, performs write, verifies by hash, records receipt with rollback | `propose → act → check → commit/rollback` with receipt emission | Smaller in core (~445 lines vs ~590 in engine.py); better only for execution discipline, not graph-memory learning |
| `/Users/jobs/Desktop/dreaming-kernel` | Manifest-routed HTTP kernel: matches task signals, executes allowlisted ops or LLM calls, persists thread/run artifacts, self-grows via manifest | Hot-reloaded manifest traversal + thread-scoped receipts | No — core ~1,364 lines before scripts/docs; larger and less minimal |
| `/Users/jobs/Desktop/tmp-meta3-engine-test/src/engine/kernel.rs` | Compact policy kernel: 9-bit state, ask/act and evidence gates, decides when to wake higher-order controller, proposes bounded meta-parameter changes | Pure control-plane gate: `bits + gates + small meta2 policy` | **Yes** — smallest meaningful steal (~204 lines); better as additive primitive |
| `/Users/jobs/Desktop/tmp-meta3-engine-test/meta3-causal-kernel/kernel.rs` | Monolithic runtime: graph reflexes, AGI/TSP solvers, ruliad expansion, task planning, HUD/stream endpoints | "One file does everything" runtime multiplexing | No — ~1,891 lines, too mixed-concern |

---

## Recommendation

**Steal the control kernel from `/Users/jobs/Desktop/tmp-meta3-engine-test/src/engine/kernel.rs` first, not a whole repo.**

- Keep `nstar-autogenesis` as the memory-learning core.
- Wrap each `turn` with `ExtendedBits` and `KernelLoop` gates so the loop gets explicit `ask_act`, `evidence`, and "wake meta-controller" decisions.
- Second transplant: borrow only the receipt/check/rollback pattern from mia-kernel, not its full intent-scoring model.

**Baseline:** `/Users/jobs/Developer/nstar-bit/nstar-autogenesis/engine.py` (~590 lines).
