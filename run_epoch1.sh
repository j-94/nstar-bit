#!/usr/bin/env bash
# Epoch 1 fixed task runner
# Usage: bash run_epoch1.sh [start_task]

TASKS="epoch1_tasks.txt"
SUMMARY="epoch1_summary.tsv"
TMPOUT="/tmp/nstar_turn_output.txt"
START=${1:-1}

if [[ "$START" -eq 1 ]]; then
    echo -e "turn\ttask\tdecision\tinv_pass\tcov\tcon\tviolations\tdiscovered\tovm_rule\thash" > "$SUMMARY"
fi

current_turns() {
    python3 -c "
import json
try:
    s = json.load(open('nstar_canonical_state.json'))
    print(s.get('turn_count', 0))
except: print(0)
" 2>/dev/null
}

ovm_rule() {
    python3 -c "
import json
try:
    s = json.load(open('nstar_canonical_state.json'))
    rule = s['graph'].get('scoring_rule', '')
    print(rule[:60] if rule else 'empty')
except: print('?')
" 2>/dev/null
}

TASK_NUM=0
# Read tasks into array to avoid stdin conflict with the while loop
mapfile -t PROMPTS < "$TASKS"
TOTAL=${#PROMPTS[@]}

for prompt in "${PROMPTS[@]}"; do
    TASK_NUM=$((TASK_NUM + 1))
    [[ "$TASK_NUM" -lt "$START" ]] && continue

    CHAIN_TURN=$(( $(current_turns) + 1 ))
    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "TASK $TASK_NUM / $TOTAL  (chain turn $CHAIN_TURN)"
    echo "PROMPT: $prompt"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

    # Run turn, capture to temp file
    cargo run --bin canonical -- "$prompt" > "$TMPOUT" 2>&1 || true

    # Parse from temp file with Python
    python3 - "$TMPOUT" << 'PYEOF'
import sys, re

text = open(sys.argv[1]).read()
print(text)   # pass through to terminal
PYEOF

    decision=$(python3 -c "
import re, sys
text = open('$TMPOUT').read()
m = re.search(r'decision\s*:\s*(\S+)', text); print(m.group(1) if m else '')
" 2>/dev/null)
    gate=$(python3 -c "
import re
text = open('$TMPOUT').read()
m = re.search(r'gate\s*:\s*(.+)', text); print(m.group(1).strip() if m else '')
" 2>/dev/null)
    inv=$(python3 -c "
import re
text = open('$TMPOUT').read()
m = re.search(r'invariants.*?passed=(\S+)', text); print(m.group(1) if m else '')
" 2>/dev/null)
    cov=$(python3 -c "
import re
text = open('$TMPOUT').read()
m = re.search(r'coverage=([0-9.]+)', text); print(m.group(1) if m else '')
" 2>/dev/null)
    con=$(python3 -c "
import re
text = open('$TMPOUT').read()
m = re.search(r'contradiction=([0-9.]+)', text); print(m.group(1) if m else '')
" 2>/dev/null)
    vio=$(python3 -c "
import re
text = open('$TMPOUT').read()
m = re.search(r'violations\s*:\s*(.+)', text); print(m.group(1).strip() if m else '')
" 2>/dev/null)
    disc=$(python3 -c "
import re
text = open('$TMPOUT').read()
m = re.search(r'discovered\s*:\s*(.+)', text); print(m.group(1).strip() if m else '')
" 2>/dev/null)
    hash=$(python3 -c "
import re
text = open('$TMPOUT').read()
m = re.search(r'receipt_hash\s*:\s*(\S+)', text); print(m.group(1) if m else '')
" 2>/dev/null)

    rule=$(ovm_rule)

    echo ""
    echo "  → decision=$decision  gate=$gate"
    echo "  → inv=$inv  cov=$cov  con=$con"
    [[ -n "$vio" ]] && echo "  → violations: $vio"
    [[ -n "$disc" ]] && echo "  → discovered: $disc"
    echo "  → ovm_rule: $rule"
    echo "  → hash: $hash"

    ACTUAL_TURN=$(current_turns)
    echo -e "${ACTUAL_TURN}\t${TASK_NUM}\t${decision}\t${inv}\t${cov}\t${con}\t${vio:-—}\t${disc:-—}\t${rule}\t${hash}" >> "$SUMMARY"

done

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "ALL TASKS DONE — running replay verifier"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
cargo run --bin replay 2>&1

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "FINAL STATE"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
cargo run --bin canonical -- --state 2>&1

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "SUMMARY TABLE"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
column -t -s $'\t' "$SUMMARY"
