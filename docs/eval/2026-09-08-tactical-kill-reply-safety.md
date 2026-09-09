# Finishing blows that survive the reply

Evidence: live verification `civvis-20260908T204713Z`, observed through turn 36
on 2026-09-08. `tools/civ6_tactics_ledger.py` reports 6 kills and 4 losses;
all four deaths occurred while defending below 50 HP. Nine of eleven wounded
military unit-turns were exposed within two hexes of a hostile. These counts
are an observation window, not a completed-game result.

At turn 33 warrior 720898 began at (28,18), 46 HP. It attacked barbarian horse
archer 4718637 at (27,18), killed it, and ended at 18 HP. Horseman 4325423,
already visible at (27,19) with 61 HP in the opening state, killed the warrior
that same turn. The enemy had zero moves in the export: next-turn reach must
refresh hostile movement. The attack was legal and executed successfully.

Two controller paths deliberately prioritize a positive direct kill over its
reply cost: `immediate_kill_value` does not price the remaining enemies, and
`advanced_military_step_with_decline` sets `lethal_kill`, which wins candidate
ordering and bypasses the usual score threshold. Subtracting reply damage
alone is insufficient: the kill reward and HP-loss penalty use different
scales, and the priority flag bypasses the score anyway.

Both paths now require the attacker to survive the direct exchange and the
roll-top incoming damage at its actual post-strike position. This reuses the
existing enemy attack envelopes and incoming-damage model. The removed target
is absent; other visible hostiles retain next-turn reach even when their
exported movement and attacks are exhausted. The frozen `advanced_v1`
tactical selector remains unchanged. Safe immediate kills retain priority.

Regression coverage exercises a wounded melee finisher against a one-HP scout
with supporting cavalry, confirms the exact strike kills and moves, confirms
the early kill pass declines it, then removes the cavalry and confirms the
same pass takes the now-safe kill. The existing major/barbarian safe-kill test
remains a positive control.

Limits: this is a core kill-priority repair, not proof of overall combat or
gene improvement. It uses the existing visible-hostile danger model, whose
estimates are conservative; coordinated elimination of all supporting enemies
can make a currently declined strike safe in a later frame. Follow-up work
must measure full-turn behavior, alternative attack routes, wounded evacuation,
idle firepower, city defense, and gene comparisons. The preceding continuation
`civvis-20260908T191919Z-cont4` had 7 losses and 2 undefended city losses,
with only 7 of 20 eligible firepower unit-turns firing; this repair does not
claim to resolve those different failures.
