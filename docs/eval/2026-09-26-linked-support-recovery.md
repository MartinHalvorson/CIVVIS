# Recovery for combat units linked to support

## Native failure and current-code reproduction

King / Gran Colombia / Tiny Pangaea / Online / Domination game
`civvis-20260926T190901Z` lost to Culture on turn 188. At turn 115, frame 0,
trebuchet `3538969` had 31 HP and shared `(39,4)` with siege tower `2752531`.
Both reported `formation_count=2`. The trebuchet moved to `(38,6)`, and the
Egyptian district at `(38,8)` killed it with a 31-damage strike. Its campaign
target was a different city, Râ-Kedet at `(34,8)`.

This was the second early trebuchet lost to a district strike; the first died
on turn 106. The last recorded state on each turn 116–143 contains no siege
unit. A Bombard first appears on turn 144. This is an observed replacement gap,
not proof of its production cause.

Archive SHA-256:
`2751e3a479dcc8e470bf347f4b1cd4721e3a0d741be55093155700816cdad94a`.
The offline replay truncates events at the first turn-115 state; its SHA-256 is
`4f10c93d9e03f540650d78c3cca32f06fe6fcfe62657bcd805aab0cfd8043e64`.
The clean task baseline `0507ae309` reproduces the exact native
`MOVE_TO (38,6)` with the recorded live-policy bundle. Changing only the two
formation counts to one does not isolate recovery: the planner links the pair
again before advancing. That counterfactual is retained as a negative diagnostic,
not evidence that an unlinked unit would have survived.

## Correction

The siege controller already admits valid combat/support formations through
`siege_train::linked_support_carrier`. The battle planner's recovery pass instead
excludes every linked unit. Thus the support link makes a wounded siege unit
eligible to advance but ineligible for the existing recovery policy.

Reuse that validated formation predicate for recovery, and reserve both members
when the carrier is held or withdrawn. Keep civilian/religious escort handling,
health thresholds, and ordinary movement legality intact. Exercise healthy return
and linked movement as well as holding in place.

## Preregistered validation

First require focused tests to fail on the current behavior, then replay the
exact archived frame before and after the correction with frozen executables.
The recorded host movement is the baseline witness; an offline safer order is
not proof of native survival or a new domination win.

Before reading candidate results, freeze the unmodified-policy evaluator and
compare four fixed-profile seeds, **37780000–37780003**, across source versions.
Use the existing evaluator command in each version:

```sh
victory_eval --domination-pair early-conquest-opening --games 4 --start-seed 37780000 --out <fresh-file>
```

Each version completes both existing toggle arms; compare matching arms across
versions. These are King, four-player, 60×38 Pangaea, Online, Gran Colombia games
with all victory conditions and the recorded live-policy bundle. Rivals are
CIVVIS controllers, not Firaxis AI. Complete every registered game without early
stopping. Record wins, loss type, ending turn, and ownership outcomes; do not
equate foreign-city totals with original major-capital captures. Four seeds are
a diagnostic pilot, not a statistical claim about stronger play.

The full Rust suite, relevant recovery/siege tests, and changed-file formatting
and quality checks remain required before shipping. No native runtime or
difficulty setting is changed by this investigation.


## Recorded results

The three new combat-recovery regressions failed before the correction. All four
focused tests now pass: a wounded pair holds, both members withdraw together
from lethal city fire, recovery releases at the existing return-health threshold,
and civilian/religious escort links retain their separate controller. The full
`cargo test --profile ci --locked` run passed **4,300 tests**, with zero failures
and 53 existing ignores. Changed-file Rust quality and formatting checks passed.

The byte-identical native snapshot replay changed trebuchet `3538969` from
`MOVE_TO (38,6)` to **`FORTIFY`**, with no move for either formation member. This
establishes the intended recovery decision, not native survival.

All four registered simulator seeds completed in both source versions. Each
version's two toggle arms were identical, so the table reports the matching
`on` arms once per seed. The evaluator returned 2 because its toggle produced no
contrast; all eight legs per version completed and remain in the raw artifacts.
The source-version comparison still differs.

| Seed | Baseline loss / turn | Candidate loss / turn | Score before → after | Observed major cities before → after | Original foreign capitals before → after |
| --- | --- | --- | --- | --- | --- |
| 37780000 | Culture / 168 | Culture / 168 | 423 → 423 | 0 → 0 | 0 → 0 |
| 37780001 | Science / 220 | Culture / 205 | 482 → 439 | 0 → 0 | 0 → 0 |
| 37780002 | Science / 191 | Culture / 181 | 435 → 398 | 0 → 0 | 0 → 0 |
| 37780003 | Score / 250 | Science / 217 | 907 → 810 | 0 → 1 | 0 → 0 |

There were **zero wins and zero domination wins in either version**. Three
candidate losses occurred earlier and ended with lower scores. These unfavorable
results are retained; this pilot does not support a stronger-play claim. All
controllers, including rivals, use the respective source version, so it does not
isolate a focal-player treatment. The narrow correction restores an existing
recovery policy for a formation that already participates in offense; the
native replay and threshold/formation tests support that invariant. Overall
campaign performance and the earlier simulated losses remain follow-up concerns.
No difficulty promotion is justified by this work.

## Reproduction artifacts

Artifacts are retained on the verification host under
`~/civvis-war-evidence-20260926/linked-recovery-replay/`: `provenance.json`, raw
truncated snapshots and decisions, `pilot-provenance.json`, both pilot JSONL files,
`analyze-pilot.py`, and `pilot-comparison.json`. The baseline evaluator source is
`202c33166237151be37a5183712fa58ea29de456` (only tests and preregistration added to
the clean claim); candidate source is `21cac75b94944a9a1ea92aac6f93ebcb2377153a`.

SHA-256 hashes of frozen executables:

| Binary | Baseline | Candidate |
| --- | --- | --- |
| `civvis_orders` | `ecae007557caa21fa5b085380fba6b100c920b9c3e8bc679caab45d803065d2a` | `3cb499065a97bf31f2365ab536a949a99566f150351c5d8aa39f66217f9dad2c` |
| `victory_eval` | `4e45850a390f0b918a67b405d925b7d100453b67f1a761d098caaf4207fbcebc` | `5158fa3f8391fdfb4170291c294804cf11997b52e984f21e43bcefcc733556c6` |

Both evaluators used `cargo build --profile ci --locked --features developer-tools
--bin victory_eval`. The candidate orders executable also enabled
`developer-tools`; that empty feature enables extra binary targets and has no
runtime conditional code. Exact replay arguments and all 20 forced policy flags
are in `provenance.json`.
