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

The fixed-input proposal probe completed twelve queries in OFF/ON/ON/OFF
order across three captured native positions, reinitializing private AI state
each time. All 204 actual registered reads matched the frozen per-case input;
complete same-arm payload repeats and one-bit effective-genome differences
were independently verified. At 42/0, ON changes only the Campus coordinates
from (17,29) to originally offered safe (15,29). Foundation 43/0 and flooded
repair 138/0 retain their orders. All other order fields remain equal.

The source was `d66b7cf2dd20abb7fea2e9ebc5de58a0e25b6118`; the executable
hash and complete inputs/results are bound in
`docs/gene_screens/fires/campus-above-the-sea.action-pairs.json` and its
archive. Recaptured byte archives are deduplicated losslessly against the
identical fixed-input blobs; each actual index retains its exact hash. This
is proposal firing evidence, not original controller-history reproduction or
native execution. The original source request had eighteen registered reads;
the constructed candidate used seventeen, and no read-count parity with the
original private controller is claimed.

Rust tests, documentation checks and isolated live-action regressions passed
in CI on that source. The treatment append-point suite passed fourteen tests.
The initial registry suite found three stale-report failures; after refresh,
those three tests passed and `tools/genes.py check` passed. All 279 deployed
genes remain exactly unchanged. Native foundation, completion and all four bounded outcomes are now documented in
`2026-11-05-campus-above-sea.native-outcomes.md`. All four games lost; one treatment
retargeted and completed its Campus. No strength gain is established.
