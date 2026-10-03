# Anonymous native aircraft effects and refusal memory

This change prevents incomplete air-combat telemetry from earning failure
cooldowns or inventing killed districts. It does not prove successful pillage,
damage to a requested plot, stronger native play, or a better terminal outcome.

## Native observations and rejected approach

The original `civvis-20261003T060034Z` King/Online/Tiny/Pangaea,
Gathering Storm/Gran Colombia Domination verification game used experimental
runtime `14c3399f3730b8c6a3488d32dcc187022549c452` and lost to Sweden's
Culture victory at turn 205. Its native events SHA-256 is
`ad52fdcadb7de9f90ddfd7167350c792408d691d537573ef7f77bf3fda901516`.

Ordinary aircraft actions do not consult `blocked_strikes`, unlike ground
strikes. Initially, a same-turn guard was proposed, and two focused regression
tests failed first. That proposal was withdrawn before any production engine
edit: Bomber `9306136` aimed at Canadian Encampment `(33,16)` at turn 196,
was refused at `06:35:43.395Z`, then received a `strike` receipt at
`06:35:45.048Z` and a matching `combat` at `06:35:46.015Z`. The latter names
the same Bomber and target, with garrison HP 100 -> 99 and walls 400 -> 367.
A whole-turn blacklist would suppress that later real strike. The discarded
test prototype and log remain in the external artifact directory below.

The shipped `Base/Assets/UI/WorldInput.lua:2077-2078` rechecks
`UnitManager.CanStartOperation` immediately before requesting `AIR_ATTACK`.
`Base/Assets/UI/MapPinManager.lua:501-518` reads CombatVis participant IDs;
they do not establish an anonymous defender's target coordinates.

At turns 183, 186, 189 and 194, our Bomber `8650780` receives a completed own
combat callback whose defender has player/id `-1/-1`, no coordinates, no
health and no damage-to-defender observation. Later same-turn exports show
movement/attack consumption, but that alone does not prove the requested
infrastructure was harmed. The Lua ledger interpreted its failed component
lookup as `gone=true`, emitted `defender_killed=true`, and the verifier then
interpreted missing target harm as `target_unharmed`. A fifth such callback
at 182 follows an earlier explicit refusal; that refusal remains a failure
under this conservative correction. No success is inferred from these IDs.

## Correction and limits

For `AIR_ATTACK` only, a same-turn own-attacker combat with a negative defender
identity makes an otherwise unattributed effect `Unverifiable`, provided
there is no explicit host refusal. Exact target combat and existing positive
target-harm checks still verify as before. Explicit refusals, ground strikes,
other attackers/turns, unknown ownership, and absent callbacks retain their
existing verdicts. A callback cannot locate a requested target and therefore
does not verify its success or clear prior failure records. Unknown effects
cannot add strikes to `HostOrderRefusals`' three-failure/ten-turn cooldown.

The mod now describes negative component identities as `type="unknown"`,
`unresolved=true`, without synthesizing coordinates, health or `gone=true`.
They consequently cannot invent a killed district/unit. Valid component
removals and health deltas remain observed normally. The canonical tactical
ledger already excludes district defenders from army-unit kill totals; this
change does not claim that its published unit-kill count was inflated by all
of these sentinel flags. It corrects the raw emitted participant and kill fact.

No game rules, action enumeration, target selection, deployment policy bundle,
mod operation arguments, or cooldown threshold are changed. There is no new
whole-turn aircraft blacklist, target permission list, or target attribution
fabricated from the pending request. Fresh native permission and actual
infrastructure postconditions remain necessary follow-up evidence.

## Recorded-order audit

The baseline and candidate are this task's own isolated ci-profile builds from
canonical `96fc1f2964658fccbc4f8526d02fcdbdb188a930` and this correction.
Orders were exported read-only from the completed games' own SQLite databases,
and `civvis_orders --mirror <original-run> --audit-orders <export>` was run
against each unchanged original event history. No AI or counterfactual game
was played by this audit. Raw inputs and reports remain available.

| Native run | Recorded AIR_ATTACK rows | Verified, before/after | Failed, before/after | Unknown, before/after | Unframed |
| --- | ---: | ---: | ---: | ---: | ---: |
| 060034Z | 33 | 21 / 21 | 12 / 8 | 0 / 4 | 0 |
| 081800Z | 127 | 58 / 58 | 67 / 19 | 0 / 48 | 2 |

The older run retains all eight `host_refused_strike` order verdicts; four
`target_unharmed` verdicts become unknown. The newer run retains all 14 explicit
refusal order verdicts, and five unattributed/no-callback `target_unharmed`
failures remain. Counts are order rows, not independent attacks, and two
terminal-turn rows in the newer game have no following frame. Its original
51 anonymous Bomber callbacks do not all qualify: the audit still gives
explicit refusals priority. None becomes an additional verified strike.

The newer `civvis-20261003T081800Z` game used experimental
`ca7f22e21ac4263e978025c4750c6a959db56f60` and lost to Rome's Science victory
at turn 236. Its events SHA-256 is
`370fcc048ff544624c2ad9bd72b73ec33166a3942bfcdb35310d2768859d07b5`.
That is not a trial of this patch. The paired audit changes retrospective
classification and demonstrates the refusal-memory input correction, not
native replacement execution, successful infrastructure damage, conquest,
survival, or victory. No full decision-prefix replay is used as proof here.

## Validation and provenance

- Six focused Rust regressions pass; two fail first on old production code.
- Five new Lua ledger assertions fail first; the final complete ledger suite
  passes, including existing valid removal and district-health controls.
- Full `cargo test --profile ci --locked`: 4,482 passed, zero failed,
  53 existing ignores (49 library, four documentation).
- All 71 discovered mod suites and all discovered Lua scripts pass using
  Lupa's Lua 5.1 runtime; CI independently runs stock Lua 5.1.
- Seven CI-wiring checks pass. Final changed-line Rust quality and independent
  CI results are recorded on the PR, not assumed from earlier skipped checks.
- No simulator-engine change; an engine soak or simulator strength test would
  not establish correctness of these native receipts. No speed claim is made.

Frozen orders binary SHA-256 values:
baseline `fa08a1fa99ebd7a4cf8b3dce97356b37edc6eae6e47f6caaf89210dfa1147dc5`;
candidate `0f29eed8341ebce15bb57ab391a6f6c49e0a7512bbfc30348e912f9f47bdae2d`.
Artifacts, discarded-guard prototype, fail-first and final logs, native evidence,
read-only order exports and audit reports persist at
`/Users/martbot-mbp-m5-max-128/civvis-tactics-results/2026-10-03/air-host-refusals-pr3876/`.

The overall goal remains active and unproven. The subsequent
`civvis-20261003T090618Z` runtime `88cfccb1c7007904bffc0f478433476fc31f73f4`
lost to Canada's Culture victory at turn 169; it did not include this patch.
Its owner started `093332Z` with frozen experimental runtime
`908b5c4fad1bea876331a2316af2e2ab3b4d2495`. No live native process, runtime
pin, game GUI, original journal, orders database, or other writer's checkout
was modified during this investigation.
