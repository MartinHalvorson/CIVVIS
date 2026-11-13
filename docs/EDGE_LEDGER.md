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

## Live: 108 Deity runs of 2026-11-12/13

`tools/civ6_edge_ledger.py --since 20261112 --until 20261113T0420` (every run in
the control dir from 2026-11-12 00:00Z to 2026-11-13 04:20Z whose event log
passed 8 MB; all Deity, all Online): 68 Gran Colombia domination runs (4p
Pangaea, 6 city-states) and 40 Rome Score runs (6p Continents, 9 city-states).
Unique units read as themselves (`UNIT_ROMAN_LEGION` is the Legion, Combat 40,
not a Swordsman); no unit type in these runs was unmapped.

**(a) Rival majors' blows on our units, by base gap** (blows, share, damage a
blow, our units killed, killed a blow):

```text
                  Gran Colombia (68 runs)               Rome (40 runs)
gap        blows share dmg  deaths death/blow    blows share dmg  deaths death/blow
<=-10        135  0.09 23.2     17   0.13           16  0.04 18.5      2   0.12
-9..-4        58  0.04 31.8      8   0.14           20  0.04 34.0      3   0.15
-3..+3       133  0.09 35.3     24   0.18           52  0.12 37.4     12   0.23
+4..+9       106  0.07 36.1     16   0.15           52  0.12 51.1     13   0.25
>=+10       1039  0.71 66.6    575   0.55          311  0.69 70.4    186   0.60
mean base gap, turns 0-49 / 50-99 / 100-149 / 150+:
          +8.7 / +17.3 / +22.8 / +19.6               +8.3 / +16.5 / +19.2 / +19.3
```

Pooled over both lanes: 70% of the majors' blows land at +10 or more and cause
761 of the 856 deaths they cause; city-states strike at +10 or more 82% of the
time and the barbarians 63%.

**(b) Our blows on rival majors' units, by total gap** (the host's preview):

```text
                  Gran Colombia                           Rome
gap        blows share dealt kills kill/blow       blows share dealt kills kill/blow
<=-10       1122  0.53   8.9    71   0.06            443  0.59  10.3    35   0.08
-9..-4       184  0.09  21.5    35   0.19             87  0.12  20.2    17   0.20
-3..+3       289  0.14  25.3    83   0.29            109  0.14  23.6    35   0.32
+4..+9       190  0.09  31.1    77   0.41             46  0.06  29.2    22   0.48
>=+10        336  0.16  41.9   212   0.63             69  0.09  37.7    41   0.59
```

Half our blows on rival units are struck at −10 or worse for 9-10 damage. The
mod's `attacker_killed` is almost never set on our own blows (4 rows in 60
runs), so the live "our deaths" column is not read.

**(c) Upgrades and (d) strength** (per-game means; gold medians):

```text
Gran Colombia  games army unlocked +material offer  gain gold   gpt  ours rivals   gap
t50               65  8.4     0.08      0.04  0.01 +12.1   64   8.3  16.6   22.8  +6.2
t100              58 14.3     0.29      0.19  0.04 +19.7  131  21.9  28.8   53.1 +24.3
t150              37 13.9     0.51      0.32  0.10 +22.1  122  12.4  44.0   71.8 +28.1
Rome
t50               35  7.7     0.20      0.08  0.03 +17.5   79   9.3  15.8   21.5  +5.4
t100              31 12.7     0.65      0.50  0.18 +18.5  142  22.7  27.0   45.8 +18.6
t150              31 13.7     0.53      0.26  0.04 +28.3  207  30.1  44.4   66.7 +21.2
```

Kill:loss from `summary.json`: 1.68 (Gran Colombia, 63 runs with a summary),
1.84 (Rome); cities lost a game 1.57 and 0.90.
