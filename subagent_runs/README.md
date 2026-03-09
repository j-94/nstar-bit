# Subagent Runs — Persisted Outputs

Subagent outputs from the architecture search, kernel comparison, and autogenesis redesign sessions. These are extracted from agent transcripts for reuse without re-running exploration.

**Source:** Parent transcript `958e9b35-eec6-4445-b0e8-4cd2bc10b402` and its subagents.

---

## Full Transcripts

Raw conversation logs (JSONL format, one message per line):

| Path | Description |
|------|--------------|
| `transcripts/parent_958e9b35.jsonl` | Full parent transcript (~470 KB, 346+ messages) |
| `transcripts/subagents/*.jsonl` | 38 subagent run transcripts (each: task prompt + reasoning + final response) |

These are the unedited agent runs. The markdown summaries below extract the key findings.

---

## Index

| File | Task | Key finding |
|------|------|-------------|
| [01_kernel_comparison_minimal](./01_kernel_comparison_minimal.md) | Compare mia-kernel, dreaming-kernel, tmp-meta3 policy kernel vs nstar-autogenesis | Steal `tmp-meta3-engine-test/src/engine/kernel.rs` (~204 lines) first |
| [02_agentic_os_assessment](./02_agentic_os_assessment.md) | Assess agentic-os as standalone vs tmp-meta3 | 5/10 — governance wrapper, not capability engine |
| [03_meta3_branch_investigation](./03_meta3_branch_investigation.md) | Best branch for nstar-bit replacement | `feat/mia-injection` — most complete |
| [04_artifact_producer_consumer_map](./04_artifact_producer_consumer_map.md) | Map war_room, champion_policy, requirements.lock, etc. | 4 highest-value chains with file evidence |
| [05_architecture_search_synthesis](./05_architecture_search_synthesis.md) | Desktop sweep: dreaming-kernel, meta3-graph-core | Eval must measure real runtime; 3-layer pattern |
| [06_ethos_and_insertion_points](./06_ethos_and_insertion_points.md) | LM-authoritative core: audit + insertion points | Tasks only — results synthesized in parent |
| [07_meta_runner_comparison_interpretation](./07_meta_runner_comparison_interpretation.md) | Interpret baseline vs consequence comparison receipt | Enforcement reduces bloat; adoption implies consequence-default |

---

## Related repo docs

- `PURGE_HARDCODED.md` — checklist to remove hardcoded control
- `CRITIQUE_NSTAR_BIT.md` — honest state assessment
- `FINDINGS.md` — proven findings from epoch runs
