# The live ladder as a screening instrument

The fleet plays ten to twenty real Civilization VI games a day and, until
2026-09-01, not one of them was an arm of anything: every ledger row's
`withheld` list was empty, and the 59 rows carrying a `forced` bundle carried
one to twenty-two tags at once, rotated by hand between batches against a
moving code revision — a before/after, never an A/B. Meanwhile the native
gene screen (`docs/GENE_SCREEN.md`) prices 2,414 games a night, and its
transfer to the host has been measured negative
(`docs/AI_GAPS.md`, "Internal ratings are not external strength"). The live
seat is the only instrument that answers the question the project asks, and
it had no way to hold a control.

This page is that instrument: a screen names ONE gene; every game of the
ladder is dealt an on/off arm of it from its own run tag; the ledger records
the arm; and `live_ledger.py screen` reads both arms back with intervals.

## What a screen is

- **One gene per screen, one arm per game.** `~/.civvis-live-screen-gene`
  holds one registry tag (or the policy file's `CIVVIS_SCREEN_GENE=` line).
  The supervisor reads it at the no-game batch boundary, exactly as it reads
  the force-on file, and passes `--screen-gene <tag>` to
  `civ6_civvis_climb.py`.
- **The arm is dealt by the tag.** `civ6_civvis_climb.screen_arm` hashes
  `<stem tag>|<gene>|<difficulty>|<lane>` (BLAKE2b) and reads the parity:
  `on` or `off`. The stem tag of `civvis-<stamp>-contN` is `civvis-<stamp>`,
  so every autosave continuation of a game inherits the game's arm by
  construction. Nobody chooses; the ledger can re-derive every arm from the
  tag; both arms are interleaved through the same days, rungs and builds.
- **The arm equal to the gene's live default is unarmed.** The other is the
  gene's one flag: `--civvis-without <tag>` for a gene that ships on (every
  repair and every host-only adapter the ledger does not hold off, and every
  opt-in the ledger turns on), `--civvis-with <tag>` for one that ships off
  (a repair the ledger holds off, or an opt-in it does not turn on).
  `python3 tools/genes.py arm <tag>` prints the live default and the flag.
- **The summary records it.** `screen_gene` and `screen_arm` (`on`/`off`)
  sit beside `forced`/`withheld` on every summary, and
  `genome_treatments` carries what the decider PLAYED (`treatments`,
  `ledger_withheld`, `forced` from its `genome` line), which `why.log` never
  brought to the ledger branch before.

## Starting and stopping a screen

    printf 'live-move-refusal-break' > ~/.civvis-live-screen-gene   # start
    rm ~/.civvis-live-screen-gene                                   # stop

No newline is needed; a trailing one is stripped as the force-on file's is,
but any OTHER whitespace, a comma-separated bundle, or a tag that is also this
batch's `--with`/`--without` drops the screen for that batch. Unlike the
force-on file, a bad screen never refuses the batch: an undealt screen is
exactly deployment, and a typo must not stop an unattended ladder. The climb
checks the tag against the registry and the ledger before any build
(`genes.live_arm`): a production gene, an unknown tag, or a gene with no live
arm is refused as a screen with a line on stderr, and the batch plays
unarmed.

The screen takes effect at the next batch boundary on each machine (the
supervisor reads the file once per batch, before build and launch). Because
`~/civvis-*.sh` home copies are what several launch paths actually invoke and
`tools/civvis_sync.sh` only reports drift, a supervisor that predates this
page must be re-synced by hand before its machine deals any arm; until then
its rows are ordinary deployment rows and the report counts them as
unassigned.

## Reading a screen

    python3 tools/live_ledger.py pull
    python3 tools/live_ledger.py screen live-move-refusal-break --since 2026-09-02
    python3 tools/live_ledger.py kpis --last 30 --gene live-move-refusal-break

Both commands read the ledger as GAMES, not rows: `<tag>-contN` segments are
joined back to their stem. The outcome, last turn and played genome are the
final segment's; the opening reading (`cities_at_60`) and every turn reading
come from the first segment that holds them; combat is summed; boosts
accumulate across segments and are read against the final tree. A game whose
stem never reached the ledger (a frozen run is killed before it publishes) is
reported as segment-only (`*` in `kpis`) and its opening readings are absent.

The arm of a game is the dealt `screen_arm` when the game was screened for
that gene — an unarmed run of a screened gene IS its default arm — else the
batch-wide `forced` (on) or `withheld` (off) word, else unassigned, which the
report counts and excludes.

Per KPI the report prints each arm's n and mean (or rate and count), the
on-minus-off difference with a 95 % interval (Welch t for means, a normal
interval over Wilson-scored rates), and `n/arm@80%`: the games per arm that
would detect the observed difference at 80 % power with a two-sided 5 % test
— the number that says how long to leave the screen running. The KPIs:

| KPI | read from | why it is here |
|---|---|---|
| kills per loss, losses per 100 turns | `summary.combat`, summed over segments | 461 combat deaths in 32 runs, 0.24–0.45 kills per loss on Emperor |
| cities at t60 | `summary.cities_at_60` | every King win came from the 4–6 band; Emperor runs sit at 2–4 |
| science ratio at t100 and t150, tech ratio at t150 | the first `state` frame of the turn: `science`/`techs` against the best rival | the t100 deficit (40–61 %) sets the space race |
| techs boosted share, civics inspired share | `summary.boosts` when the run carries it, else ever-boosted ∩ researched over the frames | 13–40 % of techs and 0–26 % of civics boosted live |
| satellite, moon, mars launch turn | the first frame the project is on `science_projects` (its order, when never completed) | the Emperor rival clock is t216 |
| abandoned at t150, reached t200, won | `summary.abandoned`, `last_turn`, `outcome` | the outcome the screen exists to move |

Low-variance KPIs converge in tens of games; the win rate needs hundreds.
Read the intermediate KPIs first, and read `n/arm@80%` before believing a
difference.

## The first three screens

1. `live-move-refusal-break` — the `did_not_move` loop: 5,873 refused
   `MOVE_TO`s across 32 runs, and the evacuation moves of dying units among
   them. Ships on (host-only); the off arm is `--civvis-without`.
2. `wounded-out-of-reach` — the recovery step reads barbarian and remembered
   raider reach. Ships off (opt-in); the on arm is `--civvis-with`.
3. `chase-every-boost` — Eureka and Inspiration hunting. Ships off (opt-in);
   the on arm is `--civvis-with`.

## Gotchas, each measured once

- `tools/ops/civvis-verified-head-launcher.sh` UNSETS `CIVVIS_WITH`,
  `CIVVIS_WITHOUT`, `CIVVIS_WITH_FILE` and now `CIVVIS_SCREEN_GENE` before
  handing over; only the policy file's honoured keys survive. Use the screen
  file or the policy key, never an export.
- The supervisor's environment is fixed at its start (measured three days
  stale); the screen file is read per batch precisely so the running host
  need not restart.
- `~/.civvis-victory-lane` overrides the lane per game and the lane is in the
  arm's hash: changing it mid-screen re-deals the coins, which is harmless to
  the arms' balance but is a second variable in the comparison — filter with
  `--lane`.
- `civvis_orders` exits 2 on a `--with`/`--without` it cannot seat, AFTER the
  supervisor has built and launched, and a force-on tag that cannot seat
  refuses the whole batch on purpose. The screen validates its tag in Python
  first and never reaches that exit.
- Every `Kind::HostOnly` gene ships ON: `enable_live_bridge_universe` turns
  on every live gene and the ledger holds off only a tag it has priced, which
  no host-only row is. `genes.py list` printed `off` for those rows because
  it printed membership in `deployment_genome`; `genes.py arm` prints the
  seat's default. The off arm of a host-only gene has always been reachable
  (`--civvis-without`); the off arm of an opt-in the ledger turns on is
  reachable from 2026-09-01 (`civvis_orders.rs::withholdable`).
- A `forced` history row (59 of them) is not a control corpus: those bundles
  rotated with the code revision. The first screen starts from zero paired
  data.
