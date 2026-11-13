# The edge ledger: how far apart the two sides stood in each fight

The unit ledger (`docs/GENE_SCREEN.md`, "The unit ledger") counts the units a
seat killed and lost. It cannot say whether a fight was winnable: a Warrior lost
to a rival two eras ahead and a Musketman lost to a careless stand both read as
one loss. The edge ledger reads every blow's **strength gap**, with one set of
definitions for the live Civilization VI logs (`tools/civ6_edge_ledger.py`) and
for the simulator (`Game::combat_ledger`, `gene_screen`), so a live lane and a
simulated batch can be set side by side.

It was built to settle one question: **does the simulator's lane recipe put our
seat as far behind in strength as the live seat is?** Every unit-handling gene
tested flat in 60-seed sims while the sim seat traded several kills per loss
and the live seat about 1.7. If the sim seat is not out-teched the way the live
one is, the sim cannot judge anything that closes that gap.

## Definitions

**Which blows count.** A blow struck by one military unit at another, neither
an aircraft. A city, district or encampment strike, an air strike, a blow on a
civilian and theological combat are not in it. A blow is booked on both seats:
as *taken* by the struck seat, split by the striker's side, and as *struck* by
the striking seat, split by the target's side. Sides are **major**,
**city-state** and **barbarian**; the live ledger adds **other** (the Free
Cities, player 62), which the simulator counts nowhere.

**Base gap.** The striker's attack strength minus the target's Combat
strength, both read off `data/units.json`: attack strength is Bombard, else
Ranged, else Combat. No promotion, terrain, wound, era or difficulty modifier.
It says how far apart the two unit *types* are, and it can be read for every
blow on both sides.

**Total gap.** The two strengths the blow was actually resolved with. Live,
this is the host's own combat preview (`preview.attacker_total_strength -
defender_total_strength`), which only our own blows carry. In the simulator it
is `melee_exchange_strengths` / `ranged_strike_strengths`, the resolver's own
arithmetic, every modifier and the wound penalty in. The simulator reads it for
blows taken as well; the live seat cannot.

**Bins.** Gaps are rounded to a whole point, then binned: `<=-10`, `-9..-4`,
`-3..+3`, `+4..+9`, `>=+10`.

**Turn buckets.** The rival majors' mean base gap on our units in 0-49, 50-99,
100-149 and 150+, by raw turn and by **Online-equivalent turn** (`turn x 50 /
the speed's cost percent`: Standard turn 100 is Online turn 50). Every live run
of 2026-11-12/13 and both simulated lanes are Online, so the two agree there.

**Marks** (tables c and d), at Online-equivalent turns 50, 100 and 150, over our
**land army** (military land units with a Combat strength):

| column | live | simulator |
|---|---|---|
| unlocked | the unit carries the host's `upgrade_to` (written only when a successor is unlocked) | `Game::unit_upgrade_target` is `Some` |
| +material | ... and the successor's strategic resource is in stock at the host's `upgrade_resource_cost` | ... and the furthest unlocked successor's resource is in stock at its `resource_cost` |
| offer | ... and the host gives no `upgrade_blocked_reason` this turn (Gold, territory, material) | `Game::unit_gold_upgrade_offer` is `Some`, read on a copy of the board with every unit fresh |
| gain | Combat of `upgrade_to` minus the unit's own | Combat of the furthest unlocked successor minus the unit's own (the host offers the furthest) |
| gold, gpt | `gold`, `gold_per_turn` (net, `GetGoldYield - GetTotalMaintenance`) | `Player::gold`, `Player::gold_per_turn` (net) |
| ours | mean Combat of our land army | the same |
| rivals | mean Combat of the rival majors' land military units in `rivals[].units` (what we can see) | the same, over units `player_can_see` and `unit_visible_to` |
| gap | rivals minus ours | the same |

Shares and strengths are per-game values averaged over the games that reached
the mark; gold and gold per turn are medians. A live run whose export predates
`upgrade_to` walks the `upgrade_to` chain on its techs instead and reads no
offer.

**Kill:loss and cities lost.** Live, `summary.json`'s `combat.kills`,
`combat.losses` and `combat.cities_lost`; simulated, the row's `units_killed`,
`units_lost` and `cities_lost`. They are context, not part of the gap.

## Reading it

Live, read only, one process, beside a live game:

```bash
nice -n 10 python3 tools/civ6_edge_ledger.py --since 20261112 --until 20261113T0420 --per-run --metrics
python3 tools/civ6_edge_ledger.py <run-dir> [<run-dir> ...] [--json out.json]
```

Simulated: every `gene_screen` row carries `edge` (the seat's `EdgeLedger`: the
four gap tables as `[side][bin]` cells of `[blows, damage to the defender,
damage to the attacker, defender killed, attacker killed]`, and the majors'
`[blows, sum of base gaps]` per turn bucket) and `edge_marks` (one `EdgeMark`
per mark the seat lived to). `--analyze` prints the tables pooled over the
measured rows, then the on−off Δ of seven blow counts per gene, and its
`--json` carries those under each gene's `edge_ledger`.

The side-by-side read is `src/bin/gene_screen/edge_ledger_tests.rs` (ignored):

```bash
cargo test --profile ci --features developer-tools --bin gene_screen edge_ledger_driver -- --ignored --nocapture
```

It plays both lane recipes with `--rival-host-tactics` and the armed genome
(`CIVVIS_LIVE_GENOME`, default `~/.civvis-live-force-on`) under each handicap
setting, the same seeds in every arm, and prints `metrics` beside the live
lane's (pasted from `--metrics`, since CI cannot read the live logs).
`tools/test_civ6_edge_ledger.py` pins the live metric names to the driver's,
and the driver pins them to the simulator's.
