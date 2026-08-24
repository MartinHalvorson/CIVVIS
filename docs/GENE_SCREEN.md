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
(the foldover and its prior-weighted variant) with one rule, and it is the
whole design:

| | |
|---|---|
| unit | **the seat** — one major seat in one game, carrying a genome and an outcome |
| draw | each screened gene on with **p = ½**; a gene the deployment genome ships on with **p = 0.75**, so the batch plays mostly the genome people actually get while every gene keeps both arms populated (`P_ON`, `P_DEFAULT_ON`; `--p-on`, `--p-default-on`) |
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

`gene_screen --games N --out rows.jsonl` — no profile flags — *is* the screen.
Every profile flag still exists, and every one of them turns a batch into a
**probe**: `tools/genes.py` refuses a source whose header does not match
this table, so the ledger cannot quietly hold two worlds in one column. The
shape lives in `SCREEN_PLAYERS`/`SCREEN_MAP`/… in `src/bin/gene_screen.rs` and
in `SCREEN` in `tools/genes.py`, and a test fails if the two drift apart.
The draw design is deliberately **not** a leg of the shape: a file written by
the earlier paired designs at this shape prices the same genes on the same
board, and the estimator reads both the same way — rows are seats.

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
Those genes pay through **score share**, and the deployment rule reads the win
axis only. Read a lane gene's share column and its `share helps` verdict before
calling it inert.

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
deployment rule reads the **win** axis only, so no number of screens can turn
this gene on through that share reading; what it is owed is a deliberate arm
(`--genes joint-tactics`), not a seat on every batch at 2.5× the bill. It is
default-off today, so holding it out changes nothing the agent plays.
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
| `--p-on`, `--p-default-on` | the draw: ½ and 0.75 by default, both strictly inside (0, 1) so every gene keeps both arms |
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
   within it **the best version takes two shares to every other version's
   one** (`BEST_VERSION_WEIGHT`; the best is the version the ledger ships,
   else the priced version with the highest tracked wins — *"regularly swap
   between different versions of the genes, biasing towards the best
   versions"*). So `war-economy` and `war-economy-2` are never on the same
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
5. The ledger ships **one version of a family** (`choose_family_heads`):
   among the versions the deployment rule would turn on, the one with the
   highest **tracked wins** — the pooled on−off win difference over every
   screen that priced it (`win_diff_pp`, the ranking's *Diff*) — ties to the
   higher version. Whatever the best version is, it is what the real games
   play (operator, 2026-08-23: *"always use the best version for our real
   games, whatever the best version is"*); every version keeps being priced
   screen after screen on its own row, and the head changes hands as the
   record grows. The others are recorded `family_runner_up` and ship off —
   the rule's verdict still on their row so the ranking shows what they
   measured — and the Rust mirror re-derives the same choice
   (`the_default_follows_the_ledgers_authority`). `HEURISTIC_GENE_RANKING.md`
   names the family's best version in its *Best version* column (`1` is the
   original) on every row of the family, and a versioned row's *Total (on)* /
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

## The gene ledger: the defaults are the best genome, and the best genome is data

Operator directive 2026-08-20: *let the defaults for the genes reflect our best
genome — unhelpful genes can default off — so our verification games use our
best genome. When we test, still test and try to improve the less helpful
genes.*

Operator directive 2026-08-22, which is now the rule: *genes can default on if
both the last 10k and 10k prior columns are positive, or if the average of the
two columns is >15 and neither is less than −10. If exactly one column is
filled, the gene can default on when that reading is >20; otherwise it defaults
off.*
Those are the two columns `HEURISTIC_GENE_RANKING.md` prints — wins added per
10,000 on-arm seats at the gene's measured on-rate, `(win_on − 1/players) ×
10,000` —
from the latest two screens that priced the gene. The verdicts below
still record what the screen *proved*, and the screen still prints them; they
no longer decide what ships, so a gene can be `helps` and off (its readings do
not clear the rule) or `hurts` and on (its win columns do).

Operator directive 2026-08-22, later the same day: *we should default every gene
to off that has Diff < 0.* *Diff* is the ranking's own column — the pooled on
win rate minus the pooled off win rate in percentage points, over **every**
screen that priced the gene, each weighted by its on-arm seats. It is the **whole**
on−off difference, twice the scale of a win column beside it, and it is now a
veto: a gene whose record is negative defaults off however its two win columns
read. The veto is one-way — a positive record promotes nothing on its own,
because the columns still have to clear their bars — and it is the one clause
that lets a screen older than the last two speak. **31 genes on, was 34**: it
turned off `war-economy` (+38/+8 over a 2026-08-20 reading of −3.84 pp),
`siege-commitment` (+1/+3 over −0.80 pp) and `apostle-promotion-by-role`
(+14/+12 over −0.83 pp). No measurement moved; each of the three carries
positive recent columns over one old screen it has not made back.

Until then "on by default" meant somebody had written `self.enable_x()` into
the bundle, and the phase-1 anchors had measured that all-on bundle at **7.5%
wins against 27% for all-off** (4p classic, 200 anchor pairs). Now:

- **`docs/gene_ledger.json`** records the screen's own profile, then per gene
  what the newest screen measured, the verdict that follows, the two win
  columns (`wins_last_10k`, `wins_prior_10k`) and the `default_on` they decide;
  **`src/ai/advanced/gene_ledger_table.rs`** is the same
  table generated into Rust. `tools/genes.py source <analysis>
  …` builds both from `gene_screen --analyze --json` outputs (the analyses
  themselves are tracked under `docs/gene_screens/`);
  `tools/test_genes.py` fails if either file has drifted from the
  recorded sources. Later sources override earlier ones per gene, so a repaired
  gene's re-screen replaces its pre-repair number while the rest of the old
  screen stands. Each source carries the `shape` it was played at: `standard`
  is the screen, `legacy` is a reading kept as history (every source today —
  the Pangaea screens the current defaults stand on). A new source that is not
  `standard` is refused unless `--legacy-shape` says otherwise.
- **Verdict rules** (`tools/genes.py`, repeated in
  `src/ai/advanced/gene_ledger.rs`): `helps` = win z ≥ 2 with share z > −2, or
  share z ≥ 2 with win z > −2 — the screen's own `*` flag; `hurts` the mirror;
  `unresolved` otherwise, including a gene whose axes disagree past |z| ≥ 2
  (`conflict`) and a gene no screen has measured. Past the family-wise bar is
  recorded as `family_wise`, not required: with sixty-odd genes that bar would
  leave three on. The newest screen that priced the gene supplies the verdict.
- **The deployment rule** (`default_from_columns` in
  `tools/genes.py`, mirrored as `columns_default_on` in
  `src/ai/advanced/gene_ledger.rs`, and re-derived from the generated table by
  `the_default_follows_the_ledgers_authority`): **on** when both win columns
  are positive, or when their average is above +15 with neither below −10;
  with exactly one populated column, **on** when it is above +20; **off**
  otherwise. That clause is `default_from_win_columns` /
  `win_columns_default_on`, and only the latest two readings reach it. Then
  **off whatever it says when `win_diff_pp` is negative** — the pooled on−off
  difference over the whole record, recorded beside the columns in
  `docs/gene_ledger.json` and in the generated Rust table so the re-derivation
  reads the same number the decision was taken on. Both arms of a screen carry
  the same number of seat observations, so the 1-in-`players` chance base
  cancels inside each screen and the pooled figure is an on-arm-seat-weighted
  average of per-screen differences —
  comparable across shapes and player counts in a way a raw win rate is not.
  ⚠ It has no recency discount: a gene repaired since its worst screen stays
  vetoed until its newer games outweigh that screen's.
- **The deployment genome.** `AdvancedAi::enable_live_bridge` and
  `enable_engine_repairs` now end with `apply_gene_ledger`: every live or
  production treatment the ledger does not default on is withheld, every
  opt-in it defaults on is enabled, and a flag the screen cannot price
  (the Firaxis-only flags) is left as the bundle set it. A **screenable gene
  nobody has screened yet ships off; one screened once ships on only above
  +20.** The
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
| `already-on` | 1 | `adjacent_camp_clear` is on in `BasicAi::new`, so an opt-in row would be on in both arms and screen as exactly inert. |
| `does-not-fire` | 4 | A gene row was written for it here and `tools/gene_fires.py` refused it: a single-gene probe over 12 map pairs left both arms byte-identical. The row was removed rather than shipped to return a zero-width interval. |

⚠ The `production-on` and `live-bridge-row` rows are the finding worth
carrying out of this: **making a shipped behaviour screenable is not a
measurement-only change.** The ledger's default rule reads "unmeasured ⇒ off",
which is the right rule for a behaviour that was already off and the wrong one
for twenty-two that were on. Twenty-two genes' worth of reach is available for
the price of deciding, per behaviour, whether it is allowed to go off while its
first screen runs — and that is a genome decision for the operator, not a
side effect of a row.

## The precision-weighted posterior, published beside the rule

A threshold in column units is not a threshold in evidence. Three things about
the deployment rule are true from this repository's own numbers:

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
places, the ranking prints the 95% interval and `P(effect > 0)`, and
`src/ai/advanced/gene_ledger.rs` re-derives the deployment call from the same
published figures.

⚠ `τ` is the load-bearing term, not a refinement. When two screens agree to
within their errors it is zero and the pool is the ordinary inverse-variance
one. When they do not — a `legacy` Pangaea reading against a `standard`
continents one — it widens the interval instead of averaging two worlds into a
confident wrong answer. `POSTERIOR_SHAPES` says which shapes the published pool
admits, and `HEURISTIC_GENE_RANKING.md` prints legacy, standard and pooled side
by side so the choice is made on the numbers.

### The switch, and why it is not thrown

`AUTHORITY` in `tools/genes.py` is the whole switch. Change it, run
`python3 tools/genes.py write`, and `docs/gene_ledger.json`, the
generated Rust table and the ranking all follow; the ledger records which rule
decided, so `--check` and the Rust mirror re-derive under the recorded one and
cannot drift. Three settings, weakest first, each containing the one before it:

| setting | what decides |
|---|---|
| `columns` | the operator's threshold rule exactly as it ships — the two win columns, vetoed by a negative pooled `Diff` |
| `posterior-veto` | the same columns, with an error bar on the veto: it fires only when the posterior's 95% interval lies **wholly below zero** |
| `posterior` | the pooled estimate decides wherever its interval excludes zero; where it straddles, `posterior-veto` decides |

It says `columns`, and on 2026-08-23 that was re-decided on the numbers rather
than deferred again. The premise the deferral rested on is gone: two `standard`
sources are in the ledger — the 23,622-pair whole-genome screen and the `g1`
direct arm — and 99 of the 101 priced genes now have a deployment-shape
reading. So the question stopped being "is the instrument right" and became
"what would the switch actually move".

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

### When the switch should be thrown

Both dials are still the right destination; neither is carried by today's
sources. The conditions are checkable, so the next agent does not have to
re-litigate the question:

1. **`POSTERIOR_SHAPES = ("standard",)`** when every priced gene has a
   `standard` reading. Two do not (`joint-tactics`, `step-and-reassess`), and
   under standard-only their posterior becomes `–` rather than narrower — the
   scope dial would delete evidence, not sharpen it. It is genome-neutral the
   day it lands (0 moves, measured above), so it costs nothing to wait.
2. **`AUTHORITY = "posterior"`** when its delta against `columns` contains a
   gene whose standard-only interval **excludes zero**. Today the delta is one
   gene whose interval is +6 [−25, +37], P(>0) 64.8% — the posterior is not
   saying it helps, it is saying the veto could not tell, and re-admitting on
   "could not tell" is how a null ships.

⚠ And when it is thrown, read `docs/EVAL_INTEGRITY.md` §4 first. Every figure
the posterior would decide on is the point estimate of one screen, so
`E[observed | promoted] > true effect`; the repository's own re-measurements
put `+207` at `+86` and `strategic_deep`'s `+45` at −8 (CI −27..+12, 220 maps,
PR #482). A posterior built from single-screen point estimates inherits that
bias — it prices the uncertainty *between* screens honestly and the selection
*within* one not at all.

⚠ Where the interval straddles zero, the `posterior` setting inherits the
column rule's answer. That is forced, not chosen: `default_on` has to be a pure
function of the recorded sources, so the fallback cannot be "whatever shipped
yesterday". The way out of the deferral is the boundary set below, not a
guess through it.

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

1. **The decision axis stays WINS.** A lane gene ships when the deployment rule
   in force says it ships, on the win columns, exactly like every other gene.
   Nothing below promotes anything. `docs/GENOME.md` records what happened the
   one time selection ran on a correlate, and
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

⭐ The genome's own worst row is the case for this. `governor-victory-lanes`
read win z **+2.46** and share z **−15.92** on `p10` — a recorded `conflict` —
and #2294's single-column clause promoted it on the +46 win column because the
rule reads the win axis only. The first standard-shape screen's **win** axis
reads it at z **−15.37**, within half a sigma of what the legacy **share** axis
had already said. `docs/gene_ranking_notes.md` carries the numbers.


## What it is not

- Not `gene_census`, which asks whether a continuous `Weights` gene moves an
  outcome at all. The genes here are the boolean treatment flags.
- Not a verdict on its own. A screen's `*` is where to point a single-gene
  run (`--genes tag`) on disjoint seeds; the ledger's rule (`docs/gene_ledger.json`,
  `tools/genes.py`) is what moves a default, and it reads both.
- Not the deployment regime. Firaxis-only flags are excluded by construction;
  the live ladder (`docs/CIV6_LADDER.md`) prices those.
