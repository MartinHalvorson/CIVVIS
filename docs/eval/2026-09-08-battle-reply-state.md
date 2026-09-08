# Price the battlefield after the strike

The preceding live review (`civvis-20260908T204713Z`, Rome) found wounded
warriors moving out of safety and dying to visible enemies. On turn 33, a
46-HP warrior killed a horse archer, advanced at 18 HP beside a horseman,
and died. Its decision log priced a non-finishing hit; PR #3233's
kill-priority repair alone does not establish prevention of that exact loss.

This review found three independently reproducible defects in the existing
battle planner and its `doomed-blow-veto` gene:

1. The beam search subtracted melee return damage from the attacker's HP,
   but priced the reply using its original defensive strength. Wounds reduce
   defense, so the planner underestimated the follow-up blow. Danger-field
   queries now include remaining HP, including in their cache key, and restore
   the speculative unit after pricing it.
2. The veto read the original attack stand and counted a predicted killed
   defender as a reply source. It now reads the capture tile for a melee
   finisher and excludes the eliminated target. Non-finishing blows keep the
   target in the reply field. The existing KILL_MARGIN remains the bar for
   predicting a kill; this is still an expectation model, not a guarantee
   about the host's random roll.
3. The rotation skipped garrisoned units before enforcing the veto. It now
   claims and fortifies a garrison whose available blows are all doomed,
   preventing the regular military ladder from reopening that sortie.

The existing genes are repaired in place. `doomed-blow-veto` stays opt-in.
All battle-planner versions receive the corrected remaining-HP price. This
change does not promote `strike-reach`, alter the combat engine, or claim to
solve the live controller's ordinary non-finishing-attack safety gap.

Four regressions cover HP-sensitive reply pricing and cache isolation,
a safe ranged finish versus a non-finishing shot, a melee finish out of a
safe garrison into lethal fire, and enforcement of the garrison veto.
All four fail when the old pricing/enforcement behavior is restored; the
27 pre-existing planner tests still pass under that diagnostic rollback.
With the repairs present, all 31 planner tests pass.

Native paired benchmark and broader test results follow below. These tests
are bounded evidence; actual host outcomes, city captures, fog-of-war threats,
and multi-target coordination still require their own evaluation.
