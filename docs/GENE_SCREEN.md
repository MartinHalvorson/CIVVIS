# Gene screen: pricing every gene from one batch of random-genome games

## The genome doctrine (operator, 2026-08-20, restated 2026-08-23)

The controller is treated as a **genome**: every behaviour flag is a gene, a
player is the set of genes it has on, and genes are tested **regularly**, not
once at birth. The vocabulary, formalized 2026-08-23: the **gene pool** is
the collection of all genes, whether on or off — the registry,
`src/ai/advanced/genes.rs`, is the pool written down. A **genome** is one
player's set of on genes, a subset of the pool; the deployment genome is the
set the ledger ships on, and every screen seat draws its own. The instrument is this screen — very large randomized batches
whose aggregate is the signal. **Every seat in every game draws its own genome
at random, independently of every other seat and every other game**; the
gene's value is the win rate of the seats that had it on against the win rate
of the seats that had it off. Because every OTHER gene is random on both
sides, a gene cannot ride on the genes it happens to be drawn beside: averaged
over thousands of random backgrounds, **each gene holds its own**.

*"We don't need the foldover pairs. We want more randomness in the tests; we
don't want exact opposite genomes being tested. Genes on/off randomly, no
complement, large batches of Monte Carlo simulation, measure win rates
above/below expected. Genes at p = ½ except for known default-on genes, which
can be p = 0.75."* — operator, 2026-08-23. That replaced the paired designs
(the foldover and its prior-weighted variant) with one rule. On 2026-08-24 the
operator moved the draw's centre onto the default genome: *"we should bias
towards picking 'on' genes as we want high level tournament competition and
want to select for genes that improve upon this performance, not some baseline
performance. Each tournament genome should default to our default genome. From
here, there should be a ¼ chance a default-on gene turns off and a ¼ chance a
default-off gene turns on. For a gene that is on at this point, there should be
a 60% chance of using the top version of the gene and a 40% chance of using a
different gene version (randomly pick among the rest). Genes with only one
version will use the one version."* That is the whole design:

| | |
|---|---|
| unit | **the seat** — one major seat in one game, carrying a genome and an outcome |
| draw | **every seat starts from the default genome**: a gene the deployment ships on stays on with **p = 0.75** (a ¼ chance of turning off), any other screened gene turns on with **p = ¼**, so the batch plays mostly the genome people actually get while every gene keeps both arms populated (`P_ON`, `P_DEFAULT_ON`; `--p-on`, `--p-default-on`); a gene that is on plays its **top version 60%** of the time and one of its other versions, drawn evenly, the other 40% (`BEST_VERSION_SHARE`) — a gene with one version plays that version |
| pairing | **none** — nothing is mirrored, complemented or matched on a map |
| estimate | seats-on minus seats-off, per gene, with every error **clustered by game** (the seats of one game share a winner) |
| adjusted | the same Δ from one regression of the seat outcome on every screened gene at once, so a gene is not credited with its neighbours' chance imbalance |

The standing cadence this implies:

1. **Screen** the full genome after each batch of landed treatments — a
   gene's price is not a constant of nature (`wide-map-capacity` measured
   **−3.4 pp** wins with all six lanes live and **+19.2 pp** with only
   `domination,score`, from the same code). That spread is why there is
   exactly ONE screen: a column only means something against a fixed world.
2. **Repair** what measurably hurts, giving the gate back the premise it
   claimed.
3. **Re-screen the repaired genes on disjoint seeds** before believing the
   fix. A repair is a hypothesis until the screen says otherwise.
4. A gene the screen flags is **confirmed by a single-gene run**
   (`--genes tag`) on disjoint seeds before it moves a default — see *Two
   stages* below.

## ⭐ ONE SCREEN (operator, 2026-08-22)

*"lets drop all the native screen / war screen stuff. we should just have 1
screen - 6p continents map for now. keep it simple. each civ gets a separate
genome and we measure the win rates across for each gene across many games when
gene is on and when gene is off."*

| leg | value |
|---|---|
| majors | **6**, each carrying its own drawn genome |
| map | **continents**, **74×46**, **9 city-states** — Civilization VI's own six-player row (`CIV6_MAP_SIZES` "small": 580 tiles and 1.5 city-states per major, three continents) |
| speed | **Online**, its own **250-turn** clock; a game that reaches it is a score victory, not a truncation |
| lanes | **all six**; no restricted-lane regime |
| civs | shuffled per map |
| majors' rung | **Emperor** — `--difficulty emperor`, the documented invocation since 2026-08-25: the live Civilization VI verification ladder plays Emperor and above, and a screen at the engine's Prince default prices genes against a slower economy than the one they are verified in. ⚠ Provenance, not an enforced leg: the code's default stays Prince (nothing was changed silently), the header records the rung, and the ledger pools both — read `difficulty` on a source before comparing two |
| barbarians | Immortal, their own rung whatever the majors play |

`gene_screen --games N --difficulty emperor --out rows.jsonl` *is* the
screen. Every **map** flag still exists, and every one of them turns a batch
into a **probe**: `tools/genes.py` refuses a source whose header does not
match the map legs of this table, so the ledger cannot quietly hold two
worlds in one column. The shape lives in `SCREEN_PLAYERS`/`SCREEN_MAP`/… in
`src/bin/gene_screen.rs` and in `SCREEN` in `tools/genes.py`, and a test
fails if the two drift apart. The three training-regime flags of 2026-08-25
(`--victory-mask`, `--difficulty`/`--difficulty-rotate`, `--rivals`) are
recorded on the source and are not legs; each has a section below.
The draw design is deliberately **not** a leg of the shape: a file written by
the earlier paired designs at this shape prices the same genes on the same
board, and the estimator reads both the same way — rows are seats.

⭐ Since 2026-08-24 one probe has a name and a section of its own: **the
contested field** (`--contested`), where rival seats are pinned to genuinely
pursue a diplomatic or culture victory so a denial gene has something to deny.
It changes no leg of the table above, is refused as a ledger source on two
header legs of its own, and is documented under *The contested field* below.

⭐ Since 2026-08-25 the screen can also play **rotating victory masks**
(`--victory-mask rotate:N`): each game closes N of the five real conditions,
drawn from its seed, and score stays on. Every lane is live across the batch,
so it is **still the standard shape** — the ledger accepts it — and each row
carries the lanes its game closed, so a lane gene can be read with its lane
open against closed. See *Rotating victory masks* below.

**What this replaced.** Two regimes: `native` (all six lanes, six players) and
`war` (`--victories domination,score`, four players), the second one added
because the 31 war and siege genes were being asked what they contribute to a
game that ended by conversion at turn 149. Their columns were never comparable
— a four-player seat wins 1-in-4 by chance against 1-in-6 — and the war regime
never entered a default, so its rows are gone rather than carried. The war
question is answered by the map instead: see below.

### Why continents

Measured on the same seeds (140000000+, 60×38, six city-states, one screened
gene so the census reads the baseline genome, on `d713b019`):

| ending | pangaea 250 | continents 250 |
|---|---:|---:|
| religious | 48% (t174) | **28%** (t181) |
| score (at the clock) | 38% | **52%** |
| culture | 11% | **18%** |
| science | 2% | 1% |
| diplomatic | 1% | 0% |
| domination | 0% | 0% |

Pangaea was a religion race that decided half its games by conversion before
anything else could pay. Continents halves that grip and doubles the games
decided on score, for **+11.9% wall per game** (34.5 → 38.6 s at that size).
That is a real de-biasing of the instrument at a price worth paying.

⚠ Those numbers are from a 60×38 six-city-state probe. **At the screen's own
74×46 nine-city-state shape the de-biasing is much larger**, measured on this
branch (seeds 170000000+, 12 pairs = 24 games, 98 genes screened):

| ending | pangaea 60×38 | continents 60×38 | **the screen (continents 74×46)** |
|---|---:|---:|---:|
| score (at the clock) | 38% | 52% | **75%** (median t250) |
| culture | 11% | 18% | **17%** (median t221) |
| religious | 48% | 28% | **8%** (median t203) |

Room is what breaks a conversion race: on the six-player map Civ VI actually
uses for six players, the faith that swept 48% of Pangaea games takes 8%. ⚠ That
last column is **24 games**, a pilot reading and not a census — it fixes the
direction and the order of magnitude, not the percentages.

**Cost at the screen's shape: ~52 CPU-seconds per game** (24 games, 1,244 CPU-s,
98 genes screened, `--jobs 10`), against 38.0 s at 60×38. So a 10,000-game
batch is **~144 CPU-hours** — about 15 hours of wall clock at ten jobs. Budget
from this number, not from the older 60×38 ones.

⚠⚠ Science and diplomatic victories land at median **t283 and t285** — past the
standard clock — so at 250 turns they are 1–2% of endings. A science or congress
gene therefore **cannot pay through the win axis**: the seat it would have
carried to a science victory shows up as a score win or a score loss instead.
Those genes pay through **score share**. Read a lane gene's share column and
its `share helps` verdict before calling it inert; this evidence informs an
explicit operator selection rather than an automatic default rule.

⚠⚠⚠ **THAT IS NO LONGER TRUE, AND THE CORRECTION IS LARGE (2026-08-25).** The
paragraph above was measured on `d713b019` in August. On the current binary the
science lane lands *inside* the clock, and at the documented Emperor rung it is
now what ends most games:

| rung | games | science | religious | culture | diplomatic | score at the clock |
|---|---:|---:|---:|---:|---:|---:|
| Prince | 24 | 33% (median t238) | 8% | 4% | 0% | 54% |
| **Emperor** | **600** | **88% (median t193)** | 8% | 3% | 0% | **0%** |

Emperor: `docs/gene_screens/2026-08-25-emperor-600-games-3600-seats.json`
(600 games, 3,600 seats, `--difficulty emperor`); Prince:
`docs/gene_screens/fires/science-victory-drive.json`. **At Emperor no game
reaches turn 250 at all** — every one of the 600 ended on a victory.

So a science gene *can* now pay through the win axis, and the standing advice
to read only its share column would have thrown away the one measurement that
mattered: `science-victory-drive` reads **+3.6 pp on wins (z +2.69) and −0.60
pp on share (z −2.48)** on that Emperor batch — a gene that helps where the
retired premise says it cannot, and hurts on the axis the premise says to read.
Read **both** columns, and read them at the rung the batch names.

### One gene is held out of the default screened set

`HELD_UNLESS_ASKED` in `gene_screen.rs` is `["joint-tactics"]`, and it is a
**cost** list, not a verdict. That gene costs **+27.3% ± 0.5% compute per
enabled major seat** (P10, 17,574 seat pairs; a direct 162-pair screen at seeds
120M read +22.5% ± 2.0%) where every other gene in P10 is inside ±1.6% and all
74 together sum to +6.0%. End to end, on the same 20 seed pairs: the full genome
runs **95.8 s/game**, and dropping this one gene runs **38.0 s/game** — a 10,000
game screen goes from **22.2 hours to 8.8**.

⚠ That is a cost argument and not a claim that the gene does nothing. Its win
columns are +3 / −4 — inside any band this instrument has printed — but P10
reads it `share HELPS **` at **share z +3.84**, past that screen's family-wise
bar of 3.403, the strongest share reading among the default-off genes. The
former deployment rule read the **win** axis only. Its share reading is now
evidence for a deliberate operator decision or an arm (`--genes joint-tactics`),
not an automatic promotion. It is default-off today, so holding it out changes
nothing the agent plays.
⚠ Under `--design prior` a `helps` gene draws on for 90% of seats, and this one
is `helps` on its share axis — check the `prior:` column in `--list` before
running a prior-weighted screen that carries it.

Today's genome is the boolean treatment flags below. The growth direction is
"hundreds of genes": the remaining `enable_*`/`disable_*` toggle pairs in
`treatment_flags.rs` (182 exist), and — for the continuous `Weights` — the
binarized genes `gene_census` already studies.

`gene_screen` (`src/bin/gene_screen.rs`) treats every boolean treatment flag on
the Advanced controller as a **gene** — on or off — and prices all of them from
ONE batch of games instead of one arm per flag. It answers, per gene and with
an interval, the question every treatment PR ends on: *does the agent win more
with this on than off?*

```sh
cargo build --profile ci --bin gene_screen
target/ci/gene_screen --list                                # the genes, in bit order (64 on 2026-08-19)
target/ci/gene_screen --games 600 --jobs 8 --out screen.jsonl
                                                            # ↑ THE screen: no profile flags
target/ci/gene_screen --analyze screen.jsonl [more.jsonl ...]  # re-read, merge, re-table
```

## ⭐ A screen carries the binary it played, and a source proves it (2026-08-23)

A column is a claim about code. Until this landed, nothing tied a column to the
code that produced it, and the failure that follows from that has happened
**three times in two days**:

| when | what happened |
|---|---|
| **2026-08-22, P10** | #2266 culled ten genes. P10's simulation binary (`d23f92d9`) was built **1h43m before that merge**, so the batch was already in flight and published a **+63** column for `holy-lane-parity` after the gene's code was gone. The reading turned out to be real: the gene was restored (#2299) and confirmed directly at **+99, z +4.05** (#2307). The project got the right answer from a careful reader, not from a gate. |
| **2026-08-22, #2307** | The direct arm's write-up states its source commit and its release binary's SHA-256 **in prose**, in a Markdown header line, because the analysis artefact had nowhere structured to put them. `docs/eval/2026-08-22-standard-gene-screen-23622-paired-seats.md` does the same. |
| **2026-08-23** | The first standard-shape screen re-priced `barbarian-hunt` from the legacy **−1.73 pp** to **+0.20 pp** (z +0.65) while a sibling change was minutes from deleting that gene on the legacy reading — which would have made a brand-new screen a source pricing a gene the code no longer had. |

### What the header now carries

`gene_screen` stamps every batch before its first game and prints the stamp on
the first line, so an eight-hour run the ledger would refuse says so at the
start rather than at the end:

```text
build: 4391f51c835f (build-tree) · 100 genes sha e6015634ff76 · binary sha b59c4c4e0648
```

| field | what it is |
|---|---|
| `commit` | the revision the binary was built from |
| `commit_source` | `env` (`CIVVIS_COMMIT`, which every supervisor already sets), `binary-name` (a promoted `civvis-<40-hex>` executable), `build-tree` (Git where the crate was compiled), or `unstamped` |
| `dirty` | tracked changes under `src`, `Cargo.toml`, `Cargo.lock`, `build.rs` or `data` — everything a played game comes out of |
| `genes_sha256` | ⭐ sha256 over the gene tags **compiled into this binary**, in header order |
| `binary_sha256` | sha256 of the executable's own bytes — the field #2307 wrote by hand |

**`genes_sha256` is the load-bearing one, and it is the only one that cannot go
stale.** It is hashed from `gene_table()` as compiled, never read back from a
file, an environment variable, or a working tree. A commit can be misreported;
the gene set of the running code cannot. Everything else is defended by
refusing to guess: `CIVVIS_COMMIT` first, then the promoted executable's own
name, then the build tree — and the build tree is dropped entirely when the
tree took a commit touching a build input **after** this executable was linked.
The revision is deliberately *not* baked in at compile time: #892 removed that
because it forced a full optimized rebuild for every promoted HEAD, and
`build.rs` is empty for exactly this reason.

### What the ledger refuses

`tools/genes.py` re-derives the gene tags at the commit a source claims —
every screenable row of the registry `src/ai/advanced/genes.rs` in order,
which is exactly what `gene_table()` builds (and, for a commit older than the
registry, the three tables that preceded it) — and refuses the source when the
fingerprints differ, **in either direction**:

- a gene **priced here and absent at that commit** — P10's shape;
- a gene **present at that commit and never compiled in** — which is what an
  unmeasured gene quietly looks like.

It also refuses an unstamped build, a dirty one, a commit this clone cannot
read (fetch it; a claim nobody can check is not a pass), an artefact whose
stamp does not describe its own header, and a source pricing a gene the
repository no longer registers — 2026-08-23's near miss.

`--unverified-build "<why>"` records one anyway, and the reason is written into
the ledger beside the source it excuses. That is deliberately the same idiom as
`--legacy-shape`: one flag at the `--source` path per guard, each recording its
deviation in the artefact. The two are independent — the shape escape does not
waive the build check and the build escape does not waive the shape check.

**Legacy sources are grandfathered and named, never silently accepted.** The
sources recorded before 2026-08-23 carry no build block, because the games are
already played; they are kept as history and printed as `pre-fingerprint` on
every line the tool writes:

```text
  source legacy   pre-fingerprint docs/gene_screens/2026-08-22-p10-…json  (35148 seats, …)
  ⚠ 7 of 7 sources predate the build stamp (2026-08-23) and are kept as pre-fingerprint history
```

A source that carries a block is checked. The absence of one is a fact about
the file's age, not a way past the guard — `gene_screen` always writes it now.

### How the two definitions are kept from drifting

The same way the screen's shape is: pinned on both sides, with a test that
fails when one moves.

- `gene_screen.rs`'s `the_gene_table_is_exactly_what_the_ledger_re_derives_from_the_tables`
  parses the two source tables by the ledger's own rule and asserts the result
  **is** the compiled `gene_table()`. If the text rule and the compiled table
  ever disagreed, the guard would refuse every honest screen.
- `tools/test_genes.py`'s `TheHeaderFieldsMatch` compares the fields of
  the Rust `Build` and `Batch` structs against `BUILD_KEYS` and `BATCH_KEYS`,
  so a field added on one side and forgotten on the other fails a test rather
  than reaching the ledger.
- `TheGeneSetDerivation` runs the derivation at **P10's own source commit** and
  asserts it reproduces the 75 gene tags P10's real binary wrote into its
  header — a genuine artefact, not a fixture.

### A screen declares its size before it plays

P10 "ended early at the operator's request" at **5,858 of a planned 10,000
games**. Stopping early is legitimate and stays legitimate; an artefact that
cannot be told apart from a finished screen is not. `--games N` pre-registers
the batch on its own, so the common case needs no extra flag, and
`--target-games N` declares the whole screen when it is split over `--append`
sessions (each segment on its own disjoint start seed; the analysis sums them).
The header records the games, the seats they imply, and the seed
window reserved; every printed table and the analysis JSON then read actual
against intended:

```text
⚠⚠ PARTIAL SCREEN · 35148 of 120000 intended seats (29.3%) · seeds 100000000..100009999 reserved — a truncated run, not a completed one
```

A file written before this says `⚠ batch size was not pre-registered`, which is
the honest answer rather than a guess. The ledger prints `⚠ PARTIAL n/N` beside
such a source and records it.

## How the screen works

- Game `i` of a batch plays seed `start_seed + i`. Every major seat draws its
  genome from `(start_seed, i, seat)`: each screened gene on with its draw
  probability, every other gene held at the deployment default. A run
  reproduces exactly, and `--append` on a disjoint `--start-seed` draws
  disjoint genomes.
- Minors and barbarians are stock. The field a seat plays against is the
  other five random genomes — effects are averaged over random opposition,
  not measured against a fixed production field. A flag that only pays
  against untreated opponents is a flag the mixed ecosystem does not have.
- One row per major seat: genome beside outcome, plus the end-of-game census
  fields below. Nothing is ever replayed to ask a new question of the rows.
- `N` games give every gene about `N·players·p` seats on and `N·players·(1−p)`
  off. A seat's chance of winning is `1/players`; the table prints every
  gene's on and off rates against it.

### Live continuous-batch accounting

A continuous JSONL file is **not** one line per game: each segment starts with
one `header` record, then every completed all-seats game writes one `game`
record for each major seat — six records per standard game. Never report
`wc -l`, a nonblank-line count, or a raw JSONL record count as games. Use the
validated reader instead:

```sh
python3 tools/continuous_screen_status.py /path/to/rows-continuous.jsonl
python3 tools/continuous_screen_status.py /path/to/rows-continuous.jsonl \
  --analysis /path/to/cutoff-analysis.json
```

It groups records by `(seed, arm)`, requires exactly one record for every seat
and exactly one winner, and keeps **records**, **games**, and **seats** as
separate labeled quantities. A partial write, duplicate seat, inconsistent
winner, undeclared seed, malformed target, or disagreement with the frozen
`gene_screen --analyze --json` file is an error, not a lower-looking count.
That reader is the required source for live status and cutoff reporting.

Why not one arm per gene: the repository's older instrument priced one flag
per batch (`live` against `live_without_<flag>`, forty to two hundred maps
each) and priced each repair against the background in which every OTHER
repair was on — a link inside an otherwise-whole chain. A random-genome batch
prices every gene from every game, against every background at once.

## What one row of the table means

```
gene                 on n/off n   on%   off%   latest 20k      previous 20k    earlier 20k      all 95% CI       z   shareΔ     z    adjΔpp   read
muster-at-command-…  30120/29880 31.0%  22.3% +8.9pp z+2.53   +8.4pp z+2.40   +8.7pp z+2.51  [ +6.0,+11.4] +6.28  +2.10pp +3.12  +8.1±1.4  helps **
```

| column | meaning |
|---|---|
| `on n/off n` | seats that played with the gene on / off |
| `on%` / `off%` | the win rate (any victory) of those seats; `on% − off%` is the win Δ |
| `latest 20k` / `previous 20k` / `earlier 20k` | three newest-first, non-overlapping chronological replications of about 20,000 seats each, whole games only. Each cell is that window's win `Δpp` / clustered `z`; `—` means the file has not accumulated that window yet |
| `all 95% CI`, `z` | the on − off estimate over every seat, errors clustered by game |
| `shareΔ`, `z` | the same contrast on **score share** (a seat's score ÷ all majors' scores): continuous, so it resolves an edge at a fraction of the seats a win count needs |
| `adjΔpp` | the win Δ from one OLS of the seat outcome on an intercept and every screened gene at once; printed once there are at least `genes + 11` seats |
| `compute cost`, `time cost` | percent change per enabled major seat in wall seconds per completed turn, and in whole-game wall seconds (see below) |
| `read` | `helps *`/`hurts *` at \|z\| ≥ 2, `HELPS **`/`HURTS **` past the family-wise 5% bar — on the win Δ first, then `share …` when the score-share z says more; `~` otherwise |

"Newest" is input order, deliberately: appended runs can use any disjoint seed
range, while input order is what "latest" means. A pooled flag remains a
screening result; consistent direction across the chronological windows is
the extra evidence to use before dropping a gene or changing the ledger.

The header lines carry a seat's overall win rate against chance, **how the
games ended** (victory type, count, median turn — the world the table was
measured in), the draw the batch used, and a **resolution line**: how many
genes, the smallest win Δ and share Δ this run resolves at 80% power, how many
`*` rows |z| ≥ 2 flags by chance alone (≈ 4.5 of 100), and the family-wise bar
(≈ |z| ≥ 3.5 for 100 genes).

⚠ The table is sorted by the win z, and on the first run every result past
the family-wise bar was on the **share** axis. Read the `read` column, not just
the top of the sort.

⚠ **`~` means unresolved at this size, never "no effect."** A screen's job is
to rank and to say what it could see; a `*` is a candidate for a single-gene
run, not a promotion. Read the resolution line before reading the table.

## Which genes

`--list` prints the genome order. It is **discovered from the gene registry**,
`src/ai/advanced/genes.rs`, never listed by hand: every gene is declared once
there — its tag, its flag, its kind and its two toggles — and the screen
varies every `screenable()` row in registry order:

- `Kind::Repair(axis)` — an engine repair the Civilization VI seat ships and a
  native board can play; on in the genome's universe.
- `Kind::Production` — what production itself turns on before the ledger
  (`strategic-wonders`); on in the universe.
- `Kind::OptIn` — off everywhere until the ledger turns it on
  (`apostle-promotion-by-role`, `war-economy-2`, …); the gene *on* means
  enabling it.
- `Kind::HostOnlyOptIn` — `joint-tactics`: shipped by the bridge as a host
  adapter *and* screenable as an opt-in, because its search runs headless.
- `Kind::HostOnly` — shipped by the Civilization VI seat but reading host
  state a native board does not have (`land-grab`, `explore-commit`,
  `bank-envoys`, `fog-land-capacity`, …): inert headless, so **never
  screened** — screening them would measure noise and report it as noise.
  Pricing the deployment bundle in the host regime is a different
  instrument (`civvis_orders --without`, the ladder).

A gene added to the registry reaches the genome, the ledger, the ranking and
the manifest without touching any of them: `tools/genes.py` is the one
Python reader, and `gene_screen.rs`'s
`the_gene_table_is_exactly_what_the_ledger_re_derives_from_the_tables` holds
the Rust and Python readings of the registry to one answer.

| flag | meaning |
|---|---|
| `--games N` | the batch size; `--target-games N` declares the whole screen when it is split over `--append` sessions |
| `--genes a,b,c` | screen only these; the rest are held at the deployment default. This is the single-gene run that confirms a flag |
| `--p-on`, `--p-default-on` | the draw: ¼ and 0.75 by default (a default-off gene turns on one time in four, a default-on gene turns off one time in four), both strictly inside (0, 1) so every gene keeps both arms |
| `--victories a,b,…` | **all six, and a batch that changes them is a probe.** The lanes decide which genes can act at all |
| `--stock-civs` | stop shuffling every seat's civilization per map (a probe); stock seating is a FIXED civ per seat, and on the first 250-pair run seats 0 and 2 won twice as often as seat 3 whoever sat there |
| `--players`, `--width`, `--height`, `--city-states`, `--speed`, `--map`, `--turns` | probe legs; the ledger refuses a batch that moved one |

## Versioning a gene: `war-economy-2` beside `war-economy`

*"For testing improvements to a gene we can use a versioning system … we would
want to test both independently and make sure improvements actually improve
the gene performance."* — operator, 2026-08-23.

An improvement to a gene is a **new gene**, screened under exactly the rules
above; nothing about it is special except that the screen and the ledger know
it belongs to a family.

**The recipe.** To improve `war-economy`:

1. Give the improvement its own flag on `AdvancedAi` (`war_economy_2`), its
   own `enable_war_economy_2` / `disable_war_economy_2` toggles in
   `treatment_flags.rs`, and its own code path — a second implementation of the
   behaviour, not a patch to the first. The original's code and tag stay as
   they are; it is version one.
2. Add the row to the registry (`src/ai/advanced/genes.rs`, the same kind as
   the original — usually `Kind::OptIn`) with the tag **`war-economy-2`**. That suffix is the
   whole declaration: a tag `<base>-<n>` with `n ≥ 2` whose `<base>` is itself
   a gene is that gene's version `n`. (`search-cadence-20` is not a version of
   anything; `war-economy-1` is not used — the original keeps its name and
   its history.) `gene_screen --list` shows the family.
3. Run the standard screen — there is no separate testing regime for
   versions (operator, 2026-08-23: *"don't have a separate testing regime for
   the versions. we should simply test them too in our large batch runs"*).
   **A seat plays at most one version of a family**: the family is drawn as
   one level — off, or exactly one version — on with the family's
   probability (`P_DEFAULT_ON` if any version ships on, else `P_ON`), and
   within it **the best version takes 60% and the other versions split the
   remaining 40% evenly** (`BEST_VERSION_SHARE`; the best is the version the
   ledger ships, else the priced version with the highest tracked wins —
   *"a 60% chance of using the top version of the gene and a 40% chance of
   using a different gene version (randomly pick among the rest)"*; a family
   with one drawable version plays that version). So `war-economy` and
   `war-economy-2` are never on the same
   seat, each is priced against the same "off", the two are priced against
   each other, and the batch mostly plays what would ship. ⚠ A version named
   alone in `--genes` while a sibling ships on is refused: the held-on
   sibling would force it off and its row would read +0.0 — name the family.
4. Read the **family table** under the main table (and `families` in the
   `--json` summary): one cell per level — `off`, `war-economy`,
   `war-economy-2` — with seats, win and share, and a contrast for every
   version against `off` and for every version against the one before it,
   clustered by game like everything else. **An improvement improved when its
   "against the version before it" contrast is positive on the win axis and
   it also beats `off`.** Every version also has its own row in the main
   table and its own row in the ledger, read by the same rule as any gene.
5. The pinned deployment genome may name **at most one version of a family**.
   It is an explicit operator choice, never the version with the highest
   tracked wins; a newer screen cannot replace a selected sibling. Every
   version keeps being priced on its own row. `HEURISTIC_GENE_RANKING.md`
   names the family's best *display* version in its *Best version* column
   (`1` is the original, so a gene with no versions reads `1`): the pinned
   version if one exists, otherwise the priced version with the highest
   tracked wins. A versioned row's *Total (on)* /
   *Total (off)* cells show the best two versions' rates side by side — each
   version's *on* is the seats that played that version; every other seat is
   its *off*.

Two consequences worth stating. A version that does not beat the original
head to head is not an improvement however well it does against `off`, and
the family table says so in one line. And the code is the contract: since the
screen never seats two versions together and the ledger never ships two, an
`enable_war_economy_2` that left `war_economy` on would only ever matter to a
hand-built seat — but write it so the newer version's enable turns the older
one off anyway, so a hand-built seat cannot play both.

## Cost

The same run prices the runtime cost of every gene without adding a timer to
any heuristic and without replaying a game. A game's timing is regressed on
how many of its major seats had each gene on (with an intercept for
machine-load drift and HC1 robust errors), so the coefficient is the cost of
enabling the gene for **one major seat**:

- **compute cost** is the percent change in wall seconds per completed turn —
  whether each simulated turn itself became dearer;
- **time cost** is the percent change in whole-game wall seconds — the
  throughput an operator pays, including a gene that ends games earlier or
  later.

Positive is slower. Old rows with absent timings produce an unknown cost rather
than a false zero. `joint-tactics` is held out of the default screened set on
this column alone (+27.3% per enabled seat; 2.52× the batch) and priced by
`--genes joint-tactics` when it is wanted.

## The rows file

The first line is a header (`kind: header`, the gene order, the screened set,
the profile, the per-gene draw probabilities, the `build` that played it and
the `batch` it was launched as); every other line is one seat:

```json
{"kind":"game","game":0,"seed":26081900,"seat":0,"genome":"0010100110…","win":true,
 "winner":0,"victory":"score","turn":250,"score":1067,"score_share":0.4496,"rank":1,
 "cities":11,"alive":true,"secs":116.3,"civ":"rome",…}
```

Interactions, subgroup tables (by civ, victory type, map), and the cost fit
above are all re-analyses of these rows. `--analyze` refuses to merge files
written at different profiles, gene orders or draws — a merged table would mix
two experiments. Files written by the paired designs (`pair` and `arm` on
their rows) still read: the two games of one seed are told apart by arm.

Beside the outcome, every row carries an end-of-game census — the religion
fields, `techs`, `military`, the war counters, and `wonders`, the wonders
standing in that seat's cities. Each exists because a claim about the agent
was being argued from prose instead of read out of a batch, and `--analyze`
prints a census line per family whenever the rows carry it. `wonders` is the
newest: `Game::score_parts` pays 15 points a wonder, the densest line of the
tally that decides three quarters of these games, so a seat's wonder count is
both the actuation question ("does it build one at all") and a mechanism
behind its score share. ⚠ Read it as a census, never as a lever — within one
arm wonders track score share, and so do cities, and wonders track cities;
only the on−off contrast says which way the causation runs
(`docs/eval/2026-08-24-the-wonder-lane-is-already-open-in-both-regimes.md`).

## Per-civilization effects — `--by-civ <tag>`

`--analyze … --by-civ war-economy` prints one gene's contrast split by the
civilization the seat played, clustered like everything else. This is the
subgroup the marginal table averages away: a flag can be worth nothing on
average and still be a real strategy for one civilization — or the reverse. It
is a subgroup scan with its own family-wise bar printed in the header; treat a
flag as where to point a run, not a finding.

## Interactions

```sh
target/ci/gene_screen --analyze screen.jsonl --interactions --top 20
```

With independent draws every two-factor product is, in expectation,
uncorrelated with every main effect and every other product, so each `γᵢⱼ` is
estimated marginally from the same seats — the regression of the centred
outcome on the centred product `zᵢzⱼ`, clustered by game. The printed figure
is **how much more one gene is worth when the other is on** (`4γ`). A hundred
genes have 4,950 terms and no affordable run fits them jointly.

⚠ The headline is a **count against an expectation**, not the top rows. 4,950
tests throw ~225 flags at |z| ≥ 2 with nothing whatever going on, and the tool
prints that expectation beside the count. The first runs read *indistinguishable
from noise*, and no pairwise coupling among the repairs has been visible at any
size run so far. Interactions are far noisier than main effects from the same
run; read the multiplicity bar.

## Instrumentation: how a game was lost, not only that it was

Every row also carries `founded_religion`, `foreign_faith_cities` (our own
cities flying somebody else's faith at the end), `faith` still banked,
`inquisition` (whether the Inquisitor gate was ever unlocked), `techs`,
`military`, and the raid counters (`raid_wars`, captures, `pillages`). The
table prints a **religion census** from them, because conversion decided two
thirds of games on the Pangaea instrument (28% at the screen's shape) and the
rows could not say one thing about how the losing seat stood in that race.

## History

The screen ran as a **foldover** from 2026-08-19 to 2026-08-23: games in
pairs, the second game the exact complement genome on the same map, so the
map cancelled out of every per-gene difference, plus a `prior` variant that
drew each arm independently from the ledger's verdicts. Every source the
ledger holds from that period still reads here — the rows are seats with
genomes — and the first run's lessons stand
(`docs/eval/2026-08-19-gene-screen-random-genome-factorial-screen.md`): 4p
Pangaea games are a religion race (65% conversions, median t148, which is what
ONE SCREEN answered with continents); score share carries the signal while win
rate barely moves; the interaction layer was noise. The foldover's one real
advantage was variance — it bought precision per game — and the operator chose
randomness over it so that no gene is ever measured against a structured
background.

## The gene ledger: the deployment genome is explicitly pinned

As of 2026-08-24, deployment is an explicit operator selection rather than a
rule inferred from screen statistics. The 45-gene set retains the prior 36
on/off selections and explicitly turns on `unit-cost-efficiency`,
`unit-objective-memory`, `camp-party`, `slot-kind-tiebreak`,
`promote-when-wounded`, `religion-sues-peace`, `lane-great-people`,
`one-launch-pad`, and `civilian-rescue`.

The former column thresholds, pooled-*Diff* veto, and posterior alternatives
are retired as deployment rules. Win columns, *Diff*, posterior intervals,
verdicts, and score share remain the evidence an operator uses for a later
selection change; a completed screen never promotes or demotes a gene by
itself. The dated notes below preserve how earlier selections were made, not a
current rule.

- **`docs/gene_ledger.json`** records the screen profile and measurements for
  every gene, plus the explicit `deployment_policy` and
  `deployment_genome`. `tools/genes.py::OPERATOR_DEFAULT_ON` is the source of
  truth; the generated `src/ai/advanced/genes.rs::DEPLOYMENT_GENOME` and
  `src/ai/advanced/gene_ledger.rs` validate and apply the same list. `python3
  tools/genes.py write` regenerates the JSON, Rust block, and ranking, while
  `tools/test_genes.py` fails on drift. Sources continue to update the measured
  evidence per gene, but cannot rewrite `default_on`.
- **Verdict rules** (`tools/genes.py`, repeated in
  `src/ai/advanced/gene_ledger.rs`): `helps` = win z ≥ 2 with share z > −2, or
  share z ≥ 2 with win z > −2 — the screen's own `*` flag; `hurts` the mirror;
  `unresolved` otherwise, including a gene whose axes disagree past |z| ≥ 2
  (`conflict`) and a gene no screen has measured. Past the family-wise bar is
  recorded as `family_wise`, not required: with sixty-odd genes that bar would
  leave three on. The newest screen that priced the gene supplies the verdict.
- **The deployment policy.** There is no score-derived threshold, veto, or
  posterior fallback. A screenable tag is on exactly when it appears in the
  pinned `deployment_genome`; all other screenable tags are off. The list
  rejects duplicates, unknown tags, and selecting two versions of one family.
  Every change is an intentional operator edit recorded in review.
- **Applying the deployment genome.** `AdvancedAi::enable_live_bridge` and
  `enable_engine_repairs` end with `apply_gene_ledger`: every selected live or
  production treatment is enabled and every unselected screenable treatment is
  withheld. A flag the screen cannot price (the Firaxis-only flags) is left as
  the bundle set it. The
  `_universe` twins (`enable_live_bridge_universe`, `enable_engine_repairs_universe`)
  set every flag and skip the ledger — they are what this screen starts from
  (it then sets each gene to its drawn state) and what the membership tests
  read. The live seat's `genome` event lists `treatments` as deployed and
  `ledger_withheld` beside it; `gene_screen --list` shows each gene's
  universe state, ledger verdict, default and prior.
- **First ledger (2026-08-20)**, from the 6p 60k native screen (13,446
  seat-pairs, `docs/gene_screens/2026-08-20-p4-…`), the 4p war screen (3,300
  seat-pairs, `…p2-war-…`) and the repaired genes' war re-screen (`…p3b-…`):
  **10 help** (on: barbarian-scouts-are-scouts, bounded-recovery,
  buildings-before-projects, garrison-under-fire, loyalty-rate-alarm,
  recorded-tactical-step, siege-muster, siege-tracks-wall, war-reinforcement,
  wide-map-capacity), **11 hurt** (off), **43 unresolved** (off). The live
  bundle therefore plays those ten plus the host-only flags. What the ledger
  bought is measured by this screen's anchors under `--baseline best` — arm 0
  the all-on universe, arm 1 the best genome, same maps: **250 pairs, 4p
  all-six, seeds 52M: all-on 18.4% wins / 20.6% share, best genome 31.2% /
  26.7%, paired win Δ +12.8 pp ± 3.3 for the best genome** (round:
  `docs/eval/2026-08-20-the-bridge-talks-more-than-once-a-turn.md`).

- **The war column, re-priced (2026-08-21).** Every gene's war verdict now
  comes from one screen against the best genome
  (`docs/gene_screens/2026-08-21-s8-war-rerank-vs-best-4p-allseats.json`:
  4p all-seats, `domination,score`, 5,844 seat-pairs, ±2.0 pp), replacing the
  pre-repair p2/p3b rows. It turned on `score-horizon`, `joint-tactics` and
  `blind-objective-strength` (unresolved natively, helps at war) and held
  off `siege-role`, `housing-districts`, `settler-site-agreement`,
  `holy-lane-parity` and `inquisition-on-threat` (unresolved natively, hurt
  at war); `wide-map-capacity` reads **+15.6 pp** at war. A screen played on
  an older build can carry a gene whose code has since been removed; the
  tool now drops those rows and says so, because the Rust table refuses a tag
  the registry does not know.

- **The default rule replaced the verdict (2026-08-22).** No screen was re-run
  and no measurement moved; the ledger simply re-decided every default from
  the two native win columns. **22 genes on, was 20.** On: `religion-sues-peace`
  (+29/+25), `one-launch-pad` (+24/+23), `whole-turn-backtrack-guard` (+23/+39),
  `siege-tracks-wall` (+21/+51), `strategic-wonders` (+21/+21), `strike-opening`
  (+21/+20), `war-patience` (+20/+3), `blind-objective-units` (+4/+6) and
  `war-reinforcement` (−5/+49, average +22). Off: `founder-temple` (+48/–) and
  `idle-faith-patronage` (+23/–), each one native screen short of a prior
  column; `siege-is-progress` (+14/−64), `war-economy` (+8/−192),
  `amenity-project-preemption` (−4/+33), `army-target-weighs-enemy` (−4/−33)
  and `joint-tactics` (−4/−18). Note `war-patience`: it ships with a `hurts`
  verdict, family-wise on the **share** axis at native share z −3.43, because
  the rule reads the win axis only.

- **A strong first reading may ship provisionally (2026-08-22).** After P10
  moved the current genome to 26 defaults, the operator extended the rule:
  exactly one populated win column defaults on when it is above +20. No
  measurement moved. Seven genes switch on, taking the genome to **33**:
  `great-person-housing` (+78/–), `settler-threat-detour` (+50/–),
  `governor-victory-lanes` (+46/–), `settle-sooner` (+41/–),
  `raid-pillage-prizes` (+30/–), `builder-worked-tile-priority` (+24/–) and
  `opportunistic-war` (+23/–). The boundary is strict: +20/– remains off, and
  a second reading replaces this provisional clause with the two-column rule.

- **A negative record vetoes the columns (2026-08-22).** The operator: *we
  should default every gene to off that has Diff < 0.* No screen was re-run and
  no measurement moved; every gene's pooled on−off difference — the ranking's
  *Diff*, already printed there — is now recorded as `win_diff_pp` beside its
  win columns and vetoes the default when it is negative. **31 genes on, was
  34**: off go `war-economy` (columns +38/+8, record −0.78 pp),
  `siege-commitment` (+1/+3, −0.21 pp) and `apostle-promotion-by-role`
  (+14/+12, −0.06 pp). Nothing switched on: the veto is one-way. Each of the
  three is condemned by the same screen, `2026-08-20-p4`, where they read
  −3.84, −0.80 and −0.83 pp; their two newer screens are positive but smaller,
  and the pooled figure weights by games rather than by recency, so a repaired
  gene stays vetoed until its newer games outweigh its worst screen. That is a
  deliberate reversal of "an older bad screen is history, not a veto", which
  was the rule between 2026-08-22's two directives. **This moved the incumbent**
  every Elo result recorded against the 33- and 34-gene genomes is filed
  against. `holy-lane-parity`, promoted by #2307 the same day, is untouched:
  its record is +1.08 pp.

- **One screen, and the war rows are gone (2026-08-22).** The operator's
  directive above collapsed the instrument to a single shape and this ledger to
  a single measurement per gene. `docs/gene_ledger.json` lost its `war` block,
  its `deciding_regime` and its per-source `regime`; it gained the screen's own
  profile and a `shape` on every source. **No win column and no default moved**
  — the war regime never supplied a column — so the deployment genome is the
  same 33 genes. What changed is seven verdicts that had been decided at war and
  now read `unresolved` (`barbarian-scouts-are-scouts`, `blind-objective-strength`,
  `peacetime-deterrence`, `housing-districts`, `housing-research`,
  `inquisition-on-threat`, `settler-site-agreement`) and `holy-lane-parity`,
  whose `conflict` flag was the war screen's `share hurts` disagreeing with the
  native win reading. Verdicts do not decide defaults, so nothing shipped moved
  with them. The four war sources — `p2`, `p3b`, `s3`, `s8` — are dropped from
  the ledger; their analysis files stay under `docs/gene_screens/` as history.
  ⚠ Every remaining source is `legacy`: 60×38 Pangaea. The current defaults
  stand on that instrument until the first standard screen re-prices them.

- **Ten more genes left the code (2026-08-21).** A second application of the
  directive behind the #2235 cull — the bottom of `HEURISTIC_GENE_RANKING.md`
  leaves the repository — removed `holy-lane-parity`, `camp-reach`,
  `wonder-prereq-reach`, `ranged-line-of-sight`, `housing-buildings`,
  `muster-at-command-radius`, `barbarian-walls-one-tier`,
  `idle-walkers-close-the-pipeline`, `suzerain-cards` and `siege-role`, with
  their `live_without_*` arms and the `advanced_holy_lane`,
  `advanced_holy_lane_v0` and `advanced_wonder_reach` arms that set their
  fields. **Every one was already held off by the ledger**, so the deployment
  genome is unchanged and every screen's "off" arm is what now ships; what is
  gone is the ability to turn them on. `holy-lane-parity` and `siege-role`
  are measured `hurts` at war; the other eight sit inside their screen's noise
  band on wins and are recorded as directive removals, not measured harms.
  ⚠ The band quoted at the time, ±110/10k, was the on−off **difference**'s band
  read against a **column**, which is half that difference — see the correction
  below. The eight are inside the corrected ±56 band too, so the reading stands;
  what does not is calling anything up to ±110 noise.
  `recon-flight` followed a day later (#2271) — see below.

- **The noise band was quoted at twice its scale (2026-08-22).** The ranking's
  win columns are `(win_on − chance) × 10,000`, and a foldover puts the on and
  off arms symmetric about chance — so a column is **half** its screen's on−off
  difference and carries half its error. The ±110/10k the ranking header quoted
  (and #2266 used to justify eight removals) is the difference's 80%-power band:
  correct for `win_delta_pp`, twice too wide for the column printed beside it.
  Nothing about a screen changed; the sentence judging it did. The header now
  derives each native screen's own band from that screen's own errors —
  `2026-08-22-p10` resolves **±51**, `2026-08-21-p7` ±56, the single-gene
  `2026-08-21-s7` ±29 — and `tools/genes.py::column_se` owns
  the arithmetic, next to the `wins_per_10k` it halves, so the printed band and
  the printed column cannot drift apart. What this changes in practice: a column
  between ±51 and ±110 used to read as noise and does not. `holy-lane-parity`'s
  +63 in P10 is the live case — see #2299.

- **The removal ledger is priced on native screens again (2026-08-22).** #2235
  deleted its eight genes' rows out of `2026-08-19-p2`, `2026-08-20-p3b` and
  `2026-08-20-p4`; #2266 left its ten "as played", which is the policy. The
  eight rows are restored (306 lines, additions only, every surviving row
  byte-identical), so every gene in *Removed from the code* is now priced on a
  **native** screen at the same 1-in-6 chance base. Before this, `siege-muster`
  had no native row left and was listed at **+5** from a 4-player *war* screen
  where chance is 1-in-4 — a number that read as "removed while helping" and is
  really −26 at p4. `tools/genes.py` filters unregistered tags, so the
  deployment ledger is byte-identical either way.

- **`holy-lane-parity` defaults ON (2026-08-22).** The operator took the call
  #2299 left open. Its direct Pangaea confirmation enters the ledger as
  **legacy history**, not a second standard-shaped screen; its columns are
  `[+63 prior, +99 last]`, both positive, and the rule defaults it on:
  **34 genes, was 33**. Exactly one ledger row changed and exactly one default
  moved. ⚠ This **moves the incumbent** every recorded Elo result is filed
  against — the deployment genome now plays a gene it did not play, so
  `--deployment-comparison` diverges from the previous head by design. It is
  the first gene to be culled, restored and promoted; the round is
  `docs/eval/2026-08-22-holy-lane-parity-direct-confirmation.md`.

- **`holy-lane-parity` came back, and its direct arm confirms it (2026-08-22).**
  The first gene to return from a cull. #2266 removed it on **−27** from the
  four-gene `s6` screen — whose column band is **±64**, so that was a null and
  not a reading. P10's binary predates the cull by 1h43m, so P10 priced the gene
  after its code was gone: **+63 at z +3.48**, past P10's family-wise bar of
  3.403 and the only such reading among the nineteen genes in the removal
  ledger. #2299 restored the code and ran the arm the cull never got to —
  1,200 map pairs on seeds 110M, every other treatment held at the deployment
  genome, all 2,400 games complete, 7,200 treated-seat pairs:
  **+99 wins/10k, z +4.05, 95% CI [+51, +147]**, `HELPS **`, against a run that
  resolves ±68. Two independent instruments on two disjoint seed windows now
  agree. Score share is null (+0.08 pp, z +1.23) and cost is nil
  (+0.49% ± 0.31% per turn). ⚠ Following the P9 precedent the direct screen was
  initially a note, not a ledger source, so the gene came back into the ranking
  **off**. The operator's later call records it as legacy history: both columns
  are positive and the gene defaults on at **rank 1**, without mislabelling a
  60×38 Pangaea result as the standard screen. `docs/eval/2026-08-22-holy-lane-parity-direct-confirmation.md`
  holds the numbers and the two things it does not settle (850 is an upper
  bound, not a tuned value; the historical war regime reads `share hurts` at z
  −2.26).

- **`recon-flight` leaves production too (2026-08-21).** It was held out of
  the cull above because `promoted_policy_envoy` turns it on, as one leg of
  the recon quartet promoted at +35 Elo (#1923). The operator's reading is the
  right one, and it is this document's own instrument that answers it: a
  screened gene's arms are drawn on a seat built from `AdvancedAi::new()`, so
  **every screen already played "production with it" against "production
  without it"** — 15,000 native seat-pairs and 5,844 war ones, and it read
  negative on all four axes (native −0.52 pp win / −0.03 pp share, war
  −0.55 / −0.24) without ever resolving. `docs/eval/README.md`'s rule finishes
  it: a composite gate licenses the composite, never its parts, and the ledger
  had already unpicked three of the quartet's four legs for deployment. The
  **frozen rating anchor is untouched** — `advanced_v1` is `legacy()`, which
  never routed through `promoted_policy_envoy`, and
  `advanced_v1_plays_the_same_game_it_always_did` stays green. Against that
  anchor over the same 60 maps, production reads +47 Elo-equivalent with the
  flag and **+58 without** — no detectable regression, at a resolution
  (±102) far too coarse to call it a gain.

- **The operator's view of the same seat-pair data** is `HEURISTIC_GENE_RANKING.md`
  at the repository root — every screenable gene ranked by wins added per
  10,000 six-player on-arm seats, each from the latest native screen that measured
  it, with the war figure and the ledger's default beside it. It is generated
  (`python3 tools/genes.py write`) and
  `tools/test_genes.py` fails when it is older than the
  ledger's sources; regenerate it in the same change that adds a source.

⚠ Two consequences to know. A `live_without_<gene>` arm for a gene the
ledger already holds off is identical to `live` — the screen is that gene's
instrument now. And a treatment PR no longer ships its flag on: it ships it
into the universe, screens it (a few hundred pairs resolve ±3 pp), and the
ledger turns it on when its native win columns clear the rule, provisionally
including a first reading above +20.

## A Δ of exactly zero is a gene that never fired, not a null

`step-and-reassess` (2026-08-20, `docs/LIVE_TACTICS.md` §11) first screened
**+0.0 [+0.0, +0.0]** on both axes over 204 pairs: every pair's two games
ended identically. That is not "no effect" — a gene with any reach at all
moves at least the score share of some game — it is the signature of a gene
whose code path is never entered in the regime the screen plays. The cause
was structural: its first cut lived only on the parallel unit planner, and
the only thing that installs a `WorkPool` is the interactive `civvis --jobs`
CLI; every evaluator, `gene_screen` included, and the live decider run units
serially. The repaired gene carries a serial leg and the next 41 pairs already
differed. Read the interval's width before the sign: a zero-width interval is
a fires-check failure, and the fix is in the gene, not in more pairs.

### And since 2026-08-23 a gate says so, instead of four documents

That paragraph was written down here; `treatment_flags.rs` wrote the same
warning from the other direction — an `ENGINE_REPAIR_TREATMENTS` tag whose
enable is missing from the bundle is off in **both** arms, "the two arms play
byte-identical games", and *"three tags reached the tables before this line and
burned 30 games saying nothing"*; `battle_bench`, `doctrine_arena` and
`gene_census` each implement a fires-check for their own instrument (so did
the retired `ai_eval`). What
none of that was, was a **gate**. Nothing stopped a tag reaching the three gene
tables with no evidence it fires, and nothing failed when a committed screen
contained a zero-width row. `competition-victory-points` is in the tables today
and cannot fire at all: `native_competitions` is `false` in `GameOptions` and no
screen sets it.

`tools/gene_fires.py --max 0` is that gate, wired into `cargo-test` beside
`civvis_inert.py --max 0`, which is the ratchet it is modelled on. It discovers
the gene set exactly the way `gene_table()` does — every screenable row of the
registry, so a new gene reaches the gate without touching the tool — and reads
each gene's own rows out of
`docs/gene_screens/**/*.json`. **A gene is proven when some committed row has a
non-zero paired statistic**, which is a number read out of an artifact rather
than a sentence somebody wrote. The cheapest artifact that carries it is a
single-gene probe, because `--genes <tag>` holds everything else at the
baseline and any divergence between the arms is then that gene and nothing
else:

```bash
target/ci/gene_screen --games 6 --jobs 6 --genes <tag> \
  --start-seed <seed> --out target/<tag>.jsonl
target/ci/gene_screen --analyze target/<tag>.jsonl \
  --json docs/gene_screens/fires/<tag>.json
```

Six games is enough, because the question is qualitative. ⚠ Look at the score
share as well as wins: `coupled-expansion`'s probe read a win Δ of exactly zero
and a share Δ of +0.29 pp — a gene that fired and did not change who won, which
is firing. Those probes are **not ledger sources**: they set no profile of their
own, they are six games, and `tools/genes.py` takes its sources by name.

### ⚠⚠ A probe's win Δ is not a measurement of the gene

The question a probe answers is *did it fire*. Its win column answers nothing —
and until 2026-08-25 it could answer something false with great confidence.

With `--genes <tag>` only that gene varies, so a gene that does not fire plays
both arms as **the same game**. Each game still has exactly one winner, and
whether that winner drew the gene is a coin flip per game. When none of them
did, the treated arm has no events at all, and the clustered estimator answers
with a two-point interval instead of refusing.

A gene whose predicate was edited to `return false` unconditionally — it cannot
fire, by construction — reported:

```text
gene_screen --games 12 --jobs 6 --genes <tag> --start-seed 99000000
  16/56   0.0%  21.4%  -21.4pp z-18.21  [-23.7, -19.1]  -21.4±1.2  HURTS **
```

A family-wise `HURTS **` verdict at |z| eighteen on an interval two points wide,
for a no-op. What exposed it: the same block had already produced that identical
number for two real implementations with **opposite** semantics, and two other
seed blocks of the same gene read `+0.0` and `-7.4 (z -0.66)`.

`Seats::empty_arm_floor` now puts a floor under the error whenever one arm of a
binary outcome has no events: the ordinary difference-of-proportions error times
the design effect of the seats sharing a game. It **widens** rather than
refuses, so the difference is still reported, and it can only ever make a row
less significant. The block above now reads `z -1.60`, `[-47.8, +4.9]`, `~`;
blocks with events in both arms are untouched to the digit. A gene that wins
every game it is drawn into also empties an arm, and that one is real — it keeps
its error, which `an_empty_arm_loses_its_precision_unless_the_effect_is_overwhelming`
holds along with the artifact.

Score share is continuous and never had this problem; it remains the half of a
probe worth reading. **Two disjoint blocks that agree beat one that does not**,
and a standard screen beats both. If two implementations with different
semantics give the same number, the number belongs to the block, not the gene.

### ⚠⚠ A starred verdict needs the power to back it

The `read` column's bars are **significance** bars — `|z| ≥ 2` for a flag, the
family-wise bar for a starred one, both at α = 0.05. A run's **resolving power**
is a different quantity: 2.8 standard errors, which is `|z| = 2.8`. Whenever a
single gene is screened the family-wise bar is 1.96, so the significance bar is
*below* the power bar and a row could be starred while sitting under the
smallest effect its run could find — the regime where a significant estimate is
most likely to be inflated.

`docs/gene_screens/fires/defensible-sites.json` is the exhibit, written on
2026-08-25:

```text
win_delta_pp +42.9   win_resolves_pp 57.1   read "HELPS **"
```

A forty-three point difference, a family-wise verdict, and a run that cannot
resolve anything under fifty-seven.

Readings in `2.0 ≤ |z| < 2.8` now keep their flag and gain a word — `helps *
(thin)` — and `**` is reserved for readings that clear both bars. **Nothing is
suppressed**: the difference is still computed and still printed. Seven
committed rows lose a star under the new rule, six of them six-game fires
probes, which is the point. No ledger number moves (`tools/genes.py check` is
clean), because the ledger reads the estimates rather than this string.

⚠ `tools/genes.py`'s own `share_verdict` still flags at a plain `|z| ≥ 2` with
no star and no power term. It is a different and milder convention on one axis,
it feeds the generated ranking, and it is left alone here rather than moved in a
PR that claims neither that file nor the tables it writes.

### ⚠⚠ A twelve-game probe resolves ±28.6 pp. Read that number first.

`--analyze` prints a `resolution:` line, and it is the first thing to read:

```text
12-game, 1 gene   resolution: this run resolves a win Δ of ±28.6 pp at 80% power
90-game, 9 genes  resolution: this run resolves a win Δ of ±10.3 pp at 80% power
```

**A Δ smaller than its own run's resolving power is inside that run's noise**,
whatever its sign, and however many blocks agree on that sign. Sign agreement
across three small samples of a null is ordinary.

This is not hypothetical. Nine genes were written between 2026-08-24 and -25,
each probed at one to three twelve-game blocks, and each shipped quoting its
probe's Δ. Every one of those readings — from +22.2 pp to −21.1 pp — was inside
±28.6. Re-measured together on one ninety-game screen, 540 seats, ~135 on-seats
each:

| gene | probe said | 540-seat screen |
|---|---:|---:|
| `conversion-majority-alarm` | +22.2 pp (z +2.03) | **+0.2 pp** (z +0.04) |
| `diplomatic-lane-forecast` | +18.5 pp | **−0.8 pp** (z −0.22) |
| `unchosen-war-keeps-the-lane` | +12.6 / +1.1 / +7.4 | +2.5 pp (z +0.64) |
| `domination-city-count` | +2.7 / +8.9 / +5.9 | +3.8 pp (z +0.91) |
| `rival-suzerainty-alarm` | +12.5 / −2.3 | +2.8 pp (z +0.78) |
| `science-chain-alarm` | +2.7 / +13.1 | +1.4 pp (z +0.38) |
| `culture-lane-forecast` | −5.4 / +4.4 | −2.1 pp (z −0.58) |
| `congress-counter-leader` | −21.1 / +24.4 | −0.9 pp (z −0.25) |
| `frontier-massing-alarm` | share +3.79 / +3.46 | **−9.0 pp (z −2.98)** |

Eight of nine came back indistinguishable from zero, and the one that resolved
did so negative. **The probes were not measuring these genes.**

The `resolution:` figure now travels with the artifact as well as the terminal:
`--json` writes a top-level `resolution` block and a per-gene `win_resolves_pp`
and `share_resolves_pp` beside each Δ, so a committed
`docs/gene_screens/fires/*.json` can be read honestly on its own.
`a_reading_carries_the_smallest_delta_its_run_could_resolve` holds the
arithmetic against the printed line.

### ⚠⚠ Do not read a probe's win Δ as a measurement of the gene

The question a probe answers is *did it fire*. Its win column answers nothing,
and until 2026-08-25 it could answer something false with great confidence.

With `--genes <tag>` only that gene varies, so a gene that does not fire plays
both arms as **the same game**. Each game still has exactly one winner, and
whether that winner happens to have drawn the gene is a coin flip per game. When
none of them did, the treated arm has no events at all: every game's cluster
score comes out the same, the sandwich estimator has no between-cluster
variation left to work with, and the standard error collapses *toward zero*
instead of blowing up.

This is what that looked like. A gene whose predicate was edited to
`return false` unconditionally — it cannot fire, by construction — reported:

```text
gene_screen --games 12 --jobs 6 --genes <tag> --start-seed 99000000
  16/56   0.0%  21.4%  -21.4pp z-18.21  [-23.7, -19.1]  -21.4±1.2  HURTS **
```

A family-wise `HURTS **` verdict, at |z| eighteen, on an interval two points
wide, for a no-op. The same block had already produced that identical number for
two real implementations with **opposite** semantics, which is what exposed it;
two other seed blocks of the same gene read `+0.0`.

Both places this could be reported now withhold precision when an arm has no
events — the per-gene contrast reports the raw difference with an infinite error,
and the OLS-adjusted column reports nothing — so the row reads `~`, which is
what it is. `an_arm_with_no_wins_reports_no_precision_and_not_a_verdict` holds
it. The guard is specific to a binary outcome with an empty arm, so score share,
which is continuous, is untouched and remains the half of a probe worth reading.

**Two probe blocks that agree still beat one that does not**, and a real screen
beats both. A six-game block that reads `HURTS` should be re-run on a disjoint
seed range before anybody believes it — and if two implementations with
different semantics produce the same number, the number is the block's, not the
gene's.

A gene that cannot be made to fire takes a waiver in
`tools/gene_fire_waivers.json` with the reason, in the shape
`tools/inert_waivers.json` uses — except that the reason is enforced, and a
waiver goes **stale** the moment its gene is proven or leaves the tables, which
fails the same ratchet. The list can only shrink.

## The toggles no screen can reach, and why each one is not a gene

`docs/EVAL_STATUS.md`'s *Genome coverage* deliberately publishes an over-count:
it makes no attempt to separate a behaviour worth pricing from host-only
plumbing, so the number is a ceiling on the debt rather than a floor. A ceiling
nobody has examined is the same shape as the defect the section exists for, so
since 2026-08-23 every toggle on that list carries a line saying why it is not a
gene, in `docs/genome_reach_debt.json`, and
`tools/test_genome_reach_debt.py` requires the file to cover **exactly** the
computed list — a toggle that gains a gene row must lose its entry, and a new
unreachable toggle must gain one. The residual is examined work.

The count went **165 / 100 reachable / 65 unreachable → 166 / 114 / 52** when
fourteen of them became opt-in genes — eighteen rows were written and the fires
gate above refused four on their own probes. What the remaining 52 are:

| group | n | why not a gene |
|---|---:|---|
| `bundle` | 6 | It turns a group of other genes on. `gene_screen` already builds its treated seat from `enable_engine_repairs_universe`; a row would vary everything inside it and file the sum under one tag. |
| `host-only` | 12 | Cannot fire on a native board, each with its reason already recorded beside its row in the registry (`Kind::HostOnly`). `step-and-reassess` is the founding example of the paragraph above. |
| `live-bridge-row` | 16 | **Screenable would mean withheld.** It is a live gene, so an opt-in row flips `ledger_default_on` from `None` to `Some(false)` and `apply_gene_ledger` takes it out of the live bridge. Its host-only classification is an argument about which rivals the weights were bred against, not a claim it cannot fire — so the move it wants is the one `culture-coverage` made, out of `FIRAXIS_ONLY_TREATMENTS` into an `ENGINE_REPAIR_*` half, taken deliberately with the bridge change owned. |
| `production-on` | 6 | Production ships it ON, so its door is `Kind::Production` — and that row is **not** neutral: `apply_gene_ledger` disables every production treatment whose `ledger_default_on` is `Some(false)`, which is exactly a screenable tag with no ledger row. Adding it would switch a shipped behaviour off; `open_water_navy` alone was promoted at +61 Elo-equivalent (200 pairs, seed 8700000, CI +21..+109, PASS on the corrected-gate matrix; see `AdvancedAi::promoted_policy_envoy`). It needs its first screen row before it can have a gene row. |
| `configured-on` | 5 | On in `AdvancedAi::configured` but not in `promoted_policy_envoy`, so `production_and_opt_in_rows_are_real` rejects the row as written, and it carries the same hazard as the group above. |
| `infrastructure` | 2 | No decision content: a cache lifetime, and a controller-wide mode that is itself an aggregate over genes that already exist. |
| `already-on` | 2 | `adjacent_camp_clear` and `barbarian_heretic_hunt` are on in `BasicAi::new`, so an opt-in row would be on in both arms and screen as exactly inert. |
| `does-not-fire` | 4 | A gene row was written for it here and `tools/gene_fires.py` refused it: a single-gene probe over 12 map pairs left both arms byte-identical. The row was removed rather than shipped to return a zero-width interval. |

⚠⚠ **A `host-only` row is also a statement about what you cannot repair.** A
gene whose value depends on a `Kind::HostOnly` behaviour firing is inert on this
screen for the same reason that behaviour is: `seat_with_genome` builds from
`enable_engine_repairs_universe()`, which is the repair halves only, so no
host-only flag is ever on in a screened seat. Worked example, 2026-08-24:
`docs/AI_GAPS.md` records a ★★★★★ live defect where the `beyond_loyalty_reach`
veto refuses 115–213 settle sites a run and idles settlers for 45–112 turns,
and names the repair — steer recon at the unexplored plots inside the vetoed
disk. `frontier_loyalty` is `Kind::HostOnly`, so **the veto never fires on the
board this screen plays**, a gene answering its question would return a
zero-width interval, and `tools/gene_fires.py --max 0` would refuse it. Check
the `Kind` of every behaviour your gene *depends on*, not only of the gene you
are adding; the evidence for that repair has to come from the live seat
(`civvis_orders --without`) instead.

⚠ The `production-on` and `live-bridge-row` rows are the finding worth
carrying out of this: **making a shipped behaviour screenable is not a
measurement-only change.** Under the pinned policy, its initial on/off state
must be chosen explicitly rather than inferred from an absent measurement.
That remains a genome decision for the operator, not a side effect of a row.

## The precision-weighted posterior, published beside the pinned genome

A threshold in column units is not a threshold in evidence. The retired
deployment rule exposed three things about the repository's own numbers:

1. **The bars are not derived from the errors.** Each screen's 80%-power band
   is in the table at the foot of `HEURISTIC_GENE_RANKING.md` and they differ by
   more than three to one — p10 ±51, p7 ±56, p4 ±60, s6 ±64, h1 ±68, s7 ±29,
   s2 ±101. A fixed +15 / −10 / +20 bar therefore decides the same reading
   differently depending only on which screen happened to price the gene, and
   #2294's single-column +20 bar sits **below every band in that table**.
2. **"Prior" is not a replication.** The two columns come from screens that
   differ in baseline (`p4` at `repairs` against `p7`/`p10` at `best`), in build
   and in shape, so "both columns positive" is not two independent
   confirmations. #2283/#2284 measured the consequence directly: five of seven
   lane genes changed sign on disjoint seeds and every flag regressed toward
   zero as the sample grew.
3. **The veto weights by games, not by precision or recency**, and it fires on
   the *sign* of a difference that carries no error at all. All three genes it
   removes are condemned by the same 2026-08-20 `p4` screen while every later
   reading is positive, and `war-economy` needs roughly 48,000 more pairs at its
   P10 margin merely to climb back over zero.

`tools/genes.py::pooled_posterior` answers all three with one estimator:
a **random-effects (DerSimonian–Laird) inverse-variance pool** of every screen's
on−off difference, on the win column's own scale, each screen weighted by its
own standard error, with the between-screen disagreement estimated as `τ²` and
carried in the interval rather than assumed away.

```text
wᵢ  = 1 / sᵢ²                    ȳ  = Σ wᵢ yᵢ / Σ wᵢ        Q = Σ wᵢ (yᵢ − ȳ)²
C   = Σ wᵢ − Σ wᵢ² / Σ wᵢ        τ² = max(0, (Q − (k−1)) / C)
wᵢ* = 1 / (sᵢ² + τ²)             m  = Σ wᵢ* yᵢ / Σ wᵢ*      se = √(1 / Σ wᵢ*)
```

`yᵢ` is `column_estimate(win_delta_pp)` and `sᵢ` is `column_se(win_se_pp)` —
both **half** the on−off difference, because a foldover holds the two arms
symmetric about chance, so `yᵢ / sᵢ` reproduces the screen's own `win_z`
exactly and the pooled figure reads directly against the win columns beside it.
The ledger records `posterior_pp` and `posterior_se_pp` per gene at six decimal
places, and the ranking prints the 95% interval and `P(effect > 0)`. Those
figures are synchronized evidence for future operator selection, not a
deployment call.

⚠ `τ` is the load-bearing term, not a refinement. When two screens agree to
within their errors it is zero and the pool is the ordinary inverse-variance
one. When they do not — a `legacy` Pangaea reading against a `standard`
continents one — it widens the interval instead of averaging two worlds into a
confident wrong answer. `POSTERIOR_SHAPES` says which shapes the published pool
admits, and `HEURISTIC_GENE_RANKING.md` prints legacy, standard and pooled side
by side so the choice is made on the numbers.

### Historical switch analysis (retired)

This section records the earlier threshold and posterior discussion so past
tables remain interpretable. `AUTHORITY`, the column threshold, and the
pooled-*Diff* veto are no longer configuration or deployment behavior. The
current policy is the explicit `OPERATOR_DEFAULT_ON` list documented above;
no statistical setting can throw a switch or move a default. Read the remaining
analysis as evidence about the old selection process, not a current decision
procedure.

⭐ **First, the thing the switch is not.** The columns rule reads the *newest*
screen that priced each gene, so the moment the standard sources landed the
deployment shape became the deciding instrument — **99 of 101 priced genes, and
33 of the 33 that ship ON, are decided by a `standard`-shape screen today.**
The genome is already chosen by the shape the game ships in. `AUTHORITY` and
`POSTERIOR_SHAPES` choose which *estimator summarises* those screens, not which
*shape decides*; the deployment-shape question this section used to defer has
already been answered by the sources, quietly, through the win columns.

**It moves one gene, and the deployment shape says that gene is nothing.**
Rebuilding the ledger under all six combinations of the two dials (#2385):

| `AUTHORITY` | `POSTERIOR_SHAPES` | genes on | moves vs shipped |
|---|---|---:|---:|
| `columns` **(in force)** | `standard, legacy` | 33 | – |
| `columns` | `standard` | 33 | 0 |
| `posterior-veto` | `standard, legacy` | 34 | 1 |
| `posterior-veto` | `standard` | 34 | 1 |
| `posterior` | `standard, legacy` | 34 | 1 |
| `posterior` | `standard` | 34 | 1 |

Two things fall out of that grid and neither was obvious in advance.

**`POSTERIOR_SHAPES` decides nothing today.** Standard-only and pooled ship the
identical genome under every authority — 0 moves in both rows. The scope dial
changes *how 15 genes read*, and for `war-economy` it changes the reading
completely (standard-only +118 ± 16 on the 23,622-pair screen, against −7 ± 63
pooled with the retired Pangaea screens, τ ≈ 124), but the columns rule already
ships that gene on, so nothing moves. **The whole delta between the two
authorities is the veto**, and `posterior-veto` produces it on its own: the
pooled estimate never overrides a column call it did not already agree with.

**The one gene the veto re-admits is `siege-commitment`, and it is the ledger's
clearest null.** Five screens have priced it, 180,912 seats, every one `~`:

| screen | shape | seats | win Δ pp | win z | share z |
|---|---|---:|---:|---:|---:|
| `p4` 2026-08-20 | legacy | 26,892 | −0.803 | −1.87 | +0.27 |
| `p7` 2026-08-21 | legacy | 30,000 | +0.053 | +0.13 | +0.92 |
| `p10` 2026-08-22 | legacy | 35,148 | +0.011 | +0.03 | +0.73 |
| standard 10k 2026-08-22 | **standard** | 47,244 | +0.119 | **+0.38** | −0.08 |
| standard 41,628 (#2374, report-only) | **standard** | 41,628 | −0.199 | **−0.55** | +0.61 |

The two deployment-shape readings straddle zero with opposite signs, and the
23,622-pair screen's own three replication tranches flip sign inside one run:
+0.20 (z +0.42), −0.26 (z −0.54), +0.94 (z +1.19). Its pooled `Diff` is
negative in every source combination (−0.10% today, −0.12% with the 41,628-seat
screen added), which is what the shipped veto fires on.

⚠ **The flip is an artifact of which screens are in the ledger this week.**
Enter the 41,628-seat standard screen as a *source* rather than report-only and
`columns` ships 36, `posterior` + standard-only ships 36, and **the flip table
is empty** — `siege-commitment` reads −10 in the last win column and both rules
hold it off. So throwing the switch today would ship one default ON that the
next screen takes straight back off.

**The rule that decided: a negative pooled `Diff` vetoes a default-on.** The
only thing either posterior authority does today is override that veto for a
gene whose deployment-shape record is +0.38σ and −0.55σ across 88,872
standard-shape seats. That is a default the numbers do not carry, and it is the
same shape of mistake as the one this section's own history records —
`governor-victory-lanes` shipped on a single +46 column and cost −237 wins per
10,000 on-arm seats (95% CI [−267, −206], 23,622 pairs).

**The arm was run anyway, and it is `~`.** A pre-registered single-gene arm at
the deployment shape — `gene_screen --genes siege-commitment --games 600
--target-games 600 --start-seed 191000000 --jobs 4`, seeds 191000000+, disjoint
from every window in the ledger (141M whole-genome, 150M `g1`, 168M 10k, 169M
41,628) — was stopped at **28 of 600 games, 168 of 3,600
pre-registered seats (4.7%)**, and the tool labels it `⚠⚠ PARTIAL SCREEN`
accordingly. It reads **+6.8 pp (91 on / 77 off), win z +1.23, 95% CI
[−4.1, +17.6]**, share +1.33 pp at z +1.65 — **`~`, unresolved**, on a run that
resolves only ±15.5 pp at 80% power. That interval contains **both**
deployment-shape readings above (+0.119 and −0.199) and discriminates neither;
it is consistent with every number in the table and with zero. ⚠ It was stopped
because the box was running at a load average of 78–150 on 18 cores with up to
13 concurrent `gene_screen` processes; win and share figures are counts and are
unaffected by that, but no wall-clock or per-game cost figure from this run is
quotable and none is published.

⚠ **A direct arm cannot settle this gene.** `python3 tools/genes.py boundary`
sizes it at **788,779 seat pairs** before the interval clears zero. This
binary prints its own 80%-power resolution per run — ±34.8 pp on 6 games — and
it falls as 1/√games, so a **600-game arm resolves ±3.5 pp** and even the
1,200-game equivalent of #2344's 600 *map pairs* resolves **±2.5 pp**. Against
a gene reading +0.119 pp that is **29× the effect**, and it is **4.0× wider**
(600 games) or **2.8× wider** (1,200 games) than the 23,622-pair screen already
in the ledger, whose own band is **±44 in column units** — the figure the band
table at the foot of `HEURISTIC_GENE_RANKING.md` prints — i.e. ±0.88 pp on the
on−off difference. ⚠ Both figures above are quoted on the **difference** scale,
which is what `gene_screen` prints; a column is half of it (#2300), so the arm
comparison is ±175 against ±44 in column units and the ratio is the same either
way. Do not compare one scale to the other. The confirmation the priority list
asks for is a *weaker* instrument than the source it would supplement. That is
the arithmetic reason the answer here is "the evidence is already in", not "run
more games".

### Using this evidence now

The selection changes only when an operator explicitly edits the pinned list.
For a proposed change, read the relevant win, *Diff*, posterior, shape, and
lane evidence together, record the rationale in the review, then regenerate
the ledger. A straddling interval is a reason to gather more evidence or defer
the operator choice; it is never an automatic fallback to the former column
rule.

## Two stages, and why not a partial foldover

The efficient plan is **two-stage**: the whole-genome screen ranks and a
single-gene run (`--genes tag`) resolves. Both are efficient, at different
jobs, and the arithmetic that says so is already in the repository's screens.
Do not re-derive it into a partial or blocked screen — randomising a subset of
the genes while holding the rest — which is neither stage. (The figures below
were measured on the foldover batches of 2026-08-20..22 and are stated in
that design's matched seat pairs — two seats each; the conclusion does not
depend on the pairing.)

**Stage one — the whole-genome screen RANKS.** `p10` priced 75 genes at ±51
each on 17,574 seat pairs. Spend the identical budget as 75 single-gene screens
and each gets 234 pairs; even at the best single-gene pairing gain this
repository has measured (`s7`'s 3.32× against `p10`'s 1.09×), 234 pairs resolve
**±145**. That is 2.84× wider than ±51, and because error falls with √N it means
**8× the games** to reach the same resolution per gene. Every game informs every
gene, which is the whole reason the screen exists.
`tools/test_genes.py::test_the_eight_times_figure_is_the_screens_own_arithmetic`
recomputes those three numbers from the ledger's own screens, so this paragraph
cannot go stale while the screens under it move.

**Stage two — a single-gene direct arm RESOLVES.** Once aimed, a single-gene
screen resolves far tighter *per pair*, because a foldover cancels only what its
two arms play in common and one gene left flipping leaves most of the game
identical: `s7` reads ±29 on 6,000 pairs at a 3.32× pairing gain, against
`p10`'s 1.09× (#2302). So the screen says *which* genes are still in doubt and
a direct arm settles them.

`python3 tools/genes.py boundary` is stage two's worklist. It
lists every gene whose posterior interval straddles zero, ranked by the expected
value of one direct arm read against the gene's **shipped** state — so a gene
the evidence likes that the rule holds off has the whole effect to buy, and one
the genome already plays has only the chance of a reversal — and prints, per
gene, how many matched seat pairs an arm needs before the combined interval
clears zero. It ends in a `--genes a,b,c` list sized to one batch. The arm's
precision is taken from the widest single-gene arm the repository has actually
run (`2026-08-22-h1`, 24.3 per-column SE at 7,200 seat pairs), which is the
conservative end: a gene that rarely fires cancels far more and resolves
tighter.

⚠ What is **not** efficient is a partial foldover — randomising a subset of the
genome and holding the rest fixed. It pays the whole-genome screen's residual
(every randomised gene's draw is in every other gene's error) without pricing
the whole genome, and it does not buy the direct arm's cancellation either,
because the arms still differ in many genes. `s6` is the measured example: four
genes over 6,000 pairs resolves ±64, *wider* than one gene over 7,200 (`h1`,
±68) is close to and far wider than one gene over 6,000 (`s7`, ±29). Rank with
everything or resolve with one; the middle is the expensive place.

## A `~` on a later screen is not a refutation: `buildings-before-projects`

#2385's disjoint-seed replication pass flagged three genes that the standard
screen had just promoted. Two shrank and held; the third stopped clearing the
bar:

| gene | 23,622-pair screen | 41,628-seat screen (disjoint) | #2385's read |
|---|---:|---:|---|
| `war-economy` | +2.354 pp, z +7.50 | +1.560 pp, z +3.76 | replicates, smaller |
| `air-surge` | +2.151 pp, z +6.99 | +1.315 pp, z +3.17 | replicates, smaller |
| `buildings-before-projects` | +1.228 pp, z +3.95 | +0.385 pp, z +0.92 | **`~`** |

That third row was called out as *"a `columns`-rule promotion worth a direct
arm"* on a gene that ships **on**. It was followed up in #2393, and the answer
is that **no arm is owed, the default is right, and the flag was a statement
about the second screen's power rather than about the gene.** The general rule
it establishes is at the bottom of this section; the arithmetic is why.

### What the whole record says

Six six-player whole-genome screens have priced this gene — the four ledger
sources plus the two `standard` batches #2374 entered as *reporting* batches
(seeds 168000000–168001666 and 169000000–169006937, disjoint from each other
and from the 141000000 discovery window):

| screen | shape | seats | win Δ pp | win z | share Δ pp | share z |
|---|---|---:|---:|---:|---:|---:|
| `p4` 2026-08-20 | legacy | 26,892 | +0.461 | +1.07 | +0.232 | +3.37 |
| `p7` 2026-08-21 | legacy | 30,000 | +0.520 | +1.33 | +0.141 | +2.12 |
| `p10` 2026-08-22 | legacy | 35,148 | −0.046 | −0.13 | +0.161 | +2.52 |
| standard 10k 2026-08-22 | **standard** | 47,244 | +1.228 | +3.95 | +0.282 | +4.89 |
| standard 10,002 (#2374) | **standard** | 10,002 | +0.919 | +1.10 | +0.159 | +0.93 |
| standard 41,628 (#2374) | **standard** | 41,628 | +0.385 | +0.92 | +0.060 | +0.70 |

**Five of six positive on the win axis, six of six positive on share.** The
two disjoint deployment-shape replications both read positive. Nothing in the
record points down.

### The replication does not contradict the discovery — it lacked the power

Two numbers settle it, and both are one line of arithmetic on figures already
in the artefacts:

- **The two readings are not distinguishable.** +1.228 ± 0.311 against
  +0.385 ± 0.417 is a difference of **+0.843 ± 0.520, z +1.62, p = 0.105**.
  The shrinkage is inside noise, and it is the winner's-curse signature
  `docs/EVAL_INTEGRITY.md` §4 predicts for a figure selected on promotion.
- **The 41,628-seat screen's power against the effect it was testing was
  22–26%.** At `win_se_pp` 0.417 it had 84% power against the discovery
  estimate (+1.228 pp) — which is why its failure to reproduce that *size* is
  informative — but only **26%** against the six-screen pooled difference
  (+0.552 pp) and **22%** against the post-discovery estimate (+0.492 pp).
  A `~` at 22% power is the expected outcome of a true positive effect, not
  evidence against one.

### The ranking already prints the resolved answer

`HEURISTIC_GENE_RANKING.md`'s main table pools **every** screen that priced a
gene, reporting batches included. Its row for this gene reads on 16.91%
(n = 108,278 on-arm seats) against off 16.35% (n = 82,636), *Diff* +0.55%,
posterior **+28 [+7, +49]** wins per 10,000 on-arm seats, **P(>0) = 99.6%** —
an interval that **excludes zero**.

⚠ **The ranking prints two different posteriors for the same gene, and the
distinction is what made this look like an open question.** The main table
pools `load_display_sources` — ledger sources *plus* the report-only batches,
six screens here — while the evidence table, the shapes-apart table and
`boundary` all pool `load_sources`, the four authoritative sources alone. That
is deliberate: the deployment ledger stays byte-for-byte tied to its own
sources, and report-only data refreshes the display without moving a default.
The consequence for a reader is that the same generated file says
**`+28 [+7, +49]`, 99.6%** on one line and **`+28 [−0, +57]`, 97.4%** on
another, and the second is the one printed beside the pinned selection. Read
the main table's posterior for *what the evidence says* and the evidence table
for *what the ledger's sources say*; for this gene the gap between them is
precisely the two disjoint standard screens #2374 held back.

Pooled over the three deployment-shape screens alone (98,874 seats), the gene
reads **+0.926 ± 0.239 pp, z +3.88, 95% CI [+0.458, +1.394]**, with
heterogeneity Q = 2.63 on 2 df — the three screens agree. The unselected half
on its own (the two #2374 batches, 51,630 seats, neither run to price this
gene) reads **+0.492 ± 0.373 pp, 95% CI [−0.239, +1.223]**: positive,
unresolved alone, and containing both the pooled figure and the discovery
estimate. Share pools to **+0.209 ± 0.046 pp, z +4.54** over the three
standard screens.

### The pinned default is on; the historical rules also agreed

Rebuilt from the ledger's own recorded sources, with no file edited:

| sources | `wins_last_10k` | `wins_prior_10k` | pooled *Diff* | `columns` |
|---|---:|---:|---:|---|
| as it ships (4 sources) | +61 | −2 | +0.606% | **on** |
| + the 41,628-seat screen entered | +10 | +61 | +0.540% | **on** |
| + both #2374 batches entered | +10 | +24 | +0.552% | **on** |

The pooled *Diff* is positive in all three, and the posterior over all six
screens excludes zero above. Those former-rule readings agree with the current
explicit selection, but neither can change it without an operator edit.

### Sizing: nothing affordable resolves it on its own

This is the same question #2385 asked of `siege-commitment`, and the answer
has the same shape for a different reason. There, a 600-game arm was **4×
wider** than the screen already in the ledger, so the confirmation would have
been a weaker instrument than its own source. Here the arm is not wider — on
the bound `boundary` sizes from it is the *same* instrument — and the effect
is simply smaller than one batch of it resolves:

| instrument | `win_se_pp × √pairs` |
|---|---:|
| direct arm, from `g1` — `boundary`'s conservative/widest bound | **46.9** |
| this gene on the 23,622-pair whole-genome foldover | **47.8** |
| this gene on the 41,628-seat independent screen | 60.2 |

⚠ **This gene's own cancellation gain is unmeasured**, and 46.9 is a bound
rather than a reading: `direct_arm_constant` takes the *widest* single-gene arm
the repository has run, because a rarely-firing gene cancels more and resolves
tighter. So the pairs below are an **upper bound on n**, and the honest range
is wide — at `h1`'s 1.28× (#2302) 80% power against the unselected estimate is
15,064 games, at `p10`'s 1.09× it is 20,774, and at `s7`'s 3.32× — the best
this repository has measured — it falls to 2,239. Structurally this gene is at
the `h1` end: it is a production-queue rule that applies in every city holding
a district and a buildable building, so the two arms of a foldover diverge in
nearly every game and there is little to cancel. That is an argument, not a
measurement, and the sizing below deliberately uses the bound `boundary` uses:

| target effect | z ≥ 1.96 | 80% power | vs one standard batch |
|---|---:|---:|---:|
| discovery +1.228 pp | 1,869 games | 3,819 games | 0.4× |
| standard-shape pool +0.926 pp | 3,286 games | 6,714 games | 0.7× |
| six-screen *Diff* +0.552 pp | 9,249 games | 18,897 games | 1.9× |
| unselected +0.492 pp | 11,643 games | 23,789 games | **2.4×** |

And what an affordable arm resolves, against an effect of about +0.5 pp:

| arm | 95% half-width | narrowing of the pooled standard-shape interval |
|---|---:|---:|
| 172 games (`boundary`'s size, below) | ±4.05 pp | **0.7%** |
| 600 games (#2385's pre-registered size) | ±2.17 pp | **2.3%** |
| 3,600 games | ±0.88 pp | 11.6% |
| 23,789 games (80% power at the bound) | ±0.34 pp | 40.8% |

A 600-game arm is 4.4× wider than the effect and moves the standing interval
by 2.3%. The confirmation is, again, a weaker instrument than its own source.
And even at the *pooled* effect size, 80% power needs 18,897 games — nearly
two whole standard batches, and 31× the 600-game arm #2385 pre-registered.

⭐ **The part that does not depend on the unmeasured cancellation gain**: there
is nothing left for an arm to resolve at *any* size. The three deployment-shape
screens already pool to z +3.88, the ranking's all-screen posterior already
excludes zero, and the gene already ships on — so the only thing an arm can buy
is the chance of a reversal, which is what `buys +0.0` says. Even if this gene
turned out to cancel like `s7` and a 2,239-game arm reached 80% power, it would
be 2,239 games spent to re-confirm a call no rule in the repository disputes.
**Nothing runnable settles this gene on its own, and the three deployment-shape
screens already in the repository settle it together.**

### ⚠ `boundary`'s `needs` column read without its `buys` column is a trap

`python3 tools/genes.py boundary` prints **516 seat pairs — 172 games — for
this gene**, the smallest number in the table. That is not a cheap
confirmation. `arm_pairs_to_resolve` answers "how big an arm tips the
*combined* interval **if the arm reproduces the current pooled mean**", and
this gene's ledger posterior sits at z = 1.94, a hair under the line, so
almost any positive reading tips it. An arm of that size resolves **±4.05 pp**
— **8× the effect it would be used to certify**. Running it and reporting the
combined interval would be manufacturing a significance, with a tool's
blessing, out of 172 games of noise.

The tool already says so in the next column: **`buys +0.0`**. `--boundary`
sorts on the expected value of the arm against the gene's *shipped* state, and
a gene the evidence already likes that the genome already plays has only a
reversal to buy. `buildings-before-projects` sits 76th of 84 on that ordering.
**Read `buys` first; `needs` is only meaningful for a row `buys` has already
put near the top.**

### What did change: the size, not the sign

The +61 win column is a **discovery estimate**: it is the reading that
flipped this gene on, so it is selected on having passed, and §4's
`E[observed | gate PASS] > true effect` applies to it in full. The honest
deployment-shape figure is **+45 [+16, +73]** per 10,000 on-arm seats pooled
over the three standard screens, and the honest *unselected* figure is
**+25 [−12, +61]**. The +61 should not be quoted as this gene's effect size,
and §4's corollary — a replication that refutes a documented *size* must land
in the document that carries the size — is discharged by this paragraph.

### The rule, so the next agent does not repeat the pass

> **A single later screen reading `~` refutes nothing until its power against
> the pooled effect is stated.** Before calling a gene a failed replication,
> compute three things from artefacts that already exist: whether the two
> readings differ by more than their errors, the later screen's power against
> the *pooled* effect rather than against the discovery estimate, and the
> pooled reading over every screen that priced the gene. Only when those
> disagree with the shipped default is an arm owed — and then size the arm
> against the pooled effect, never against the discovery estimate, because
> sizing on the number selected for being large is how a confirmation ends up
> too small to confirm anything.

The two-line check for any gene, costing no games:

```sh
python3 tools/genes.py boundary | grep '<tag>'      # read `buys`, then `needs`
grep '`<tag>`' HEURISTIC_GENE_RANKING.md      # all-screen posterior, P(>0)
```

⚠ Applied to #2385's other two rows: `war-economy` and `air-surge` both
replicate at significance on the disjoint window, so neither was ever in
question. This section is only about the row that did not, and its conclusion
is that the row was mis-read, not that the gene was mis-shipped.

## Pre-registered: how a lane gene is judged

⚠⚠ **Fixed before the next screen, deliberately, so the axis is not chosen
after the numbers arrive.**

At the standing 250-turn Online clock, science and diplomatic victories land at
median **t283 and t285** — past the clock — so they are 1–2% of endings and
`docs/VICTORY_GENES.md` records **science 0/8** and **diplomacy 1/8** for
exactly that reason. A science or congress gene therefore **cannot pay through
the win axis at all**: the seat it would have carried to a science victory shows
up as a score win or a score loss instead. Judging such a gene by its win column
is not a strict test, it is a test of nothing.

**Who is a lane gene.** Discovered from the code, never listed: every gene whose
flag field `src/ai/advanced/victory_lane.rs` reads
(`tools/genes.py::lane_tags`). A gene joins the set by being a
lane gene, not by being written into a list that is complete the day it is
written.

**The rule, pre-registered.**

1. **The deployment choice is explicit.** A lane gene is on only when the
   pinned deployment genome names it, exactly like every other gene. Nothing
   below promotes anything automatically. `docs/GENOME.md` records what
   happened the one time selection ran on a correlate, and
   `docs/eval/README.md`'s rule stands: a screen's `*` is where to point an arm.
2. **The secondary axis is score share**, printed for every gene as
   *Share Δpp (z)* in `HEURISTIC_GENE_RANKING.md` and listed again for the lane
   genes alone. Its verdict is the screen's own `*` convention on that axis:
   `helps *` at share z ≥ 2, `hurts *` at ≤ −2, `~` otherwise.
3. **A lane gene whose share reading is `hurts *` is a removal candidate even
   with a positive win column.** The share axis is continuous and resolves an
   edge at a fraction of the games a win count needs; a lane gene that loses
   score share while its win column sits inside the band is losing, and the win
   axis at this clock cannot see it.
4. **A lane gene whose share reading is `helps *` with a win column inside the
   band is a candidate for a direct arm, never a promotion.** It is on the
   `--boundary` list like anything else.
5. **A lane gene resolved on neither axis after two screens goes to the bottom
   of the ranking with everything else** and is culled on the standing
   directive. Being hard to measure is not a reason to keep a gene; it is the
   reason its lane needs a clock it can finish on.

⭐ The former genome's own worst row is the case for this.
`governor-victory-lanes` read win z **+2.46** and share z **−15.92** on `p10`
— a recorded `conflict` — and #2294's single-column clause promoted it on the
+46 win column because the rule reads the win axis only. The first
standard-shape screen's **win** axis reads it at z **−15.37**, within half a
sigma of what the legacy **share** axis had already said. The 2026-08-24 cull
subsequently removed the gene under the explicit negative-Diff threshold;
`docs/gene_ranking_notes.md` carries the historical numbers.


## ⭐ THE CONTESTED FIELD (2026-08-24): a screen with something to deny

The screen above draws every seat's genome from one controller and reads a gene
as seats-on against seats-off. It is a good instrument for what it measures and
**structurally blind to what actually beats us**:

| ending | the standard screen | the live seat, against Firaxis' AI |
|---|---:|---:|
| diplomatic | 0–1% | **32 of 74 rival wins** — 19.6% of terminal games |
| culture | 11–18% | **27 of 74** |
| religious | 28–48% | 8 |
| domination | 0% | 0 |

Diplomatic and culture take **83% of every early loss on the live seat**, and
in the fieldless screen they barely happen. So every **denial** gene in the
tables has been priced against a field that never threatens the thing the gene
denies. That is not a hypothesis about the instrument; it is a mistake already
on the record. `congress_counter_leader`'s own field doc declines the
`world_leader` veto because a census found *"no diplomatic victory in 40 games.
There is no headroom there to take"* — a census taken headless, in a regime
where diplomatic victories do not happen at all, while on the live seat
diplomacy is the single largest killer. Two counter flags are off on that
reasoning.

`--contested` is the instrument that can ask the question. **It is an added
mode, never a redefinition**: `gene_screen --games N --out rows.jsonl` still
plays exactly the screen this document opens with, every recorded column keeps
comparing, and a contested batch is refused as a ledger source.

### What it does

| leg | contested | the screen |
|---|---|---|
| majors | 6 | 6 |
| **pinned pursuers** | **2 — one `diplomatic`, one `culture`** | none |
| measured seats | **4 drawn genomes** | 6 drawn genomes |
| **native scored competitions** | **on** | off |
| map, size, city-states, speed, clock, lanes, civ shuffle | unchanged | unchanged |

A pinned seat is `AdvancedAi::new()` — the deployment genome, the rival the
agent actually meets — handed to `AdvancedAi::retarget(lane)`. That is the same
call the rollout planner and the retired `live_target_<lane>` arms used, so the
pursuit is the controller's real victory-lane behaviour and not a label:
`victory_focus` resolves to the assigned lane, and the congress ballot, the
Great Person race, the policy deck, the culture spending pass and the space race
all read it. A pinned Diplomacy seat is also the only kind of seat that scores
`world_leader` outcome A **on itself** at 1,000 — the branch that nominates an
empire for the +2 Diplomatic Victory Points that decide that lane.

Three properties are load-bearing and each is a test:

1. **The pinned seats are not measured.** No row is written for them. They are
   the threat, not the observation, and mixing a pursuer's row into a denial
   gene's arms would price the gene on the seat it is not for.
2. **The pinned positions rotate with the game index.** Seat position is not
   neutral on this board — the note on `--stock-civs` records seats 0 and 2
   winning twice as often as seat 3 whoever sat there — so a fixed pin would
   confound the field with the chair.
3. **The field is constant across the batch.** It draws no genome, so it adds
   no variance to any gene's contrast.

⚠ A fourth was **tried and reverted**. Seating the pursuers with the seven
victory-lane opt-ins — the deciders that read the raced lane, all of which ship
off — is the obvious way to make a pursuer race properly, and it measured
*worse*: the pursuers held their own lane's lead in 4 of 35 games against 8 of
27 for the deployment genome, and neither field ever won a game.
`--contested-field-genes lanes` keeps it behind a flag;
`CONTESTED_FIELD_GENES` in `gene_screen.rs` carries the arithmetic.

### The two boards are the same maps, seat for seat

`draw_genome` is keyed on `(start_seed, game, players, seat)` and the map on the
seed, and none of those move when a field is pinned. So a contested batch run at
the same `--start-seed` as a fieldless one is **the fieldless batch with two of
its six seats replaced**: the same maps, the same civilizations, and the four
measured seats carrying the same drawn genomes they carried without the field.
Nothing else differs, which is what makes a before/after census a measurement of
the field rather than of two unrelated batches.

### Why native competitions come with it

⚠ **The diplomatic lane needs a route to 20 points to exist at all.** A native
CIVVIS game's Diplomatic Victory Points come from the congress (±2 from the
Modern era), three wonders that 31 of 32 diplomatic games finish none of, and
two Future-era tree nodes worth 1 each. The competition sources that pay through
the whole second half of a real game — and that make the live board's diplomatic
victories land at a median of turn 234 — are `Game::native_competitions`, which
**ships off**. Pinning a seat to a lane it has no route to finish is the
cosmetic version of this feature, so `--contested` turns the flag on.

This needed no engine change: `native_competitions` is a field on `Game` that
`civvis simulate --native-competitions` already sets, and the screen sets it the
same way. It also gives `competition-victory-points` the first regime it can
fire in — see the debt recorded at the bottom of `HEURISTIC_GENE_RANKING.md`.
`--native-competitions` without a field is available on its own, and the census
below is the reason to reach for it.

### ⚠⚠ It is refused as a ledger source, and that is the whole safety property

A contested batch differs from the standard screen in **no map leg at all** —
same players, map, size, city-states, speed, clock, lanes and civ shuffle. Two
header legs are what tell them apart, `contested_field` and
`native_competitions`, and both `shape_of` in `gene_screen.rs` and `FIELDLESS`
in `tools/genes.py` refuse on them. Without that check a contested batch would
read `standard` and pool with the ledger, re-pricing a hundred genes against a
board none of them was measured on.

⚠ The leg is `contested_field`, not `field`. Every header the retired paired
designs wrote already carries a `field` — the name of the agent the treated seat
played against — and nine of them are recorded sources; naming the new leg
`field` reclassified all nine and `tools/genes.py check` reported drift on the
ledger's own history. The gate caught it; the name is the fix.

### The standard screen still plays the same games, checked rather than argued

Fieldless, `pinned_seats` pins nobody and `native_competitions` is set to the
`false` it already was, so nothing about a standard game changes. That is an
argument, and the repository's rule is that an argument is not a measurement, so
it was run: `origin/main`'s `gene_screen` and this one, built from the same tree
and the same lockfile, played the same **6 games** — seeds 77000000..77000005,
bare defaults, `--genes strike-opening` — and every one of the 24 fields both
binaries write was compared on all **36 rows**. **0 rows differed.** The rows do
gain five new columns, all `#[serde(default)]` end-of-game reads that no
simulation consumes.

### What the rows carry now

`dvp` and `rival_dvp` (this seat's Diplomatic Victory Points and the best other
major's, against the 20 a diplomatic victory needs), `tourists` and
`rival_tourists` (visiting tourists, what a culture victory is decided on) and
`domestic` (this seat's domestic tourists — the bar a culture pursuer has to
clear, because `check_culture_victory` has no fixed threshold). All
`#[serde(default)]`, so every earlier file still analyses, and the census that
reads them prints on a fieldless batch too — so the screen's own answer to *"was
anybody even racing?"* is visible beside the contested one rather than argued.
The analysis JSON carries the same counts in an `endings` block, so a committed
artifact proves its own census instead of leaving it in a terminal transcript.

### `--analyze --denial`: the axis a denial gene is actually read on

The win column answers *"does this seat win more"*. A denial gene is not for
winning more; it is for stopping somebody else winning, and on a six-player
board those are different numbers — a denial that works hands the game to one of
the four empires that are not us about four times in five, and the win column
cannot tell that from nothing happening. `docs/FIDELITY.md` records the live
seat losing 107 of 299 terminal games to a rival's victory, **15 of them while
leading on score**; that is the axis those games live on.

`--denial` prints, per gene, the change in how often the seat **lost** to a
rival's victory of each kind that actually ended games in the batch, estimated
exactly like the win column — seats-on minus seats-off, errors clustered by game
— so it is read with the same bars and the same warning: a hundred genes at
|z| ≥ 2 flag about 4.5 of them by chance.

### What it can and cannot see

**Can.** Whether a gene changes how often the seat loses to a rival's culture or
diplomatic victory, on a board where those endings actually happen; whether a
gene helps or hurts a seat that is under that pressure; and — for the first time
— `competition-victory-points`, whose branch no fieldless screen can enter.

**Cannot.**

- **It is not the ledger.** No column here moves a default. The explicit pinned
  deployment genome (`docs/gene_ledger.json`, `tools/genes.py`) is unchanged,
  and this board is refused as a source by construction. What
  a contested reading is for is deciding whether a gene deserves a direct arm,
  and for catching the opposite error: a gene declined on a census taken where
  the lane it denies cannot complete.
- **It does not make the field an equal.** A pinned seat trades the adaptive
  planner for one lane, and `assess` still returns `Expansion` for it while it
  is short of cities — measured at 19.7% of a diplomatic seat's turns and 20.6%
  of a culture seat's (`src/ai/advanced/victory_lane.rs`). In the census below
  the pursuers won **0 of 62** contested games across both field genomes. They
  are lane-shapers, not stronger players, and a reader should size their effect
  from the census rather than from the intent.
- **It cannot price a gene that has no gene row.** `congress_counter_leader` —
  the flag whose own doc carries the "no headroom" finding — has no `enable`/
  `disable` pair and no row in `src/ai/advanced/genes.rs`, so `gene_screen`
  cannot vary it at all, on any board. Registering it is a separate change to
  the registry and is the obvious follow-up. `congress_counter_votes` is
  screenable and is what this round priced.
- **Four measured seats, not six.** A contested game yields two thirds of the
  rows a fieldless one does at very nearly the same price; the cost section
  below turns that into the number to budget from.

### The census: what changed on the board

⚠ Read the n before the percentages. These are **tens of games**, taken on a
host shared with the rest of the fleet at one-minute load averages between 65
and 130, and they fix directions and orders of magnitude — not rates. Arms A, B
and C are the same 74×46 continents board at Online's 250-turn clock with the
same five congress genes screened, all started at `--start-seed 92000000`, so
their maps and their measured seats' drawn genomes are identical and only the
two rival chairs and the competitions move. Arm D moves one further leg — the
clock — and runs on its own seeds, 94000000..94000023.

| arm | games | score | religious | culture | **diplomatic** | science |
|---|---:|---:|---:|---:|---:|---:|
| **A** fieldless — *the standard screen* | 35 | 66% (t250) | 14% (t170) | 11% (t224) | **0%** | 9% (t243) |
| **B** fieldless + native competitions | 30 | 60% (t250) | 20% (t236) | 13% (t224) | **3%** (t245) | 3% (t247) |
| **C** contested — two pursuers + competitions | 27 | 37% (t250) | 44% (t206) | 11% (t223) | **4%** (t246) | 4% (t243) |
| **D** contested at a **400**-turn clock | 24 | 0% | 25% (t182) | **25%** (t238) | **8%** (t305) | 42% (t276) |

Games in which **some empire reached the 20 Diplomatic Victory Points** a
diplomatic victory needs: **A 0 of 35 · B 1 of 30 · C 1 of 27 · D 2 of 24** — the `endings` block of
each artifact carries the same counts, so the table can be checked against its
own evidence rather than trusted.
The best total any empire reached in arm A was **19**.

Three things fall out of that table, and the third is the one that was not
expected.

**1. Native competitions are what open the diplomatic lane, and the pins are
not.** Arm A never reaches 20 points in 35 games and never ends
diplomatically. Arm B — the same 30 maps, the same measured seats, *no field at
all*, competitions on — crosses 20 and produces this instrument's first
diplomatic ending. Arm C adds two committed pursuers on top and produces the
same 1 game in 27. ⚠ At **tens of games** the pins' contribution is
**unresolved**, not zero, and saying otherwise would be the exact error this
mode exists to correct — but nothing here yet shows the pins buying a
diplomatic ending that the competitions had not already bought. **For a
diplomatic-denial question today the cheaper instrument is
`--native-competitions` with no field**: it measures six seats a game instead
of four, on a board where the lane completes.

**2. The pins do change the board, and one of the changes is a bias.**
Religious endings run 44% in arm C against 14% in arm A. That is what pinning
two of six empires out of the faith race does — the remaining four contest it
against two seats that will not, and the pursuers' cities are conversion
targets. A religious-denial gene must therefore not be priced on a contested
field, and any gene read there should be read against arm B rather than arm A.

**3. The 250-turn clock is what binds the diplomatic lane, not the content.**
Arm D moves one leg — the clock — and the diplomatic ending lands at **turn
305**, fifty-five turns past the standard clock, while culture triples to 33%
and score endings vanish. `docs/FIDELITY.md` records a rival's diplomatic
victory on the live seat landing at a median of turn 234 and never before 202;
natively the same lane lands at a median of t285. A 250-turn screen sees the
**edge** of this lane, and a longer clock is the only thing that shows the whole
of it. ⚠ That is a description of what the instrument can see, **not** a
proposal to move the standard screen: 250-turn Online is the operator's
standing regime and a game that reaches the clock is a score victory.
### The first denial gene priced on a board that threatens something

`congress_counter_votes` backs the ballot opposing the empire closest to
winning with everything the treasury can spare. It ships **off**, and the
reasoning that keeps its sibling off is the sentence in
`congress_counter_leader`'s field doc: the `world_leader` veto *"already lands
95–98.5% of the time with **no diplomatic victory in 40 games**. There is no
headroom there to take."*

⚠ **`congress_counter_leader` cannot be priced by this or any screen.** It has
no `enable`/`disable` pair and no row in `src/ai/advanced/genes.rs`, so
`gene_screen` cannot vary it on any board. Registering it is a separate change
to the registry and is the obvious follow-up; `congress_counter_votes` is the
screenable half and is what was measured.

Every board, same gene, same five-gene draw, seats and interval beside each
number:

| board | seats | win Δ | 95% CI | z | Δ in *lost to a rival's diplomatic victory* |
|---|---:|---:|---|---:|---|
| A fieldless (the screen) | 210 | +7.0 pp | [−2.6, +16.7] | +1.42 | **no such column — 0 games ended that way** |
| B fieldless + competitions | 180 | +1.5 pp | [−8.7, +11.7] | +0.29 | −0.87 pp (z −0.88), base 2.8% |
| C contested | 108 | −0.0 pp | [−15.9, +15.9] | −0.00 | −2.50 pp (z −1.03), base 2.8% |
| C′ contested, lane-gene pursuers | 140 | −7.1 pp | [−21.0, +6.7] | −1.01 | (no diplomatic ending in this arm) |
| the ledger's standard screen (#2362) | 23,622 | −0.25 pp | — | −0.83 | — |

**Every one of those is unresolved and every interval contains zero.** Nothing
here promotes or demotes anything, and a reader who takes the +7.0 pp as a
result would be repeating the mistake this section is about. What the table
does establish is the thing the brief was written to test:

- **On the standard screen this gene has no denial axis at all.** Not a small
  one — *none*: the column cannot be computed, because in 35 games no seat ever
  lost to a rival's diplomatic victory and no empire ever reached 20 points.
- **On a board where the lane completes, the axis exists and its sign is
  denial** on both such arms (−0.87 and −2.50 percentage points), at a base
  rate of 2.8% that tens of games cannot resolve.

So the *"no headroom"* finding does **not** survive as evidence — not because
the gene was shown to help, but because the census that produced it could not
tell "no headroom" from "no lane", and the control now says which it was. Two
counter flags are switched off on that reasoning, and the instrument that can
re-ask the question exists as of this document. **What it needs is n**: at the
2.8% base rate these arms produce, resolving a denial of a third of that lane
at 80% power needs on the order of ten thousand seats, which is one overnight
batch on a quiet host and not a claim anybody should make from thirty games.
### Cost: 1.18× a game, 1.76× a measured seat

Measured as **CPU seconds** (`user + sys`, not wall clock), 12 games an arm,
the full default screened gene set, `--jobs 8`, the two arms back to back on
the same seeds (91000000..91000011):

| arm | CPU-s per game | CPU-s per **measured seat** |
|---|---:|---:|
| fieldless | 38.6 | 6.44 (6 seats a game) |
| contested | 45.4 | **11.35** (4 seats a game) |
| | **+17.6%** | **+76.4%** |

⚠ **The per-seat number is the one to budget from.** A contested game costs a
fifth more and yields two thirds as many rows, so a contested seat costs
**1.76× a fieldless one**. For scale, `joint-tactics` is held out of the
default screened set at 2.5× and this document calls that a real budget
decision; this is in the same territory. Arm B — `--native-competitions` with
no field — keeps all six seats and is the cheap way to buy the diplomatic lane.

⚠ **Provenance for the numbers themselves.** Taken on `mbp-m5-max-128` with the
one-minute load average moving between **19.9 and 38.0** across the two arms
(recorded in `target/screens/cost.load` at run time), on a host shared with the
rest of the fleet. CPU time is far less sensitive to scheduling than wall clock
and both arms were measured minutes apart under the same conditions, so **the
ratio is the reusable number**; the absolute figures are **not** comparable
with the ~52 CPU-s/game recorded earlier in this document, which was a
different commit on a quiet host. 12 games an arm is a cost probe, not a cost
census.

### The artifacts

Every number above is read out of a committed analysis, and each carries its
own `endings` block, its build stamp and its pre-registered size:

| arm | file under `docs/gene_screens/` |
|---|---|
| A fieldless control | `2026-08-24-fieldless-control-for-the-contested-field.json` |
| B competitions, no field | `2026-08-24-native-competitions-without-a-field.json` |
| C contested | `2026-08-24-contested-field-pursuers-on-the-deployment-genome.json` |
| C′ contested, lane-gene pursuers | `2026-08-24-contested-field-pursuers-with-the-lane-genes.json` |
| D contested at 400 turns | `2026-08-24-contested-field-on-a-400-turn-clock.json` |

Arm A reads `"shape": "standard"` and should — it **is** the screen, played on
35 games as the control, and that is exactly what makes it a valid "before". It
is a 210-seat partial run of a 360-seat pre-registration and says so; nobody
should record it as a source. The other four read `"shape": "legacy"`, and
`tools/genes.py` refuses each of them at the write path — the contested three on
`contested_field`, arm B on `native_competitions` alone.

## ⭐ Rotating victory masks: `--victory-mask rotate:N` (2026-08-25)

The standard screen leaves all six lanes live in every game. On this board
science and diplomatic victories land past the clock (median t283 and t285)
and religious conversion decides most of the games that end early, while the
live Civilization VI ladder loses **diplomatic 32 : culture 27 : religious 8 :
science 4 : domination 1**
(`docs/eval/2026-08-18-we-screen-against-a-religion-game-and-lose-a-diplomacy-game.md`).
So a gene for a lane nobody finishes is priced on a board where its lane never
decides anything, and a gene for the lane that does decide is priced against a
board where that lane is always open. Restricting `--victories` was tried for
the war genes and became a second regime whose columns never pooled with the
six-lane ones; it is a probe.

The mask is the version of that idea the ledger can hold. **Per game,
deterministically from the game seed, `rotate:N` closes N of the five real
conditions (science, culture, religious, diplomatic, domination); score is on
in every game — it is the clock.** The C(5,N) N-subsets are enumerated once in
a fixed order and the game on `seed` plays subset `seed % C(5,N)`, so a
consecutive seed window plays every mask an equal number of times and every
lane is closed in exactly N/5 of the games. `rotate:2` — the intended use — is
ten masks at a tenth each, every lane open in 60% of games.

| leg | what a rotating batch records |
|---|---|
| header `victories` | the **batch-level** set, all six — unchanged |
| header `victory_mask` | `rotate:2` |
| header `victory_mask_games` | the games this segment pre-registered per mask, from its seed window, before the first game |
| row `victories_off` | the lanes this seat's game closed, sorted (`["culture","science"]`); absent on an unmasked row |

**It is the standard shape.** `shape_of` in `gene_screen.rs` and `tools/genes.py`
read `victories` as the batch-level set, and across a rotating batch every lane
is live and every game still ends on score at the clock, so the batch pools
with the ledger like any other; `tools/genes.py` records `victory_mask` on the
source as provenance, not as a leg. A `--victories` restriction is a different
world and stays a probe. The mask cannot be combined with a contested field (a
pinned pursuer would chase a closed lane in some games), and a rotation must
leave at least one real lane open every game.

**Reading it.** `--analyze` prints a *Victory masks* section (and writes
`victory_masks` into the `--json` summary): games per mask, games per lane
open/closed, and for every lane gene **its win Δ with its own lane OPEN against
CLOSED**, each estimated on the subset exactly as the main table is (clustered
by game; the two subsets share no game, so the difference's error is the root
sum of squares). Which lane a gene is read on comes from the registry's own
words: `lane-congress-ballot`, `lane-congress-favor` and
`competition-victory-points` → diplomatic; `lane-culture-spending` → culture;
`lane-space-race` → science; `holy-lane-parity` → religious; the remaining
`lane-*` genes (`lane-great-people`, `lane-policy-deck`, `lane-commit`)
substitute whichever lane the seat is racing at one decider, so they are read
on all five. A Δ that is larger open than closed is the lane paying; one that
is the same either way is the gene paying through score share or not at all.

The 10-game fires run (`--games 10 --jobs 8 --victory-mask rotate:2`, seeds
26081900..26081909) played each of the ten masks exactly once —
`culture+diplomatic×1 · culture+domination×1 · culture+religious×1 ·
culture+science×1 · diplomatic+domination×1 · diplomatic+religious×1 ·
diplomatic+science×1 · domination+religious×1 · domination+science×1 ·
religious+science×1` — every lane open 6 / closed 4, and the analysis read
`standard`. Ten games price nothing; the split is a reading for a batch of
thousands.

## ⭐ The majors' rung: `--difficulty` and `--difficulty-rotate` (2026-08-25)

The difficulty is the AI handicap every major seat plays with — the yield,
combat, experience and era-boost bonuses of `data/difficulties.json` — and
every screen so far played the engine's Prince default while the live
Civilization VI verification ladder plays Emperor and above. Two flags now
name the majors' rung, and the barbarian seat keeps its own rung
(`default_barbarian_difficulty`, Immortal) whatever the majors play:

- `--difficulty emperor` — one rung for every game; the header records
  `difficulty: "emperor"` and every row carries `difficulty`.
- `--difficulty-rotate king:1,emperor:2,immortal:1` — a weighted list drawn
  **per game from the seed**: the weights are laid end to end and the game on
  `seed` takes the rung at `seed % total`, so a consecutive seed window plays
  each rung in exactly its share (250 / 500 / 250 of a thousand games for that
  list). The header records `difficulty_rotate` and `difficulty_games` (the
  games this segment pre-registered per rung, from its seed window, before the
  first game); each row carries the rung its game played.

Both are **provenance, not a shape leg**: `shape_of` in `gene_screen.rs` and
`tools/genes.py` do not read them, and `tools/genes.py` records `difficulty`
and `difficulty_rotate` on the source (`RECORDED_WHEN_SET`, written only when
present, so every record made before them stays byte-stable). A row without
the field was played at the Prince default and `--analyze` reads it so.

**Reading it.** When a batch rotated (or its rows carry more than one rung),
`--analyze` prints a *Difficulty rungs* section and writes `difficulty` into
the `--json` summary: games per rung in ladder order, and **the top ten genes
by |win Δ| read on every rung separately** — the same subset estimate as the
main table, clustered by game. A gene whose sign holds on every rung pays at
every handicap; one that flips is a gene for one rung.

## ⭐ The rival mix: `--rivals firaxis-mix` (2026-08-25)

The standard screen's opposition is the other drawn genomes, so every effect
is averaged over random opposing genomes from the same controller — the right
instrument for "does this gene help against the ecosystem", and a blind one
for "does it help against a rival that is not us". With `--rivals firaxis-mix`
**one major seat per game plays a fixed opponent instead of a drawn genome**.
Its chair rotates with the game index (as a contested pin does, so no position
is always the rival) and its kind rotates per game from the seed, one third
each:

| kind | the seat |
|---|---|
| `legacy` | `AdvancedAi::legacy()`, the frozen anchor |
| `firaxis-mix` | the deployment genome (`AdvancedAi::new()`) retargeted at one lane drawn in the shares the live Civilization VI ladder actually loses to — **diplomatic 32 : culture 27 : religious 8 : science 4 : domination 1** (the Hall of Fame census, `docs/eval/2026-08-18-we-screen-against-a-religion-game-and-lose-a-diplomacy-game.md`); 72 consecutive firaxis-mix games play each lane exactly its share |
| `random` | a genome with every screened gene on at one half, drawn by the screen's own draw |

**The rival seat is not measured.** Its row is written with `kind: "rival"`
(and `rival_mix` naming the kind, `rival_target` the lane a firaxis-mix rival
pursued), so every estimator — all of which read `kind == "game"` — skips it;
every measured row of the game says `rival_mix: "measured"`. The header
records `rivals: "firaxis-mix"` and `rival_games` (games pre-registered per
kind), and `target_seats` counts measured seats only (five a game). The mix is
provenance on the source, not a shape leg (`RECORDED_WHEN_SET` in
`tools/genes.py`); it cannot be combined with a contested field.

**Reading it.** `--analyze` prints a *Rival mix* section (and `rival_mix` in
the `--json` summary): games and rival wins per kind — the anchor's and the
lane-pursuer's own win rates are a census of the opposition — and **every gene
past the family-wise bar read on the three kinds apart**, with `agree` when
every kind's sign is the whole batch's and `SPLIT` when a gene pays against
one rival and not another.

## ⭐ The drift meter (2026-08-25)

Every `--analyze` now prints, under *how the games ended*, the batch's share
of games ended by each condition — **by game**, not by seat — beside two live
columns: the **live loss census**, how the live Civilization VI seat's games
ended when a rival won (diplomatic 32 : culture 27 : religious 8 : science 4 :
domination 1, the Hall of Fame census in
`docs/eval/2026-08-18-we-screen-against-a-religion-game-and-lose-a-diplomacy-game.md`,
hard-coded as `LIVE_LOSS_CENSUS`), and the **live ladder** column read from
`docs/civ6_ladder.json`'s `attempts[].victory_type` when that file is readable
from the working directory (every finished live game, ours included;
`VICTORY_DEFAULT` and unfinished attempts are not endings). The same table is
`drift` in the `--json` summary. A lane the batch never ends on is a lane its
genes cannot pay through on the win axis; that is the reading the meter is
for, and `--victory-mask` and `--difficulty` are the two knobs that move it.

## What it is not

- Not `gene_census`, which asks whether a continuous `Weights` gene moves an
  outcome at all. The genes here are the boolean treatment flags.
- Not a verdict on its own. A screen's `*` is where to point a single-gene
  run (`--genes tag`) on disjoint seeds; an explicit operator edit to the
  pinned ledger (`docs/gene_ledger.json`, `tools/genes.py`) is what moves a
  default.
- Not the deployment regime. Firaxis-only flags are excluded by construction;
  the live ladder (`docs/CIV6_LADDER.md`) prices those.
