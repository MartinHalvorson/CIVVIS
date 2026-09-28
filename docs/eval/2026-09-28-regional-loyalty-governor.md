# Regional loyalty governor candidate — 2026-09-28

Status: experimental, default off, not demonstrated to improve whole-game outcomes.

## Recorded loss

`civvis-20260928T032649Z/events.jsonl` (SHA-256
`e49a8d3b4b8eecb57c8522cbed4c738dae1715f56aed1935757ccd13fcb46534`):
Guayaquil, native city 327684, rebelled on turn 120 without city or wall damage.
It already had Magnus and a Monument. Titles arrived separately at t102 and
t108 and funded Pingala Researcher and Connoisseur. The empire left its Dark
Age at t121. Holding observed changes fixed, adding four loyalty from t111
would leave about 33.67 at t120; that sensitivity calculation is not a native
replay or evidence of later survival.

## Decision replay that exposed the integration defect

Binary built from `e17569a46`; source log prefixes retain first decision frames,
not the later frames after the title was spent. The initial replay used the CLI's
default Science target and deployment switches, rather than the recorded
Domination configuration. It exposed the fallback spending defect, but was not
a configuration-matched replay. The corrected comparison below supplies and
reads back the recorded configuration. Replay reads snapshots and prints
orders; no orders are sent to Civilization VI.

| Case | Control | Regional candidate at e17569a46 |
| --- | --- | --- |
| t102 frame 0, source through line 7735 | Promote Pingala Researcher | Journal says save; outbound order appoints Liang |
| t108 frame 0, source through line 8392 | Promote Pingala Connoisseur | Journal says save; outbound order appoints Liang |
| t108 with the earlier title restored and Researcher removed | Promote Pingala Researcher | Appoint Victor in Caracas (native city 131073) |
| t108 after a simulated successful Victor assignment to Caracas | Not evaluated | Promote Victor Garrison Commander |

The two modified t108 cases hold all other observed state fixed, including
research previously gained. They are decision counterfactuals, not replays of
the intervening native turns or evidence that native operations succeeded.

The governor-only unit tests passed while the actual pipeline still consumed
the saved title in BasicAi's ancillary research pass. A regression through
`AdvancedAi::take_turn` failed with zero titles retained instead of one.
Therefore the reservation must also suppress that fallback governor spending.

## Replay after the fallback correction

Source revision `96f6ff3d0f69005ee9e4cb565b36cac0d0e766a6`; executable SHA-256
`4bc2678373cb7bd2b5a0e96254e69f59051bd8a6961be0ebd40d26f46161c840`. All eight case/arm invocations exited successfully.
The recorded `why.log:1` genome, still active at t102 and t108, selects
Domination and 16 forced switches. The corrected replay supplies those switches,
four players, and the recorded 650-turn clock. Its emitted genome is checked
against all 186 recorded active treatments: the control matches exactly, and
the treatment adds only `regional-loyalty-governor`. The victory target is also
read back as Domination. The earlier Science/default replay is retained as
separate evidence; it is not used as the configuration-matched comparison.

The controls still promote Pingala. With the regional gene enabled:

- Observed t102 and t108 first frames emit **no governor order**: the first
  title is now retained instead of being consumed by a fallback Liang appointment.
- The t108 saved-title counterfactual appoints Victor in Caracas (131073).
- The simulated post-appointment t108 frame promotes Garrison Commander.

This verifies the controller's complete offline decision/output path on the
recorded inputs and configuration. It does not verify native operation execution
or subsequent city survival. No live game was modified by these replays.

## Native governor evidence

Installed Gathering Storm `DLC/Expansion2/Data/Expansion1_Governors.xml:249-250`
connects Garrison Commander to `PRESTIGE_IDENTITY_PRESSURE_TO_DOMESTIC_CITIES`;
lines 873-880 specify `Amount=4` and `DomesticCities=true`.

`civvis-20260927T225002Z-cont5/events.jsonl:31` reports Online speed and Gathering
Storm; line 36 reports Victor's `turns_to_establish=3`. SHA-256:
`860a26e2996f8a601c6d36493d79722d55dce90c85b1197ae675f40692be24c8`.

The tooltip in `DLC/Expansion2/Text/en_US/Expansion1_InGameText.xml:533` says
other cities within nine tiles. The simulator explicitly includes Victor's own
city. Two native, same-turn promotion transitions support the inclusive model:

- `civvis-20260927T225002Z-cont4`, lines 10275 -> 10363, t154: Victor remains
  established in Panama (131073), gains Garrison Commander, and that city's
  loyalty rate rises 29 -> 33. Its other changed exported city field is the
  predicted religion-conversion timer. Source SHA-256:
  `b72851012c90098a8a459de5e6add95f96f640b590e53420f7abecbdda664884`.
- `civvis-20260927T203347Z-cont3`, lines 12311 -> 12425, t236: Victor remains
  established in Bogota (65536), gains the promotion, and loyalty rate rises
  29 -> 33. Other exported city changes are culture yield and defense. Source
  SHA-256: `5240ced524d6231db92a18b3ecf6ebe67293c7283aeeb1b05685417c2f0439d2`.

These were natural transitions with policy changes in the same frame, not
isolated interventions. Neither changed policy set introduces a loyalty card;
city amenities, population, buildings and current religion stay unchanged.
Retain the existing inclusive simulation rule; do not reverse it from tooltip
wording alone. This PR changes no simulation mechanic.

## Remaining validation

A subsequent complete-turn regression exposed another interaction: with two
titles and a distant, ungoverned loyalty emergency, the regional planner appoints
and promotes Victor correctly, then the baseline emergency pass immediately
moves him away. The regression failed with Victor in the distant city rather
than the regional anchor, even though Pingala could safely address the other
emergency. This is a constructed integration case, not an observed native loss.

The candidate now gives the emergency pass a preference to retain Victor while
a low-loyalty city within his aura needs the support. The pass prefers another
eligible donor, retaining its original safe-source checks. Victor remains a
fallback when no other eligible donor exists. The preference releases when the
covered city no longer needs the aura, and is absent with the candidate disabled.
The original recorded-position replays and six-game batches predate this
correction; they are evidence for their stated revisions. The new complete-turn
regressions exercise the competing-emergency behavior, and the recorded-position
matrix was repeated after the correction as described below.

After the relocation correction, all 12 focused tests pass, including the
competing-emergency regression, the sole-donor fallback, and recovery/disabled
behavior. The current library suite passes 4,207 tests with 49 ignored and the
18 previously confirmed sandbox socket-bind failures explicitly filtered out.
Those excluded tests still require a socket-permitting environment; this is not
a claim that the unrestricted full suite passed.

On code revision `b456142959bdfad3d13863ad3b6b4646b2acd56c`, the remaining
binary/integration suites also pass all 229 tests, and the changed-line Rust
quality check passes. The failing regression log and these validation logs are
preserved locally under
`~/civvis-civ6-runs/analysis/guayaquil-loyalty/relocation-regression/`.

The repeated eight-case/arm offline matrix uses the same recorded configuration
checks and input snapshots. Every invocation exits zero, its emitted active
treatment set and Domination target match the intended arm, and **all outbound
orders** match the earlier configuration-matched replay, including the title
reservation, Victor appointment, and promotion. This checks that the correction
retains the original rescue decisions; those snapshots do not contain the new
competing-emergency regression's constructed situation.

The repeated replay evidence is under
`~/civvis-civ6-runs/analysis/guayaquil-loyalty/replay/results-b45614295-recorded-domination/`.
Executable SHA-256:
`ab8650e81b390c7d8d9591efddea01e31183280c540de69413dae15b720fd241`;
`provenance.json` SHA-256:
`eec23392ce0e961014dabb4e95a9974809b466013c169e90eb51bcfdecbdef71`.
Source identity comes from the clean checkout and successful explicit Cargo
build before replay. Runtime revision readback is null for this unstamped binary;
it is recorded as such, not presented as independent source verification.

Before the relocation correction, the focused suite passed all nine tests,
including a complete turn retaining
the reserved title and a subsequent controlled-player turn releasing it after
pressure recovers. The ancillary pass continues its other work and suppresses
only fallback governor spending while the reservation is active. The 191 existing
BasicAi tests also pass (two ignored), and all three registry append-point tests
pass. The rebuilt mirror replay verifies the outbound orders as recorded above.

The full local `cargo test --profile ci --locked` attempt reached the library
suite: 4,204 passed, 18 failed, 49 ignored. Every failure was a server test
attempting to bind a socket and receiving `PermissionDenied: Operation not
permitted` in the restricted sandbox. The command exited 101 and is not a green
full-suite result. A separate `cargo test --profile ci --locked --bins --test
'*'` run completed the remaining default binary/integration suites: 229 passed,
zero failed. `cargo test --profile ci --locked --doc` also exited successfully,
with all four documentation tests ignored. Socket tests still require an
environment permitting local binds. The repository's changed-line Rust quality
check (`rust_quality.py --base 10b26acd0 --head HEAD`) passed: owned changes are
formatted and warning-free.

A six-game exploratory simulation probe completed from code revision
`96f6ff3d0`, with only this gene varied, seeds 38450000–38450005 and three workers.
The probe header records six players on 74×46 Continents, nine city-states,
Online speed, 250 turns, Prince major difficulty, Deity barbarians, and the
observed-player contract. The tested opt-in is drawn independently at p=0.25;
other genes stay at their published defaults. The header records a clean build.

Seven of 36 seats drew the candidate. Those seats won 1/7 games; the other
seats won 5/29. The analyzer reports a marginal win contrast of -2.956 percentage
points with standard error 15.641 points, and a score-share contrast of -0.237
points with standard error 2.768 points. Both are unresolved. These are seat
contrasts within six games, not 36 independent games.

The small probe is not evidence of a win-rate improvement. The current screen's
`independent` design also means that a nonzero marginal contrast is not by itself
proof of firing: chance assignment can associate an inert flag with the winner.
`tools/gene_fires.py` still describes its accepted statistics as paired. The
recorded-position control/treatment replays above provide direct decision-level
evidence separately; do not claim that a six-game contrast establishes a causal
improvement or that the firing gate repairs this design mismatch.

Natural firing evidence, opportunity-cost evaluation, generated gene metadata,
the full suite, and integration checks remain pending. No live or simulation
win-rate improvement is established by the focused tests or these replays.

A matched six-seed deployment-genome control also completed. Code inspection
shows that `--deployment-genome` pins every gene to its published default using
the same `on_probabilities` machinery. Because the candidate is default-off,
this supplies the all-off control without changing the other genes. Compare
headers and seat assignments before comparing game outcomes; the initial
`--p-on 0` invocation was rejected before any game ran, so the control uses the
supported deployment-genome mode. This is a firing/outcome-divergence check,
not an additional independent sample for estimating a win-rate improvement.

All six seed pairs have identical exported game and seat fields, including the
full exported trajectory and victory-portfolio histories, after excluding wall
time and the separately verified candidate genome bit. The comparison verifies
matching setup, civilizations, assigned victory targets, all other genome bits,
and the executable hash. Five pairs had at least one candidate-enabled seat;
seed 38450002 had none and supplies a negative control. There is **no observed
outcome or trajectory divergence** in this batch. This does not establish that
every individual action was identical: the exports are not complete action logs.
It does show why the nonzero marginal contrast above cannot establish an effect
of the candidate. Keep it default-off; direct decision replay and whole-game
benefit remain separate questions.

Both arms used executable SHA-256
`b39c803aa81fa86998c5ae001e2db3fccb31334cdbe2ffcbad7048de9615fb04`.
The control header records clean revision `4aff6e9db`, a documentation-only
successor to the treatment's clean revision `96f6ff3d0`; their executable and
gene fingerprints match. Local evidence is preserved under
`~/civvis-civ6-runs/analysis/guayaquil-loyalty/simulation-probe/`:

- `regional-loyalty-probe-6.jsonl`, SHA-256
  `ae7cd69f07e2a111af6af88a3232afa60f6e1094aefddc123f79642b5b2de82a`.
- `regional-loyalty-control-6.jsonl`, SHA-256
  `12c9c35eb657e3b45540d4b28d26c4ceddd195d9da4a1a330970f7227f92251f`.
- `regional-loyalty-paired-comparison.json`, SHA-256
  `2986b6b2a06f25217e1347de70305cdd9f2b3f0125f39345bdefb3777015b5e8`.

The sibling `compare_probes.py` compares only complete six-seat pairs and rejects
setup, binary, and noncandidate-genome mismatches. Checks using temporary copies
verified that it detects altered scores and histories, excludes an incomplete
pair, and rejects those mismatches. Synthetic check inputs were discarded and
never included in the game evidence.

## Firing-gate audit and integration requirements

A direct audit of `gene_fires.firing_evidence()` against a temporary directory
containing only the actual six-game analysis accepts `regional-loyalty-governor`
as fired. The matched comparison still reports zero exported divergence in all
six games. This reproduces the gate's design mismatch; it does not establish
that every action was identical. The artifact has not been added to the ledger
or committed firing sources. Local audit receipt:
`~/civvis-civ6-runs/analysis/guayaquil-loyalty/firing-gate-audit.json`, SHA-256
`9af98f478b5c0b673133919bc2166a8629b56428ceefbb84750b9fe64c9f9f1a`.

At revision `e3751e8e6`, `python3 tools/genes.py check` reports only
`docs/GENE_RANKING_EVIDENCE.md` as generated-file drift. The actual firing survey
reports this candidate as the sole unproven gene, with no waiver. Do not satisfy
that gate by submitting the misleading marginal contrast. Integration needs
properly scoped firing evidence or an explicit waiver explaining the uncovered
whole-game regime, plus generated metadata and remote checks. GitHub remains
unreachable, so expanding PR ownership and completing integration are pending.

## Broader city-loss audit

The five completed native games and three newer retired attempts contain 11
`city_lost` exports across 25 event logs. Recovery metadata matters: the winning
game's fifth continuation reloaded `AutoSave_0209.Civ6Save` after the fourth had
reached t234. Its earlier t217 and t223 losses belong to the discarded path.
The audit verifies every declared reload's saved turn against its continuation's
first observed state and separates superseded events. No loss lies exactly on
a reload boundary requiring manual resolution.

The retained paths contain nine recorded city losses: eight in completed games
and one in a retired attempt. Eight have a final reported loyalty and rate that
would exhaust loyalty on the next turn; all nine have an assigned governor in
the matching last-held state. Four report zero city and wall damage. These are
snapshot warnings, not independent proof of the cause of each transfer. Every
falling city's final rate is below -4, so adding four at that last snapshot would
not stabilize it. This candidate requires earlier opportunities and is not a
general solution for the observed loss pattern.

Four retained losses in the eventual winning game occur while the assigned
governor is still establishing. For two, native occupation events establish
the capture-to-loss interval directly in `civvis-20260927T225002Z-cont4/events.jsonl`:
Ulski is occupied at t192 (line 18559) and lost at t195 (19097); Apu is kept and
occupied at t203 (21360, 21397) and lost at t206 (22108). Their final rates are
-19.4297 and -20.3516 respectively. Both relevant continuations' emitted genomes
select Domination and omit `capture-hold-chain`. That existing opt-in is a
follow-up evaluation candidate, not a demonstrated fix or permission to enable
it without evaluating the recorded decisions.

The reproducible audit and its source hashes, recovery-chain records, and
per-loss snapshots are saved locally as
`~/civvis-civ6-runs/analysis/recent-city-losses.py` and `recent-city-losses.json`.
Report SHA-256:
`ed5976124c7d27552364f344a9865dee14aca80773f38fa35f8440d5e3d0b34b`.
Retired attempts remain separate from completed game outcomes.
