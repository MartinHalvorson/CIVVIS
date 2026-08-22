#!/bin/zsh
# Self-test for ~/civvis-batch-loop.sh. Run it after ANY edit to that script.
#
# The thing being defended against is a guard that has quietly stopped working, which
# looks exactly like a guard with nothing to report: `run_is_live` stuck on "live"
# idles the loop for ever and reads as correct deference; stuck on "idle" starts a
# batch on top of a running one and corrupts the ledger. Neither announces itself.
#
# So this does NOT assume the machine is idle — it can't, batches are supposed to run
# here. It computes ground truth independently of the guard and checks they agree,
# then plants a fault in each direction.
#
#   zsh ~/civvis-batch-loop-selftest.zsh

CIVVIS_BATCH_LOOP_SELFTEST=1 source /Users/martin/civvis-batch-loop.sh

fail=0
check() {
  if [[ $2 == $3 ]]; then print "  PASS  $1 -> $2"
  else print "  FAIL  $1 -> got '$2' want '$3'"; fail=1; fi
}

# Ground truth, computed WITHOUT the guard's own code path: pgrep, not a ps snapshot.
#
# ⚠ `/civvis_orders` with the slash, for the same reason the guard uses it. This line
# said bare `civvis_orders` first and promptly failed against a CORRECT guard, because
# the loop's own `cargo build --release --bin civvis_orders` was running: the test's
# ground truth was the stale half. An oracle that repeats the bug it is checking for
# does not check for it.
truth() {
  if pgrep -f 'civ6_civvis_climb\.py|civ6_play\.py|civ6_brain\.py|/civvis_orders' >/dev/null 2>&1
  then print live; else print idle; fi
}

print "1. guard agrees with independent ground truth"
run_is_live && got=live || got=idle
want=$(truth)
check "guard vs pgrep" $got $want
[[ $want == live ]] && print "     (a batch is genuinely running; that is the expected state here)"

print "2. planted fault: a process that MUST be seen"
/bin/sh -c 'exec -a "python3 -u tools/civ6_brain.py --serve civvis_orders" sleep 5' &
fake=$!; sleep 0.4
run_is_live && got=live || got=idle
check "fake decider detected" $got live
kill $fake 2>/dev/null; wait $fake 2>/dev/null

print "3. planted fault: a process that must NOT be seen"
# popup_clear.py and the mirror stager run permanently alongside batches. If either
# tripped the guard the loop would never start anything at all.
/bin/sh -c 'exec -a "python3 -u tools/civ6_control/popup_clear.py --interval 2.5" sleep 5' &
decoy=$!; sleep 0.4
run_is_live && got=live || got=idle
if [[ $(truth) == idle ]]; then
  check "decoy ignored" $got idle
else
  # Can't prove it from the guard's answer while something real is up, so prove it
  # from the pattern directly rather than silently passing.
  print -r -- "python3 -u tools/civ6_control/popup_clear.py --interval 2.5" \
    | grep -qE 'civ6_civvis_climb\.py|civ6_play\.py|civ6_brain\.py|civvis_orders' \
    && { print "  FAIL  decoy matches the guard pattern"; fail=1 } \
    || print "  PASS  decoy does not match the guard pattern (checked directly; a real run is up)"
fi
kill $decoy 2>/dev/null; wait $decoy 2>/dev/null

print "4. planted fault: a BUILD of the decider must not read as a live game"
# The loop's own `cargo build --release --bin civvis_orders` contains the decider's
# name.  A bare pattern matched it, so the loop would have deferred to its own build
# step -- and to anyone else's -- for ever.  The decider is always invoked BY PATH.
for line in \
  '/Users/martin/civvis-batch-runner/target/release/civvis_orders --mirror /x --serve:LIVE' \
  '/Users/martin/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo build --release --bin civvis_orders:ignored' \
  'python3 -u tools/civ6_brain.py --run-dir /x:LIVE'
do
  cmd=${line%:*}; want=${line##*:}
  print -r -- "$cmd" \
    | grep -qE 'civ6_civvis_climb\.py|civ6_play\.py|civ6_brain\.py|/civvis_orders' \
    && got=LIVE || got=ignored
  check "${cmd[1,52]}" $got $want
done

print "5. state round-trip"
STATE=$(mktemp /tmp/civvis-state-test.XXXXXX)
state_set last_good abc123
state_set nogame "deadbee 2"
check "last_good"             "$(state_get last_good)"   "abc123"
check "nogame count for sha"  "$(nogame_count deadbee)"  "2"
check "a DIFFERENT sha reads 0 (suspicion is never inherited)" "$(nogame_count f114601)" "0"
state_set last_good zzz999
check "overwrite wins"        "$(state_get last_good)"   "zzz999"
check "no duplicate key line" "$(grep -c '^last_good ' $STATE)" "1"
rm -f $STATE

print ""
(( fail )) && print "SELF-TEST FAILED" || print "SELF-TEST PASSED"
exit $fail
