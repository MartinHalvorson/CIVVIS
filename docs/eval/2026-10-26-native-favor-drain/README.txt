Native Favor income readback after a Diplomatic loss

The control mod now exports favor_per_turn and favor_sources directly from
Player:GetFavorPerTurn() and GetFavorPerTurnToolTip(), the same accessors used
by the shipped Expansion2 TopPanel. Each call is independently guarded. An
unavailable answer remains absent, zero remains zero, and negative rates keep
their sign. The breakdown removes icon markup, compacts spacing and is bounded
at 1,600 characters. No strategy, ballot, rules, force flag, or game setting
changes. The Rust snapshot does not consume these new diagnostic fields.

Why the measurement is needed

This is a retrospective audit of the already-known Diplomatic loss at turn
239 in civvis-20261026T014409Z. Selection was the predetermined first confirmed
Aqueduct-correction delivery, not a newly chosen successful military game.
Its executing frozen binary was ce132b317cf1acaba786372445a14e0eb23cd9de9787b9a2bc8bae50f00d5c85,
from cd06a0a3912f3b40e011c2eb1119a8700f3d22f9. The original eight-segment cohort
still includes continuations, stops, missing outcomes and source/profile drift;
none are replaced, and overlapping cohorts are not independent replication.

The final Diplomatic Victory ballot was submitted and verified correctly:
option 2, target player 1, one vote, zero Favor at ballot. Both option and
target matched the recorded ballot. The resolved A side received seven votes;
the B side received our one plus England's four. This is an underfunded denial,
not evidence of a missing or misdirected request. We do not calculate a saved
game, assume a tie rule, or assign the leader's five-point final increase to
particular resolutions without a complete point-award trace.

First exported states show Favor 227 at turn 180, 214 at 181, 200 at 182 and
zero at 194. Congress spending alone does not explain this repeated decline.
The old export contains no native per-turn Favor rate or source breakdown;
balance differences also include spending, deals and awards. Emissions,
captured capitals and grievances are observable context, not an established
attribution of the drain. The added readback lets future games answer it.

The economic ledger also deteriorated: Gold reached zero at 194 with income
-143.875/turn. At 239 it remained zero with -243.797/turn; unit, building and
district maintenance read 199, 312 and 65, and all 16 active trade routes
together paid zero Gold. Original Tenochtitlan was captured at 219 but the
Aztecs survived and reached 21 Diplomatic points. Capturing that capital did
not end their clock. The event stream reports 17 captures; occupation callbacks
include repeated ownership at Hungary's defeat and must not be counted as
17 unique callbacks or used to claim a treatment gain.

Evidence and validation

native-loss.json.gz retains a compact ledger for every turn 1-239, every one
of the 72 wc_* events, all policy readbacks, deals, occupation callbacks and
the terminal outcome. It includes the retrospective registration and exact
original archive/raw hashes. audit.py generated it from the frozen archive
after verifying both compressed and decompressed bytes. It reads no live
control files and issues no game input. Rival city counts in this ledger are
explicitly known cities, not fog-complete empire counts. Extraction v1 had
omitted wc_outcome/wc_ballot_verdict by filtering event names too loosely;
that rejected external draft remains retained. This packet includes all wc_*.

validation.json.gz retains the genuine missing-field RED failure against the
unchanged original Lua source, the same test bytes passing after the change,
and all 104 Lua mod tests passing in fresh Lua 5.1 runtimes. It also retains
syntax checks and a comparison of the heuristic scope checker with the exact
baseline: existing findings must remain identical after normalizing line
numbers. A missing system lua5.1 executable was a tooling failure before the
genuine RED; the installed lupa.lua51 runtime was used thereafter. The local
engine TopPanel path, whole-file hash and exact relevant API lines are retained.

Run: python3 docs/eval/2026-10-26-native-favor-drain/verify.py

This is evidence and observability, not a native strength result. No original
game is replayed with a changed ballot, no live game is restarted, and no
verification setup, leader, rung, force profile, lease or pin is changed.
Future live readback and behavior changes require their own qualified evidence.
