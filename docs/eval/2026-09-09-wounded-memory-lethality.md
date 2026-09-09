# A healthy unit can still be outmatched by a remembered gun

In `civvis-20260909T025914Z`, Heavy Chariot 720902 saw Field Cannon
10158089 at (38,40) on turn 165. The cannon disappeared from subsequent
visible exports. On turn 167 the Chariot began at (40,35), 80 HP, attacked
a Warrior, ended at 61 HP, and was killed by the cannon from (39,36).

The reconstructed pre-action board permits four adjacent destinations:
(40,36), (40,34), (41,34), and (41,35). The latter three increase distance
from the gun's last observed location from five to six hexes. This establishes
available movement in the native model, not acceptance of an unissued host
order or counterfactual survival.

The deployed withdrawal policy admits wounded units, visibly lethal incoming
attacks, and exposed shooters. A healthy melee unit can remain outside all
three triggers despite a powerful recently observed gun. The next experiment
is a mutually exclusive `wounded-out-of-reach-2`, retaining version one's
behavior and considering a nominal shot from one remembered gun alongside
visible incoming damage. It remains opt-in; the deployment selection is not
changed by this experiment.

The nominal shot uses the unit type's static firing strength and the known
defender's defenses. Hidden health, promotions, formation, and actual current
position are not observed. The memory radius remains an uncertainty projection,
not an exact path or line-of-sight prediction. This estimate is therefore a
policy heuristic, not a guaranteed maximum damage bound.

The new trigger uses the strongest single remembered shot plus visible
incoming damage. It does not sum a speculative army of remembered guns.
Candidate refuges retain the visible-damage ordering and then compare the
nominal remembered shot before the existing location priorities.

Five focused controls passed, covering the healthy Chariot and live prepass
reservation, weak/off/unseen/expired/peace memories, multiple weak memories,
known defenses and the naval penalty, and agreement with the actual combat
price of an unmodified visible gun. The full default-feature test suite passed
3,282 tests with 50 ignored. The gene-screen binary's 61 tests and changed-line
formatting/clippy passed. Integration with current main passed cargo check.

A same-binary persistent replay of seven frames, turn 165 frame 0 through
turn 167 frame 0, changes the Chariot's turn 166 and 167 orders from ATTACK at
(39,35) under deployed version one to MOVE_TO at (41,35) under version two.
Source `0d20ae5082c76e638c0146afbddba966fd311120` and both binary hashes are in
the local evidence artifact `memory-lethality-provenance.json`.

The standard 12-game, 72-seat full-gene screen completed on disjoint seeds
916731000–916731011, six-player Continents, Online, 250 turns, Emperor, all
victory types. Its full result is committed as
`docs/gene_screens/fires/2026-09-09-wounded-memory-lethality.json`.
The family drew 26 off seats, 28 version-one seats, and 18 version-two seats.
Version two minus version one measured +11.51 percentage points wins
(standard error 13.44); version two minus off measured +2.99 points
(standard error 13.74). Both contrasts are inconclusive. With the full gene
pool varying, these small-sample differences do not isolate the new trigger;
the persistent replay supplies the direct evidence of its changed decision.
No deployment promotion or rescued-host-game claim is made.

The PR's five-pair cost check passed: median +1.02% per completed turn,
with identical default-policy games on every tested seed. The screen kept
its original process through the host governor's pauses for local validation.
