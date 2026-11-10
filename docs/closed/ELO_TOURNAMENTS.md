# Elo tournaments (retired 2026-08-23)

> **Closed.** `civvis tournament`, `league`, `arena` and `rating` were removed
> in #2357, and the gene screen (`docs/GENE_SCREEN.md`) prices behaviours now.
> This is the section that used to sit in `docs/AI_GUIDE.md`, kept as the record
> of how those results were measured. None of its commands run against this
> tree. `docs/closed/LEAGUE.md` and `docs/closed/RATING.md` carry the instruments.

> ⚠ **Retired 2026-08-23 (#2357).** The commands in this section — `civvis
> tournament`, `civvis league`, `civvis arena`, `civvis rating` — do not run
> against this tree: the league and the Elo ledgers were removed at the
> operator's direction. The section is kept as the record of how those
> results were measured; `docs/closed/LEAGUE.md` and `docs/closed/RATING.md`
> carry the retired instruments.

```bash
civvis tournament \
  --ais advanced-20260801-diplomacy=advanced,advanced_v1,basic-20260801-diplomacy=basic,random-20260730=random \
  --games 40 --players 4
civvis tournament --standings          # verify and print without playing
```

The CLI checkpoints every completed game to the tracked
`data/elo_ratings.json` ledger (override it with `--ratings path`). Its online
rating key is the **player** — a human account or named AI strategy — accumulated
across every leader and civilization it draws. Separate
`player × leader × civilization` rows remain as matchup diagnostics. A newly
seen combination inherits that player's current Elo instead of silently
starting the established player over at the base rating.

An entrant may be written as `rating-identity=controller`. The left side is
what the ledger measures; the right side is the builtin the game constructs.
Use a new immutable identity whenever a mutable controller changes, for example
`advanced-20260815=advanced`. Otherwise a row named only `advanced` is a
lifetime average of several implementations and will dilute improvements over
time. The default command dates every mutable controller; only the deliberately
configuration-pinned `advanced_v1` keeps its bare identity and anchors successive versions on
one connected scale. After every update the ledger translates every rating by
the same amount to keep that contract-pinned control at exactly 1500. Pairwise gaps
and win expectations are unchanged, while fresh weak identities can no longer
inflate later generations relative to inactive older ones. Custom tournaments
can select another entrant with `--anchor identity`; `--anchor none` leaves a
one-off pool floating. The CLI also refuses two identities that resolve to the
same effective controller and refuses a learned entrant that silently degraded
because a definitional artifact is absent.

`advanced_v1` is not a copied historical implementation: it freezes the
victory-planning configuration but shares the underlying `BasicAi` and
`AdvancedAi` code. CI pins both source files under an explicit anchor contract.
If an edit affects the legacy path, the Elo protocol must change and the run
must use a new ledger; an edit proved to be gated away from that path still
requires an explicit review and re-pin. This guard prevents the word “frozen”
from hiding a moving control, while allowing candidate-only code to evolve.

The fog-honest city-pressure and island-settlement repairs changed the shared
legacy path, so protocol-v1 and protocol-v2 remain read-only at
`data/elo_ratings_v1.json` and `data/elo_ratings_v2.json`. The intergovernment
diplomacy pass changes that path again: both scripted controllers now make
mission, treaty, promise, and Joint-War decisions. It starts protocol v4 with
a replay-verified 40-game ledger at `data/elo_ratings.json`; the complete
protocol-v3 baseline is preserved read-only at `data/elo_ratings_v3.json`.
Historical ledgers remain useful evidence but must not be extended or mixed
with a later protocol. The protocol-v4 baseline rates
`advanced-20260801-diplomacy` at 1609.8 online Elo and 1663.6 direct Elo
against `advanced_v1`, from a 29/40 pair score (72.5%, 95% 57.2–83.9%). This is
a fresh controller-definition record, not an effect-size claim for diplomacy.

Schema 3 binds a ledger to the complete rating profile: an explicit experiment
protocol version, a deterministic fingerprint of the fully merged rules JSON,
a readable contract for every fixed lobby default, ordered controller roster,
table size, dimensions, turn limit, city-state count, speed, map
script/shape/poles, active mods, and K. The setup contract records Civ6 rules,
an Ancient start, Prince difficulty, barbarians, disaster intensity 2, no game
modes, the Civ6 leader pool, deterministic stock civilization fill, no humans,
free-for-all teams, and all six victory types. The readable mod names say what
was loaded; the fingerprint binds their actual content, so editing a mod in
place cannot reuse its old scale. A later run with any different field is
rejected with a request to use another `--ratings` path. Versioned player
identities may change while their ordered controller roles remain `advanced,
advanced_v1, basic, random`; this is what lets a new challenger join without
quietly changing its multiplayer controls. Bump the protocol when engine
behavior, implicit setup defaults, or scoring semantics change enough to
define a different contest; rules-data changes are detected automatically.
The preserved protocol-v1 ledger is a canonical 40-game,
1500-centred baseline bound to the CLI's stock 4-player Standard game (rules
`fnv1a64:3423bd46da2b8cd7`, 60×38, 500 turns, six city-states, Pangaea,
flat/poles, no mods, K=24). Protocol-v2 uses the same fixed setup with a new
40-game evidence set: `advanced-20260731` is 1570.5 online Elo and 1623.6
direct Elo against `advanced_v1`, from a 27/40 pair score (95% 52.0–79.9%, or
1514.0–1739.9 direct Elo). The historical frozen July 30 run rates
`advanced-20260730` at 1589 and the
`advanced_v1` anchor at 1500, an +89-point online gap. The order-independent
direct result is 1708, from a 31/40 pair score whose 95% interval is
62.5–87.7%, or 1589–1841 after the same monotone Elo transform. Future dated
challengers can therefore be compared through the unchanged control, with both
effect and uncertainty visible. This prevents a short smoke test, a mod, or another map size from
quietly changing what its Elo scale measures. Settings control the experiment;
versioned identities control what player generated each observation. Both are
required for a longitudinal number.

The ledger retains games and wins at both levels. If fewer entrants than seats
would cause an AI to occupy several seats, a persistent run refuses: controlling
twice as much of the map changes the contest even if the arithmetic deduplicates
the comparisons. In-memory/manual pools still defensively count a cloned player
once per world.

Every fresh schema-3 ledger also retains the raw scored table for every game,
not only the resulting point estimates. On load, the aggregates are replayed
and checked against that evidence, so a hand-edited or corrupted Elo cannot
pass as a result. Persistent events are keyed by run seed, game index, map seed,
and ordered identities. Repeating a run is idempotent; replaying the same key
with a different outcome is an error that points back to a reused mutable
identity. Writes are atomic and briefly locked per game, and keyed events are
sorted before replay, so concurrent workers preserve every result *and* finish
with the same ratings regardless of lock-acquisition order. Migrated schema-1/2
aggregates cannot recover games that were never stored and say so explicitly in
the leaderboard; start a fresh path for a fully auditable baseline. A keyed
history also has one canonical order. Every raw event must contain exactly the
profile's table size with distinct player identities, and its K must match the
bound profile; replay-consistent but structurally truncated evidence is still
rejected.

The leaderboard additionally derives a **direct performance Elo** for every
player co-seated with the fixed anchor. It converts that player's aggregate
pair score into the usual 400-point logistic scale, with a Jeffreys half-result
on each side so finite undefeated samples remain finite. This diagnostic is
order-independent and is recomputed from raw evidence. Its printed 95% Wilson
interval appears both on the observed pair-score scale and after the same
monotone Elo transform, so a short run cannot masquerade as a settled Elo gap.
An interval touching 0% or 100% correctly has an infinite Elo endpoint. Use
this direct-anchor result at a fixed game count as the longitudinal baseline;
use the incremental K-factor Elo as the continuously updated
matchmaking/leaderboard state.
On a migrated schema-1/2 pool, the heading explicitly says “post-migration” and
“retained raw games only”: that direct slice is valid for those new games, but
it excludes the unreconstructable aggregate prior and is not the standardized
full-history baseline.

Entrants use a seeded round-robin seat schedule instead of independent random
sampling. Across one complete cycle, every fixed civilization seat sees every
configured entrant exactly once. Score ties are Elo draws; an actual victory
outranks score even when the winner has fewer score points.

To price a behaviour flag, use the gene screen — every seat of every game
draws its own random genome and each gene is read as seats-on against
seats-off (`docs/GENE_SCREEN.md`, `GENE_HEURISTIC_RANKING.md`):

```bash
cargo run --release --features developer-tools --bin gene_screen -- --games 600 --jobs 8 --out screen.jsonl
cargo run --release --features developer-tools --bin gene_screen -- --analyze screen.jsonl --json screen.json
```

```rust
use civvis::elo::{
    builtin_ai, leaderboard, run_persistent_tournament, TourneyCfg,
    DEFAULT_RATINGS_PATH,
};
let names = ["mybot", "advanced_v1", "basic", "random"]
    .into_iter()
    .map(str::to_string)
    .collect::<Vec<_>>();
let pool = run_persistent_tournament(&names, |name, seed| match name {
    "mybot" => Box::new(MyBot),
    other => builtin_ai(other, seed),
}, &TourneyCfg::default(), DEFAULT_RATINGS_PATH)?;
println!("{}", leaderboard(&pool));
# Ok::<(), std::io::Error>(())
```

Multiplayer games score as simultaneous pairwise Elo results by final placement
(K/(n-1) scaling). `run_tournament` remains available for an in-memory,
non-persisted evaluation. Game generation and seating are deterministic given
`cfg.seed`; persistent ratings also depend on the ledger's prior state.

Non-tournament callers can rate human and AI players through the same API by
constructing each result with `RatedPlayer::new(player, leader, civilization,
score, won)` and passing the finished table to `EloPool::record_game`.

Use the shared ledger to prioritize low-rated player/leader/civilization rows
with meaningful sample counts, then rerun the same evaluation battery after a
strategy change. Missing rows are unmeasured, not evidence of parity.
