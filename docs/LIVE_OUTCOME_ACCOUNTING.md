# Live outcome accounting

The retained Emperor run `civvis-20260908T164553Z` reported ten city losses
from five ownership losses. Occupation-status callbacks can repeat after a
capture. The ledger now follows ownership changes, using birth owner and name
across changing city IDs, and prefers explicit own-roster `city_lost` events
when available. Older unnamed streams can only deduplicate owner/ID pairs;
renamed cities without roster evidence remain a limitation.

Combat kills and losses require a unit participant with an owner and ID and
count each death once. Lethal city/district strikes are not unit deaths.
Replaying that retained stream produces 36 kills, 58 unit combat losses and
five city losses. Its 80 military roster disappearances remain a separate
measure: removal is not proof of combat, and a full-health disappearance at
zero Gold is context, not a proven bankruptcy disband.

The ladder retains treasury and visible-threat fields when constructing slim
states. The race audit reports first-frame empty-treasury deficit turns and
exact-turn Gold, income and military checkpoints. Missing economic fields do
not count as observed solvent turns.

`python3 tools/civ6_race_audit.py RUN_DIR [RUN_DIR ...] --out report.json`
groups resumed segments into games and reports wins/losses separately for each
fixed settings, genes, controller revision and binary cohort. Only verified,
opening-retained games with an explicit terminal outcome enter win rates.
Unfinished games and mixed-controller runs are excluded, not losses or wins.
Compare completed cohorts on difficulty 6 before promoting the same controller
to 7 and 8; retain exclusions and sample sizes. Native six-seat tournaments
continue unchanged and are not pooled with live game outcomes.
