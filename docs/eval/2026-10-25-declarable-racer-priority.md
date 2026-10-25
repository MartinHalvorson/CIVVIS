# A declarable racer and a slower elimination war

Current status: the default-off v2 candidate is implemented and all eleven
focused development tests pass at `1bc9c751ff7933f47a5b50bbeda3c0a19b23a9d3`.
The full optimized local suite passes 6,279 tests with zero failures and 56
ignored. All 54 observational queries complete with no new air activation;
actions, plans and resulting boards match across all three arms on every
prefix. The registered game comparisons are running. This is not a native
strength claim. The
sections below retain the successive registrations and failed development
attempts; later status entries supersede earlier work-status statements.

## Native development witness

In Emperor Domination game `civvis-20261025T051724Z`, the army's journal
names Aztec as the Culture counter near the end but keeps the campaign
aimed at France's ordinary city Nantes. France has sixteen diplomatic
points at turns 200 and 202. Aztec is at peace, is legally declarable, and
has a native Culture clock of nine turns at 200 and three at 202. Aztec
then wins on Culture at turn 203. The companion archive retains selected
complete states, the full journal from turn 150, outcome and runtime/genome
receipts. It is a post-hoc development witness, not a holdout or replay.

The seat has 1,307 military power against Aztec's 2,506 at turn 202. A
different target might still fail: the game does not prove that a legal
declaration is useful, that a route can deliver suppression before the
deadline, or that an alternative campaign wins. It does expose a priority
gate worth testing.

## Existing gate and proposed comparison

`capital_moves_on::elimination_yields_to_a_launch` only releases the
Diplomatic Victory elimination front for another urgent racer already at
war with us. Its Culture branch requires the engine clock and real tourists;
its Science branch requires Mars or Exoplanet. The older shorter-clock
override additionally requires holding the diplomatic contender's original
capital. Neither releases France in this witness.

Investigate a separately reversible, default-off version
`elimination-yields-to-a-launch-2`, preserving the original implementation.
The candidate retains the existing urgent at-war behavior and considers a
different major at peace only when actual targeting/declaration constraints
allow an actionable interception. A legal declaration alone is insufficient
proof of reach or useful suppression. The design must name how it avoids
abandoning the current campaign for an unreachable or protected racer.

First establish integration fixtures for the actual plan target and
declaration path, including the one-war and campaign commitments. A helper
returning true while the army still targets France is not a successful fix.
Record genuine no-op failures before implementing the behavior. Controls
must retain a blocked declaration, no route, a long or unbacked clock, a
nonmajor or dead rival, a forced target, and a non-Domination contract. The
original version's behavior remains independently testable.

Register the final test contract and full-game evaluation cases before
behavioral edits. A trial must demonstrate actual activation, retain all
assigned cases and failures, and report native limits separately from
internal simulations. No new flag, behavioral implementation, default
selection or native experiment has been activated by this registration.

## Work status

The initial causal-path investigation is incomplete. In particular, the
target resolver's one-war and persistent campaign fallbacks may retain the
old front even after the diplomatic override yields. Trace those callers
before choosing the smallest complete intervention. The native material
trial in #4119/#4120 remains independent and must not receive this candidate
while its registered eight-start cohort is running.

## Feasibility census and registered mission diagnostic

The frozen last-eighty-terminal-game census finds eighteen first-state
frames in seven observed games with ready siege aircraft and a declarable
Culture clock of at most twenty native turns. Fifty-three selected games
have missing event files; retain them without replacement or a frequency
estimate. The screen establishes cooccurrence, not a legal or safe sortie.
The original Aztec-loss witness has two ready Bombers at turn 185 and four
later in the countdown. The one-war journal independently refuses Aztec for
the 1.5x military threshold, so releasing the diplomatic priority alone would
not constitute a complete intervention.

Before any mission result, fix all eighteen candidate frames for a model
diagnostic using the unmodified parent library at a605bd591. Freeze each
event prefix just before the first await after its first state for that turn;
reject a missing boundary or an intervening second state. Preserve exact
bytes, state and prefix hashes and all missing/failed reconstructions. This
is an observational reconstruction, not the original decider's read stream
or persistent controller state. The older game has no captured-read receipt.

For each selected racer, enumerate model-legal war declarations, apply one
to a disposable clone and try each reconstructed ready Bomber against each
known, unpillaged Theater Square owned by that racer. Record every refusal,
interception, surviving HP and actual district pillage. An accepted action
without district suppression is not success; require surviving HP >= 60 for
the diagnostic's conservative immediate-mission category. Also report legal
current-turn rebases with geometric range to known Theaters, explicitly as
staging possibilities requiring future readiness and observations, not safe
next-turn missions. Keep every frame and zero. Model permission and visible
AA cannot establish native acceptance or hidden-defense safety. No candidate
policy or live declaration is authorized by this diagnostic alone.

All eighteen prefixes froze successfully and all eighteen model probes exited
zero. Only `011023Z` turn 187 has immediate accepted sorties: one Bomber can
pillage either of two Kongo Theaters and survives at >=60 HP. This does not
qualify the complete policy's home-defense or target-selection gates. The
other seventeen frames have no accepted immediate Theater mission, including
all seven frames of the original Aztec witness. At turns 185/187 its visible
targets are out of range; later, the targets in range are not currently
visible to the model. Three games have legal current rebases with geometric
Theater range, including Aztec, but future observations, readiness, survival
and suppression remain unproved. All refusals and reconstruction warnings
are retained, including dropped Great People and unknown promotion labels.

The mission archive stores the longest prefix for each of the seven games;
each case is its registered byte-length prefix, checked against the manifest
hash. It also retains all probe outputs, build/source/library receipts and
the census's first candidate states. Reconstruct all eighteen inputs before
rerunning the driver. No success rate or alternate-victory claim follows.

The bridge translates modeled `AirPillage` and `AirStrike` to the same
`AIR_ATTACK` order (`src/bin/civvis_orders.rs:6238`); the mod rechecks
`CanStartOperation` with the exact target parameters before requesting it
(`CivvisControlAgent.lua:15887`). This diagnostic does not call that native
permission API. The evidence does not justify a broad military-power bypass
or a declaration based on rebase geometry alone. Keep the proposed version
unimplemented until a complete, observable staging/interception path and its
home-defense conditions are specified and tested.

## Conditional staging forecast, registered before evaluation

Keep the same eighteen frames and every current legal rebase from the first
diagnostic. For each in-range Theater, require its exported plot to be
currently native-visible; retain false/unknown observations as refusals.
On a disposable clone after that rebase, restore only the plane's original
ready movement/attack counts as an explicit next-turn assumption, apply one
currently legal declaration, then try the modeled sortie and record damage,
refusal and pillage. Preserve the original immediate-mission outputs exactly.

This is a conditional forecast, not future readiness or native permission.
Recheck visibility, declaration permission, defenses, base ownership and
readiness on any future real turn. The rival city export has no building
inventory, and plot exports have a district pillage bit but no foreign
building layers; a modeled district pillage does not prove that one native
sortie disables the whole Theater or delays victory. Home-defense readiness,
the natural target resolver and the actual policy's preferred air action are
also outside this physical forecast. Keep zero, unknown and failed cases;
do not use this diagnostic alone to enable a new war policy.

All eighteen conditional probes complete, and their original immediate
outputs match the first diagnostic exactly after removing the new forecast
field. Seven frames across two games have at least one conditional modeled
pillage with >=60 HP: the Kongo frame and Aztec at turns 185, 187, 194, 196,
198 and 200. These are overlapping route/aircraft alternatives, not
independent trials. Aztec turn 202 has eight forecast attempts and all are
refused; no immediate sortie is accepted there either. Macedonia's nine
geometric staging targets fail the current native-visibility requirement.

This identifies an earlier staging opportunity in the development witness,
subject to the stated future-readiness and observation assumptions. It does
not prove a native suppression, a safe second war or a win. The next policy
design must connect staging to a fresh, actually selectable interception and
account for home defense and the existing ground campaign; releasing the
diplomatic priority or power threshold in isolation is still insufficient.
The separate staging archive preserves every result, projection comparison
and build receipt without replacing the first diagnostic.


## Version 2 implementation and evaluation contract, before behavior edits

Implement `elimination-yields-to-a-launch-2` as its own default-off flag and
registry row. Retain version 1's at-war launch/clock release for either
version without weakening its predicates. Version 2 adds a physical Culture
air-interception path in a separate module; no default or native lease changes.
The earlier native development witness and eighteen baseline diagnostics are
the pre-edit no-op evidence, not holdout games.

At peace, stage at most one ready, healthy (HP >=80) siege aircraft per turn
to a currently legal friendly base. Require at least two such aircraft in the
force, a real engine-backed Culture clock no longer than forty standard turns,
real tourists, a currently visible, unpillaged Theater, legal declaration and
a clone-accepted useful pillage surviving at HP >=60 under the explicitly
assumed future readiness of the moved plane. Do not reset readiness on the
real board or declare from a staging forecast. Already useful planes keep
their position. Record the staging action and turn, not just an intention.

A fresh current-board package must contain two distinct ready aircraft whose
actual selected Theater sorties are useful, legal and survive at HP >=60
when applied sequentially after a legal declaration on a disposable clone.
The second selection observes the first result; one district cannot count
twice after it is pillaged. A single aircraft or a rebase promise cannot
release the existing front. Recheck before the declaration. The package
may redirect the current plan after the persistent campaign overlays,
including between ordinary reassessments. Real tactical selection must put
the same useful Theater mission before siege-wall or rebase alternatives.

Both staging and opening require committed Domination, no forced rival,
no Recovery plan, no threatened home city, two owned cities, the normal
turn-35 and peace-cooldown limits, a legal campaign target and affordable war.
Keep native visibility conservative (`host_observed` on a mirrored seat,
player visibility in simulation). A package permits this narrowly specified
second war without the empire-wide power/production ratios or a ground siege
bill; it does not waive the preceding safety conditions. No foreign building
inventory is invented, and modeled pillage remains distinct from native
suppression. The ground front is kept while staging; when a package is ready
its urgent rival may own the strategic target. The ordinary path remains
unchanged with the new flag off or any physical check failing.

Fixtures must use four majors, an existing ordinary-city war against a
sixteen-DVP contender, an outgunning Culture racer at peace, two planes and
two useful Theaters. First preserve the off/v1 refusal. Prove the v2 plan
names the racer after other overlays and actual diplomacy opens its legal
war; execute the selected real air actions and assert useful pillages. Test
staging then fresh readiness across turns, one-stage-per-turn, and the
visibility/readiness/base/declaration recheck. Negative controls include one
plane, spent/hurt planes, one useful district, no route, fog, strong AA,
blocked declaration, treasury, home pressure, forced target, Recovery,
non-Domination and an unbacked/long clock. Do not treat a helper-only boolean
change as success.

Before full-game observations, assign seeds 59002800 through 59002807, three
arms (off, v1, v2), six majors, 74x46 Continents, nine city-states, Online,
Emperor major difficulty, Deity barbarians and a 300-turn horizon. Freeze the
shared native deployment genome captured for the earlier diagnostics, clear
this family then vary only our seat's version. Our seat is committed to
Domination; other majors retain ordinary autonomous victory selection so a
Culture threat can develop. Run all 24 assignments and retain failures and
nonactivation. Report wins, original capitals, terminal rival wins, turns,
score, action activations and CPU separately. This is an internal development
comparison, not a native permission or highest-difficulty victory claim.
If no natural activation occurs, state that and keep the limitation visible;
do not choose replacement seeds from outcomes. A standard family fires screen
is also required before a new registry row can be shipped without a waiver.

The full-game driver is `.compare.rs`, frozen genome `.genome.json` (SHA-256 8317f5479b3d33fe545e8498cf5d53c81c7784cd62ce81e3a01c22cd583d48e7). Rotate arm order cyclically by seed, run at most two games concurrently, and retain a two-hour per-game wall deadline as failure, without replacement. Every game saves the initial/terminal board and each our-seat action/plan/reasoning update. CPU is measured by the subprocess controller, separately from wall time.


Before candidate activation, also register fresh-controller public planner
queries on every one of the same eighteen frozen native prefixes, in off,
v1 and v2 arms. Configure the same frozen development genome for all three
arms, varying only this family. Preserve all complete model boards, applied
actions, plans, reasoning, warnings and 120-second per-query failures. These
queries do not restore persistent native planner state or claim to reproduce
the original native decisions. No future readiness is introduced by this
instrument; staging must be a real action in its returned board. The v2 arm
will run only after its own library compiles. The two baseline arms are a
pre-edit diagnostic, not additional independent native games.


The pre-behavior public queries complete all 36 assignments (eighteen boards,
off/v1). Their resulting boards, actions and strategic plans match in every
pair; seventeen pairs also match all reasoning. At `032555Z` turn 205 only a
reasoning entry differs. In the original Aztec witness both arms retain
France at turns 185, 187, 194, 198, 200 and 202, without a new air rebase;
at 196 they already name Aztec. The turn-200/202 outputs independently
reproduce the second-front 1.5x refusal and France's sixteen-DVP elimination
priority under the fixed development genome. These are fresh observational
queries, not persistent native decision replays or independent trial games.

The first constructed fixture incorrectly placed two aircraft at a one-slot
City Center. Its cold test build was stopped before a test result after that
validity issue was identified; its source, raw compiler log and exact owned
process termination receipt remain attached. The corrected fixture uses a
real Aerodrome, asserts its capacity, and uses valid rear/forward bases for
staging. No pass is claimed for the stopped fixture. A separate attempt to
construct the same board in an external driver could not access crate-private
fixture helpers; the source and compile errors are retained, and no game ran.
The public native-query instrument above uses the supported mirror APIs.

The full-game driver compiles against the frozen baseline library. A clearly
marked STUB-only controller check retains all 24 assignments, including one
intentional exit-9 failure and its partial artifact, with per-child CPU usage.
Those stub records are instrumentation checks, not AI games or fires evidence.

The implementation now adds the independent opt-in flag, shares v1's unchanged
at-war release predicates, stages at most one plane per turn, preflights two
fresh selected sorties, overlays the urgent target after persistent campaign
plans, and opens only that checked package. Real useful sorties run after
base evacuation but before fire/city-assault assignments can spend the same
planes. The ordinary air selector uses the same tactical predicate. Current
library type checking passes; focused/full tests, fires evidence, new-arm
native queries and all 24 internal games remain pending. The flag remains
default-off and no native trial or deployment default is changed.

The pre-implementation archive has 1221668 bytes, SHA-256
`bf821d9e4c06802f9e6c3109b23028f023b041416c90cfb0e17227955608f81e`; all 191 member digests were verified. It contains the complete
native query outputs/build receipt and the separately labeled stopped,
rejected and stub instrumentation. Frozen native input prefixes remain in
the previously committed mission archive, with each case hash rechecked by
the query runner.

Candidate `cargo check --profile ci --locked --tests` also passes (51.27 seconds). The new registry row still lacks family fires evidence, as expected before its registered runs; this remains an explicit pre-ship requirement.

The standard six-game family firing probe is registered in `.fires-registration.json` before play: seeds 59002900–59002905, both family versions, Emperor, six majors, 74x46 Continents, nine city-states, Online/250 turns, the standard seven-way target mixture and two jobs. Its variance row is a ratchet artifact, not proof that the new interception branch activated or improved wins; direct activation is reported separately. Retain partial runs and use no replacement seeds.


## Development repairs and observation-regime check before game trials

The first CI run fails during fixture city creation: axial `(29,20)` is
outside this 36x24 map. It reaches no policy result. Correct that city to a
valid position, preserve the failure and stopped redundant local compilation,
and use an unoptimized development build for quicker runtime feedback. The
required optimized full validation remains pending. Also correct the cached
field's append range and place the new registry row after every existing row
but before the empty append-marker run, preserving existing positional bits.
A real registry append conflict with main is resolved by keeping main's
already published defender gene before this new version. Main is integrated
through `8b131f110`; fourteen append-point checks pass. Regenerating family
metadata without reselection preserves the complete 279-gene deployment list.

The first executable development pass has four passing and five failing
fixtures. Two incorrectly assert that the early one-war hold must reject an
urgent rival; that early helper exempts urgency, while the later second-front
selector and declaration path apply the independent hold. Retain the real
baseline assertion on that selector and the final declaration. Strengthen the
home-exposure fixture to actually create local pressure after declaring, and
record complete serial reasoning when the real turn fails. The rejected
single-city/double-Theater construction is repaired to use two distinct rival
towns, with both physical sorties checked independently of the finishing guard.
An unsupported synthetic wait action is a compile error, not a game result;
holding instead leaves planes unassigned and preserves their real readiness.
Temporary readiness masks are restored after early coordinated assignments,
then the existing reservation set excludes those planes from later unit work.

A second integration gap is identified by tracing the serial path: after one
plane stages, the ordinary planner can send the first ready plane back toward
the old front while the second recovers. A two-plane forecast at their current
firing positions may authorize waiting only. Opening still requires the real,
fresh package. The final-capital guard is target-sensitive: it preserves an
existing finishing rival and permits a Culture racer that itself owns the last
capital. No future readiness is written onto the real board.

`Game::culture_turns_to_victory` reads only an authoritative host observation
(`src/game.rs`); ordinary shared simulation provides none. Keep that native
eligibility requirement. The registered 24 internal games therefore test
compatibility, inherited v1 behavior and cost; they cannot establish that the
new air-interception branch fires or improves strength. The six-game family
row likewise cannot prove new-path activation. Require captured native model
queries and prospective native games for that evidence. A cheap no-clock guard
avoids city scans and speculative copies in the unsupported simulation regime.
The source amendment is recorded before any comparison game runs, retaining
every original seed and arm assignment.

The development-attempt archive preserves CI failures, partial local builds,
compile errors and registry checks separately. Early failed development runs
have raw logs but no complete frozen source snapshot; do not overstate their
reproducibility. The current source is captured by file hash while its new
unoptimized build is live, with no source edit between start and capture.

## Completed development behavior checks

At `1bc9c751f`, `cargo test --locked --lib
ai::advanced::culture_air_interception` passes all eleven tests, with no
failures or ignored tests. The complete-turn serial test stages one plane at
a time, holds the first in position through real end turns, then opens from
actual readiness and applies both useful sorties. The current-plan fixture
uses a real aged denunciation and legal Formal War after disabling generic
leader denial to isolate reassessment cadence. The serial fixture retains
the native genome's `recovery-reads-the-war`, so Recovery is assessed against
the existing weaker war rather than the stronger rival still at peace. Neither
repair waives a production gate.

Twenty physical/policy negative cases preserve the old target, including
post-declaration home pressure. Separate complete-turn controls preserve the
old version exactly when no host Culture clock exists. Final-capital controls
preserve a different finishing front while allowing the racer itself to own
the final capital. The inherited at-war Science release remains covered.
The source seal records 798 build-input file hashes before the full optimized
local test run. The repaired checkpoint is pushed; the previous paired-cost
check passes, while the family-fires ratchet remains an explicit open gate.

## Frozen candidate validation and observational nonactivation

The final source `1bc9c751ff7933f47a5b50bbeda3c0a19b23a9d3` passes
`cargo test --profile ci --locked`: 6,279 passed, zero failed, 56 ignored.
GitHub's optimized Rust tests and documentation examples also pass at that
exact head; its remaining failures are the missing new-family fires artifact,
not a gameplay-test failure. The complete required CI gate is not green.

Freeze the compiled local library and directly link the registered native-query,
comparison and gene-screen drivers with the same optimization and debug-assertion
settings. Build receipts retain exact commands, dependencies, binary/library and
source hashes, compiler identity and registry fingerprint. The whole local suite
was still running at link time, explicitly recorded in the receipts; its later
successful result is retained separately. No production source changes between
compilation and these runs.

All 54 registered fresh-controller queries complete: eighteen frozen prefixes
by off/v1/v2. Every arm has identical resulting boards, actions and plans on
every prefix. No new interception reasoning or counters appears. Counter absence
alone is insufficient because the unit-preservation adapter replays selected
actions from a disposable board; the exact action/plan/board comparisons establish
the observed no-op. These are not persistent native replays and do not establish
that the candidate helps the development witness. Investigate the exact gates
before any promotion or broader behavioral claim.

A post-hoc before-planning instrument preserves all eighteen reconstructed
boards and original aircraft readiness for that investigation. Its first compile
attempt fails on crate-private `air_capacity_at`; the corrected instrument only
exports public data. This is diagnostic instrumentation, not another evaluation
assignment or evidence of activation.

The candidate-query archive is 3,145,428 bytes, SHA-256
`7bee66a23f7e65475bc49b82b1060395f9d35fc9c71e0875ff7b228935f1bc0b`.
All 149 members are verified against their sources, including every query output,
full local test log, source/build seals, exact drivers, before-board diagnostics
and the compiler failure. All 24 internal game assignments and the six family
probe seeds are now running on the frozen `1bc9c751f` binary. Shared simulation
still lacks the host clock; its results remain compatibility/inheritance/cost
evidence and cannot rescue the absent new-air activation.
