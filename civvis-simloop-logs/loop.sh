#!/usr/bin/env bash
#
# Run iterations back to back, one at a time, until told to stop.
#
# A full cycle is a couple of minutes and the cron that supervises this fires
# every thirty, so driving one iteration per fire would leave the machine idle
# for most of the hour. This keeps the alternation going continuously; the cron
# becomes a health check that restarts it if it dies.
#
# Deliberately tiny. A running supervisor holds the copy of *this* file it
# started with, so anything that changes often belongs in `iterate.sh`, which
# is re-read on every pass because it is invoked as a fresh subprocess.
#
#   ./loop.sh &                       start
#   touch ~/civvis-simloop-logs/.stop stop after the current iteration
set -uo pipefail

LOGS=/Users/martin/civvis-simloop-logs
cd "$LOGS" || exit 1

while [ ! -f "$LOGS/.stop" ]; do
  ./iterate.sh >> "$LOGS/loop.log" 2>&1
  status=$?
  # 75 is "another iteration holds the lock" — something else is driving, so
  # wait rather than spinning against it.
  if [ "$status" -eq 75 ]; then
    /bin/sleep 60
    continue
  fi
  # A build or a game that failed should not be retried at full speed; a
  # wedged engine would otherwise fill the disk with identical failures.
  [ "$status" -ne 0 ] && /bin/sleep 120
done

rm -f "$LOGS/.stop"
echo "loop stopped at $(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$LOGS/loop.log"
