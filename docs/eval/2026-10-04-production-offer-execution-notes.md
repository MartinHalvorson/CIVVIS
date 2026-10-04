# Native proposal-refusal execution correction

The native observed-player executor stopped an entire batch after an invalid
peace proposal. On consumed Deity map 61007101, the diagnostic in #3928 found
Builder 362's planned move behind that refusal on turns 63–69, while the actual
Builder executed no action. The first authoritative difference after this
repair is at turn 63: the control ends its turn, while the candidate continues
with a city production order. Proposal validation returns before creating any
pending deal, changing diplomatic access, or modifying the tactical map.

The repair continues independent orders after a refused `ProposeDeal` and
requests the usual observation refresh after the batch. Regressions verify a
worked Mine improvement and an army shot execute after a rejected peace offer,
and that an invalid access proposal creates neither a deal nor access while
leaving an army shot executable. Existing movement, direct territory-access
trade and district-placement refusal tests retain their conservative behavior.

## Production evidence

Both executables, dependency hashes, and the eight-pair fresh-pilot protocol
were frozen before treatment play. The focal public Domination controller
explicitly enables the live repair bundle; rivals retain the fixed stock
configuration. This is native simulation, with no Firaxis production claim.

Six of eight consumed candidate worlds match their controls byte for byte.
All four consumed Emperor maps have unchanged turn-75 Production. Of the
three consumed Deity maps with turn-75 checkpoints, only 61007101 changes,
from 28.6 to 29.4. That map loses early science and military power, and its
turn-150 Production falls from 67.9 to 47.7 despite more completed productive
improvements. These consumed diagnostics are not pooled into the fresh pilot.

| Fresh pilot | P at T75, control | P at T75, candidate | Paired mean change |
| --- | ---: | ---: | ---: |
| Emperor, four pairs | 51.3500 | 51.3500 | 0.00% |
| Deity, four pairs | 40.6375 | 38.0250 | -6.43% |

All 16 fresh pilot executions completed, and all focal seats were alive at
turn 75. The Deity early-production loss comes from map 61007901: the control
has five cities and 21 population at T75, versus four cities and 17 population
for the candidate; candidate military power is higher. This is an observed
tradeoff, not proof of a general strength regression or improvement. The
frozen gate required positive T75 Production at both difficulties, so the
confirmation maps remain unplayed. Faster early production remains unproven.

The runtime change is retained as a native execution correctness repair.
Future production-policy evaluation must account for its restored unit orders.
The live Civ6 bridge uses its own adapter and has not been changed here.

## Validation

Full local `cargo test --profile ci --locked -- --test-threads=4` passed 4574
with zero failures and 54 ignored, in a task-owned target. All 12 focused
executor tests passed. Incremental Rust quality passed. Corrected CI passed
4574 Rust tests, skipped 50, ignored four documentation examples, and passed
both new regressions. The initial CI failure was a test fixture assuming
closed borders before Early Empire; the recipient now explicitly learns that
civic, and the proposer remains unable to offer bilateral access. The initial
local build was interrupted after that CI finding and before tests executed;
its linked library was preserved. The corrected full local suite completed.

Raw simulation artifacts, immutable native binaries/dependencies and the test
executable are preserved under the local `civvis-production-evidence` directory.
The tracked manifests and result catalogs record their hashes and source scope.
