# AI Player Elo Rankings

League round **60**, generated from `data/league/league.json`. This complete table
contains every one of the **156** recorded leader/civilization-specific Glicko-2
ratings. It includes active, retired, and human strategies whenever a rating record
exists, and is sorted by exact pair Elo descending.

The league roster has **56** named strategies; **39** have at least one exact pair
rating. The remaining **17** are listed separately with their global rating because
CIVVIS has not yet recorded an Elo for a specific civilization/leader pair.

Refresh this document after updating the committed league snapshot:

`python3 tools/update_ai_player_elo_rankings.py`

Use `--check` to verify that this generated file is current.

| Rank | Elo | Player (strategy) — civilization — leader | RD | Games | Wins | Status |
|---:|---:|---|---:|---:|---:|---|
| 1 | 2157.8 | WildCard9 (`g56-48`) — Rome — Trajan | ±131.9 | 4 | 4 | retired |
| 2 | 1920.6 | WildCard10 (`g56-50`) — Rome — Trajan | ±137.6 | 3 | 1 | active |
| 3 | 1918.3 | DarkHorse4 (`g24-26`) — Rome — Trajan | ±143.9 | 3 | 2 | retired |
| 4 | 1914.3 | Maverick2 (`g20-21`) — Rome — Trajan | ±45.1 | 46 | 24 | active |
| 5 | 1907.9 | WildCard7 (`g52-45`) — Rome — Trajan | ±125.8 | 4 | 2 | retired |
| 6 | 1894.6 | FreeSpirit4 (`g36-33`) — Rome — Trajan | ±82.7 | 10 | 5 | retired |
| 7 | 1865.3 | WildCard10 (`g56-50`) — Egypt — Cleopatra | ±84.9 | 6 | 2 | active |
| 8 | 1859.2 | Opportunist4 (`g28-29`) — Greece — Pericles | ±110.1 | 4 | 1 | retired |
| 9 | 1847.0 | JackKnife (`g44-41`) — China — Qin Shi Huang | ±51.0 | 24 | 10 | active |
| 10 | 1846.2 | DarkHorse6 (`g44-39`) — China — Qin Shi Huang | ±99.6 | 6 | 2 | retired |
| 11 | 1843.1 | OldGuard (`advanced_v1`) — China — Qin Shi Huang | ±42.6 | 54 | 24 | active |
| 12 | 1835.7 | FreeSpirit6 (`g48-44`) — Greece — Pericles | ±61.6 | 17 | 8 | active |
| 13 | 1833.9 | Opportunist6 (`g40-37`) — Rome — Trajan | ±59.1 | 18 | 9 | retired |
| 14 | 1830.7 | WildCard6 (`g48-43`) — China — Qin Shi Huang | ±60.2 | 14 | 6 | active |
| 15 | 1828.3 | Opportunist3 (`g28-28`) — China — Qin Shi Huang | ±46.8 | 31 | 14 | active |
| 16 | 1826.3 | WildCard (`g24-24`) — Rome — Trajan | ±114.5 | 3 | 1 | retired |
| 17 | 1820.0 | WildCard2 (`g28-27`) — China — Qin Shi Huang | ±54.3 | 23 | 9 | retired |
| 18 | 1819.4 | JackKnife (`g44-41`) — Rome — Trajan | ±55.9 | 20 | 9 | active |
| 19 | 1819.2 | WildCard6 (`g48-43`) — Greece — Pericles | ±65.9 | 13 | 5 | active |
| 20 | 1817.9 | DarkHorse5 (`g40-38`) — Rome — Trajan | ±92.5 | 6 | 3 | retired |
| 21 | 1804.0 | DarkHorse (`g4-10`) — Rome — Trajan | ±43.8 | 43 | 21 | retired |
| 22 | 1795.5 | WildCard6 (`g48-43`) — Rome — Trajan | ±49.9 | 21 | 9 | active |
| 23 | 1793.5 | Maverick6 (`g56-49`) — Egypt — Cleopatra | ±115.9 | 5 | 0 | retired |
| 24 | 1793.1 | Maverick2 (`g20-21`) — China — Qin Shi Huang | ±40.1 | 70 | 32 | active |
| 25 | 1783.7 | WildCard (`g24-24`) — China — Qin Shi Huang | ±95.8 | 5 | 2 | retired |
| 26 | 1781.3 | Opportunist6 (`g40-37`) — China — Qin Shi Huang | ±54.4 | 19 | 7 | retired |
| 27 | 1780.3 | HolyRoller (`g12-15`) — Rome — Trajan | ±94.0 | 5 | 3 | retired |
| 28 | 1780.0 | Opportunist3 (`g28-28`) — Rome — Trajan | ±40.7 | 53 | 19 | active |
| 29 | 1779.7 | WildCard4 (`g40-36`) — Rome — Trajan | ±61.2 | 15 | 4 | retired |
| 30 | 1779.1 | JackOfAllTrades (`advanced`) — Rome — Trajan | ±40.3 | 48 | 15 | active |
| 31 | 1777.5 | WildCard8 (`g52-46`) — Rome — Trajan | ±72.8 | 9 | 3 | retired |
| 32 | 1777.4 | FreeSpirit5 (`g36-34`) — Greece — Pericles | ±69.6 | 10 | 3 | retired |
| 33 | 1772.8 | DarkHorse (`g4-10`) — China — Qin Shi Huang | ±42.5 | 47 | 22 | retired |
| 34 | 1770.6 | Maverick6 (`g56-49`) — Greece — Pericles | ±95.5 | 5 | 2 | retired |
| 35 | 1769.6 | OldGuard (`advanced_v1`) — Rome — Trajan | ±39.5 | 60 | 20 | active |
| 36 | 1769.2 | Maverick5 (`g48-42`) — Greece — Pericles | ±61.8 | 15 | 2 | retired |
| 37 | 1767.5 | WildCard7 (`g52-45`) — Egypt — Cleopatra | ±87.9 | 7 | 2 | retired |
| 38 | 1766.2 | Maverick3 (`g24-25`) — Rome — Trajan | ±67.9 | 10 | 5 | retired |
| 39 | 1765.1 | WildCard2 (`g28-27`) — Greece — Pericles | ±47.0 | 30 | 7 | retired |
| 40 | 1764.6 | FreeSpirit2 (`g16-19`) — Rome — Trajan | ±45.5 | 30 | 10 | retired |
| 41 | 1761.4 | FreeSpirit6 (`g48-44`) — Rome — Trajan | ±66.8 | 12 | 3 | active |
| 42 | 1760.2 | WildCard5 (`g44-40`) — Greece — Pericles | ±104.5 | 5 | 1 | retired |
| 43 | 1759.8 | Maverick5 (`g48-42`) — China — Qin Shi Huang | ±74.1 | 9 | 4 | retired |
| 44 | 1759.4 | WildCard10 (`g56-50`) — Greece — Pericles | ±70.7 | 9 | 1 | active |
| 45 | 1751.8 | WildCard10 (`g56-50`) — China — Qin Shi Huang | ±120.0 | 3 | 0 | active |
| 46 | 1750.4 | Maverick6 (`g56-49`) — Rome — Trajan | ±89.8 | 6 | 1 | retired |
| 47 | 1749.7 | FreeSpirit2 (`g16-19`) — Greece — Pericles | ±55.7 | 16 | 4 | retired |
| 48 | 1749.1 | FreeSpirit (`g16-18`) — Rome — Trajan | ±68.3 | 11 | 5 | retired |
| 49 | 1746.3 | Maverick4 (`g36-35`) — Rome — Trajan | ±146.3 | 2 | 0 | retired |
| 50 | 1745.4 | FreeSpirit2 (`g16-19`) — China — Qin Shi Huang | ±45.9 | 30 | 10 | retired |
| 51 | 1744.2 | WildCard8 (`g52-46`) — Greece — Pericles | ±66.6 | 12 | 2 | retired |
| 52 | 1744.0 | WildCard5 (`g44-40`) — Rome — Trajan | ±90.6 | 6 | 3 | retired |
| 53 | 1740.3 | FreeSpirit (`g16-18`) — China — Qin Shi Huang | ±65.2 | 12 | 4 | retired |
| 54 | 1740.1 | WildCard3 (`g32-31`) — Greece — Pericles | ±98.5 | 5 | 0 | retired |
| 55 | 1739.4 | Opportunist3 (`g28-28`) — Greece — Pericles | ±43.1 | 43 | 12 | active |
| 56 | 1737.8 | Opportunist3 (`g28-28`) — Egypt — Cleopatra | ±43.0 | 43 | 16 | active |
| 57 | 1737.6 | WildCard4 (`g40-36`) — China — Qin Shi Huang | ±51.9 | 21 | 2 | retired |
| 58 | 1730.1 | WildCard2 (`g28-27`) — Rome — Trajan | ±43.6 | 36 | 12 | retired |
| 59 | 1727.0 | DarkHorse3 (`g20-22`) — China — Qin Shi Huang | ±102.5 | 5 | 3 | retired |
| 60 | 1726.8 | WildCard4 (`g40-36`) — Greece — Pericles | ±60.9 | 14 | 4 | retired |
| 61 | 1722.5 | FreeSpirit5 (`g36-34`) — Egypt — Cleopatra | ±75.4 | 8 | 2 | retired |
| 62 | 1720.6 | FreeSpirit4 (`g36-33`) — China — Qin Shi Huang | ±78.4 | 9 | 3 | retired |
| 63 | 1719.9 | OldGuard (`advanced_v1`) — Greece — Pericles | ±41.2 | 49 | 9 | active |
| 64 | 1719.3 | DarkHorse2 (`g16-20`) — China — Qin Shi Huang | ±47.8 | 25 | 7 | retired |
| 65 | 1715.8 | FreeSpirit6 (`g48-44`) — Egypt — Cleopatra | ±54.4 | 18 | 4 | active |
| 66 | 1713.0 | JackOfAllTrades (`advanced`) — Greece — Pericles | ±42.2 | 60 | 13 | active |
| 67 | 1706.2 | DarkHorse3 (`g20-22`) — Greece — Pericles | ±108.8 | 4 | 1 | retired |
| 68 | 1704.3 | DarkHorse2 (`g16-20`) — Rome — Trajan | ±47.7 | 26 | 5 | retired |
| 69 | 1703.7 | FreeSpirit5 (`g36-34`) — Rome — Trajan | ±60.4 | 14 | 3 | retired |
| 70 | 1703.1 | DarkHorse (`g4-10`) — Greece — Pericles | ±47.0 | 31 | 5 | retired |
| 71 | 1699.9 | Maverick2 (`g20-21`) — Egypt — Cleopatra | ±42.3 | 51 | 16 | active |
| 72 | 1699.5 | FreeSpirit3 (`g32-30`) — China — Qin Shi Huang | ±96.5 | 6 | 2 | retired |
| 73 | 1696.1 | Maverick3 (`g24-25`) — China — Qin Shi Huang | ±82.0 | 7 | 2 | retired |
| 74 | 1692.7 | Maverick2 (`g20-21`) — Greece — Pericles | ±43.9 | 49 | 10 | active |
| 75 | 1690.9 | Opportunist5 (`g32-32`) — Greece — Pericles | ±81.4 | 7 | 2 | retired |
| 76 | 1690.7 | Maverick5 (`g48-42`) — Rome — Trajan | ±65.1 | 12 | 4 | retired |
| 77 | 1686.3 | HolyRoller (`g12-15`) — Greece — Pericles | ±114.6 | 4 | 2 | retired |
| 78 | 1685.8 | JackOfAllTrades (`advanced`) — China — Qin Shi Huang | ±40.5 | 63 | 17 | active |
| 79 | 1681.4 | TrainingWheels (`basic`) — Rome — Trajan | ±44.5 | 41 | 7 | active |
| 80 | 1680.3 | OldGuard (`advanced_v1`) — Egypt — Cleopatra | ±41.7 | 54 | 15 | active |
| 81 | 1673.8 | WildCard5 (`g44-40`) — China — Qin Shi Huang | ±85.0 | 6 | 1 | retired |
| 82 | 1673.7 | FreeSpirit3 (`g32-30`) — Rome — Trajan | ±77.0 | 8 | 2 | retired |
| 83 | 1673.4 | Opportunist2 (`g20-23`) — China — Qin Shi Huang | ±98.8 | 5 | 1 | retired |
| 84 | 1670.9 | JackKnife (`g44-41`) — Egypt — Cleopatra | ±52.5 | 22 | 5 | active |
| 85 | 1670.6 | Maverick4 (`g36-35`) — China — Qin Shi Huang | ±109.3 | 4 | 1 | retired |
| 86 | 1665.4 | WildCard4 (`g40-36`) — Egypt — Cleopatra | ±64.8 | 13 | 3 | retired |
| 87 | 1661.4 | WildCard9 (`g56-48`) — Egypt — Cleopatra | ±65.6 | 11 | 2 | retired |
| 88 | 1659.1 | FreeSpirit4 (`g36-33`) — Greece — Pericles | ±69.5 | 13 | 0 | retired |
| 89 | 1658.7 | FreeSpirit6 (`g48-44`) — China — Qin Shi Huang | ±55.9 | 16 | 3 | active |
| 90 | 1657.7 | Maverick4 (`g36-35`) — Greece — Pericles | ±67.1 | 11 | 2 | retired |
| 91 | 1654.1 | DarkHorse2 (`g16-20`) — Greece — Pericles | ±47.2 | 25 | 4 | retired |
| 92 | 1652.8 | WildCard3 (`g32-31`) — China — Qin Shi Huang | ±121.9 | 3 | 0 | retired |
| 93 | 1652.7 | Opportunist4 (`g28-29`) — Rome — Trajan | ±73.6 | 9 | 3 | retired |
| 94 | 1652.3 | DarkHorse (`g4-10`) — Egypt — Cleopatra | ±46.0 | 31 | 7 | retired |
| 95 | 1651.4 | JackKnife (`g44-41`) — Greece — Pericles | ±56.2 | 18 | 2 | active |
| 96 | 1649.7 | DarkHorse4 (`g24-26`) — Egypt — Cleopatra | ±93.4 | 5 | 1 | retired |
| 97 | 1645.1 | WildCard7 (`g52-45`) — China — Qin Shi Huang | ±130.8 | 3 | 1 | retired |
| 98 | 1642.8 | ScoreKeeper (`g8-14`) — Rome — Trajan | ±69.4 | 9 | 0 | retired |
| 99 | 1642.7 | WildCard2 (`g28-27`) — Egypt — Cleopatra | ±43.2 | 37 | 5 | retired |
| 100 | 1638.3 | WildCard5 (`g44-40`) — Egypt — Cleopatra | ±119.5 | 4 | 0 | retired |
| 101 | 1636.2 | Opportunist5 (`g32-32`) — Rome — Trajan | ±87.1 | 6 | 1 | retired |
| 102 | 1635.6 | Opportunist2 (`g20-23`) — Greece — Pericles | ±80.6 | 8 | 1 | retired |
| 103 | 1631.6 | Opportunist4 (`g28-29`) — China — Qin Shi Huang | ±107.8 | 4 | 0 | retired |
| 104 | 1625.6 | Maverick6 (`g56-49`) — China — Qin Shi Huang | ±88.0 | 7 | 0 | retired |
| 105 | 1625.1 | Opportunist5 (`g32-32`) — Egypt — Cleopatra | ±104.2 | 4 | 0 | retired |
| 106 | 1625.0 | Maverick4 (`g36-35`) — Egypt — Cleopatra | ±110.7 | 4 | 1 | retired |
| 107 | 1624.3 | WildCard8 (`g52-46`) — Egypt — Cleopatra | ±68.8 | 12 | 2 | retired |
| 108 | 1620.4 | DarkHorse2 (`g16-20`) — Egypt — Cleopatra | ±48.1 | 27 | 7 | retired |
| 109 | 1620.3 | FreeSpirit (`g16-18`) — Egypt — Cleopatra | ±52.4 | 20 | 4 | retired |
| 110 | 1618.9 | Opportunist6 (`g40-37`) — Greece — Pericles | ±57.9 | 16 | 3 | retired |
| 111 | 1617.5 | WildCard8 (`g52-46`) — China — Qin Shi Huang | ±74.3 | 10 | 2 | retired |
| 112 | 1615.7 | Opportunist6 (`g40-37`) — Egypt — Cleopatra | ±68.8 | 11 | 3 | retired |
| 113 | 1613.8 | FreeSpirit (`g16-18`) — Greece — Pericles | ±50.8 | 20 | 2 | retired |
| 114 | 1613.5 | DarkHorse4 (`g24-26`) — Greece — Pericles | ±69.6 | 9 | 1 | retired |
| 115 | 1603.4 | Maverick3 (`g24-25`) — Egypt — Cleopatra | ±60.7 | 14 | 1 | retired |
| 116 | 1596.7 | Opportunist2 (`g20-23`) — Egypt — Cleopatra | ±119.4 | 3 | 0 | retired |
| 117 | 1594.7 | WildCard3 (`g32-31`) — Egypt — Cleopatra | ±87.8 | 6 | 0 | retired |
| 118 | 1590.9 | WildCard3 (`g32-31`) — Rome — Trajan | ±87.2 | 6 | 0 | retired |
| 119 | 1587.7 | FreeSpirit5 (`g36-34`) — China — Qin Shi Huang | ±80.6 | 10 | 2 | retired |
| 120 | 1575.5 | FreeSpirit2 (`g16-19`) — Egypt — Cleopatra | ±48.0 | 29 | 2 | retired |
| 121 | 1570.5 | FreeSpirit3 (`g32-30`) — Greece — Pericles | ±109.7 | 4 | 0 | retired |
| 122 | 1568.5 | FreeSpirit4 (`g36-33`) — Egypt — Cleopatra | ±66.6 | 12 | 1 | retired |
| 123 | 1564.0 | DarkHorse3 (`g20-22`) — Rome — Trajan | ±72.0 | 9 | 1 | retired |
| 124 | 1559.3 | WildCard (`g24-24`) — Greece — Pericles | ±71.7 | 9 | 2 | retired |
| 125 | 1557.0 | DarkHorse6 (`g44-39`) — Egypt — Cleopatra | ±89.7 | 6 | 0 | retired |
| 126 | 1555.5 | WildCard7 (`g52-45`) — Greece — Pericles | ±86.0 | 7 | 2 | retired |
| 127 | 1541.3 | Opportunist5 (`g32-32`) — China — Qin Shi Huang | ±108.4 | 5 | 1 | retired |
| 128 | 1540.6 | DarkHorse6 (`g44-39`) — Greece — Pericles | ±149.2 | 2 | 0 | retired |
| 129 | 1540.5 | Opportunist4 (`g28-29`) — Egypt — Cleopatra | ±99.8 | 5 | 1 | retired |
| 130 | 1539.6 | TrainingWheels (`basic`) — Greece — Pericles | ±44.8 | 58 | 0 | active |
| 131 | 1533.5 | ScoreKeeper (`g8-14`) — Greece — Pericles | ±70.6 | 11 | 1 | retired |
| 132 | 1526.0 | ScoreKeeper (`g8-14`) — Egypt — Cleopatra | ±69.7 | 11 | 0 | retired |
| 133 | 1517.4 | Maverick3 (`g24-25`) — Greece — Pericles | ±67.9 | 11 | 0 | retired |
| 134 | 1517.2 | WildCard (`g24-24`) — Egypt — Cleopatra | ±101.9 | 5 | 1 | retired |
| 135 | 1512.9 | FreeSpirit3 (`g32-30`) — Egypt — Cleopatra | ±97.7 | 5 | 0 | retired |
| 136 | 1508.1 | DarkHorse4 (`g24-26`) — China — Qin Shi Huang | ±110.8 | 4 | 0 | retired |
| 137 | 1501.3 | JackOfAllTrades (`advanced`) — Egypt — Cleopatra | ±46.7 | 48 | 7 | active |
| 138 | 1499.0 | WildCard6 (`g48-43`) — Egypt — Cleopatra | ±58.7 | 19 | 1 | active |
| 139 | 1475.2 | ScoreKeeper (`g8-14`) — China — Qin Shi Huang | ±70.9 | 10 | 0 | retired |
| 140 | 1469.8 | Maverick5 (`g48-42`) — Egypt — Cleopatra | ±98.4 | 6 | 0 | retired |
| 141 | 1468.7 | DarkHorse5 (`g40-38`) — Egypt — Cleopatra | ±113.1 | 5 | 1 | retired |
| 142 | 1467.0 | Opportunist2 (`g20-23`) — Rome — Trajan | ±95.5 | 6 | 0 | retired |
| 143 | 1431.5 | DarkHorse5 (`g40-38`) — China — Qin Shi Huang | ±142.9 | 4 | 0 | retired |
| 144 | 1414.2 | HolyRoller (`g12-15`) — China — Qin Shi Huang | ±102.5 | 5 | 0 | retired |
| 145 | 1396.4 | DarkHorse5 (`g40-38`) — Greece — Pericles | ±98.7 | 6 | 0 | retired |
| 146 | 1373.8 | WildCard9 (`g56-48`) — Greece — Pericles | ±147.1 | 4 | 0 | retired |
| 147 | 1370.3 | WildCard9 (`g56-48`) — China — Qin Shi Huang | ±216.5 | 1 | 0 | retired |
| 148 | 1356.2 | TrainingWheels (`basic`) — China — Qin Shi Huang | ±54.5 | 53 | 1 | active |
| 149 | 1315.9 | DarkHorse3 (`g20-22`) — Egypt — Cleopatra | ±134.8 | 3 | 0 | retired |
| 150 | 1312.4 | DarkHorse6 (`g44-39`) — Rome — Trajan | ±123.9 | 6 | 0 | retired |
| 151 | 1300.3 | SilverTongue2 (`g52-47`) — Greece — Pericles | ±112.8 | 6 | 0 | retired |
| 152 | 1269.3 | SilverTongue2 (`g52-47`) — China — Qin Shi Huang | ±118.6 | 7 | 0 | retired |
| 153 | 1246.5 | SilverTongue2 (`g52-47`) — Egypt — Cleopatra | ±132.7 | 4 | 0 | retired |
| 154 | 1221.1 | HolyRoller (`g12-15`) — Egypt — Cleopatra | ±108.7 | 6 | 0 | retired |
| 155 | 1211.1 | TrainingWheels (`basic`) — Egypt — Cleopatra | ±60.4 | 60 | 0 | active |
| 156 | 1166.2 | SilverTongue2 (`g52-47`) — Rome — Trajan | ±158.9 | 4 | 0 | retired |

## Strategies without a civilization/leader Elo

These roster strategies have no `leader_elo` entries. Their global Glicko-2 rating is
shown in descending order, but it is deliberately not mixed into the exact
civilization/leader ranking above.

| Global Elo | Player (strategy) | RD | Games | Wins | Status |
|---:|---|---:|---:|---:|---|
| 1702.7 | TheHeir (`advanced_evolved`) | ±350.0 | 0 | 0 | active |
| 1645.8 | ApostlePaula (`g8-13`) | ±32.8 | 61 | 22 | retired |
| 1620.5 | PointHoarder (`adv-score`) | ±30.6 | 115 | 24 | retired |
| 1600.6 | ProphetMotive (`adv-religious`) | ±30.8 | 116 | 58 | retired |
| 1591.3 | Maverick (`g12-17`) | ±50.5 | 20 | 3 | retired |
| 1576.3 | Opportunist (`g12-16`) | ±45.2 | 23 | 3 | retired |
| 1512.1 | FaithHealer (`g4-11`) | ±32.8 | 68 | 17 | retired |
| 1500.0 | DarkHorse7 (`g60-53`) | ±350.0 | 0 | 0 | active |
| 1500.0 | DeepThought (`strategic`) | ±350.0 | 0 | 0 | active |
| 1500.0 | JackKnife2 (`g60-52`) | ±350.0 | 0 | 0 | active |
| 1500.0 | WildCard11 (`g60-51`) | ±350.0 | 0 | 0 | active |
| 1466.1 | Warmonger (`adv-domination`) | ±34.3 | 73 | 5 | retired |
| 1441.9 | BloodAndIron (`g8-12`) | ±52.8 | 20 | 2 | retired |
| 1285.9 | SilverTongue (`adv-diplomatic`) | ±39.3 | 71 | 5 | retired |
| 1216.8 | TechPriest (`adv-science`) | ±46.7 | 49 | 1 | retired |
| 1183.4 | CultureVulture (`adv-culture`) | ±42.8 | 51 | 1 | retired |
| 1139.7 | Eureka (`g4-9`) | ±62.6 | 23 | 0 | retired |
