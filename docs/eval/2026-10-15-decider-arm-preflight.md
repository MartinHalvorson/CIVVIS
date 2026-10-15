# Check the requested decider arm before a native climb attempt

Computer: MacBook Pro

## Observed failure

Native attempt `civvis-20261015T112154Z` named the Python harness checkout
`1d770be4c8e0d2b8c660a6ff648e8740c05b47da`, but an inherited
`--orders-bin` argument selected the older `d187571478e1c22d15714f340927296cb99810d4`
executable. Its actual SHA-256 was
`fe7bb9177eee057a74a1670f7427eb75da957d64ad5dae2f27f47d3f5a6cb165`.
The generic preflight passed because it invoked that executable without the
282 requested policy flags. At runtime the executable rejected
`medina-quarter-houses-the-districts`; each decision worker closed, no orders
were applied, and the native game reported our defeat at turn 6
(last completed turn 5). This is an invalid candidate evaluation.

The private experiment launcher was corrected separately to check its actual
parsed binary path, verified artifact hash, and source revision. The shared
climb also needs to reject unsupported requested arms, regardless of whether
a supplied binary comes from the harness checkout.

## Change

After any rebuild and after freezing the executable, each climb attempt now
probes that frozen binary with its requested victory lane, nonempty strategy,
forced treatments, withheld treatments, and dealt screen arm. The treatment
arguments come from the actual player command builder. A rejected arm stops
the climb before it starts the player or records a game. A compatible custom
binary remains supported.

The probe uses the actual `--serve --fresh-board` protocol with one turn
request and requires a successful exit and an orders response. On a new seat
with no recorded board, it still checks startup and the empty-board response;
it warns that no historical board was available. Empty orders are legal.

## Validation

- Both affected Python suites: **249 tests passed**, including eight new
  controls covering argument transport, a dealt screen arm, rejection before
  player launch, an unsupported arm on the first game, nonzero exits, and the
  existing orders-response contract.
- Direct readback using the actual old executable: the baseline generic
  preflight from main `73b37dfb7707e39e80d4e768f7857eac818719f5` passes; the
  repaired check rejects it with all 282 requested policy flags.
- The verified candidate executable SHA-256
  `2f9f7de0e1fb288426865962a8ce158831273e612876d1b8c0407df483e9b4e2`
  passes with the identical 282 flags. Both old rejection and candidate
  acceptance were checked with recorded history and an empty home directory.
- These readbacks invoke the decider only. They start no native UI or game.
  The regression suite separately verifies the climb refuses a failed arm
  before a player process or game record is created.

This prevents a configuration failure from consuming a verification game.
It changes no AI policy and establishes no strength or win-rate improvement.
