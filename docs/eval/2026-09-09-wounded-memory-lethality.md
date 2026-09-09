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

Validation is in progress. No win-rate or rescued-host-game claim is made.
