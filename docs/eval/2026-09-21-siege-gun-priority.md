# Rejected siege-gun priority experiment

Native run `civvis-20260921T150141Z` lost to Germany by Culture on turn 154 without capturing an enemy major capital. Babylon still had 200 city health and 31 wall health at the final snapshot. The 62 wall health observed through turns 141–147 was a delay, not a permanent deadlock.

An apparent Trader detour by Bombard 6029313 on turn 144 prompted a prototype that called the existing siege doctrine before optional civilian pursuit for Domination siege guns assigned to an enemy city. Four focused priority tests and all 28 siege-train tests passed. A paired replay of all 416 completed decision frames through turn 147/2 changed **zero exported orders and zero internal actions**. The prototype is rejected and removed; this branch contains diagnosis only and must not be presented as a gameplay fix.

Diagnostic builds established that the Bombard was already controlled by siege doctrine: it belonged to Land force 149 targeting Babylon, was not claimed by the battle planner, was not guarding a civilian, and selected firing post axial (20,30). The Trader at axial (22,28) lay on the approach route. The native CAPTURE order was translated from a movement into its tile; it does not establish optional loot pursuit. The nearer firing post (22,31) was within two tiles of hostile infantry and the selected post was farther from that threat. Favoring proximity alone is not justified by this trace.

The native turn-144 movement requests failed; later movement reached (35,27), and the gun fired on an enemy unit on turn 146, verified on 147. The cause of the host movement failure and the best handling of shifting firing posts remain unresolved.

Artifacts on the investigating Mac: `/tmp/civvis-siege-gun-priority-replay/`, `/tmp/civvis-3668-comparison.json`, `/tmp/civvis-3668-rejected-prototype.tar.gz`. The comparison baseline has source-equivalent production code to d4264a2aa; the native match itself ran 5f7d7db7f. The next native match, `civvis-20260921T152749Z`, adopted d4264a2aa with the declaration-window fix.

Follow-up: German foreign tourists rose from 33 at turn 140 to 85 at 154, while our domestic tourists stayed at 32. Investigate culture-defense lead time and capital-taking tempo. Meritocracy was absent from the actual host policy menus at 110, 130, 140, and 154, so omission from the chooser is not evidence of a missed legal card in this game. Another owner is addressing the prior match’s missing Ada Lovelace district-capacity allowance in #3667; do not duplicate it.
