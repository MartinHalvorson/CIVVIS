# A walled capital hid an open foothold

The four-player King Gran Colombia run `civvis-20260928T191009Z` entered a
war with Japan at turn 84. The campaign aimed at Kyoto. Its walls rose from
zero to 100 at turn 89 while its city HP stayed at 200. Our attacks through
turn 102 hit Japanese field units but did no damage to Kyoto. Nagoya was
visible and unwalled through turn 90; at turn 89 a healthy heavy chariot was
three tiles from it. Nagoya built Walls by turn 94. The prior objective was
held by `siege-commitment`, and `capture_opportunity_city` only considered an
already damaged alternative. That excluded this finite opening.

The domination campaign may now switch from a fully walled, undamaged
objective to a visible, weaker unwalled city even at full HP, provided a
healthy land taker with at least 25 strength is within three tiles. A city
already damaged still uses the previous 60-HP/30-strength taker threshold.
The old objective keeps its priority if its wall is already breached, and
the normal siege commitment pins the selected foothold afterward.

The focused regression covers a full-HP open city with a heavy chariot, a
damaged city with a man-at-arms, no nearby taker, and a breached prior
objective. The `domination_` test selection passed 139 tests. This change is
queued for the next live game; the recorded game was pinned to revision
`f53fe8a68` and cannot measure its effect.
