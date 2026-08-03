# Current AI Strategy Rankings

League round **4002**, generated from `data/league/league.json`. This table contains the
**55** strategies currently eligible for live games and ranks their exact **8-player**
evidence. Retired, human, and offline-only entries are omitted (1 roster entry at this
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
| 1 | DarkHorse4 (`g24-26`) | generalist | 189/1069 | 15.5% | 1768.4 | ±59.7 | 2026-08-02 |
| 2 | Opportunist3 (`g28-28`) | generalist | 536/3510 | 14.1% | 1719.6 | ±58.2 | 2026-08-02 |
| 3 | TheHeir (`advanced_evolved`) | builtin advanced_evolved | 536/3703 | 13.4% | 1671.2 | ±57.7 | 2026-08-02 |
| 4 | Maverick2 (`g20-21`) | generalist | 496/3721 | 12.3% | 1749.5 | ±58.3 | 2026-08-02 |
| 5 | WildCard6 (`g48-43`) | generalist | 480/3822 | 11.5% | 1704.7 | ±57.9 | 2026-08-02 |
| 6 | FreeSpirit6 (`g48-44`) | generalist | 433/3661 | 10.8% | 1740.4 | ±57.7 | 2026-08-02 |
| 7 | JackOfAllTrades (`advanced`) | builtin advanced | 404/3442 | 10.7% | 1720.1 | ±58.5 | 2026-08-02 |
| 8 | JackKnife (`g44-41`) | generalist | 415/3566 | 10.6% | 1704.0 | ±57.6 | 2026-08-02 |
| 9 | DarkHorse6 (`g44-39`) | generalist | 58/457 | 9.9% | 1743.8 | ±60.1 | 2026-08-02 |
| 10 | FreeSpirit5 (`g36-34`) | generalist | 9/51 | 9.6% | 1710.2 | ±59.2 | 2026-08-02 |
| 11 | WildCard10 (`g56-50`) | generalist | 193/2042 | 8.3% | 1700.3 | ±59.0 | 2026-08-02 |
| 12 | WildCard8 (`g52-46`) | generalist | 17/132 | 8.2% | 1705.2 | ±60.7 | 2026-08-02 |
| 13 | WildCard7 (`g52-45`) | generalist | 6/35 | 8.1% | 1744.3 | ±59.1 | 2026-08-02 |
| 14 | DarkHorse5 (`g40-38`) | generalist | 7/45 | 7.7% | 1725.1 | ±59.9 | 2026-08-02 |
| 15 | Maverick3 (`g24-25`) | generalist | 4/22 | 7.3% | 1679.9 | ±53.3 | 2026-08-02 |
| 16 | WildCard3 (`g32-31`) | generalist | 4/23 | 7.0% | 1717.4 | ±57.3 | 2026-08-02 |
| 17 | DarkHorse (`g4-10`) | generalist | 8/60 | 6.9% | 1735.9 | ±60.2 | 2026-08-02 |
| 18 | OldGuard (`advanced_v1`) | builtin advanced_v1 | 100/1291 | 6.4% | 1618.3 | ±61.6 | 2026-08-02 |
| 19 | WildCard11 (`g60-51`) | generalist | 4/27 | 5.9% | 1779.4 | ±76.2 | 2026-08-02 |
| 20 | WildCard4 (`g40-36`) | generalist | 4/27 | 5.9% | 1717.7 | ±54.1 | 2026-08-02 |
| 21 | Maverick4 (`g36-35`) | generalist | 4/27 | 5.9% | 1665.2 | ±58.0 | 2026-08-02 |
| 22 | FreeSpirit2 (`g16-19`) | generalist | 5/37 | 5.9% | 1708.6 | ±56.6 | 2026-08-02 |
| 23 | ProphetMotive (`adv-religious`) | religious specialist | 3/21 | 5.0% | 1581.6 | ±51.9 | 2026-08-02 |
| 24 | FreeSpirit4 (`g36-33`) | generalist | 4/35 | 4.5% | 1686.0 | ±57.0 | 2026-08-02 |
| 25 | JackKnife2 (`g60-52`) | generalist | 4/36 | 4.4% | 1739.2 | ±68.4 | 2026-08-02 |
| 26 | WildCard2 (`g28-27`) | generalist | 3/35 | 3.0% | 1705.0 | ±56.5 | 2026-08-02 |
| 27 | Opportunist5 (`g32-32`) | generalist | 2/21 | 2.7% | 1637.5 | ±56.7 | 2026-08-02 |
| 28 | Maverick5 (`g48-42`) | generalist | 2/26 | 2.1% | 1712.1 | ±55.2 | 2026-08-02 |
| 29 | WildCard5 (`g44-40`) | generalist | 2/26 | 2.1% | 1693.9 | ±57.7 | 2026-08-02 |
| 30 | DarkHorse7 (`g60-53`) | generalist | 2/27 | 2.1% | 1690.9 | ±74.5 | 2026-08-02 |
| 31 | Opportunist6 (`g40-37`) | generalist | 2/31 | 1.8% | 1739.5 | ±55.6 | 2026-08-02 |
| 32 | FreeSpirit (`g16-18`) | generalist | 1/20 | 0.9% | 1655.3 | ±51.4 | 2026-08-02 |
| 33 | DarkHorse3 (`g20-22`) | generalist | 1/20 | 0.9% | 1624.3 | ±56.8 | 2026-08-02 |
| 34 | HolyRoller (`g12-15`) | religious specialist | 1/20 | 0.9% | 1565.5 | ±52.0 | 2026-08-02 |
| 35 | DarkHorse2 (`g16-20`) | generalist | 1/22 | 0.8% | 1655.4 | ±51.6 | 2026-08-02 |
| 36 | FreeSpirit3 (`g32-30`) | generalist | 1/23 | 0.8% | 1669.1 | ±57.0 | 2026-08-02 |
| 37 | Maverick6 (`g56-49`) | generalist | 1/24 | 0.7% | 1670.0 | ±56.6 | 2026-08-02 |
| 38 | WildCard (`g24-24`) | generalist | 0/22 | 0.0% | 1663.8 | ±56.7 | 2026-08-02 |
| 39 | WildCard9 (`g56-48`) | generalist | 0/23 | 0.0% | 1731.2 | ±57.2 | 2026-08-02 |
| 40 | Opportunist4 (`g28-29`) | generalist | 0/21 | 0.0% | 1630.4 | ±56.7 | 2026-08-02 |
| 41 | Opportunist (`g12-16`) | generalist | 0/20 | 0.0% | 1627.5 | ±56.1 | 2026-08-02 |
| 42 | Maverick (`g12-17`) | generalist | 0/20 | 0.0% | 1629.9 | ±58.0 | 2026-08-02 |
| 43 | PointHoarder (`adv-score`) | score specialist | 0/20 | 0.0% | 1589.5 | ±51.0 | 2026-08-02 |
| 44 | ApostlePaula (`g8-13`) | religious specialist | 0/20 | 0.0% | 1590.4 | ±51.6 | 2026-08-02 |
| 45 | Opportunist2 (`g20-23`) | generalist | 0/20 | 0.0% | 1586.9 | ±58.0 | 2026-08-02 |
| 46 | ScoreKeeper (`g8-14`) | score specialist | 0/20 | 0.0% | 1560.1 | ±51.4 | 2026-08-02 |
| 47 | TrainingWheels (`basic`) | builtin basic | 0/20 | 0.0% | 1486.4 | ±53.5 | 2026-08-02 |
| 48 | FaithHealer (`g4-11`) | religious specialist | 0/20 | 0.0% | 1482.9 | ±53.0 | 2026-08-02 |
| 49 | Warmonger (`adv-domination`) | domination specialist | 0/20 | 0.0% | 1441.3 | ±53.8 | 2026-08-02 |
| 50 | BloodAndIron (`g8-12`) | domination specialist | 0/20 | 0.0% | 1443.2 | ±62.2 | 2026-08-02 |
| 51 | SilverTongue (`adv-diplomatic`) | diplomatic specialist | 0/20 | 0.0% | 1279.9 | ±58.6 | 2026-08-02 |
| 52 | TechPriest (`adv-science`) | science specialist | 0/19 | 0.0% | 1221.2 | ±62.8 | 2026-08-02 |
| 53 | CultureVulture (`adv-culture`) | culture specialist | 0/18 | 0.0% | 1178.5 | ±60.1 | 2026-08-02 |
| 54 | SilverTongue2 (`g52-47`) | diplomatic specialist | 0/18 | 0.0% | 1215.6 | ±81.0 | 2026-08-02 |
| 55 | Eureka (`g4-9`) | science specialist | 0/18 | 0.0% | 1127.0 | ±73.8 | 2026-08-02 |

## Current strategies without 8-player evidence

Every current strategy has retained 8-player evidence.
