# A declarable racer and a slower elimination war

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
