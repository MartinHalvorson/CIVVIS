# Campus above the sea: native hypothesis, 2026-11-05

The completed ordinary Deity Online Science game `civvis-20261105T173830Z`
(Indonesia, Continents Small, Gathering Storm, source
`05309b25793348f86e751bf6c66ca20766302c9a`) lost to a native Science victory
at turn 167. It lost no cities and completed two launch projects. The
failure examined here is one avoidable exposure of its research economy,
not a claim that this change would have won the game.

At turn 40 Ternate (native city 196610) had a host-quoted eight-turn Campus
with four legal sites: (17,29), (17,28), (15,29), (14,29). The first carried
native coastal-lowland band `cl=1`; the other three carried `cl=-1`.
At turn 42 the controller ordered the first site. The next native state
confirmed its Campus foundation there. The district and its Library,
University and Research Lab were subsequently completed.

At turn 138 that exact plot exported `flooded=true`. The city exported a
pillaged Campus and all three pillaged science buildings. Its repair order
was refused with `Cannot repair, location flooded.` The district and
buildings remained pillaged at the terminal turn 167. A repair-priority
change alone would not fix this host refusal.

The observation comes from the running host, not simulated sea-level timing.
`tools/civ6_control/mod/CivvisControlAgent.lua:11116` reads
`TerrainManager.GetCoastalLowlandType(plot)` and the following lines read
native flood/submergence state. Exact event, decision, summary and journal
bytes are frozen with SHA-256 receipts in the operator's
`civvis-tactics-results/2026-11-05/campus-above-the-sea/` archive.

The opt-in `campus-above-the-sea` changes a fresh native Science Campus
production order's position to the highest-yield safe legal site in the
same city. It uses the ordinary Campus yield ranking and positional tie
break among those safe sites. Civilization-specific Campus replacements
use the canonical variant mapping. An already safe proposal, active Flood
Barrier, queued Campus, existing district or paused foundation keeps its
position. If every legal offer is lowland, the original proposal survives.
Other victory lanes and the default-off controller remain unchanged.

This is a native placement experiment. It can sacrifice early adjacency to
avoid later loss of the district and its buildings. No win-rate estimate,
saved-turn estimate, population effect or default promotion is claimed.
Before promotion, verify actual changed orders and native foundations,
compare adjacency and research timing, and measure native outcomes on fresh
worlds. A fixed-state proposal probe must be labelled as such; it cannot
recover the original controller's private history.

Validation receipts will be added to the PR after the running checks finish.
