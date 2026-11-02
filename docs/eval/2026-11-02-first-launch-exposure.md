# First-launch research: diagnostic exposure check

The 64-game pilot remains unresolved. This follow-up uses existing seeds
202611020000–202611020002. They were initially selected with a Python
character-truthiness error: `"0"` was treated as enabled. Correct explicit
bit parsing shows that seeds 000 and 001 have the candidate OFF in every
seat; seed 002 has it ON in seat 2 (assigned Religious) and seat 4 (assigned
Science). The first enabled Science seat is 002/4; the next is 011/4.
Retain the chosen prefix without expanding it after seeing outcomes. It
provides two negative controls and one game with treatment exposure, not
additional independent strength observations. The reserved confirmation
seeds remain unused.

First replay that prefix with the original frozen binary and `--journal`
for seeds 000 and 002. Verify the full seat rows against the original pilot,
excluding elapsed seconds. Retain the exact binary hash, invocation and
headers, including any unstamped-header warning. The binary hash identifies
the executable even if its runtime source-tree lookup has moved.

The initial diagnostic exposed an instrument limitation: `drain_journal`
keeps Strategy, Military and Diplomacy but drops the Research topic. Its
first replay matches all six seat rows of seed 000, but its journal cannot
answer which research choices occurred. Preserve that failed observation
path as evidence. Add Research decisions to the diagnostic filter, verify
that it retains their reasons and cursor semantics, and replay the same
prefix while checking its genomes and game results against the originals.

If necessary, compare the same prefix with the tested tag disabled on all
seats. Before interpreting that comparison, verify that all other gene bits
and game settings match; compare by tag if the registry gained default-OFF
genes. An outcome or trace difference can show that some policy action was
affected in these games. A journal entry naming Satellites or a reached
project alone cannot establish that enabling the tag changed that choice.

No default change, ledger ingestion, or win-rate estimate will follow from
these selected diagnostic examples. Record any replay mismatch explicitly;
do not use a changed game as if it were the original pilot's missing log.

The original executable replay completed: all 18 seat rows match the pilot
apart from elapsed seconds. Its explicit `--deployment-genome` counterpart
also completed. Every realized noncandidate gene bit, civilization and game
setting matches; only the two enabled candidate bits in seed 002 are removed.
All 12 rows from seeds 000/001 match completely. Seed 002 changes its
trajectory and winner, establishing that the candidate can affect this model
profile. Both assigned-Science runs lose; their Earth/Moon timings are the
same (114/119), and Exoplanet differs 142 versus 141. These are selected-game
diagnostics with interference between seats, not an estimate of benefit or
an effect attributable to one of the two enabled seats.

The failed `--p-on 0` invocation is retained: the screen correctly requires
both arms. The supported deployment-genome probe fixes the shipped-OFF
candidate without weakening that screen guard. Neither replay is ingested
as additional strength data. Independent explicit-bit recounting reproduces
the published pilot totals: 18/106 enabled wins and 46/278 control wins;
assigned Science is 6/16 versus 14/37. The diagnostic selection mistake did
not affect those reported estimates.

The logging fix passes all 79 `gene_screen` tests, including real Research
decision/reason retention and advancing the cursor past filtered Detail
entries. The longer replay using that changed diagnostic binary is still
running; its header matches the original profile and priors, with the two
new registry tags fixed OFF. Its full realized rows must match before its
journals can be treated as the original pilot's additional observations.
