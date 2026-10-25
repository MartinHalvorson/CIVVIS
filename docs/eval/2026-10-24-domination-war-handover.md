# Existing-war Domination campaign handover

## Hypothesis and limits

The assigned-victory branch can choose Expansion during an existing major
war before the generic already-at-war Conquest branch is reached. Its normal
four-city handover also requires the elective-war power margin. A major war
that already exists need not be treated as a decision to open a new one.

Native Gran Colombia Emperor run `civvis-20261025T011023Z`, source
`7b6b83d0e5f9232fdaa955d00f5b493a237bf2f5`, first logs Conquest at turn 81.
It ultimately loses to team 2's Science victory at turn 190, with one city
taken and three lost. This motivates investigation, not a causal conclusion:
modernization and economic delivery also lag. The natural-speed clock cap is
already implemented; the text about the first half is not proof of a
650-turn-cap timing bug.

## Preregistered treatment

`domination-war-hands-over` is a new default-off opt-in. Only an effective
assigned Domination lane with at least the existing four-city opening band,
an already-active living major war, and no bounded-raid-only war may skip
Expansion for Conquest. Earlier Recovery, emergency, and imminent-victory
decisions retain priority. Peace, minors, barbarians, dead rivals, other lanes,
and prewar declaration/readiness thresholds remain unchanged.

The initial scaffold registers the flag and tests without adding its strategy
branch. Positive integration tests must first fail on Expansion versus
Conquest, not on compilation or fixture setup. Only valid failures count as
RED. Fixture amendments, if needed, remain unit-only and are recorded before
the behavioral hook is added.

## Validation and evaluation plan

1. Run an identical-binary recorded-history control through turn 70 of the
   native run before interpreting any replay treatment. Use the original
   immutable native binary and its actual 283 recorded forced policies.
   Require the replay tool's complete-reply/native control gate; do not
   replace a failed earliest control with a later matching frame.
2. Run the new strategy integration tests against the no-op scaffold, then
   with the hook, covering active major war, multiple wars with a bounded
   raid, peace, Recovery, small openings, minor/barbarian/dead opponents, and
   non-Domination lanes. Run existing handover tests, the full CI-profile
   suite, Rust quality, and generated-manifest checks.
3. For policy replay, use one candidate binary with the flag off/on and the
   same event prefixes. An executable wrapper may add the opt-in only to the
   treated process; preserve its text and both real binary hashes. A current-
   source untreated control must match the recorded native decisions before
   attributing a difference to the new flag. Replay measures decisions, not
   counterfactual survival or native wins.
4. Run the fixed same-binary focal-policy simulator comparison on Immortal
   seeds `59002500` through `59002503` and Deity seeds `59002600` through
   `59002603`: 16 complete executions, eight maps, both existing evaluator
   arms. Keep its four-major Gran Colombia, adaptive CIVVIS rivals (not
   Firaxis AI), Tiny Pangaea, six minors, Online, all victories, and natural
   250-turn clock unchanged. Record complete outcomes, capital/major-city
   milestones, and full focal journals. Count the new handover reason.
5. A batch with no active-major-war handovers is a coverage failure, not
   evidence of safety or strength. Do not provoke extra wars or alter the
   game conditions merely to make this probe activate. These are development
   cases, not a fresh holdout. No native strength claim is allowed without
   actual host victories under a registered experimental arm.
6. Keep published defaults, the deployment genome, forced-policy file, live
   pins, and native settings unchanged during this investigation. Any native
   experiment or promotion needs its own accurate registration and readback.
   Ship only a sound, truthfully evaluated change; do not call the broader
   highest-difficulty Domination objective complete from passing unit tests.

## Results

### Recorded-history observer control

The identical original-binary control finishes 190 frames through turn 70
in 8.07 seconds, with zero differences between its two replay arms. Request
identity matches all frames, but only 174/190 complete native replies match.
The native-history gate therefore fails and the command exits 2.

The first mismatch is turn 14, frame 0: the recorded and replayed scout routes
differ. Sixteen frames differ from native overall, all between turns 14 and
29. This is not a treatment failure or evidence for the new policy. It means
this history cannot support the preregistered native replay attribution.
Do not rescue the study by selecting a later matching frame, relaxing the
control, or interpreting a treated replay against this rejected baseline.

Original binary SHA-256:
`d04a2b39f80e9f87d36b3e5586d256cebcae8c44959518907f189e2747de09af`.
Recorded 283-policy force-file SHA-256:
`d7ac5367dda9dc54093ebc55f59d3630c908c577fe84309bbe5378cecec3de55`.

No new default selection, earlier capture, or win-rate improvement is claimed.

### Observer qualification amendment 1, before any treatment

The rejected history remains rejected. Before adding the behavioral hook,
qualify independent, untouched histories in this fixed chronological order:
`civvis-20261025T013135Z`, then `civvis-20261025T014329Z`. Do not scan policy
effects or terminal winners to choose a history.

Each candidate must have the original binary, complete event/decision files,
no withheld or screen policy, an initial state by turn 5, observations through
turn 70, and at least one four-city state in an existing major war by that
turn. Run the same identical-arm full-history/native-reply control and stop
at the first complete pass. Preserve failed and ineligible cases. This is
observer qualification on separate histories, not selection of a later
matching frame in the failed original or a strength holdout.

The first independent history is rejected: 158/185 native complete replies
match, request identity is valid throughout, and identical replay arms differ
in zero complete replies. The second history is ineligible under the
preregistered configuration requirements. No qualifying native observer is
available from these histories; no treated replay is interpreted.

### Initial no-op tests and fixture amendment

The original no-op scaffold at `60c16a297713c430ed0a854b48cd20ff1b11dd1e`
finishes its ten focused tests: seven pass, both intended positive cases fail
on Expansion versus Conquest, and one defensive fixture fails before its
strategy assertion. The last failure is a fixture error, not policy RED:
one nearby Swordsman is below the existing local-pressure threshold while
five friendly Warriors defend the damaged city.

Before adding the hook, amend only that fixture to use three visible nearby
Swordsmen and four distant friendly Warriors. Assert the local-pressure
floor and empire power margin separately, so city danger rather than a global
army deficit triggers Recovery. Repeat the no-op tests before implementation.
The original failed run is preserved; no native, simulator, or production
settings are changed to obtain test coverage.

The amended no-op run finishes with eight passes and exactly the two intended
Expansion-versus-Conquest failures. The threatened-city fixture now passes
with its independent visibility, local-pressure, and empire-power checks.
The validation receipt preserves both the corrected log and tested source
hashes. Only after that result, the hook is added inside the existing
Expansion branch. Its journal reason therefore identifies an actual skipped
Expansion decision; an already-selected Conquest plan keeps its prior reason.

The implemented hook at `7dc694c1916d3887b3275f0df7e62a161b738370` passes
all ten focused integration tests. Both positive war cases now select
Conquest, and all eight controls pass. The local full default CI-profile
suite also passes: 6,254 tests passed and 56 ignored.
The default selection is unchanged; these tests establish strategy behavior,
not native strength.

GitHub's Rust suite also passes all 6,254 tests, with 52 skipped and four
documentation examples ignored. The 117 developer-tool regressions pass.
The overall cargo-test job remains red at its subsequent gene-fires gate
because the new tag has no committed firing artifact. The fixed comparison
and its coverage limit are recorded below.

The hook's CI paired-cost gate passes: five pairs measure -0.50% per
completed turn, inside the instrument's +/-1% noise floor and +8% budget.
This establishes neither a speed improvement nor a regression.

### Independent native material-purchase pilot

The separately registered four-assignment `army-upgrade-materials` pilot
from #4042 is complete. Each hash-derived arm, live driver flag, actual brain
header, frozen binary, installed Lua trade setting, and native seat readback
is verified. All four use `8810924ee` and binary
`20fe763757f68a6ccf024ad8d57ab7bf6b35acdadc56842761b802d36f0b531e`.

| Native run | Arm | Rival victory | Turn | Score | Cities taken/lost | Kills/losses |
| --- | --- | --- | ---: | ---: | ---: | ---: |
| `civvis-20261025T032555Z` | off | Diplomatic | 221 | 786 | 2/1 | 69/51 |
| `civvis-20261025T034102Z` | on | Science | 198 | 687 | 1/2 | 37/24 |
| `civvis-20261025T035310Z` | on | Science | 200 | 594 | 1/3 | 27/79 |
| `civvis-20261025T040737Z` | off | Science | 217 | 362 | 0/7 | 47/45 |

There are no native wins. Two unpaired maps per arm do not establish a
causal win-rate comparison. Structured notes show zero material-buyer markers
in the first on game. The second on game has 32 `army_material_buy=1` rows,
30 no-seller holds, 14 treasury holds, and 18 in-flight-deal holds. These are
recorded note rows, not counts of completed purchases or successful upgrades.

At turn 114 of that second on game, the planner offers ten of its fifteen
Iron despite two healthy Heavy Chariots each having a native eighty-Gold,
ten-Iron Knight quote. The next turn holds seven Iron and requests three
back. This development witness motivates the separately registered reserve
repair in #4119. Keeping stock may defer profitable trades, and this witness
does not establish the repair's effect on conquest or wins.

The material-pilot archive retains all four summaries and readbacks, observer
source and start receipts, complete structured notes, selected deal/city
events, and the detailed material trace. The original four-start observer
removes its own screen without extending the cohort. No handover policy or
reserve-repair binary is played in this pilot. Published selections stay
unchanged, and the 24-game handover experiment remains pending.

### Fixed focal simulator comparison

All eight registered pairs complete, for sixteen executions. Every pair has
identical complete action records and identical terminal outcomes. The new
handover reason appears zero times in all eight treated focal journals.
Every game ends in a rival's Science victory; neither arm wins or observes
a captured foreign major city or capital.

| Difficulty | Seed | Terminal turn | Rival winner | Focal score, both arms |
| --- | ---: | ---: | ---: | ---: |
| Immortal | 59002500 | 155 | 2 | 503 |
| Immortal | 59002501 | 153 | 1 | 359 |
| Immortal | 59002502 | 140 | 3 | 93 |
| Immortal | 59002503 | 143 | 1 | 388 |
| Deity | 59002600 | 129 | 2 | 344 |
| Deity | 59002601 | 125 | 2 | 367 |
| Deity | 59002602 | 146 | 2 | 276 |
| Deity | 59002603 | 144 | 3 | 99 |

Both evaluator processes correctly exit 2 through the no-contrast guard
after completing their four pairs. The controller stops before its screen
stage. The fixed-pairs archive preserves every raw row, complete focal
journal, command, build digest, and full-suite log; no cases are replaced.
This is a coverage failure, with no measured safety or strength effect.

Continue the already registered six-game screen separately with the same
binary, frozen genome file, conditions, and seeds. Its startup reports 412
registered genes from the 447-name native header; 35 names are not carried
by this screen build. Preserve that limitation and all raw results. This
instrument measures independent seat associations and qualifies only the
artifact format. Keep #4118 in draft even if its statistic has nonzero width.
Actual full-game activation still needs separate prospective qualification;
the rejected replay controls and original fixed cases remain unchanged.

### Additional untreated native observations

Two subsequent unmodified `8810924ee` native Emperor games also lose to
Science: `civvis-20261025T021437Z` at turn 229 and
`civvis-20261025T022946Z` at turn 202. The latter logs Conquest at turn 71,
then Expansion at turn 73 with six cities and power 269 against the strongest
rival's 257, while the state records a major war. It again logs Expansion at
turn 76, power 311 against 327. This supports checking campaign continuity;
it does not establish that the new treatment would have won either game.

The accompanying native diagnostic retains strategy lines and state
projections. Carrying the last logged strategy across state turns counts
44, 4, 21, and 13 four-city major-war turns in Expansion respectively for
`011023Z`, `013135Z`, `021437Z`, and `022946Z`. These descriptive counts
do not reconstruct bounded-raid memory or prove exact treatment eligibility.

Modernization is an additional constraint. In the second newer game, the
turn-100 first frame has 256 gold and ten exported upgrade offers: eight
report missing Iron or Niter and two require friendly territory. At turn 140,
all seven offers report missing strategic resources despite 223 gold.
The diagnostic includes all first-frame offers at fixed 20-turn intervals
from turn 80 through 200 for both newer games. A larger upgrade treasury
alone cannot resolve these particular resource or territory blockers.

## Registered subsequent native experiment

After the implementation passes focused and full tests, the fixed simulator
batch finishes, and the opt-in merges, start a real-game screen through the
existing `~/.civvis-live-screen-gene` mechanism. Activation is pending. Verify
the file is unclaimed and empty or absent first; preserve any existing screen
instead of replacing it. Set only `domination-war-hands-over`. The managed
supervisor reads it between games and deals each game's on/off arm from its
run tag. Keep the current head-tracking pin, force bundle, Domination lane,
map, speed, modes, and ladder progression intact. Do not interrupt a game.

The analysis cohort is the first 24 newly assigned game stems after
activation, including abandoned attempts and continuations joined to their
stem. Preserve additional assignments as overrun data. Record the activation
time, first assigned run, actual source revision and binary digest, screen
gene and dealt arm, played genome, forced/withheld policies, configuration
readback, and all available events, decisions, and journals. Check the hash-
derived arm and actual played flag on both arms. Failed readback or missing
outcomes remain explicit missing observations rather than disappearing.

Report interim and final counts per difficulty and exact code revision,
including handover activations, capital/city gains and losses, science and
army modernization where available, combat losses, terminal outcomes, and
native Domination wins. Ordinary production games are not an off-arm cohort.
Do not pool different source revisions or rungs into a causal win-rate claim,
promote defaults from a small sample, or equate replay decisions with wins.
The initial simulator seeds remain development cases regardless of outcome.

## Additional gene-screen reachability qualification

The new registry entry must satisfy the existing gene-fires gate before it
can ship. Before implementing or evaluating the hook, register one separate
six-game reachability probe, seeds `59002700` through `59002705`, using
`gene_screen --games 6 --target-games 6 --jobs 1 --genes
domination-war-hands-over --p-on 0.5 --target-mix domination --difficulty
emperor`. Keep the instrument's six majors, 74x46 Continents, nine minors,
Online/250, and all victory conditions. Hold unscreened genes at the frozen
actual `treatments` header of untreated native `civvis-20261025T022946Z`
through `--live-genome`, rather than selecting new supporting genes.

This additional instrument qualifies registry reachability. It does not
replace, alter, or extend the preregistered eight-map focal comparison, is
not a native strength holdout, and does not select published defaults. Keep
every raw row and the complete analysis, including a zero-width result if
one occurs. A nonzero screen statistic satisfies the current ratchet's
format; it does not by itself demonstrate a causal win-rate improvement or
replace actual activation counts from the focal journals and native games.

The current `gene_screen` design draws each seat independently rather than
playing paired counterfactual games (`src/bin/gene_screen.rs`). A nonzero
association can therefore exist without proving a decision changed. Treat
that artifact as format qualification only. Actual full-game activation
requires the new handover journal reason under a verified played flag. If
the fixed focal batch supplies none, keep the tag in draft pending separately
registered native or observer qualification; do not ship it solely because
the independent screen produces variance. Do not alter the fixed batch to
manufacture exposure or reinterpret the rejected replay histories.
