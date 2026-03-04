#!/usr/bin/env bash
set -u -o pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
EXP_DIR="$ROOT/experiments"
PROMPT_DIR="$EXP_DIR/prompts"
OUT_BASE="$EXP_DIR/output"
RUN_ID="$(date +"%Y%m%d-%H%M%S")"
RUN_DIR="$OUT_BASE/$RUN_ID"
WORK_BASE="$RUN_DIR/work"
LOG_DIR="$RUN_DIR/logs"
DATA_DIR="$RUN_DIR/data"
STATUS_DIR="$RUN_DIR/status"

PROMPT_LIMIT="${PROMPT_LIMIT:-4}"

mkdir -p "$WORK_BASE" "$LOG_DIR" "$DATA_DIR" "$STATUS_DIR"

log() {
  printf '[%s] %s\n' "$(date +"%H:%M:%S")" "$*"
}

mark_status() {
  local name="$1"
  local value="$2"
  printf '%s\n' "$value" > "$STATUS_DIR/${name}.status"
}

key_available() {
  if [[ -n "${ROUTER_API_KEY:-}" || -n "${OPENROUTER_API_KEY:-}" ]]; then
    return 0
  fi
  if command -v security >/dev/null 2>&1; then
    if security find-generic-password -s OPENROUTER_API_KEY -w >/dev/null 2>&1; then
      return 0
    fi
  fi
  return 1
}

avg_quality_jsonl() {
  local file="$1"
  if [[ ! -s "$file" ]]; then
    printf '0\n'
    return
  fi
  jq -s 'if length == 0 then 0 else (map(.quality) | add / length) end' "$file"
}

count_nonclear_gates_jsonl() {
  local file="$1"
  if [[ ! -s "$file" ]]; then
    printf '0\n'
    return
  fi
  jq -s '[.[] | select(.gate_summary != "CLEAR: proceed")] | length' "$file"
}

count_discovered_jsonl() {
  local file="$1"
  if [[ ! -s "$file" ]]; then
    printf '0\n'
    return
  fi
  jq -s '[.[] | .discovered | select(. != null)] | length' "$file"
}

half_avg_jsonl() {
  local file="$1"
  local mode="$2"
  if [[ ! -s "$file" ]]; then
    printf '0\n'
    return
  fi
  if [[ "$mode" == "first" ]]; then
    jq -s 'def avg(xs): if (xs|length)==0 then 0 else (xs|add/length) end; avg((.[0:(length/2|floor)] | map(.quality)))' "$file"
  else
    jq -s 'def avg(xs): if (xs|length)==0 then 0 else (xs|add/length) end; avg((.[(length/2|floor):] | map(.quality)))' "$file"
  fi
}

run_series_stateful() {
  local name="$1"
  local prompt_file="$2"
  local prefix="${3:-}"
  local workdir="$WORK_BASE/$name"
  local logfile="$LOG_DIR/${name}.log"
  local turns=0
  local rc=0

  mkdir -p "$workdir"
  : > "$logfile"

  (
    cd "$workdir" || exit 1
    "$ROOT/target/debug/nstar-bit" --reset
    while IFS= read -r prompt; do
      [[ -z "$prompt" || "${prompt:0:1}" == "#" ]] && continue
      turns=$((turns + 1))
      if [[ "$turns" -gt "$PROMPT_LIMIT" ]]; then
        break
      fi
      if [[ -n "$prefix" ]]; then
        run_prompt="$prefix $prompt"
      else
        run_prompt="$prompt"
      fi
      printf '\n=== TURN %d ===\n' "$turns"
      printf 'PROMPT: %s\n' "$run_prompt"
      "$ROOT/target/debug/nstar-bit" "$run_prompt"
    done < "$prompt_file"
  ) > "$logfile" 2>&1 || rc=$?

  if [[ "$rc" -eq 0 ]]; then
    mark_status "$name" "ok"
  else
    mark_status "$name" "fail:$rc"
  fi

  if [[ -f "$workdir/receipts.jsonl" ]]; then
    cp "$workdir/receipts.jsonl" "$DATA_DIR/${name}_receipts.jsonl"
  fi
  if [[ -f "$workdir/nstar_state.json" ]]; then
    cp "$workdir/nstar_state.json" "$DATA_DIR/${name}_state.json"
  fi
}

run_series_stateless() {
  local name="$1"
  local prompt_file="$2"
  local workdir="$WORK_BASE/$name"
  local logfile="$LOG_DIR/${name}.log"
  local merged="$DATA_DIR/${name}_receipts.jsonl"
  local turns=0
  local failures=0

  mkdir -p "$workdir"
  : > "$logfile"
  : > "$merged"

  while IFS= read -r prompt; do
    [[ -z "$prompt" || "${prompt:0:1}" == "#" ]] && continue
    turns=$((turns + 1))
    if [[ "$turns" -gt "$PROMPT_LIMIT" ]]; then
      break
    fi
    local_turn_dir="$workdir/turn_$turns"
    mkdir -p "$local_turn_dir"
    (
      cd "$local_turn_dir" || exit 1
      "$ROOT/target/debug/nstar-bit" --reset
      printf '\n=== TURN %d ===\n' "$turns"
      printf 'PROMPT: %s\n' "$prompt"
      "$ROOT/target/debug/nstar-bit" "$prompt"
    ) >> "$logfile" 2>&1 || failures=$((failures + 1))

    if [[ -f "$local_turn_dir/receipts.jsonl" ]]; then
      cat "$local_turn_dir/receipts.jsonl" >> "$merged"
    fi
  done < "$prompt_file"

  if [[ "$failures" -eq 0 ]]; then
    mark_status "$name" "ok"
  else
    mark_status "$name" "failures:$failures"
  fi
}

log "run_id=$RUN_ID"
printf '%s\n' "$RUN_ID" > "$RUN_DIR/run_id.txt"

if key_available; then
  log "api_key=available"
  printf 'available\n' > "$RUN_DIR/api_key_status.txt"
else
  log "api_key=missing"
  printf 'missing\n' > "$RUN_DIR/api_key_status.txt"
fi

log "building binaries"
if (cd "$ROOT" && cargo build --bins) > "$LOG_DIR/build.log" 2>&1; then
  mark_status "build" "ok"
else
  mark_status "build" "fail"
fi

if [[ ! -x "$ROOT/target/debug/nstar-bit" ]]; then
  log "nstar-bit binary missing; stopping"
  mark_status "fatal" "nstar-bit binary missing"
  exit 1
fi

log "running exp1_minimal_collapse"
run_series_stateful "exp1_minimal_collapse" "$PROMPT_DIR/single_domain_debugging.txt"

log "running exp2_dynamic_vs_stateless (stateful arm)"
run_series_stateful "exp2_dynamic_stateful" "$PROMPT_DIR/single_domain_debugging.txt"
log "running exp2_dynamic_vs_stateless (stateless arm)"
run_series_stateless "exp2_dynamic_stateless" "$PROMPT_DIR/single_domain_debugging.txt"

log "running exp3_node_state_interchangeability_proxy"
run_series_stateful "exp3_mixed_domain" "$PROMPT_DIR/mixed_domain.txt"

log "running exp6_rejection_distillation_proxy"
run_series_stateful \
  "exp6_rejection_cued" \
  "$PROMPT_DIR/single_domain_debugging.txt" \
  "Before answering, explicitly surface uncertainty, verify assumptions, and avoid premature completion."

# Reuse exp3 run for exp7 metrics (cross-domain in one stream)
cp "$DATA_DIR/exp3_mixed_domain_receipts.jsonl" "$DATA_DIR/exp7_cross_domain_receipts.jsonl" 2>/dev/null || true
cp "$DATA_DIR/exp3_mixed_domain_state.json" "$DATA_DIR/exp7_cross_domain_state.json" 2>/dev/null || true
mark_status "exp7_cross_domain_adaptation" "derived_from_exp3_mixed_domain"

exp1_receipts="$DATA_DIR/exp1_minimal_collapse_receipts.jsonl"
exp1_state="$DATA_DIR/exp1_minimal_collapse_state.json"
exp2_stateful="$DATA_DIR/exp2_dynamic_stateful_receipts.jsonl"
exp2_stateless="$DATA_DIR/exp2_dynamic_stateless_receipts.jsonl"
exp3_receipts="$DATA_DIR/exp3_mixed_domain_receipts.jsonl"
exp3_state="$DATA_DIR/exp3_mixed_domain_state.json"
exp6_plain="$DATA_DIR/exp1_minimal_collapse_receipts.jsonl"
exp6_cued="$DATA_DIR/exp6_rejection_cued_receipts.jsonl"
exp7_receipts="$DATA_DIR/exp7_cross_domain_receipts.jsonl"
exp8_state="$DATA_DIR/exp1_minimal_collapse_state.json"

exp1_q_first="$(half_avg_jsonl "$exp1_receipts" first)"
exp1_q_second="$(half_avg_jsonl "$exp1_receipts" second)"
exp1_disc="$(count_discovered_jsonl "$exp1_receipts")"
exp1_gates="$(count_nonclear_gates_jsonl "$exp1_receipts")"

exp2_q_stateful="$(avg_quality_jsonl "$exp2_stateful")"
exp2_q_stateless="$(avg_quality_jsonl "$exp2_stateless")"
exp2_delta="$(awk -v a="$exp2_q_stateful" -v b="$exp2_q_stateless" 'BEGIN {printf "%.4f", a-b}')"

exp3_disc="$(count_discovered_jsonl "$exp3_receipts")"
exp3_pred_count="$(jq '.predicates | length' "$exp3_state" 2>/dev/null || printf '0\n')"

exp4_warn_total=0
exp4_warn_hits=0
exp4_lead_precision=0
if [[ -s "$exp1_receipts" ]]; then
  read -r exp4_warn_total exp4_warn_hits exp4_lead_precision < <(
    jq -r '[.turn,.quality, (.gate_summary != "CLEAR: proceed")] | @tsv' "$exp1_receipts" \
      | awk '
        NR==1 {pq=$2; pg=$3; next}
        {
          if (pg=="true") {
            warn_total++
            if (($2+0) < (pq+0)) warn_hits++
          }
          pq=$2; pg=$3
        }
        END {
          if (warn_total==0) printf "0 0 0.0000\n";
          else printf "%d %d %.4f\n", warn_total, warn_hits, warn_hits/warn_total;
        }'
  )
fi
mark_status "exp4_multiscale_proxy" "computed"

exp5_low_total=0
exp5_low_sampled=0
exp5_low_cov=0
if [[ -s "$exp1_receipts" ]]; then
  read -r exp5_low_total exp5_low_sampled exp5_low_cov < <(
    jq -r '[.turn,.quality] | @tsv' "$exp1_receipts" \
      | awk '
        ($2+0) < 0.6 {
          low_total++
          if (($1 % 3) == 0) low_sampled++
        }
        END {
          if (low_total==0) printf "0 0 0.0000\n";
          else printf "%d %d %.4f\n", low_total, low_sampled, low_sampled/low_total;
        }'
  )
fi
mark_status "exp5_stochastic_audit_proxy" "computed"

exp6_q_plain="$(avg_quality_jsonl "$exp6_plain")"
exp6_q_cued="$(avg_quality_jsonl "$exp6_cued")"
exp6_gate_plain="$(count_nonclear_gates_jsonl "$exp6_plain")"
exp6_gate_cued="$(count_nonclear_gates_jsonl "$exp6_cued")"

exp7_disc_first="$(jq -s '[.[0:(length/2|floor)][] | .discovered | select(. != null)] | length' "$exp7_receipts" 2>/dev/null || printf '0\n')"
exp7_disc_second="$(jq -s '[.[(length/2|floor):][] | .discovered | select(. != null)] | length' "$exp7_receipts" 2>/dev/null || printf '0\n')"
mark_status "exp7_cross_domain_adaptation" "computed"

exp8_median_n="$(jq '[.collapses[].n] | if length==0 then 0 else sort | .[length/2|floor] end' "$exp8_state" 2>/dev/null || printf '0\n')"
exp8_pred_total=0
exp8_pred_ok=0
exp8_pred_acc=0
if [[ -f "$exp8_state" ]]; then
  read -r exp8_pred_total exp8_pred_ok exp8_pred_acc < <(
    jq -r '.collapses[] | [.turn, .n, .quality, (.gates_fired | length)] | @tsv' "$exp8_state" \
      | awk -v m="$exp8_median_n" '
        NR==1 {pn=$2; pg=$4; next}
        {
          pred = ((pn+0) > (m+0) || (pg+0) > 0) ? 1 : 0
          actual = (($3+0) < 0.6) ? 1 : 0
          total++
          if (pred == actual) ok++
          pn=$2; pg=$4
        }
        END {
          if (total==0) printf "0 0 0.0000\n";
          else printf "%d %d %.4f\n", total, ok, ok/total;
        }'
  )
fi
mark_status "exp8_operator_legibility" "computed"

{
  echo "# nstar-bit experiment run"
  echo
  echo "- run_id: \`$RUN_ID\`"
  echo "- prompt_limit_per_series: \`$PROMPT_LIMIT\`"
  echo "- api_key_status: \`$(cat "$RUN_DIR/api_key_status.txt")\`"
  echo
  echo "## Results"
  echo
  echo "| Experiment | Key metrics |"
  echo "|---|---|"
  echo "| exp1_minimal_collapse | discovered=$exp1_disc, nonclear_gates=$exp1_gates, q_first=$exp1_q_first, q_second=$exp1_q_second |"
  echo "| exp2_dynamic_vs_stateless | q_stateful=$exp2_q_stateful, q_stateless=$exp2_q_stateless, delta=$exp2_delta |"
  echo "| exp3_node_state_interchangeability_proxy | discovered=$exp3_disc, predicates_in_state=$exp3_pred_count |"
  echo "| exp4_multiscale_proxy | warning_total=$exp4_warn_total, warning_hits=$exp4_warn_hits, lead_precision=$exp4_lead_precision |"
  echo "| exp5_stochastic_audit_proxy | low_quality_total=$exp5_low_total, low_quality_sampled=$exp5_low_sampled, sampled_coverage=$exp5_low_cov |"
  echo "| exp6_rejection_distillation_proxy | q_plain=$exp6_q_plain, q_cued=$exp6_q_cued, gates_plain=$exp6_gate_plain, gates_cued=$exp6_gate_cued |"
  echo "| exp7_cross_domain_adaptation | discovered_first_half=$exp7_disc_first, discovered_second_half=$exp7_disc_second |"
  echo "| exp8_operator_legibility | median_n=$exp8_median_n, next_turn_pred_total=$exp8_pred_total, next_turn_pred_ok=$exp8_pred_ok, next_turn_pred_acc=$exp8_pred_acc |"
  echo
  echo "## exp8 dashboard"
  echo
  echo "| turn | event_id | n | quality | gates | discovered |"
  echo "|---|---:|---:|---:|---:|---|"
  jq -r '.collapses[] | "| " + (.turn|tostring) + " | " + (.coordinate.event_id|tostring) + " | " + (.n|tostring) + " | " + (.quality|tostring) + " | " + ((.gates_fired|length)|tostring) + " | " + (.discovered.name // "-") + " |"' "$exp8_state" 2>/dev/null
} > "$RUN_DIR/summary.md"

printf 'run_dir=%s\n' "$RUN_DIR" > "$RUN_DIR/run_location.txt"
log "done: $RUN_DIR"
