# Current AI Strategy Rankings

League round **4119**, generated from `data/league/league.json`. This table contains the
**53** strategies currently eligible for live games and ranks their exact **8-player**
evidence. Retired, human, and offline-only entries are omitted (9 roster entries at this
round), and historical leader/civilization rows are not carried into the public
leaderboard.

Rank is the lower 1.96σ Wilson bound on outright wins, the same conservative objective
the league uses for table-size-aware selection. Placement Elo is retained only as
matchmaking context; it does not decide this order. Confidence intervals can overlap, so
rank 1 is the current selection leader rather than a claim that every alternative is
statistically separated.

Last played is the latest retained UTC date for the strategy. Exact leader/civilization
evidence remains reproducible in the league snapshot; CIVVIS publishes a pair
recommendation only where conservative win evidence actually separates it from every
rival, as documented in the README.

Refresh this document after updating the committed league snapshot:

`python3 tools/update_ai_player_elo_rankings.py`

Use `--check` to verify that this generated file is current.

| Rank | Player (strategy) | Role | 8p wins/games | Conservative win bound | Placement Elo | RD | Last played |
|---:|---|---|---:|---:|---:|---:|---|
| 1 | DarkHorse4 (`g24-26`) | generalist | 198/1174 | 14.8% | 1690.1 | ±59.7 | 2026-08-15 |
| 2 | Opportunist3 (`g28-28`) | generalist | 544/3615 | 13.9% | 1770.8 | ±58.3 | 2026-08-15 |
| 3 | TheHeir (`advanced_evolved`) | builtin advanced_evolved | 554/3808 | 13.5% | 1763.7 | ±57.7 | 2026-08-15 |
| 4 | FreeSpirit5 (`g36-34`) | generalist | 25/140 | 12.4% | 1673.5 | ±60.5 | 2026-08-15 |
| 5 | WildCard3 (`g32-31`) | generalist | 9/41 | 12.0% | 1729.1 | ±59.3 | 2026-08-15 |
| 6 | JackOfAllTrades (`advanced`) | builtin advanced | 412/3540 | 10.6% | 1704.4 | ±58.3 | 2026-08-15 |
| 7 | DarkHorse6 (`g44-39`) | generalist | 66/520 | 10.1% | 1687.0 | ±60.2 | 2026-08-15 |
| 8 | Maverick3 (`g24-25`) | generalist | 6/33 | 8.6% | 1677.2 | ±56.6 | 2026-08-15 |
| 9 | WildCard7 (`g52-45`) | generalist | 8/51 | 8.2% | 1700.1 | ±60.0 | 2026-08-15 |
| 10 | DarkHorse (`g4-10`) | generalist | 10/70 | 7.9% | 1761.6 | ±60.5 | 2026-08-15 |
| 11 | WildCard8 (`g52-46`) | generalist | 18/165 | 7.0% | 1704.3 | ±60.5 | 2026-08-15 |
| 12 | DarkHorse5 (`g40-38`) | generalist | 7/53 | 6.5% | 1689.6 | ±60.1 | 2026-08-15 |
| 13 | FreeSpirit2 (`g16-19`) | generalist | 5/38 | 5.8% | 1702.3 | ±56.8 | 2026-08-15 |
| 14 | WildCard4 (`g40-36`) | generalist | 4/28 | 5.7% | 1719.1 | ±54.5 | 2026-08-15 |
| 15 | Maverick4 (`g36-35`) | generalist | 4/28 | 5.7% | 1673.5 | ±58.1 | 2026-08-15 |
| 16 | WildCard11 (`g60-51`) | generalist | 4/29 | 5.5% | 1791.9 | ±74.3 | 2026-08-15 |
| 17 | ProphetMotive (`adv-religious`) | religious specialist | 3/22 | 4.7% | 1576.8 | ±52.4 | 2026-08-15 |
| 18 | FreeSpirit4 (`g36-33`) | generalist | 4/36 | 4.4% | 1677.4 | ±57.2 | 2026-08-15 |
| 19 | JackKnife2 (`g60-52`) | generalist | 4/37 | 4.3% | 1747.8 | ±67.9 | 2026-08-15 |
| 20 | WildCard2 (`g28-27`) | generalist | 3/36 | 2.9% | 1701.2 | ±56.7 | 2026-08-15 |
| 21 | FreeSpirit (`g16-18`) | generalist | 2/22 | 2.5% | 1669.0 | ±52.5 | 2026-08-15 |
| 22 | DarkHorse3 (`g20-22`) | generalist | 2/22 | 2.5% | 1641.9 | ±57.2 | 2026-08-15 |
| 23 | Opportunist5 (`g32-32`) | generalist | 2/22 | 2.5% | 1638.1 | ±57.0 | 2026-08-15 |
| 24 | Maverick5 (`g48-42`) | generalist | 2/27 | 2.1% | 1708.8 | ±55.5 | 2026-08-15 |
| 25 | WildCard5 (`g44-40`) | generalist | 2/27 | 2.1% | 1695.8 | ±57.8 | 2026-08-15 |
| 26 | DarkHorse7 (`g60-53`) | generalist | 2/28 | 2.0% | 1690.1 | ±73.6 | 2026-08-15 |
| 27 | Opportunist6 (`g40-37`) | generalist | 2/32 | 1.7% | 1740.2 | ±55.8 | 2026-08-15 |
| 28 | Opportunist2 (`g20-23`) | generalist | 1/22 | 0.8% | 1609.7 | ±58.5 | 2026-08-15 |
| 29 | HolyRoller (`g12-15`) | religious specialist | 1/22 | 0.8% | 1567.4 | ±53.1 | 2026-08-15 |
| 30 | DarkHorse2 (`g16-20`) | generalist | 1/23 | 0.8% | 1657.6 | ±52.1 | 2026-08-15 |
| 31 | FreeSpirit3 (`g32-30`) | generalist | 1/24 | 0.7% | 1666.3 | ±57.2 | 2026-08-15 |
| 32 | Maverick6 (`g56-49`) | generalist | 1/25 | 0.7% | 1664.9 | ±56.8 | 2026-08-15 |
| 33 | Opportunist (`g12-16`) | generalist | 0/22 | 0.0% | 1637.3 | ±56.6 | 2026-08-15 |
| 34 | Maverick (`g12-17`) | generalist | 0/22 | 0.0% | 1639.7 | ±58.4 | 2026-08-15 |
| 35 | Opportunist4 (`g28-29`) | generalist | 0/22 | 0.0% | 1634.0 | ±56.9 | 2026-08-15 |
| 36 | ApostlePaula (`g8-13`) | religious specialist | 0/22 | 0.0% | 1582.6 | ±52.7 | 2026-08-15 |
| 37 | ScoreKeeper (`g8-14`) | score specialist | 0/22 | 0.0% | 1551.2 | ±52.6 | 2026-08-15 |
| 38 | TrainingWheels (`basic`) | builtin basic | 0/22 | 0.0% | 1491.2 | ±54.6 | 2026-08-15 |
| 39 | FaithHealer (`g4-11`) | religious specialist | 0/22 | 0.0% | 1483.3 | ±54.3 | 2026-08-15 |
| 40 | Warmonger (`adv-domination`) | domination specialist | 0/22 | 0.0% | 1435.5 | ±55.0 | 2026-08-15 |
| 41 | BloodAndIron (`g8-12`) | domination specialist | 0/22 | 0.0% | 1438.7 | ±62.7 | 2026-08-15 |
| 42 | SilverTongue (`adv-diplomatic`) | diplomatic specialist | 0/22 | 0.0% | 1282.6 | ±59.9 | 2026-08-15 |
| 43 | PointHoarder (`adv-score`) | score specialist | 0/21 | 0.0% | 1584.7 | ±51.6 | 2026-08-15 |
| 44 | TechPriest (`adv-science`) | science specialist | 0/21 | 0.0% | 1225.1 | ±64.1 | 2026-08-15 |
| 45 | CultureVulture (`adv-culture`) | culture specialist | 0/20 | 0.0% | 1182.7 | ±61.6 | 2026-08-15 |
| 46 | SilverTongue2 (`g52-47`) | diplomatic specialist | 0/20 | 0.0% | 1211.5 | ±81.5 | 2026-08-15 |
| 47 | Opportunist7 (`g4032-57`) | generalist | 0/1 | 0.0% | 1524.6 | ±269.8 | 2026-08-15 |
| 48 | Eureka (`g4-9`) | science specialist | 0/20 | 0.0% | 1124.9 | ±74.9 | 2026-08-15 |
| 49 | OperaGhost (`g4032-58`) | culture specialist | 0/1 | 0.0% | 1404.8 | ±268.4 | 2026-08-15 |
| 50 | FreeSpirit7 (`g4032-56`) | generalist | 0/1 | 0.0% | 1408.3 | ±270.6 | 2026-08-15 |

## Current strategies without 8-player evidence

These strategies remain eligible, but have no retained 8-player win record. Their
placement rating is shown for identification only; they are deliberately not mixed into
the evidence-backed ranking above.

| Player (strategy) | Role | Placement Elo | RD | Last played |
|---|---|---:|---:|---|
| BloodAndIron2 (`g4096-61`) | domination specialist | 1500.0 | ±350.0 | — |
| DarkHorse8 (`g4096-59`) | generalist | 1500.0 | ±350.0 | — |
| Maverick7 (`g4096-60`) | generalist | 1500.0 | ±350.0 | — |
