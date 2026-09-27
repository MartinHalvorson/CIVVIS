# Fixed-front capital prioritization: registered trial

This measures the existing, default-OFF `domination-capital-focus` option.
There is no controller, deployment, gene-default, or evaluator change. The
native Domination objective and difficulty ladder remain unchanged.

## Registration before outcomes

Registered at **2026-09-27 03:46:05.404739 UTC**, before any fresh trial game.
The frozen source is claim commit
`a2949626977077a5f039fe247b422c606967a441`, whose complete tracked tree is
identical to parent main `be8a36019d3c7af08aa9e8767a5f939fd334eefb`.
This includes #3799 and #3801; it excludes the withheld #3791, #3796,
and #3800 policies and the in-progress #3802 prototype.

Eight fresh seeds, **37923000–37923007**, use the existing
`victory_eval --domination-pair domination-capital-focus --difficulty prince`
probe. Both arms have four majors, Gran Colombia in seat zero targeting
Domination, Tiny 60×38 Pangaea, six city-states, Online speed, the natural
250-turn clock, all victories, and Prince players and barbarians. Only seat
zero's named option changes. All rival controllers remain fixed.

The exact 21-option focal force bundle is `deploy/live-force-on.txt`, SHA-256
`439757684071b807eb4151e055b4a9adf6ac24b7b8b6c0b5da7327b5344dcea0`.
It does not contain `domination-capital-focus`. Four independent two-pair
segments preserve the alternating OFF/ON execution order. Outcomes are
inspected only after all eight pairs finish. Every ending, action comparison,
major-city conquest, capital retention, and home-capital loss is retained.
An evaluator rejection for absent behavioral contrast remains a rejection.

Before fresh games, record binary hashes, actual policy readback, relevant
existing tests, and a persistent replay of all 202 archived native frames.
Those archived future states stay fixed: changed replay orders cannot establish
counterfactual conquest, city survival, or a native win. This eight-pair pilot
is directional evidence. A promising result requires a separately registered
16-pair replication before considering activation. There is no tuning after
fresh outcomes without a new registration.

## What the existing option can change

`AdvancedAi::assess` selects the rival first. Its capital fallback ordinarily
uses the cheapest missing capital across rivals, then requires that capital's
present owner to match the selected front. The option instead ranks eligible
missing capitals inside that front. The helper uses current ownership and can
include recapturing our own original capital.

Emergency objectives, an opening rush capital, victory suppression, conversion
campaigns, and an existing city campaign all precede this fallback. Siege
commitment can retain an already selected enemy city afterward. War readiness
and capture-deferral checks still apply. The option therefore does not promise
to redirect every campaign or overcome an inadequate army.

## Validation and native replay before fresh games

The frozen source passed **124 existing Domination tests** and the full
`cargo test --profile ci --locked` suite: **4,381 passed, zero failed,
53 ignored**. The ledger, evaluation manifest, and unchanged firing gate passed
with **326/326 genes shown to fire, zero waivers**. No new firing artifact or
ledger entry is required for this existing option. The developer-tools build
has the inherited `civilian_reach_safety_on` dead-code warning; this trial
changes no Rust source.

The binaries were built after the registration document was committed as
`39a3a3c1383a3b6c97c99620a6172252a3f28d40`; the only tracked difference from
frozen source `a2949626` was that document. Frozen binary SHA-256 values:

| Binary | SHA-256 |
| --- | --- |
| `civvis_orders` | `4bb2121395fd2561b4fcf41c5ffff41b4812debe6112afa5d18e8ec73ee46b11` |
| `victory_eval` | `3dfbbd2d63ec38a58ae024f17324c3bb1bab45c199eb19af240320e4cd8993a2` |

Both persistent native replay arms completed all **202 frames**, exit zero,
using prefix SHA-256
`afd1616cbcdca0d5e88a470d992807524e8dca70e53800482dd423dd01b2d778`.
Actual stderr genome readback identifies `AdvancedAi::new`, the Domination
target, OFF inactive/unforced, and ON active/forced. The treatment sets differ
only on `domination-capital-focus`.

Every full JSON decision and issued-order list is identical between arms.
Reported strategies cover 142 Expansion frames, 46 Conquest frames, 13 Recovery
frames, and one frame without a strategy note; these are repeated observed
frames, not independent turns. The replay establishes no response to this
option on that archived history and no repair of the native city loss.

`pilot-freeze.json` was written at **2026-09-27 03:52:25.954235 UTC**, before
the fresh games started or any fresh outcome was read.

## Fresh eight-pair result: no behavioral contrast

Every registered pair completed. All eight full applied-action streams and
**every reported OFF/ON outcome field are identical**. Each of the four
segments returned **exit 2**: the unchanged evaluator rejected a batch with
no behavioral contrast. These are complete game records, not successful
fitness evaluations. No guard was weakened, outcomes omitted, or source tuned.

| Outcome total across eight independent seeds | OFF | ON |
| --- | ---: | ---: |
| Focal wins | 1 | 1 |
| Focal Domination wins | 1 | 1 |
| Home-capital losses at end | 0 | 0 |
| Major cities observed held, summed | 26 | 26 |
| Foreign capitals observed held, summed | 3 | 3 |
| Foreign capitals retained at end, summed | 3 | 3 |

All paired outcomes are shown below. The ending identifies the actual winner;
a loss ending in Science or Culture is an opponent's victory.

| Seed | Same ending in both arms | Turn | Winner | Focal score | Applied actions | Major cities ever | Foreign cities at end | Capitals ever/end | First major city / capital |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | --- | --- |
| 37923000 | Domination WIN | 218 | 0 | 2110 | 43196 | 20 | 23 | 3/3 | 153 / 162 |
| 37923001 | Culture loss | 202 | 1 | 557 | 39046 | 0 | 0 | 0/0 | — / — |
| 37923002 | Science loss | 233 | 1 | 1030 | 48482 | 2 | 2 | 0/0 | 178 / — |
| 37923003 | Culture loss | 188 | 1 | 632 | 36704 | 0 | 0 | 0/0 | — / — |
| 37923004 | Science loss | 241 | 1 | 1002 | 50595 | 4 | 5 | 0/0 | 115 / — |
| 37923005 | Science loss | 245 | 3 | 467 | 51371 | 0 | 0 | 0/0 | — / — |
| 37923006 | Science loss | 229 | 1 | 657 | 44933 | 0 | 1 | 0/0 | — / — |
| 37923007 | Science loss | 232 | 2 | 858 | 42725 | 0 | 1 | 0/0 | — / — |

The existing controller wins Domination on seed 37923000 in both arms; that
win supplies no treatment benefit. Seven losses include five opponent Science
victories and two opponent Culture victories. Five seeds have no observed
major-city conquest. This cohort provides **no basis to enable the option**,
no estimate of an activated treatment's effect, and no native promotion proof.
No 16-pair replication is registered or launched because this pilot supplies
no treatment contrast or improvement. The option stays OFF.

Conquest observations occur at round boundaries and once at the end; transient
ownership between those observations is not counted. The summed totals are
descriptive counts, not extra independent games. No higher-priority objective
or army-readiness rule was relaxed to manufacture a contrast.

Raw eight-pair `pilot.jsonl` SHA-256:
`b55e59227511c67fa1776a81078897c42fbe484385ff9a5ac3f439119ff15a6f`.
`pilot-completion.json` preserves all four exit-2 receipts;
`pilot-comparison.json` records every pair and equality of the reported fields.

## Two known-case plan diagnostics

Registered at **2026-09-27 03:59:06.570290 UTC**, after the complete fresh
batch, these deliberately reuse seeds 37923000 and 37923004. The first is the
identical-arm Domination win; the second lost to Science after four major-city
conquests and no capital. They are case studies, not fresh games or replication.

The diagnostic links the frozen controller library, SHA-256
`62983628fb6652f1739c79d931a0072f1063918b609b9fa24d5126c7a3235b8f`.
An observer wrapper delegates every `Ai` trait method to the original
controller and uses the unchanged public `run_game_observed` driver. It records
the public current plan and immutable world facts after seat zero's turn.
It performs no additional assessments or decisions. External harness compilation corrections
were completed before any diagnostic game; their failure logs are retained.
No controller or policy changed.

Both diagnostic pairs reproduce **every original reported outcome field and
action count**. Their OFF/ON action streams and complete observer rows are
identical, and both preserve the evaluator's exit-2 rejection. The original
full-action hashes were not retained, so this is not an independent proof that
the entire diagnostic stream equals the original stream.

| Recorded focal turns | Domination win 37923000 | Science loss 37923004 |
| --- | ---: | ---: |
| Expansion strategy | 125 | 124 |
| Conquest strategy | 84 | 49 |
| Recovery strategy | 9 | 22 |
| Diplomacy strategy | 0 | 10 |
| Science strategy | 0 | 36 |
| Foreign capital named after own turn | 111 | 0 |
| Foreign ordinary city named after own turn | 45 | 202 |
| No target city | 60 | 39 |
| Already-owned city named after own turn | 2 | 0 |
| First observed major-city conquest | 153 | 115 |

The loss starts taking major cities earlier but never converts those conquests
into a capital. Its recorded Science-strategy intervals are 195–208, 213–229,
231, and 233–236. A grand-strategy label does not establish a changed assigned
victory target. These observations point toward investigating how campaigns
continue after early captures; they do not identify the exact priority or
readiness gate that prevented a capital objective.

World-observer military power at turn 140 is 1,177.45 for the eventual winner
versus 682.67 for its selected major rival. In the loss it is 562.38 for the
focal seat versus 1,034.44 for the eventual Science winner, while the selected
front is city-state 9. By turn 200 the losing focal army's world power is
2,249.00 versus that Science winner's 807.90, but the recorded strategy is
Science and the selected front is city-state 8. Those totals include the whole
world roster, do not measure a ready local force, and were not supplied to the
controller. They provide no justification to lower war-readiness thresholds or
override protected objectives.

Diagnostic binary SHA-256:
`ca163ab5aff34d5a8b0c80c4c5df79b8789ccbd7bd4b51633b5eea217c1c643c`.
`plan-diagnostic-registration.json`, `plan-diagnostic-build.json`, both
`diagnostic-37923000/` and `diagnostic-37923004/` directories, and
`plan-diagnostic-comparison.json` retain the selection, provenance, snapshots,
outcomes, and limitations.

## Documentation integration

The report branch subsequently merged main `871b872c4` (#3804, preservation
of public strategic-resource income). The merged documentation branch passed
the full Rust suite again: **4,389 passed, zero failed, 53 ignored**.
This integration validation does not change the source of the earlier frozen
fresh games or diagnostics and does not evaluate #3804's gameplay strength.
`integration-validation.json` preserves that distinction. This PR ships
evidence only; the existing option, deployment bundle, defaults, and ladder
remain unchanged.

## Evidence location and status

Local evidence root:
`~/civvis-war-evidence-20260926/prince-capital-focus/`.
`preregistration.json` records the complete registration and fixed force bundle.
`candidate-build.json`, `validation-completion.json`, `gate-validation.json`,
`native-policy-readback.json`, and `native-replay-comparison.json` preserve the
build and replay receipts. The fresh eight-pair comparison is complete; all
absent-contrast failures are retained. The option stays OFF.
