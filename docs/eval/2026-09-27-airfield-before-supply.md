# Earlier alternative airfield: registered King source comparison

Status: prototype; strength unproved; deployment withheld pending the registered trial.

## Native trigger

The completed native game `civvis-20260927T103819Z` lost to a rival Culture victory
on turn 198. Gran Colombia retained its own original capital and captured no
foreign major cities or original capitals. The source was `1f005c184` and the
native decider SHA-256 was
`5c2e1682c7f1dbdd48e0046ec0c4363be61a971f8aced2c63efe2e065dfe90eb`.

An active air surge was appointed against Qaraqorum on turn 168, three technologies
before Advanced Flight. Panamá started an Aerodrome on turn 172 with a native
estimate of 52 turns. At the turn-173 observation it had 3 production and 51 turns
remaining. This does not establish a wrong initial city choice: Panamá was the
only idle eligible city when that order was issued. Cuenca gained a legal field
slot at population four on turn 173 but was still training a Spy. It later had
14 production and a 12-turn Aerodrome menu estimate. Aluminum income first
appeared on turn 191; Cuenca's field was ordered on turn 192. The game ended
with neither field usable and no Bomber.

## Candidate and limits

Treat a strictly faster alternative field as a pending requirement even when
the fuel-dependent Bomber goal is zero. Remove the Aluminum-ready prerequisite
from comparing that field and from preserving its queue through production review.
The existing calculation includes remaining field construction and production of
the two-Bomber launch wing. It still requires an active appointment, exactly one
existing field commitment, a missing launch wing, an idle unthreatened city, a
legal field site, a strictly faster total, and construction within the remaining
game. A fresh third field receives no reservation. Appointment, resource and
war-opening budget guards retain their existing behavior.

This tests scheduling, not whether Aluminum can be acquired or whether an earlier
base can beat the rival Culture finish. Native replay keeps future observations
fixed; a changed proposal cannot establish a counterfactual capture or victory.

## Registration

Registered at `2026-09-27T11:25:41.840881+00:00` before replay results or fresh games.

Compare frozen baseline and candidate sources with focal `air-surge-2` ON.
Eight fresh seeds are fixed: `38250000` through `38250007`. The existing policy
pair harness also runs OFF for each source; those rows are secondary because
rival controllers can consume the source change too.

Profile: four players, Gran Colombia focal, Domination target, Pangaea 60×38,
six city states, Online speed, 250 turns, King player and barbarian difficulty,
focal human handicap exemption, and every victory condition enabled. Rivals are
adaptive CIVVIS controllers; this is not native Firaxis difficulty equivalence.
Keep all 21 deployed forced genes (SHA-256
`439757684071b807eb4151e055b4a9adf6ac24b7b8b6c0b5da7327b5344dcea0`).

Four independent two-seed workers alternate baseline/candidate source order by
seed. Each single-seed harness runs focal OFF then ON; OFF is secondary. This
scheduling detail was corrected before fresh launch and retained in
`schedule-amendment.json`. Inspect outcomes only after every worker is terminal. Do not replace seeds
or tune after launch. Native preflight uses every observed state from the
completed 574-frame turn-1–198 history, including same-turn replans, and requires
an earlier alternative field proposal without an extra third field.

Promising only if candidate has more Domination wins OR more final foreign original capitals than baseline, with no decrease in Domination wins or final foreign original capitals and no increase in own original capital losses. Any promising pilot requires a separately preregistered 16-source-pair replication before shipping. Otherwise retain deployed baseline and mark prototype WITHHELD. Earlier native proposals alone cannot satisfy this rule.

## Evidence and validation

External evidence root:
`~/civvis-war-evidence-20260927/airfield-before-supply/`.
Registration, native input and hash, complete source-equivalence patch, and
independent immutable baseline binaries are retained there. The reused baseline
binaries were compiled from `0583bcbb4544ca43608ef4f46a8982693f38dea9`;
the twelve files changed through baseline `307fdf2` are exclusively docs/Python.
Rust, compiled data, Cargo inputs, gene ledger and the force list are identical.
Candidate binaries will be built in this task worktree's own target directory
and frozen before the trial.

Focused regression: baseline produced two expected failures (earlier field order
and unfueled queue preservation); all six production-queue tests pass after the
candidate. An intermediate candidate still failed the earlier-order test because
the producer returned early when its fuel-dependent Bomber goal was zero; the
separate alternative-field requirement resolves that path. Baseline persistent
replay completed all 574 frames and reproduced the native orders: Panamá on 172,
Cuenca on 192. Candidate replay, full Rust checks and fresh outcomes are pending.
