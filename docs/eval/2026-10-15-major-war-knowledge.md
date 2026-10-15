# A living major war whose cities have not been located

Native Emperor attempt `civvis-20261015T041838Z` declared its faith-defense
war on Sumeria at turn 47. The turn-47/frame-1 host state reported three
Sumerian cities through `public_stats.city_count`, with no city locations
visible. Cahokia joined its suzerain's war. The plan selected Cahokia as its
primary campaign, and its siege force began marching toward that minor.
This is an observed targeting diversion, not proof of the game's outcome.

The mirror already preserves the host's public empire count and derives
`Game::unseen_city_owners` when the count exceeds its located city records.
The campaign's active-war filter currently requires a located city, so it
discards the major despite that positive evidence. The existing major-first
priority then sees only the joined city-state.

The regression exercises the production assessment with a living, contacted
major at war, three publicly reported but unlocated cities, and a joined
minor with a known city. It requires the major to remain the primary rival
and leaves its city objective absent. No hidden city is created. Controls
retain the existing fallback without public evidence, a reported zero,
peace with the major, a located major city, and an eliminated major.

Checkpoint `217fe4827` ran before the repair in GitHub run `38692575247`.
After all fixture preconditions passed, the production assessment named
player 2 (the minor) instead of player 1 (the publicly surviving major).
Four other controls passed before the run stopped on that failure; the
located-city control was not reached in that fail-fast run.

The repair admits an at-war major when the existing unseen-city guard
proves it still owns cities, preserving the living-player, war and barbarian
filters. An absent location stays absent. The six repaired controls and
full integration checks remain pending.

The verified native source `048be52e8` independently reproduces the same
selection on the exact recorded turn-48/frame-0 input (snapshot 119,
6,038,677 bytes, SHA-256
`490d32cb747899887f23f62daa9b9fde6cbe4aaab41ca7a6ff67d8f1aad9d307`).
Its fresh one-shot plan names modeled player 4, Cahokia. This is a frozen
input mechanism check, not a reconstruction of persistent AI state or a
counterfactual victory. Native repair validation is being built separately.
No native conquest, win-rate improvement, or highest-level victory is
established by this repair.
