# A doomed-unit veto must reach the attack search

Investigation: cont3 turn121, pikeman4063242 (50HP at12,24) finishes
pike-and-shot4849677 (3HP at11,24), loses49HP in return, then dies to a
city strike. Historical runtime27743a332 and main-base65df99458 reproduce
the attack. With repaired post-strike accounting from #3237 and the
`doomed-blow-veto` plus `strike-reach` genes, the replay still attacks.
The host preview had predicted2 damage and64 return; these numbers also
show a separate model/host discrepancy to study.

`kill_sequence_in` computes doomed shooters and removes them from the set
released to the individual-unit planner. It nevertheless passes every
candidate to `search_kill_sequence`, then applies selected blows before
rotation. An individually valuable doomed trade can therefore execute even
with the veto enabled. The existing regression covered a case where the
search already declined the attack, so it did not expose this bypass.

The veto now removes doomed shooters' candidates before the kill search.
The regression first establishes that the gene-off planner chooses a
profitable sacrifice, then verifies that the enabled veto removes that
strike and that the full battle pass leaves the unit alive and claimed.
With the old behavior restored, it fails at the selected-blows assertion.
All 32 battle-planner tests pass with the fix.

On the saved turn121 frame, the repaired gene now orders the pikeman to
move to (12,23) and fortify. The same fixed binary with the gene off still
orders the fatal attack. These are one-shot replays, not a claim that the
host executed the retreat or that it changes the eventual game result.
The ongoing host continuation is pinned to27743a332 with refresh disabled;
it does not yet evaluate any of these repairs.

## Paired battle check

Each binary compares `advanced+battle-planner-2+strike-reach+doomed-blow-veto`
against `advanced+battle-planner-2+strike-reach`. The old binary is2041cb806
(the #3237 behavior plus the native-neutral mirrored-peace repair). The new
binary adds only this attack-selection repair on base528dd4dfd. Both use the
`ci` profile and `developer-tools`. Each has a 12-seed identical-agent
control starting900000, with exactly zero material difference on every seed.

Each composition uses100 seeds, both seatings,24 turns,28x20 maps,
separation6 and2 workers. Values are the within-version gene's mean material
advantage and standard error; these are not direct old-versus-new duels.

| Army | First seed | Before | After |
| --- | ---: | ---: | ---: |
| 2 warriors, spearman, 2 archers, horseman | 99120900 | +28.60 ±36.22 | +31.80 ±35.29 |
| 2 warriors, 4 archers | 99121100 | +36.80 ±28.33 | +47.00 ±28.29 |
| 4 warriors, 2 spearmen | 99121300 | +14.20 ±15.81 | -1.20 ±16.20 |

The results are mixed and none establishes a significant positive mean at
p<0.05. Keep the gene opt-in. The change makes its stated veto apply to the
search; it does not prove that refusing every individually doomed trade is
the best doctrine. Coordinated sacrifices, cities and campaign outcomes
need separate evaluation. Raw outputs and exact-frame replays are retained
under `civvis-tactical-evidence-20260908/selection-*` and `cont3-turn121-*`
on the verification host.
