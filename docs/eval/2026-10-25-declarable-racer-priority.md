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
