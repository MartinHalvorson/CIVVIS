# Defense deadlines after small hits

**WITHHELD FROM DEPLOYMENT.** The combined command candidate changes native
orders and passes4,344 tests, but its fresh four-seed Prince pilot adds a home
capital loss and gives no wins or Domination wins. The original deadline-only
candidate also remains rejected: it changes assignments without changing any
of202 emitted decisions. Both complete negative evaluations are retained.
No native retention or difficulty promotion is proved.

## Native diagnosis

Prince run `civvis-20260926T222220Z` used source
`3d8d41e43c982b92bd703c1d503c6e86e1a081e5`, Simón Bolívar / Gran Colombia,
Tiny Pangaea, four players, Online, Expansion 2, all victory conditions enabled.
Székesfehérvár (native city262147, offset19,17) was captured on41 and lost to
Samarkand's army on76, absent from our roster at77. Loyalty stayed100.

Persistent replay of all202 observed state frames through77 using the exact
native runtime binary reproduces the allocation failure: Defend Székesfehérvár
has no force at73 (deadline21) and75 (deadline5), then four defenders at76.
Force IDs differ from the live log; the target, need, deadline and delayed
assignment agree. Other cities and the active Szeged siege compete for units.

The board records183 HP and9 recent damage at73. Its damage branch computes
ceil(183/9)=21 and skips the nearby-hostile arrival estimate used for an
undamaged city. Small initial damage can therefore postpone relief. This
mechanism is reproduced, but a better allocation and survival are not yet
established.

Walls were absent from native buildable items through73, available74 with a
five-turn estimate, selected75. An earlier70 wall order is not established as
legal. Samarkand was at peace through69 and hostile70; the commercial hub
selected68 predates that confirmed hostility.

## Preregistered evaluation

Before any policy change, retain the clean baseline source `de6a184d24208a4c19bf64c0c02004cc8d2aaebf`
and its binaries. Candidate and baseline replay the same complete observed
prefix, SHA256 `afd1616cbcdca0d5e88a470d992807524e8dca70e53800482dd423dd01b2d778`,
with the recorded20-policy force bundle and one persistent decider. Replay
orders cannot change the archived future states; this is decision evidence,
not a counterfactual city survival result.

Four Prince paired seeds37910000–37910003 run to completion for each source:

```text
victory_eval --domination-pair early-conquest-opening --difficulty prince --games 4 --start-seed 37910000 --out <fresh-file>
```

Compare all registered arms and reported outcomes, captures, city losses,
original foreign capitals and decision cost. All controllers use the selected
source; opponents are CIVVIS, not Firaxis. The off/on toggle is a within-source
pair; it does not create eight independent source-comparison seeds. Retain
negative results. No source tuning after inspecting outcomes. A harmed pilot
requires withholding the candidate until explained or resolved with new
preregistered evidence. Promotion requires native domination outcomes.

Evidence is retained outside task worktrees at
`~/civvis-war-evidence-20260926/prince-city-retention/`.

## Initial deadline-only candidate and results

Frozen candidate `9de4a7053fa34418bdcd6415c7e94246f72d90aa` takes the minimum
of the existing observed-hostile approach estimate and the measured damage
deadline. With no observed hostile nearby it preserves the damage estimate.
Two regressions fail first on `fec690a08`; all28 objective-board tests then
pass, including earlier defense-priority protection. Full local validation:
4,337 passed, zero failed, 53 existing ignores. Changed-line Rust quality
passes. This is AI scheduling, with no engine rule change.

The complete native replay changes the target's force from none to one unit
at73, none to three at74, and none to three at75. Deadline21 at73 and5 at75
both become2. All202 full decision objects remain identical across baseline
and candidate, as do the membership-only diagnostic's decisions. Reassignment
alone is therefore not an effective native retention repair.

The diagnostic identifies native Archer917510 (axial6,21) as the initial
reassigned defender. Its turn73 combat against Hungary's Heavy Chariot at
axial7,22 and movement away from the city persist. At74 the nearby native
Man-at-Arms2555923,62HP, belongs to the Settler escort rather than the defense
pool. Both the remote combat choice and civilian guard reservation require
causal follow-up; this candidate changes neither. No city survival is proved.

All eight registered paired-arm records completed for each source:

| Seed | Toggle | Before → after ending | Before → after score | Major cities ever held | Foreign original capitals ever held |
| --- | --- | --- | --- | --- | --- |
| 37910000 | off | Score250 → Score250 | 931 → 812 | 0 → 2 | 0 → 1 |
| 37910000 | on | Culture187 → Culture150 | 375 → 343 | 0 → 0 | 0 → 0 |
| 37910001 | off | Culture208 → Culture208 | 844 → 844 | 0 → 0 | 0 → 0 |
| 37910001 | on | Culture208 → Culture208 | 844 → 844 | 0 → 0 | 0 → 0 |
| 37910002 | off | Culture171 → Culture171 | 523 → 523 | 0 → 0 | 0 → 0 |
| 37910002 | on | Culture171 → Culture171 | 523 → 523 | 0 → 0 | 0 → 0 |
| 37910003 | off | Diplomatic245 → Diplomatic245 | 891 → 891 | 0 → 0 | 0 → 0 |
| 37910003 | on | Diplomatic245 → Diplomatic245 | 891 → 891 | 0 → 0 | 0 → 0 |

Every focal seat loses; no Domination win occurs. The original home capital
remains held in every arm. The foreign original capital captured by seed
37910000 off is lost again before the end (foreign capitals held at end:0). The two legs report equal outcomes on the last
three seeds; they are still paired observations, not additional independent
seeds. Profile, civilizations and forced bundle match for each source pair.
This all-controller comparison changes every simulator AI, so the first
seed's mixed result cannot identify which controller benefited or suffered.

Binary SHA256 receipts:

| Source | civvis_orders | victory_eval |
| --- | --- | --- |
| baseline de6a184d2 | 319859d31971f2f6d95e1cf1cbce896aa9a2a30d48b2a04a8a223bc68ef98eef | a7dd4662115996973f290e88b83172ba47764bc2c549bbe15bf4c9afd75a244b |
| candidate9de4a7053 | db3753fff40f412da8701703e67f5752be9e5a21e9cffba7459ef6f569435a08 | e22117f2d3a8d6fd637fcca96b7d9bfbc9c26dce7d6b6a05ce824f350c570b57 |

The complete raw results, profile checks, exact replay commands, diagnostic
patch and binary hashes are preserved in the evidence folder. Sequential
replay wall times were collected under concurrent build/pilot load and are
not a controlled cost comparison; the repository's paired-cost CI is the
performance gate. Neither simulator outcomes nor green tests justify a
native promotion or deployment of this incomplete repair.

## Command handoff: fresh evaluation registration

After the initial candidate failed to change native orders, the next phase
will make an urgent assigned defense retain a finite arrival deadline. The
kill prepass and ordinary attack scan must not repeatedly consume its travel
time at a remote front. Direct city-defense attacks and immediate survival
rotation remain available. Civilian guard reservation is a separate observed
diversion; change it only if causal replay demonstrates that the command
handoff still cannot deliver available relief.

Register fresh seeds37911000–37911003 before reading their outcomes. Reuse
the clean pre-policy baseline `de6a184d2` and freeze the combined candidate
after native decision replay and focused tests, before inspecting fresh
pilot results. Run four complete pairs per source with the same command,
replacing `--start-seed 37910000` with `--start-seed 37911000` and fresh output
paths. Retain the initial negative candidate and every registered arm.
Outcome-dependent tuning requires another fresh registration. Decision
replay against fixed future host states never establishes city survival.

The command candidate uses force formation plus the current urgent defense
deadline as a bounded due turn. A remote attack is refused when spending this
turn leaves too little time even at the unit's full observed movement allowance
to reach the existing two-hex defense stand distance. This is an optimistic
hex-travel bound, not proof of a terrain-legal arrival. Both the global kill
prepass and ordinary military scan use it. Visible military attackers that
can strike the defended city next turn remain eligible via the existing
strike-reach probe. The board-off path, recovery and movement threat scoring
retain their existing behavior.

An unfrozen command prototype changes actual unit orders at turn74 in the
202-frame persistent replay. Native Archer917510 replaces remote fire with
movement ending at offset16,21, one hex closer to Székesfehérvár. Turn73 still
allows a remote kill within the initial travel slack; turn75's unit orders
remain unchanged because the archived future still supplies the old unit
positions. This establishes a command effect, not timely arrival or retention.

The first ordinary-dispatch movement assertion incorrectly assumed that a
1HP enemy Archer made the approach safe. The defender instead moves away from
its counterfire. The regression retains an actual advance assertion on the
next turn after that hostile is removed, with a fresh movement allowance and
zero projected counterdamage on the route. That stronger assertion still
fails: the spacing score appears only within five hexes, so a six-to-five
approach acquires the whole role-depth penalty at once. The combined candidate
caps the spacing distance at five and charges the capped term outside the
ring as well, making the boundary continuous. All far positions receive the
same capped term, preserving their relative ranking. Counterdamage, recovery
and focus-target policy are unchanged. The frozen `AdvancedAi::legacy()`
control keeps the historical spacing score via its existing `legacy_movement`
gate, verified by a separate regression. This affects current ordinary force movement
as well as relief, so the fresh pilot evaluates the combined change. Final
outcome evidence and validation remain pending; the draft stays withheld.

Source9440e9e47 and its202-frame replay are retained as a preliminary
prototype. Before inspecting any fresh pilot outcomes, review adds the frozen
legacy-control guard and fixes two changed-line formatting findings. The
eligible pilot source must include those checks and receive another exact
native replay. No fresh outcomes informed this refinement.

## Combined command candidate: completed evaluation

Eligible source `668e37d647ff87d5c07dc536a7ea4a506f31de02` is frozen before
fresh outcome inspection. All35 objective-board tests and the complete local
suite pass:4,344 passed, zero failed,53 existing ignores. Changed-line Rust
quality passes. The compatibility regression preserves the frozen legacy
controller's historical movement score.

Exact eligible-source replay completes all202 frames. It changes23 full
decision objects and unit orders in14 frames, across turns42,47,49,51,53,54,
55,56,68,69 and74. Native decisions match the preliminary9440 prototype
exactly after the legacy guard and quality fixes. Turn74's Archer917510 now
ends its move at offset16,21, one hex closer to the defended city. The command
effect is established; timely arrival and city survival are not. Broader
movement changes come from the spacing correction.

All four registered pairs37911000–37911003 complete for both sources:

| Seed | Arm | Before → after ending | Score | Major cities ever observed held | Foreign original capitals held at end | Home original capital held at end |
| --- | --- | --- | --- | --- | --- | --- |
| 37911000 | off | Culture161 → Science250 | 700 → 1319 | 0 → 0 | 0 → 0 | yes → yes |
| 37911000 | on | Culture161 → Science250 | 700 → 1319 | 0 → 0 | 0 → 0 | yes → yes |
| 37911001 | off | Science240 → Culture224 | 1177 → 973 | 1 → 3 | 0 → 0 | yes → yes |
| 37911001 | on | Science241 → Science241 | 1289 → 1075 | 6 → 4 | 1 → 1 | yes → yes |
| 37911002 | off | Science242 → Religious163 | 245 → 348 | 0 → 0 | 0 → 0 | no → no |
| 37911002 | on | Science250 → Religious163 | 509 → 348 | 0 → 0 | 0 → 0 | yes → no |
| 37911003 | off | Culture219 → Diplomatic245 | 762 → 974 | 0 → 0 | 0 → 0 | yes → yes |
| 37911003 | on | Culture219 → Diplomatic245 | 762 → 974 | 0 → 0 | 0 → 0 | yes → yes |

Every focal seat loses and no Domination win occurs. Home-capital losses
increase from one to two across the paired observations. Total major cities
ever observed held and foreign capitals held at end are unchanged, with
mixed changes by arm. In37911001 on, foreign cities held at end fall8→6;
the winning rival changes from1 to2 despite the same Science241 ending.
Four seed pairs are the independent units, not eight source-comparison seeds.
Profiles, civilizations and forced focal policies match. Every simulator
controller uses the chosen source, so this comparison cannot attribute a
result change to the focal controller alone. It is insufficient to dismiss
the extra capital loss or authorize deployment.

Eligible binary SHA256 receipts:

| Binary | SHA256 |
| --- | --- |
| civvis_orders | bd3e66f51a4b57c4595f39376edee09385e40eeb4cd97bcedcbef0910a46bd91 |
| victory_eval | 05d8260dd06680f623b29b47c0f917e997385356be795c2d1328ff583bca43ba |

Raw results, the pre-inspection freeze receipt, source/binary hashes, complete
replay comparison, negative tests and final validation are retained under
`prince-city-retention/command-handoff/`. Keep PR3791 draft and withheld.

## Next registered phase: narrow the movement change

Register fresh Prince seeds37912000–37912003 for the next candidate before
reading them. The planned refinement applies spacing continuity only to an
urgent assigned city-relief body, preserving the earlier finite attack budget
and direct-defense exception. It must retain the actual safe-advance and
frozen-control regressions, then replay the same complete native prefix.
Freeze its exact source and binaries before opening fresh results. Use clean
baseline `de6a184d2`, the same four-pair command and profile, fresh output
paths, and all reported outcomes and capital-retention metrics. No next-phase
source or outcomes exist yet. The broad candidate668e and its harmful pilot
remain unchanged; further outcome-driven tuning needs another registration.
