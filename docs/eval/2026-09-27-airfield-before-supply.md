# Earlier alternative airfield: King source comparison

Status: **WITHHELD**. The pilot did not satisfy its registered promotion rule.
The final PR contains this evaluation report only; deployed policy is unchanged.
The experimental implementation remains available in checkpoint
[`a0a14a9e1`](https://github.com/MartinHalvorson/CIVVIS/commit/a0a14a9e1ca7e145ef0a4724b95b4891caae16b2)
and the frozen candidate patch/binaries.

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
an earlier alternative field proposal without an extra third field. The native
check was amended before fresh launch as described below; the strength decision
rule and candidate policy code did not change.

Promising only if candidate has more Domination wins OR more final foreign original capitals than baseline, with no decrease in Domination wins or final foreign original capitals and no increase in own original capital losses. Any promising pilot requires a separately preregistered 16-source-pair replication before shipping. Otherwise retain deployed baseline and mark prototype WITHHELD. Earlier native proposals alone cannot satisfy this rule.

## Evidence and validation

External evidence root:
`~/civvis-war-evidence-20260927/airfield-before-supply/`.
Registration, native input and hash, complete source-equivalence patch, and
independent immutable baseline binaries are retained there. The reused baseline
binaries were compiled from `0583bcbb4544ca43608ef4f46a8982693f38dea9`;
the twelve files changed through baseline `307fdf2` are exclusively docs/Python.
Rust, compiled data, Cargo inputs, gene ledger and the force list are identical.
Candidate binaries were built in this task worktree's own target directory
and frozen before the trial from `a0a14a9e1ca7e145ef0a4724b95b4891caae16b2`:

| Binary | Baseline SHA-256 | Candidate SHA-256 |
| --- | --- | --- |
| `civvis_orders` | `23e797618293c0e41b215517db8a77585042f0990c42308a619a70b856b19a95` | `95cd336e3c3646a63131d60ee7e2c3ec564cd688d6e025acddff6cc9462ad98c` |
| `victory_eval` | `afade64f074882b49a46ee5add09898ce7b74eb70573ecf8d9dbdd7768ed7264` | `ad73d0729bc6be683137af84aa4cb91ef1b92e1f33fc03679e8df0c8af9b7f4d` |

Focused regression: baseline produced two expected failures (earlier field order
and unfueled queue preservation); all six production-queue tests pass after the
candidate. An intermediate candidate still failed the earlier-order test because
the producer returned early when its fuel-dependent Bomber goal was zero; the
separate alternative-field requirement resolves that path. Baseline persistent
replay completed all 574 frames and reproduced the native orders: Panamá on 172,
Cuenca on 192. Candidate replay also completed all 574 frames. Both actual genomes report
Domination with air-surge-2 ON, v1 OFF, identical treatment sets, and every one
of the 21 requested live policies active. Orders differ in 23 frames. The first
field remains Panamá on turn 172; the candidate proposes an alternative in
Caracas on 179, whereas baseline first proposes Cuenca on 192.

### Native check calibration before fresh launch

The initial verifier failed because it specifically required an earlier Cuenca
order and counted unique field cities requested across the whole history as
concurrent commitments. That is too strong for replay whose future boards are
fixed to deployed play: they do not include the counterfactual Caracas field.
The original failed report is retained as `native-policy-readback.initial.json`;
the method amendment is timestamped in `native-method-amendment.json` before
fresh launch. No policy code, fixed seed or strength decision rule was changed.

The corrected check requires an earlier first alternative, the unchanged original
field, and at most two observed-plus-proposed field-city commitments in each
frame. It passes: Panamá 172, Caracas 179/180/181/182, and Cuenca 192 proposals
have one or two commitments on their respective observed boards. This does not
prove the later native feedback, successful Caracas construction, or absence of
a third field in an actual counterfactual game. The queue fixture independently
checks that two actual commitments block another reservation.

Local validation: `cargo test --profile ci --locked` passed 4,420 tests with
zero failures and 53 ignored; `python3 tools/rust_quality.py --base 307fdf2 --head
a0a14a9e1` reports the changed lines formatted and warning-free; `python3 -m
unittest discover -s tools -p 'test_docs*.py'` passed 12 tests. The six focused
production-queue tests passed. An initial attempt to run a nonexistent
`tools.test_doc_hygiene` module failed before tests and was replaced by discovery.

## Fresh trial result

All four workers finished on 2026-09-27 at 11:52:37 UTC before outcome inspection
began at 11:52:54 UTC. All sixteen single-seed harness invocations returned zero;
all 32 games and all eight source pairs are retained. Every profile, fixed seed,
forced-policy list, focal civilization/target and paired civilization roster was
validated. No seed replacement or policy tuning occurred after launch.

The primary comparison is focal air-surge-2 ON across the two sources:

| Metric, eight games per source | Baseline | Candidate |
| --- | ---: | ---: |
| Domination wins | 0 | 0 |
| Final foreign original capitals | 1 | 1 |
| Foreign original capitals ever observed held | 1 | 1 |
| Own original capital losses | 1 | 1 |
| Foreign major cities ever observed held | 5 | 4 |
| Major declarations | 7 | 4 |



| Seed | Baseline finish | Candidate finish | Major cities ever, baseline/candidate | Final foreign capitals, baseline/candidate | Own capital held, baseline/candidate |
| --- | --- | --- | ---: | ---: | --- |
| 38250000 | science 173 | science 173 | 0/0 | 0/0 | yes/yes |
| 38250001 | science 203 | science 201 | 0/0 | 0/0 | yes/yes |
| 38250002 | science 191 | science 187 | 0/0 | 0/0 | no/yes |
| 38250003 | science 191 | science 196 | 1/0 | 0/0 | yes/yes |
| 38250004 | science 199 | science 196 | 1/2 | 0/0 | yes/yes |
| 38250005 | culture 163 | culture 161 | 0/0 | 0/0 | yes/yes |
| 38250006 | science 220 | culture 177 | 3/2 | 1/1 | yes/yes |
| 38250007 | culture 160 | science 205 | 0/0 | 0/0 | yes/no |

The candidate has neither more Domination wins nor more final foreign original
capitals. The other three registered guards pass, but that does not satisfy the
improvement requirement. Keep the deployed baseline and air-surge-2 ON; do not
start replication or activate this prototype. These eight pairs do not establish
general harm or equivalence. The earlier native proposal is a scheduling effect,
not evidence of a winning policy.

Secondary OFF rows have zero Domination wins and final foreign original capitals
for both sources. Baseline/candidate own-capital losses are 1/0 and major cities
ever observed held are 0/1. Those rows do not alter the primary decision; source
changes also reach the adaptive rival controllers.

## Later native observation and next investigation

A separate deployed game, `civvis-20260927T110717Z`, finished in a rival Science
victory on turn 239. Actual seat readback confirms four players, King, Gran
Colombia/Simón Bolívar, Tiny Pangaea, Online, Gathering Storm, maximum 250 turns
and all victory conditions enabled. It ran source `1f005c184` with the unchanged
native decider SHA-256 recorded above; the experimental candidate was never
installed in that game.

Radio and positive Aluminum income were first observed on 172, Advanced Flight
on 181, field queues on 184, usable fields and a Bomber queue on 191, the first
Bomber on 203, and two Bombers on 228. The first foreign major city was observed
held on 231. At the finish there were two usable fields, two Bombers, gross
Aluminum income four, one held foreign major city, zero foreign original capitals,
and a retained home original capital. These observations do not attribute that
capture to aircraft.

This case changes the next diagnostic question: inspect the complete native
queue/production feedback and launch assessments between the first Bomber on
203 and the second on 228, including interruption, production-rate changes,
resource and home-defense gates. A ready field and Aluminum income do not imply
a timely two-Bomber force or original-capital capture. Keep this later case
separate from the registered fixed-history preflight.

At the normal completed-game boundary, the next native batch started on shipped
observer revision `307fdf2` without a policy or host-force change. Verify automatic
`air_supply` recording when that batch completes. No native Domination win yet.

Final report-only validation: the two policy files are restored exactly to
`origin/main` (`307fdf2`). `cargo test --profile ci --locked` passes 4,418 tests
with zero failures and 53 ignored; the 12 documentation tests pass. The final
diff contains only this report. The candidate's 32-game soak remains experimental
evidence; the final contribution changes no engine or controller behavior.
PR #3825 supplies the independent CI and shipping record.
