# Strategy league (Glicko-2 ratings + selection)

`civvis league` maintains a **persistent, distributed rated pool of high-level
AI strategies** and searches it for improvements for as long as simulators keep
running: strategies earn uncertainty-aware Glicko-2 ratings on mirrored games,
high-rated ones breed offspring, and confidently weak ones retire. Multiple
machines can safely contribute to the same league without a separate
coordinator. It answers two questions the one-shot
`tournament` command cannot: *how strong is each strategy, with an
uncertainty bar, accumulated across runs* — and *what candidate should be tested
next*. Breeding and rating do not guarantee continuous strength gains; current
evidence shows genome gains can be profile-specific and score-oriented changes
can fail to produce more wins.

```bash
civvis league                      # 10 Standard-speed rounds; resumes league/
civvis league --rounds 50 --games 16 --players 6 --speed online --seed 1
civvis league --standings          # print the table without playing
```

Each invocation continues from the current checkpoint. More invocations, CPU
cores, or machines mean more claimed games and therefore faster rating and
selection—not independent leagues that must be reconciled later.

`--speed` changes both the game rules and, unless `--turns` overrides it, the
natural turn budget for that speed. It defaults to `standard` for compatibility.
Use `--speed online` when the league is meant to improve the Online-speed
spectator agent: profile-specific gains have not transferred reliably between
the two speeds.

## Continuous multi-machine workers

Put the league directory on a shared filesystem with atomic exclusive file
creation and rename (a local disk, NFS, or SMB share), then give every machine
a stable worker name. The mount path may differ on each machine:

```bash
export CIVVIS_LEAGUE_DIR=/mnt/civvis/league
export CIVVIS_WORKER_ID=render-01
civvis league --rounds 100
```

Run the same command as `render-02`, `render-03`, and so on. `--dir` and
`--worker` are equivalent one-shot overrides. All workers should use the same
simulation/selection flags and league-work protocol/build; an incompatible
binary refuses a pending manifest instead of contaminating its evidence. The
first worker to reach a new round
writes an immutable manifest containing the exact roster, genomes, game speed,
settings, maps, seats, and job IDs. Workers then:

1. atomically claim unplayed games, up to their local `--jobs` capacity;
2. simulate without holding the league lock;
3. publish immutable, validated result JSON under that job ID;
4. let any worker atomically finalize the complete rating period and breed the
   next generation.

A killed worker cannot wedge the league: its leases are reclaimed after one
hour by default (`--lease-seconds` changes this). A late duplicate is harmless
because a job ID can publish only one result, and a completed round can update
`league.json` only once. Do not use consumer cloud-sync folders whose clients
do not provide shared-filesystem exclusive-create semantics.

## Entrants

A strategy is either a built-in agent (`advanced`, `basic`, ...) or a
**parameterized AdvancedAi**: a 40-gene `Weights` genome plus an optional
fixed victory lane (`science`, `culture`, `religious`, `diplomatic`,
`domination`, `score`). A fresh league seeds itself with `advanced`,
`advanced_evolved`, `basic`, and `advanced_v1`; one parameterized strategy per
victory lane; and the embedded champion as a parameterized genome. It also
seeds `strategic` as an offline-only, non-retiring anchor. That entry lets
league rounds compare search with genomes the breeder can create, but
`league_only` prevents it from being seated in the exhibition or offered for
auto-play.

## Players

Every AI strategy is represented as a player under a **username themed to what
it plays**, using the same player identity model as a human account. It is
listed with its Elo on the leaderboard: founders keep fixed handles
(`JackOfAllTrades` = advanced, `TrainingWheels` = basic, `TechPriest` =
science lane, `Warmonger` = domination, ...) and bred offspring draw a
fresh handle from their victory lane's pool (`LabRat`, `SiegeLord`,
`PointHoarder2`, ...), so a name tells you the strategy at a glance.
Handles are unique per league and deterministic; rosters saved before
usernames existed are backfilled on load. `civvis league --standings`
prints the ranked player table — username, current Elo ± RD, strategy,
record, birth round, status.

People are players in exactly the same table. Starting a single player game
against a rated league registers a **new** player for the seat
(`league::register_player`, handles `Player`, `Player2`, ...), and the finished
game rates them by the same arithmetic that rates the agents — a person is
never handed an entrant's identity, and no entrant is ever credited with a
game a person played. The one thing a person is not is an entrant: they carry
`human: true`, and `League::active` — everything that schedules, breeds,
retires or seats — leaves them out, because nothing may play a game in a
player's name that they never sat down for. `--standings` lists them with the
status `person`. See [docs/SINGLE_PLAYER.md](SINGLE_PLAYER.md) for what the
person sees on their side of it.

## Rating: Glicko-2, rounds as rating periods

Each round deals shuffled passes over the active roster, so everyone plays a
near-equal amount. Every matchup is a mirrored series on one identical map:
the strategies rotate through every starting seat and civilization. This
paired design removes much of the spawn, civ, and first-move variance. A
requested `--games` count is rounded up to finish the final `--players`-game
mirror series.

A batch game decomposes into pairwise results by placement: equal engine
scores are draws, while a declared victory always outranks score. Those
correlated comparisons receive total weight one per player per game; a
four-player finish no longer masquerades as three independent observations
and make RD falsely precise. The whole round updates at once as one
Glicko-2 rating period (start 1500, RD 350, vol 0.06, tau 0.5; the
implementation reproduces the worked example in Glickman's paper — see
`league::tests`). Glicko-2 rather than Elo because the roster churns:
newcomers carry a wide RD and converge in a few rounds, idle or benched
strategies grow uncertain instead of stale-precise, and retirement can
demand a *confident* rating. Rating periods also make the result
independent of game order within a round, so `--jobs` never changes
ratings (there is a test for byte-identical leagues at different job
counts). A live server rating one finished game uses the same code with
idle ageing switched off — see "Watching players in the game HUD".

The league audits its own forecasts rather than assuming a lower RD means
well-calibrated probabilities. Every non-self pair records a symmetric
pre-game expectation that includes both players' RDs, then accumulates Brier
score and log loss against the actual win/draw/loss. Cumulative figures appear
in `league.json` and the standings; `calibration.csv` records both the current
period and cumulative metrics. Lower is better for both. Old snapshots begin
with an empty audit and measure forward, because reconstructing predictions
from final ratings would leak future results.

## Leader/civilization-specific ratings

Besides its overall summary, every player keeps a nested **per-leader and
per-civilization Glicko table** (`leader_elo`): leader → civilization → rating.
Both dimensions matter because one leader may lead multiple civilizations;
Eleanor/England and Eleanor/France must not share evidence. Every game compares
and updates the exact player/leader/civilization combinations that participated.

Combination tables are sparse and update only in periods actually played. A
new combination uses the player's current global rating as its prior, with an
additional 200 RD for the unknown leader/civilization effect. Its own rating is
shown after the first game and remains marked provisional until 5 games. This
avoids pretending an established 1800 player is a fresh 1500 player merely
because they selected a leader they have not played before.

- `civvis league --civ Rome` — who plays the ruleset's Rome leader best.
- `civvis league --civs` — each observed leader/civ combination's champion.

## Watching players in the game HUD

`civvis play --spectate --league league/` ranks the live-eligible strategies
for each leader/civilization, then samples from the top three with 3:2:1 rank
weight while avoiding repeats until the roster is exhausted. The spectator HUD
lists, per player: **civ, league username + strategy, its
elo** (exact leader/civ rating after game one, ±RD on hover) **and two win
odds** — the odds this seat had before a tile was drawn, and the odds it holds
right now. Both are shares of the one win a table has to give
(`elo::win_shares`), so the seats sum to 100%; averaging the pairwise
expectations instead would put every seat near 50% and could never be
checked against a winner.

The **start** figure is the pregame prediction: the ratings at this table, the
edge the roster has measured for the civilizations they drew, and what the
difficulty setting hands each seat. Nothing on the board moves it, which is what
makes it auditable — compare it against who actually wins over many games. The
**now** figure corrects that prior with the position each empire has built
(Score, military strength, cities held, the closest victory race), weighted up
as the clock runs down, and drops to zero for a seat that is out of the game.
Both come from `crate::odds`, which documents every coefficient and what
calibrates it. Without
`--league`, a `league/` dir in the working directory still labels the
default fleet with the "advanced" entrant's elo; the AIs themselves are
unchanged.

Add `--league-record` and each finished game is rated into that roster
as its own one-game rating period: the table moves as the exhibition
plays, and the next game seats from the ratings the last one produced.
Only the six seats that played are touched — a league round schedules
the whole roster, so a missing strategy really idled and its RD should
grow, but a six-seat game is not an idle period for everyone who could
not have entered it, and ageing them per game would pin the roster at
maximum uncertainty within an afternoon. The roster is re-read from disk
at the moment of recording and seats are matched by strategy *name*, so
a game long enough to outlive a concurrent update writes its result on
top rather than reverting it. A live exhibition is deliberately left unrated
while a distributed manifest is pending, because injecting a one-game period
would invalidate the in-flight roster snapshot. Results also append to `matches.csv`,
`ratings.csv`, and `calibration.csv` beside `league.json`. The live-server API
supplies a strict placement list, so only batch rounds can retain score ties.

A snapshot of a finished league lives in the repo at `data/league/`
(see its README for provenance) and is compiled into the binary, so any
build — including the WASM one, which has no filesystem — can show rated,
named players out of the box no matter which directory it was started
from. `league::shipped_league` is the reader; it is a read-only prior and
nothing is ever recorded into it. The spectator
supervisor (`tools/spectator_supervisor.py`) defaults to `--league auto`,
which seeds a runtime copy of that snapshot at the repo-root `league/`
path (gitignored) and records into it — the committed snapshot is the
starting position, not a file the exhibition rewrites. That runtime carries a
managed-roster marker, so a later build can append newly required builtin
controller families that the long-lived copy predates. Admission never resets
an existing identity or its evidence, and it waits for any current distributed
round manifest to finalize before changing membership. Explicitly named league
directories have no marker and retain their exact experimental roster. Delete
that directory to start again from the snapshot. Pass `--league off` to run
the exhibition unrated, `--no-league-record` to seat rated players
without moving their ratings, or `--league <dir>` to use a live local
league instead.

## Selection

Every `--evolve-every` rounds (default 4):

- **Breed** `max(1, --pop / 4)` offspring with quality-diversity pressure
  across seven niches: the six victory lanes plus an untargeted generalist.
  Each birth goes to the currently least-represented niche, with ties rotating
  deterministically between selection generations. One parent comes from the
  top half of that niche's full historical archive when it exists (otherwise
  the active pool), and the other from the top half of active genome carriers;
  both pools rank by conservative 95% skill (`rating - 1.96 × RD`). Thus a
  retired specialist can seed a better successor without re-entering the
  schedule, while a strong generalist contributes broadly useful genes. A
  child is a uniform crossover plus bounded mutation (the same operators
  `civvis evolve` uses), is assigned the selected niche, and enters at
  1500 ± 350 to earn its place.
- **Retire** strategies with the lowest optimistic 95% bound
  (`rating + 1.96 × RD`) until the active roster is back
  under `--pop`, but only with evidence: never anchors, never anyone with
  fewer than 20 games or RD above 110, and never the conservatively strongest
  active genome in a represented niche. Weaker duplicates remain eligible, so
  this preserves strategic coverage without freezing improvement. Retired
  strategies keep their history and genomes in the archive; only scheduling
  stops.

This explicit niche archive matters in practice: an unconstrained league can
rate generalists highly enough that they become nearly every parent, after
which probabilistic lane inheritance makes specialists rarer still. The
committed 60-round snapshot exhibited that feedback loop — all evolved active
strategies were generalists and every victory-lane specialist had retired.
Quality-diversity selection makes rating strength and strategic breadth joint
objectives instead of asking a single scalar leaderboard to provide both.

The fixed anchors (`advanced`, `basic`, and the offline-only `strategic`) are
never retired, which pins the scale and keeps the otherwise-unreachable search
controller measurable: a league leader's margin over `advanced` is comparable
across hundreds of rounds even after every non-anchor founder has been replaced.

## State on disk (`--dir`, default `league/`, gitignored)

- `league.json` — the roster: every AI player's strategy kind/genome,
  aggregate rating, leader/civ ratings, RD, volatility, record, lineage
  (`parents`), and status. The single source of truth; delete it to start a
  fresh league.
- `.civvis-managed-roster` — present only on the supervisor's mutable
  `--league auto` copy; opts that roster into required builtin admission while
  leaving explicitly constructed comparison pools untouched.
- `ratings.csv` — per-round rating history of active strategies (for
  plotting progress over time).
- `calibration.csv` — per-round and cumulative pairwise prediction count,
  Brier score, and log loss.
- `matches.csv` — every game: round, seed, end turn, victory type,
  placements.
- `work/round-N/manifest.json` — immutable roster, game speed, settings,
  mirrored schedule, and job IDs for one rating period.
- `work/round-N/results/*.json` — immutable, validated match evidence. These
  files make rating changes auditable and safely deduplicate late workers.
- `work/round-N/finalized.json` — the period's commit summary, births, and
  retirements. Temporary `claims/` entries disappear as games finish.

Everything is deterministic for a given `--seed` and build, including
across resumed invocations and worker counts (round RNGs derive from
`(seed, round)`, results are applied together, and selection runs once).

## Reading results honestly

- **Use the natural game length** (500 turns at Standard speed, 250 at Online).
  A shorter `--turns` cap makes games end as truncated score victories and
  structurally favors score-lane strategies; the first 20-round trial run
  showed exactly that collapse.
- A rating is only as settled as its RD: 1800 ± 90 vs 1700 ± 35 is not a
  confident gap. The known `advanced` vs `basic` separation (~90-120
  points) is a sanity check any healthy league should reproduce.
- Ratings are relative to the current pool; cross-league numbers are not
  comparable. The anchors are the bridge.
