# nstar-bit experiment run

- run_id: `20260303-133431`
- prompt_limit_per_series: `4`
- api_key_status: `available`

## Results

| Experiment | Key metrics |
|---|---|
| exp1_minimal_collapse | discovered=2, nonclear_gates=3, q_first=0.975, q_second=0.95 |
| exp2_dynamic_vs_stateless | q_stateful=0.825, q_stateless=0.95, delta=-0.1250 |
| exp3_node_state_interchangeability_proxy | discovered=2, predicates_in_state=2 |
| exp4_multiscale_proxy | warning_total=2, warning_hits=0, lead_precision=0.0000 |
| exp5_stochastic_audit_proxy | low_quality_total=0, low_quality_sampled=0, sampled_coverage=0.0000 |
| exp6_rejection_distillation_proxy | q_plain=0.9624999999999999, q_cued=0.9249999999999999, gates_plain=3, gates_cued=2 |
| exp7_cross_domain_adaptation | discovered_first_half=2, discovered_second_half=0 |
| exp8_operator_legibility | median_n=1, next_turn_pred_total=3, next_turn_pred_ok=1, next_turn_pred_acc=0.3333 |

## exp8 dashboard

| turn | event_id | n | quality | gates | discovered |
|---|---:|---:|---:|---:|---|
| 1 | 1 | 0 | 1.0 | 0 | programming_correctness_check |
| 2 | 2 | 1 | 0.95 | 1 | - |
| 3 | 2 | 1 | 0.95 | 1 | - |
| 4 | 2 | 1 | 0.95 | 1 | performance_analysis_check |
