# experiments

This folder contains a runnable experiment harness for `nstar-bit`.

It runs eight experiment tracks and writes all artifacts to:

- `experiments/output/<timestamp>/`

## Run

```bash
bash experiments/scripts/run_all.sh
```

## Experiments

1. `exp1_minimal_collapse`  
   Stateful single-domain run over a fixed prompt set.

2. `exp2_dynamic_vs_stateless`  
   Same prompt set, compared with stateful memory vs reset-each-turn baseline.

3. `exp3_node_state_interchangeability_proxy`  
   Mixed prompt domains to observe emergent predicate mixing in one state graph.

4. `exp4_multiscale_proxy`  
   Session-level warning lead check from turn-level gate outputs.

5. `exp5_stochastic_audit_proxy`  
   Sampled-audit coverage analysis over low-quality turns.

6. `exp6_rejection_distillation_proxy`  
   Plain prompts vs prompts with explicit rejection/verification cue.

7. `exp7_cross_domain_adaptation`  
   Domain switch within one run to test second-half predicate emergence.

8. `exp8_operator_legibility`  
   Dashboard generation from collapse/state traces and simple next-turn risk score.

## Notes

- Runs are isolated in per-experiment working dirs under `output/<timestamp>/work/`.
- Existing repo state files are not modified by this harness.
- If no API key is available, runs will still emit logs and status, and failed experiments are marked.
