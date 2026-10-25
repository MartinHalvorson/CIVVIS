# Preserve the turn after a Gran Colombia promotion

The shipped rule is explicit:
`DLC/GranColombia_Maya/Data/GranColombia_Maya_Civilizations.xml:295-296`
attaches `TRAIT_PROMOTE_NO_FINISH_MOVES` to Ejercito Patriota. Lines 327-328
give `MODIFIER_PLAYER_UNITS_PROMOTE_NO_FINISH_MOVES`, and lines 361-363 set
`NoFinishMoves` to true. The active read-only `DebugGameplay.sqlite` rows
independently contain the same trait, modifier type and argument value 1.

The existing `Game::do_promote` instead sets movement and attacks to zero
for every civilization. This removes a legal attack from the simulated turn
after taking the promotion's heal. In fixed native Emperor pilot `035310Z`, the
first next exported state at turn 44/frame 1 shows Warrior 131073 promoted
to Tortoise, healing 26 to 76 HP while retaining three movement and one attack
at the same tile. Turn 56/frame 1 likewise shows Archer 1114121 promoted to
Garrison, healing 23 to 73 HP while retaining three movement and one attack.
These are development observations, not a policy comparison or victory claim.
Retain complete before/after states, decision-to-native ID mappings, emitted
orders and intervening events before treating them as execution witnesses.

The complete native audit is `2026-10-25-gran-colombia-promotions.native.json.gz`
(4,688,889 bytes; SHA-256
`95c22b8670ceafdb5cabc3d22bc5276523e99ddc6de881944c7ea99d25b0cd49`).
Its companion `.native-audit.py` documents selection and classification. It
includes all 134 unit-promotion requests in the four pre-existing pilots, in
fixed chronology: 032555Z, 034102Z, 035310Z, 040737Z. Each observed seat is
Gran Colombia, Emperor, Gathering Storm, with all victory conditions enabled.
The archive retains original raw decision lines, complete before/immediate-next
state lines, every intervening event, file digests, seat readbacks, source
provenance, quoted shipped XML lines and the captured read-only gameplay
database rows. It records the complete XML file's digest and stores only the
seven relevant citation lines. All 9,207 content-addressed blobs were independently
hash-checked.

The audit maps local unit IDs through each decision's `unit_ids`; it requires
a unique before-state identity and takes the immediate next exported state,
without skipping a failed or inconvenient transition. Its conservative guards
exclude any combat in the interval, even combat by unrelated units. Thirty-three
requests pass the same-turn promotion, heal, experience, position and order
guards. Thirty-two also retain movement, maximum movement and attacks. The
remaining Apostle retains its six movement and one attack but its observed
maximum movement changes from six to five; this does not establish an isolated
maximum-movement rule. None of these 33 cases starts with partially spent budgets.
The no-refill test follows the shipped `NoFinishMoves` rule rather than claiming
native coverage of spent budgets.

The first pilot-035310Z promotion, Archer 458755 at turn 28/frame 0 to frame 1,
passes the conservative guards: Garrison is newly present, HP rises 82 to 100,
and movement/maximum movement/attacks remain 3/3/1 at the same tile. The turn-44
and turn-56 examples above retain their budgets but have combat elsewhere in
the interval, so they are excluded from this guarded count. The archive also
retains all missing-state mappings, refused promotions, unchanged promotion
sets, cross-turn observations and movement-restoration intervals. Flag counts
overlap and must not be summed as independent requests.

The gameplay cache changes with the running game. A later read found no active
modifier rows and the collector stopped before writing an archive. The committed
artifact preserves the earlier successful read; rerunning the script requires
a gameplay database with the Gran Colombia expansion modifier loaded.

First add the data trait and focused action tests without a behavioral hook.
Require three intended failures on retained movement or the legal subsequent
attack; ownership, experience, duplicate promotion and ordinary-civilization
controls must pass. Compilation or fixture-precondition failures do not count
as RED. Only after genuine RED, make promotion preserve the remaining movement
and attack budget for that trait. Preserve the existing heal, experience,
promotion eligibility and ordinary-civilization turn spending.

The first compile attempt stopped on a fixture typo (`Action::Attack` uses
`target`, not `to`); this is not behavioral RED. Its log and source receipt are
retained separately from the corrected-fixture behavior run.
The corrected-fixture run finished at 08:32:24Z on October 25: exactly three
intended behavior failures and three passing controls, with all three sealed
source/data files unchanged. The follow-up attack was legal before promotion
and then failed with `no moves left`; movement assertions observed zero instead
of two and zero instead of one. Only this run qualifies as behavioral RED.
The receipt records starting HEAD `8812ad392` plus the corrected file's explicit
digest; checkpoint `371c602ac` contains exactly the three source/data files
identified by those RED digests. The fix is `27da45b2c`. Its six focused cases
all passed at 08:39:44Z, with the full tracked source/data seal unchanged.

The bridge already translates every recorded player action, including
`Action::Promote`, in order (`src/bin/civvis_orders.rs`, `plan_orders` translation
loop and `translate`). `coalesce_unit_paths_except` preserves a non-walk unit
followup when `state.seat.order_queue` is true. All four retained native seat
readbacks have that capability. The Lua `PROMOTE:` handler at
`tools/civ6_control/mod/CivvisControlAgent.lua:16362-16432` checks the offered
promotion and calls the stock `UnitManager.RequestCommand`; it does not restore
movement itself. Existing generic queued-followup and promotion-translation
tests are included in the required full suite. This is a source/capability check;
the native archive contains no candidate-after-fix orders and establishes no
newly deployed native followup execution.

The first hosted full suite on `27da45b2c` found a registry omission: three
existing mod-loading tests reject the newly introduced effect because
`src/validate.rs::CIV_EFFECTS` does not include it. The identical three failures
were reproduced with the already built Mac test executable; three other
mod-loader controls pass. CI stopped after 5,403 passes and three failures,
with 855 tests unrun and 52 skipped. This is a real integration failure.

Before repairing the registry, deliberately terminate the three obsolete Mac
full-build/lint/queued-soak commands. Their source seals remain unchanged and
their receipts record SIGTERM rather than success. The queued soak had started
no battles; its two registered seeds remain reserved. Preserve the CI log,
local reproducer and cancellation manifest. The restarted validation uses the
hosted full CI-profile suite (nextest and documentation tests), focused Mac
engine/mod-loader cases, the hosted Rust-quality gate, data-effect checks and
the same registered local CI-profile combat soak. Do not label the cancelled
Mac full-suite build as passed.

Registry repair `dd32e038d` passed all 13 focused Mac cases at 08:58:01Z:
six promotion cases, six existing mod-loader cases and the existing AI
command-phase control. The data-effect guard also passes with 840 effect keys
and zero unconsumed keys. Both commands retained all 792 source/data digests
unchanged. The native identity mapper resolves `CIVILIZATION_GRAN_COLOMBIA`
through the canonical roster to `Gran Colombia` and applies it to the mirrored
player, so the new civilization effect uses that existing conversion path.

The next hosted run includes upstream #4122 and gets past the repaired loader
cases. Its remaining failure is the audited ruleset fingerprint test: the new
civilization data gives `fnv1a64:d5702e477e1be48d`, while the old test expects
`fnv1a64:d05291ed7cbbe612`. Retain this run's 5,521 passes, one failure, 740
unrun cases and 52 skips, and the matching one-test Mac reproducer. Update
only that existing test's expected fingerprint; the shipped rule and its
semantic cases above independently establish why the data change is correct.
No production code in `src/rules.rs` changes.

Merge `e251829fd` integrates upstream #4122 (`b71b9fa8c`) after all prior
sealed commands finish and current-head checks become terminal. The actual
hosted checkout was `cbdb3a4aa21f6277adcf782917dece89da23987a`; a direct
comparison against the integrated branch finds exactly one difference across
all tracked Rust/data/Cargo/build inputs: the expected fingerprint string in
the existing `src/rules.rs` test module. That file's production prefix is
byte-identical, SHA-256
`756993fe33511270da07cb76929c0eaf9787ed9286b3ac66af812f75848568e2`.
Retain the comparison receipt and complete checked-out CI log alongside the
failed intermediate runs. The full hosted suite and Rust-quality gate still
cover the final pushed candidate.

The integrated focused run finished at 09:31:14Z on October 25: all 43 cases
pass on `e251829fd`, including six promotion cases, six mod-loader cases, the
existing AI command-phase control and all 30 rules/validation cases. All 792
tracked source/data inputs remain unchanged. Its log retains the existing
Mac debug-linker unwind-size warning; no warning-free Mac build is claimed.

`2026-10-25-gran-colombia-promotions.validation.json.gz` preserves 37 entries
(1,193,918 bytes; SHA-256
`eae99f50811bc5f84b2853c76744df840b8abde5b0cad28989f5c43de7d139c5`).
It contains the initial compile error, genuine three-failure behavioral RED,
passing controls, both intermediate hosted failures, local loader and
fingerprint reproducers, deliberate cancellation manifests and exit receipts,
passing six/13/43-case runs, the completed registered soak, data-effect results,
source comparison, native archive verification and packaging/validation scripts.
Every archived entry's digest was checked after decompression. The final hosted
full-suite verdict and deployment readback belong to PR #4129's current-head
checks and integration record, rather than being inferred from this evidence
snapshot or the cancelled Mac full build.

This task models the promotion component only. The existing civilization's
combat/XP estimates and its separate movement bonus need a broader fidelity
audit; this change does not claim complete civilization modeling. Native
movement facts remain authoritative for observed units.

Validate the focused engine cases, existing promotion/combat and command-phase
coverage, required full CI-profile suite, Rust quality, data-effect guards,
and a relevant generative combat/soak case. Run source-sealed commands and
retain failures. Confirm the bridge can preserve valid promotion followups;
do not send experimental orders into another owner's live native trial.
Ship accurate, green rule correction through the normal PR workflow and
verify deployment. Do not infer highest-difficulty Domination improvement
from a mechanic test alone.

Register the additional mechanical combat soak before running it: two independent
battles, fixed seeds 59010510 and 59010511, Gran Colombia versus Rome, Deity,
Online, 40x28 Battlefield, no minor seats, all victory conditions, one worker,
and an 80-turn limit. After the full test build, run:

```text
cargo run --profile ci --locked --bin civvis -- soak --games 2 --jobs 1
  --players 2 --civs "Gran Colombia,Rome" --difficulty deity --speed online
  --width 40 --height 28 --map battlefield --city-states 0 --turns 80
  --tactics-turn-limit 80 --tactics-best-of 1 --start-seed 59010510
  --victories science,culture,religious,diplomatic,domination,score
```

Retain both terminal reports and all failures. This is generated combat and
engine-stability coverage; it is not a matched native policy comparison or
evidence of improved Deity Domination wins.

The registered soak finished at 09:15:15Z on October 25 with exit zero and
its entire source seal unchanged at `dd32e038d`. Both reserved games completed:
seed 59010510 ended at turn 62 with a Rome Domination win; seed 59010511 reached
turn 80 as a draw. Retain both reports, including the losing/drawn outcomes.
The run includes no matched treatment control and no separately logged
promotion activation count; it establishes generated combat stability only.
Its build took 17m50s and was not restarted for slowness.
This local soak predates upstream #4122; that change adds a separate default-off
siege gene. The combined-source focused rules/validation cases and final hosted
checks qualify integration without claiming that the earlier soak exercised
the newly added opt-in policy.
