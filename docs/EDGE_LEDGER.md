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

## Simulated: the lane recipes beside live

`edge_ledger_driver` on GitHub's runners (CI is the only compiler here),
`--live-genome` the armed set of 2026-11-13 (564 tags, every one a gene of
this build), `--rival-host-tactics` on every arm, 8 worlds an arm asked.
Rounds 1 and 2 played seeds 64300000-07, round 3 fresh seeds 64310000-07;
round 2 stopped starting worlds at 16 minutes, so its Gran Colombia arms
hold 7. The settings, on top of each lane's recipe:

| setting | what changes |
|---|---|
| `recipe` | nothing: `--rival-rung -2 --rival-pace` (Emperor's yield bonus at Deity) |
| `no-rung` | no `--rival-rung`: the rival chairs keep Deity's full yield bonus |
| `rung-1` | `--rival-rung -1` (Immortal's) |
| `seat-pace` | the recipe plus `--seat-pace` (our seat capped at the live seat's city curve) |
| `no-rung-war` | `no-rung` plus `--rival-war-pace 100` (wars on our seat at the live hazard) |

To replay a round (8 worlds an arm; round 3 shown, round 1 is `recipe,no-rung`
and round 2 `rung-1,seat-pace,no-rung-war` with the default seed):

```bash
CIVVIS_EDGE_SEED=64310000 CIVVIS_EDGE_SETTINGS=recipe,no-rung \
  cargo test --profile ci --features developer-tools --bin gene_screen edge_ledger_driver -- --ignored --nocapture
```

Metric names are `edge_ledger::metrics` / `civ6_edge_ledger.py --metrics`;
`major_*` are the rival majors' blows on our units (base gap), `struck_major_*`
our blows on their units (total gap), `*_50/100/150` the marks, `/N` the round.

**Gran Colombia domination** (`--players 4 --width 60 --height 38 --city-states 6 --map pangaea`, `--target-mix domination`):

```text
metric                                    live      recipe/1      recipe/3     no-rung/1     no-rung/3      rung-1/2   seat-pace/2 no-rung-war/2
major_blows_per_game                    21.632        20.750        27.250        15.000        46.125        17.286        21.571        35.143
major_share_ge10                         0.706         0.482         0.725         0.733         0.762         0.570         0.444         0.691
major_death_per_blow_ge10                0.553         0.450         0.513         0.557         0.598         0.522         0.448         0.506
major_death_per_blow_lt10                0.150         0.198         0.133         0.125         0.159         0.135         0.179         0.237
major_gap_0_49                           8.704             -             -             -        20.000             -             -         6.667
major_gap_50_99                         17.256         6.789        15.233        19.943        17.276         9.444         8.727        14.338
major_gap_100_149                       22.850         7.837        21.506        23.300        20.922         7.289         4.770        17.692
major_gap_150                           19.576        39.273             -             -        42.000        19.154        33.000             -
struck_major_share_le_m10                0.529         0.486         0.636         0.571         0.682         0.568         0.393         0.684
struck_major_share_ge10                  0.158         0.102         0.059         0.071         0.059         0.075         0.130         0.029
struck_major_dmg_le_m10                  8.873         8.737         9.524         8.550        10.969        11.711        12.975        10.576
struck_major_dmg_ge10                   41.940        43.106        42.737        40.667        38.410        42.312        39.635        38.417
struck_major_kill_per_blow_ge10          0.631         0.617         0.421         0.667         0.667         0.625         0.596         0.583
unlocked_50                              0.076         0.106         0.057         0.054         0.058         0.065         0.075         0.000
material_50                              0.040         0.106         0.057         0.054         0.058         0.065         0.075         0.000
offer_50                                 0.012         0.087         0.000         0.000         0.013         0.024         0.040         0.000
gain_50                                 12.143        10.000        10.000        10.000        10.000        10.000        10.000             -
gold_50                                 64.000        69.850        46.550        65.950       124.350       144.400        33.400       125.500
gpt_50                                   8.301         1.450         7.150        12.750         6.100        30.700         2.000        31.000
ours_50                                 16.562        15.227        16.834        16.350        16.688        15.886        15.058        17.044
rivals_50                               22.793        11.667        13.250        18.750        13.600        18.500        12.222        15.938
strength_gap_50                          6.246        -4.438        -4.229         2.667        -3.451         4.038        -3.183        -1.406
unlocked_100                             0.293         0.693         0.466         0.755         0.614         0.631         0.632         0.615
material_100                             0.189         0.345         0.255         0.524         0.329         0.402         0.390         0.347
offer_100                                0.042         0.057         0.013         0.086         0.013         0.137         0.095         0.107
gain_100                                19.659        18.640        17.193        18.070        17.012        17.658        19.189        17.732
gold_100                               131.000        65.925       109.700        78.800        93.750        96.200        87.100        89.900
gpt_100                                 21.855        21.850         8.050        19.200        12.500        10.000        12.600        23.300
ours_100                                28.802        26.110        27.141        24.532        24.569        28.203        27.466        26.565
rivals_100                              53.099        33.197        37.112        52.048        40.593        28.804        31.653        37.223
strength_gap_100                        24.277         6.655         9.971        26.280        16.024        -0.537         4.169        10.658
unlocked_150                             0.513         0.536         0.721         0.667         0.619         0.691         0.352         0.676
material_150                             0.318         0.369         0.154         0.000         0.524         0.504         0.070         0.000
offer_150                                0.100         0.094         0.015         0.000         0.048         0.114         0.019         0.000
gain_150                                22.136        23.108        20.767        29.000        19.692        25.296        20.700        24.720
gold_150                               122.000        84.400        44.200        54.100       106.979        83.602       142.624        36.250
gpt_150                                 12.359         1.900         0.500        13.700         3.800         9.980        13.700       -18.900
ours_150                                43.988        39.925        29.684        35.667        37.667        39.502        52.733        28.973
rivals_150                              71.819        54.432        66.125        61.792        73.857        62.361        59.585        65.000
strength_gap_150                        28.110        14.507        36.441        26.126        36.190        22.859         6.852        36.027
kill_loss                                1.676         3.158         2.080         1.634         1.474         2.383         3.780         1.542
cities_lost_per_game                     1.571         2.000         2.250         2.000         2.625         2.143         1.000         1.714
```

**Rome Score** (`--players 6 --width 74 --height 46 --city-states 9 --map continents`, `--target-mix score --seat-civ Rome --turns 170`):

```text
metric                                    live      recipe/1      recipe/3     no-rung/1     no-rung/3      rung-1/2   seat-pace/2 no-rung-war/2
major_blows_per_game                    11.275         0.750         0.625         0.750         0.250         0.625         0.500        10.429
major_share_ge10                         0.690         0.667         0.400         0.667         1.000         1.000         0.500         0.685
major_death_per_blow_ge10                0.598         1.000         1.000         0.500         1.000         1.000         0.500         0.660
major_death_per_blow_lt10                0.214         0.000         0.000         0.000             -             -         0.000         0.261
major_gap_0_49                           8.256             -             -             -             -             -             -             -
major_gap_50_99                         16.526             -             -             -             -             -             -        13.312
major_gap_100_149                       19.229        27.333         0.000        18.167        47.000        48.800        13.333        14.000
major_gap_150                           19.341             -        29.250             -             -             -         5.000             -
struck_major_share_le_m10                0.588         0.739         0.846         0.739         1.000         1.000         0.476         0.512
struck_major_share_ge10                  0.092         0.043         0.000         0.000         0.000         0.000         0.143         0.105
struck_major_dmg_le_m10                 10.339        11.176         8.727        10.529         9.500         3.762         7.800        11.432
struck_major_dmg_ge10                   37.696        34.000             -             -             -             -        38.000        40.889
struck_major_kill_per_blow_ge10          0.594         1.000             -             -             -             -         0.333         0.444
unlocked_50                              0.199         0.208         0.248         0.372         0.146         0.311         0.200         0.276
material_50                              0.081         0.112         0.144         0.089         0.011         0.125         0.101         0.082
offer_50                                 0.026         0.081         0.000         0.068         0.000         0.018         0.000         0.024
gain_50                                 17.460        16.875        13.000        15.429        17.500        17.167        16.875        14.800
gold_50                                 79.000        62.400       147.000       155.500       139.450        84.000        93.800       100.050
gpt_50                                   9.293         4.500         3.500         7.200        17.650         2.950         4.500         4.600
ours_50                                 15.848        15.969        15.951        15.484        17.821        15.089        15.271        15.308
rivals_50                               21.450        19.667        16.833        16.250        12.250        10.000        15.050        18.528
strength_gap_50                          5.407         4.310        -2.914         0.854        -4.141        -4.595        -0.050         2.671
unlocked_100                             0.655         0.519         0.579         0.588         0.683         0.363         0.536         0.727
material_100                             0.496         0.337         0.386         0.372         0.439         0.209         0.399         0.403
offer_100                                0.176         0.021         0.106         0.000         0.065         0.008         0.044         0.112
gain_100                                18.463        19.307        17.372        22.926        19.939        23.012        20.617        21.708
gold_100                               142.000        47.700       107.250        91.300       120.750       104.600        82.900       183.000
gpt_100                                 22.699         6.350        35.500        12.300         7.600         4.450         6.100        21.000
ours_100                                26.970        21.883        25.365        19.803        22.191        19.320        20.792        20.790
rivals_100                              45.783        32.250        17.433        32.417        27.500        28.724        21.562        27.950
strength_gap_100                        18.553         9.590        -9.009        12.311         5.007         9.245         2.309         7.292
unlocked_150                             0.526         0.561         0.520             -         1.000         0.625         0.440             -
material_150                             0.257         0.303         0.380             -         0.000         0.531         0.368             -
offer_150                                0.035         0.033         0.064             -         0.000         0.107         0.030             -
gain_150                                28.301        24.646        22.205             -        10.000        21.827        25.167             -
gold_150                               207.000        91.034       134.850             -       102.150        94.825       207.900             -
gpt_150                                 30.105         2.875        19.650             -         6.150        18.150        16.000             -
ours_150                                44.365        34.169        45.280             -        45.000        30.853        35.329             -
rivals_150                              66.682        57.173        61.606             -        70.100        61.292        54.049             -
strength_gap_150                        21.188        22.664        16.325             -        25.100        30.439        18.423             -
kill_loss                                1.842         4.727         7.644         3.290         4.343         3.531         4.507         1.804
cities_lost_per_game                     0.900         1.000         1.125         0.500         1.125         0.625         0.250         1.571
```

Every simulated game in rounds 2 and 3 ended on a rival chair's Science or
Culture victory: Gran Colombia t101-183 over 37 games (recipe t128-156, median 141; no-rung t120-153), Rome t105-168 over 39 games (recipe t118-168, median 154; no-rung t120-157), never our seat's (round 1 did not print endings, but its `no-rung` Rome arm reached no t150 mark). Live, the Gran Colombia runs that ended
on a rival's victory did so at t171-210 for Science (median 186, 19 runs),
t138-196 for Culture and t76-164 for Religion; 27 of 40 Rome runs reached the
t160-171 Score limit. So every simulated `*_150` cell and `major_gap_150`
rests on fewer games than its live twin, and `-` there means no game got that
far.

## The answer

**No, not at the recipe's own handicap, and not reliably at 8 worlds. Dropping
`--rival-rung` (the rival chairs keep Deity's full yield bonus) does, in both
seed sets; Rome also needs the live war hazard.**

- **What is short in the recipe is the rivals, not our seat.** At t100 our land
  army's mean Combat is 26-27 in Gran Colombia (live 29) and 22-25 in Rome
  (live 27), but the rival majors' land units we can see average 33-37 (live
  53) and 17-32 (live 46). So the visible strength gap at t100 is +7 and +10
  in Gran Colombia (live +24) and +10 and −9 in Rome (live +19), and our seat
  trades 3.2 and 2.1 kills a loss in Gran Colombia (live 1.7), 4.7 and 7.6 in
  Rome (live 1.8). The recipe's per-blow gap in Gran Colombia is the noisy
  read: far below live on seeds 64300000-07 (48% of the majors' blows at +10 or
  more, mean base gap +7/+8 at turns 50-149) and close to it on 64310000-07
  (73%, +15/+22).
- **Rome's recipe cannot judge the gap at all**: the rival majors strike our
  seat's units 0.3-0.8 times a game (live 11.3), with or without the rung.
- **`no-rung` reproduces Gran Colombia** on both seed sets: 73% and 76% of the
  majors' blows at +10 or more (live 71%), mean base gap +20/+23 and +17/+21 at
  turns 50-149 (live +17/+23), visible gap at t100 +26 and +16 (live +24),
  kill:loss 1.63 and 1.47 (live 1.68). Cities lost rise to 2.0 and 2.6 a game
  (live 1.6; the recipe 2.0 and 2.25).
- **`no-rung-war` (no rung, `--rival-war-pace 100`) reproduces Rome's blow
  tables**: 10.4 major blows a game (live 11.3), 69% at +10 or more (live 69%),
  death a blow 0.66 / 0.26 above / below +10 (live 0.60 / 0.21), kill:loss
  1.80 (live 1.84). Cities lost 1.6 a game (live 0.9), and the visible gap at
  t100 stays low (+7, live +19). In Gran Colombia the same setting reads 69%,
  +14/+18, kill:loss 1.54, cities lost 1.7.
- **`rung-1` and `seat-pace` do not.** Gran Colombia: 57% and 44% of the
  majors' blows at +10 or more, mean gap +9/+7 and +9/+5, kill:loss 2.4 and
  3.8. Capping our seat's width (`seat-pace`) lowers cities lost (1.0) and
  widens nothing: the gap is the rivals' to open.

So a gene meant to close the gap (upgrades, military tech, Gold for them) can
only be read on the simulator with `--rival-rung` dropped, and on Rome with
`--rival-war-pace 100` as well; read it on the blow tables and the t100 marks,
since every simulated game is over before the live ones are.

## What it is not

- **Not a verdict on any gene.** It is a metric and a calibration read; the
  on−off columns `--analyze` prints are where a gene meets it.
- **Not the whole strength story.** Base gap ignores promotions, terrain,
  wounds, era bonuses and the difficulty's combat bonus; total gap carries all
  of them but live only for our own blows. A unit can lose at +0 base gap and
  win at +10.
- **Small simulated samples.** 7-8 worlds an arm. The Rome lane's simulated
  blow tables rest on a handful of blows outside `no-rung-war`, and the marks
  read the rival units one seat happens to see.
- **Not seed-paired across rounds.** Within a round every arm plays the same
  seeds; between rounds the seeds (round 3) or main (one opt-in gene, #5230)
  differ.
