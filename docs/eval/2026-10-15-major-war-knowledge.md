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

This checkpoint adds the failed-first regression before changing selection.
Execution and any repair remain pending. It establishes no native conquest,
win-rate improvement, or highest-level victory.
