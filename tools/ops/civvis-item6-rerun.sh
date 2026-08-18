#!/bin/zsh
# Item 6 of docs/EVAL_INTEGRITY.md: re-measure the confounded comparisons.
#
# ⚠ Run AGAINST THE CORRECTED WORKLIST, not §8's hand-counted one. §8 is wrong on
# five of six rows because a netless `strategic`/`production` differs in
# *evaluator* as well as architecture, and that axis is invisible to a reading of
# the entrant names. See the "Recomputed 2026-07-31" section of §8 (PR #693).
#
# Two of the original eight need WITHDRAWING rather than re-running:
# `policy_wide` and `policy_wide_frozen` against `advanced_evolved` report
# `arms differ on: none` — they are self-comparisons and measured nothing.
#
# Waits for the deployment-profile run to finish first. One heavy eval at a time:
# the machine also carries the league, the exhibition and a live Civ 6 loop, and
# a searching seat is ~6.4x a scripted one.

set -u
cd $HOME/civvis-spectator-src || exit 1
LOG=$HOME/item6-rerun.log
DEPLOY_PID=${1:-26019}

echo "=== item 6 re-runs queued $(date -u +%FT%TZ) ===" >> $LOG
while kill -0 "$DEPLOY_PID" 2>/dev/null; do sleep 60; done
echo "deployment run $DEPLOY_PID finished; starting $(date -u +%FT%TZ)" >> $LOG

run() {
  local tag=$1; shift
  echo "" >> $LOG
  echo "--- $tag :: $* ---" >> $LOG
  nice -n 15 ./target/release/ai_eval "$@" --jobs 3 2>&1 \
    | grep -E "^arms differ on:|^paired-map score|^promotion gate:|^effect size:|^paired direction:|^mirrored head-to-head" >> $LOG
}

# 1. THE one-axis search comparison. The only entrant pairing in the repository
#    that isolates search compute; everything else varies architecture too.
run "one-axis: search budget" strategic_cheap strategic_score \
    --pairs 120 --players 4 --seed 950000000

# 2-3. Replacement questions. Genuinely multi-axis, so filed as what they are:
#      they answer "should this replace stock", not "what is component X worth".
run "replacement: production" production advanced_evolved --deployment-comparison \
    --pairs 120 --players 4 --seed 951000000

run "replacement: strategic_cheap" strategic_cheap advanced_evolved --deployment-comparison \
    --pairs 120 --players 4 --seed 952000000

echo "" >> $LOG
echo "=== done $(date -u +%FT%TZ) — record these in docs/EVAL.md ===" >> $LOG
echo "NOTE: quote the 'effect size:' line's label, not the bare number." >> $LOG
