# AI Player Elo Rankings

League round **150**, generated from `data/league/league.json`. This complete table
contains every one of the **496** recorded leader/civilization-specific Glicko-2
ratings. It includes active, retired, and human strategies whenever a rating record
exists, and is sorted by exact pair Elo descending.

The league roster has **56** named strategies; **40** have at least one exact pair
rating. The remaining **16** are listed separately with their global rating because
CIVVIS has not yet recorded an Elo for a specific civilization/leader pair.

Last played is the UTC date of the most recent game credited to that exact
player/civilization/leader rating.

Refresh this document after updating the committed league snapshot:

`python3 tools/update_ai_player_elo_rankings.py`

Use `--check` to verify that this generated file is current.

| Rank | Elo | Player (strategy) — civilization — leader | RD | Games | Wins | Status | Last played |
|---:|---:|---|---:|---:|---:|---|---|
| 1 | 2157.8 | WildCard9 (`g56-48`) — Rome — Trajan | ±131.9 | 4 | 4 | retired | 2026-07-23 |
| 2 | 1956.6 | TheHeir (`advanced_evolved`) — China — Qin Shi Huang | ±198.0 | 3 | 1 | active | 2026-08-01 |
| 3 | 1929.3 | WildCard10 (`g56-50`) — Sweden — Kristina | ±171.2 | 2 | 1 | active | 2026-08-01 |
| 4 | 1922.1 | Maverick2 (`g20-21`) — Scythia — Tomyris | ±170.7 | 2 | 2 | active | 2026-08-01 |
| 5 | 1920.6 | WildCard10 (`g56-50`) — Rome — Trajan | ±137.6 | 3 | 1 | active | 2026-07-23 |
| 6 | 1919.4 | JackKnife (`g44-41`) — Byzantium — Basil II | ±156.6 | 3 | 2 | active | 2026-08-01 |
| 7 | 1918.3 | DarkHorse4 (`g24-26`) — Rome — Trajan | ±143.9 | 3 | 2 | retired | 2026-07-23 |
| 8 | 1912.2 | Maverick2 (`g20-21`) — Rome — Trajan | ±46.9 | 48 | 24 | active | 2026-08-01 |
| 9 | 1911.6 | WildCard6 (`g48-43`) — Byzantium — Basil II | ±155.6 | 3 | 2 | active | 2026-08-01 |
| 10 | 1907.9 | FreeSpirit6 (`g48-44`) — Canada — Wilfrid Laurier | ±147.6 | 4 | 2 | active | 2026-08-01 |
| 11 | 1907.9 | WildCard7 (`g52-45`) — Rome — Trajan | ±125.8 | 4 | 2 | retired | 2026-07-23 |
| 12 | 1906.8 | Maverick2 (`g20-21`) — Macedon — Alexander | ±146.2 | 4 | 1 | active | 2026-08-01 |
| 13 | 1906.7 | FreeSpirit6 (`g48-44`) — Scotland — Robert the Bruce | ±157.1 | 3 | 2 | active | 2026-08-01 |
| 14 | 1903.9 | TheHeir (`advanced_evolved`) — Byzantium — Basil II | ±189.0 | 1 | 1 | active | 2026-08-01 |
| 15 | 1900.8 | Opportunist3 (`g28-28`) — Vietnam — Ba Trieu | ±155.3 | 3 | 1 | active | 2026-08-01 |
| 16 | 1900.1 | TheHeir (`advanced_evolved`) — Arabia — Saladin | ±173.1 | 2 | 1 | active | 2026-08-01 |
| 17 | 1894.6 | FreeSpirit4 (`g36-33`) — Rome — Trajan | ±82.7 | 10 | 5 | retired | 2026-07-23 |
| 18 | 1892.1 | TheHeir (`advanced_evolved`) — England — Victoria | ±190.8 | 1 | 1 | active | 2026-08-01 |
| 19 | 1878.8 | Maverick2 (`g20-21`) — Poland — Jadwiga | ±169.9 | 2 | 0 | active | 2026-08-01 |
| 20 | 1878.8 | FreeSpirit6 (`g48-44`) — Indonesia — Gitarja | ±155.8 | 3 | 1 | active | 2026-08-01 |
| 21 | 1878.3 | JackKnife (`g44-41`) — India — Gandhi | ±186.1 | 1 | 1 | active | 2026-08-01 |
| 22 | 1877.8 | TheHeir (`advanced_evolved`) — Indonesia — Gitarja | ±160.5 | 3 | 1 | active | 2026-08-01 |
| 23 | 1876.9 | TheHeir (`advanced_evolved`) — Mali — Mansa Musa | ±187.0 | 1 | 0 | active | 2026-08-01 |
| 24 | 1876.7 | Maverick2 (`g20-21`) — Vietnam — Ba Trieu | ±147.7 | 4 | 1 | active | 2026-08-01 |
| 25 | 1875.6 | WildCard10 (`g56-50`) — Byzantium — Basil II | ±185.9 | 1 | 0 | active | 2026-08-01 |
| 26 | 1874.0 | TheHeir (`advanced_evolved`) — Macedon — Alexander | ±188.2 | 1 | 1 | active | 2026-08-01 |
| 27 | 1873.9 | Opportunist3 (`g28-28`) — America — Abraham Lincoln | ±186.0 | 1 | 1 | active | 2026-08-01 |
| 28 | 1873.6 | JackKnife (`g44-41`) — France — Catherine de Medici | ±155.4 | 3 | 1 | active | 2026-08-01 |
| 29 | 1872.1 | WildCard10 (`g56-50`) — Georgia — Tamar | ±184.7 | 1 | 0 | active | 2026-08-01 |
| 30 | 1866.0 | JackKnife (`g44-41`) — Kongo — Mvemba a Nzinga | ±185.7 | 1 | 1 | active | 2026-08-01 |
| 31 | 1865.6 | Opportunist3 (`g28-28`) — Mali — Mansa Musa | ±184.9 | 1 | 1 | active | 2026-08-01 |
| 32 | 1860.6 | WildCard10 (`g56-50`) — Japan — Hojo Tokimune | ±186.5 | 1 | 0 | active | 2026-08-01 |
| 33 | 1859.2 | Opportunist4 (`g28-29`) — Greece — Pericles | ±110.1 | 4 | 1 | retired | 2026-07-23 |
| 34 | 1857.9 | WildCard10 (`g56-50`) — Netherlands — Wilhelmina | ±170.2 | 2 | 0 | active | 2026-08-01 |
| 35 | 1857.8 | JackKnife (`g44-41`) — Australia — John Curtin | ±185.9 | 1 | 1 | active | 2026-08-01 |
| 36 | 1855.2 | TheHeir (`advanced_evolved`) — Brazil — Pedro II | ±191.2 | 1 | 1 | active | 2026-08-01 |
| 37 | 1855.0 | TheHeir (`advanced_evolved`) — Zulu — Shaka | ±186.2 | 1 | 0 | active | 2026-08-01 |
| 38 | 1854.1 | JackOfAllTrades (`advanced`) — Byzantium — Basil II | ±186.5 | 1 | 1 | active | 2026-08-01 |
| 39 | 1854.0 | WildCard10 (`g56-50`) — Egypt — Cleopatra | ±83.9 | 7 | 2 | active | 2026-08-01 |
| 40 | 1851.9 | Opportunist3 (`g28-28`) — England — Victoria | ±168.4 | 2 | 0 | active | 2026-08-01 |
| 41 | 1850.6 | Opportunist3 (`g28-28`) — Inca — Pachacuti | ±186.7 | 1 | 1 | active | 2026-08-01 |
| 42 | 1850.0 | OldGuard (`advanced_v1`) — Vietnam — Ba Trieu | ±183.6 | 1 | 1 | active | 2026-08-01 |
| 43 | 1849.8 | OldGuard (`advanced_v1`) — Korea — Seondeok | ±169.7 | 2 | 1 | active | 2026-08-01 |
| 44 | 1849.1 | Opportunist3 (`g28-28`) — Mapuche — Lautaro | ±181.3 | 1 | 1 | active | 2026-08-01 |
| 45 | 1849.0 | Opportunist3 (`g28-28`) — Korea — Seondeok | ±168.7 | 2 | 1 | active | 2026-08-01 |
| 46 | 1846.8 | TheHeir (`advanced_evolved`) — Korea — Seondeok | ±145.3 | 5 | 2 | active | 2026-08-01 |
| 47 | 1846.2 | TheHeir (`advanced_evolved`) — Phoenicia — Dido | ±188.0 | 1 | 0 | active | 2026-08-01 |
| 48 | 1846.2 | DarkHorse6 (`g44-39`) — China — Qin Shi Huang | ±99.6 | 6 | 2 | retired | 2026-07-23 |
| 49 | 1846.1 | Opportunist3 (`g28-28`) — Kongo — Mvemba a Nzinga | ±168.4 | 2 | 0 | active | 2026-08-01 |
| 50 | 1845.9 | FreeSpirit6 (`g48-44`) — Greece — Pericles | ±62.5 | 22 | 10 | active | 2026-08-01 |
| 51 | 1845.6 | WildCard10 (`g56-50`) — Australia — John Curtin | ±157.5 | 3 | 0 | active | 2026-08-01 |
| 52 | 1844.6 | JackKnife (`g44-41`) — England — Victoria | ±183.4 | 1 | 1 | active | 2026-08-01 |
| 53 | 1844.3 | WildCard10 (`g56-50`) — Sumeria — Gilgamesh | ±186.3 | 1 | 0 | active | 2026-08-01 |
| 54 | 1843.7 | Maverick2 (`g20-21`) — India — Gandhi | ±146.2 | 4 | 2 | active | 2026-08-01 |
| 55 | 1843.4 | Opportunist3 (`g28-28`) — Netherlands — Wilhelmina | ±155.7 | 3 | 1 | active | 2026-08-01 |
| 56 | 1843.2 | WildCard10 (`g56-50`) — Korea — Seondeok | ±139.3 | 5 | 0 | active | 2026-08-01 |
| 57 | 1843.1 | JackOfAllTrades (`advanced`) — England — Victoria | ±137.0 | 5 | 1 | active | 2026-08-01 |
| 58 | 1842.7 | OldGuard (`advanced_v1`) — China — Qin Shi Huang | ±43.6 | 55 | 24 | active | 2026-08-01 |
| 59 | 1841.5 | Maverick2 (`g20-21`) — Kongo — Mvemba a Nzinga | ±185.3 | 1 | 0 | active | 2026-08-01 |
| 60 | 1841.3 | Maverick2 (`g20-21`) — Aztec — Montezuma | ±184.8 | 1 | 0 | active | 2026-08-01 |
| 61 | 1841.2 | Maverick2 (`g20-21`) — Khmer — Jayavarman VII | ±169.0 | 2 | 1 | active | 2026-08-01 |
| 62 | 1841.0 | Maverick2 (`g20-21`) — Portugal — João III | ±182.0 | 1 | 0 | active | 2026-08-01 |
| 63 | 1839.8 | Opportunist3 (`g28-28`) — Maya — Lady Six Sky | ±184.6 | 1 | 1 | active | 2026-08-01 |
| 64 | 1839.4 | JackKnife (`g44-41`) — Gaul — Ambiorix | ±185.4 | 1 | 0 | active | 2026-08-01 |
| 65 | 1839.2 | Maverick2 (`g20-21`) — France — Catherine de Medici | ±169.9 | 2 | 1 | active | 2026-08-01 |
| 66 | 1838.3 | JackKnife (`g44-41`) — Zulu — Shaka | ±184.2 | 1 | 1 | active | 2026-08-01 |
| 67 | 1838.2 | Maverick2 (`g20-21`) — Maya — Lady Six Sky | ±168.3 | 2 | 0 | active | 2026-08-01 |
| 68 | 1836.6 | TheHeir (`advanced_evolved`) — Canada — Wilfrid Laurier | ±187.5 | 1 | 0 | active | 2026-08-01 |
| 69 | 1836.6 | TheHeir (`advanced_evolved`) — Poland — Jadwiga | ±186.3 | 1 | 0 | active | 2026-08-01 |
| 70 | 1836.3 | WildCard10 (`g56-50`) — Ottomans — Suleiman | ±185.1 | 1 | 0 | active | 2026-08-01 |
| 71 | 1835.7 | JackKnife (`g44-41`) — Germany — Frederick Barbarossa | ±182.5 | 1 | 1 | active | 2026-08-01 |
| 72 | 1835.6 | JackKnife (`g44-41`) — Maya — Lady Six Sky | ±145.8 | 4 | 1 | active | 2026-08-01 |
| 73 | 1835.4 | WildCard6 (`g48-43`) — China — Qin Shi Huang | ±60.9 | 17 | 6 | active | 2026-08-01 |
| 74 | 1835.2 | WildCard10 (`g56-50`) — Hungary — Matthias Corvinus | ±156.5 | 3 | 0 | active | 2026-08-01 |
| 75 | 1835.2 | TheHeir (`advanced_evolved`) — Cree — Poundmaker | ±159.0 | 3 | 0 | active | 2026-08-01 |
| 76 | 1834.7 | JackOfAllTrades (`advanced`) — Poland — Jadwiga | ±168.6 | 2 | 1 | active | 2026-08-01 |
| 77 | 1834.1 | JackKnife (`g44-41`) — China — Qin Shi Huang | ±52.3 | 26 | 10 | active | 2026-08-01 |
| 78 | 1833.9 | Opportunist6 (`g40-37`) — Rome — Trajan | ±59.1 | 18 | 9 | retired | 2026-07-23 |
| 79 | 1833.6 | Opportunist3 (`g28-28`) — Mongolia — Genghis Khan | ±155.6 | 3 | 1 | active | 2026-08-01 |
| 80 | 1833.4 | Maverick2 (`g20-21`) — America — Abraham Lincoln | ±186.7 | 1 | 0 | active | 2026-08-01 |
| 81 | 1832.1 | TheHeir (`advanced_evolved`) — Greece — Pericles | ±158.6 | 3 | 0 | active | 2026-08-01 |
| 82 | 1831.7 | WildCard6 (`g48-43`) — India — Gandhi | ±185.9 | 1 | 1 | active | 2026-08-01 |
| 83 | 1830.9 | TheHeir (`advanced_evolved`) — Inca — Pachacuti | ±158.4 | 4 | 1 | active | 2026-08-01 |
| 84 | 1830.8 | WildCard6 (`g48-43`) — Babylon — Hammurabi | ±183.7 | 1 | 1 | active | 2026-08-01 |
| 85 | 1830.7 | JackOfAllTrades (`advanced`) — Georgia — Tamar | ±186.0 | 1 | 1 | active | 2026-08-01 |
| 86 | 1830.4 | WildCard10 (`g56-50`) — Maori — Kupe | ±169.2 | 2 | 1 | active | 2026-08-01 |
| 87 | 1829.9 | Opportunist3 (`g28-28`) — Gaul — Ambiorix | ±169.3 | 2 | 0 | active | 2026-08-01 |
| 88 | 1829.9 | WildCard10 (`g56-50`) — America — Abraham Lincoln | ±186.4 | 1 | 0 | active | 2026-08-01 |
| 89 | 1829.2 | Opportunist3 (`g28-28`) — Aztec — Montezuma | ±186.5 | 1 | 1 | active | 2026-08-01 |
| 90 | 1826.7 | Opportunist3 (`g28-28`) — Canada — Wilfrid Laurier | ±169.5 | 2 | 0 | active | 2026-08-01 |
| 91 | 1826.3 | WildCard (`g24-24`) — Rome — Trajan | ±114.5 | 3 | 1 | retired | 2026-07-23 |
| 92 | 1824.8 | WildCard10 (`g56-50`) — Cree — Poundmaker | ±147.5 | 4 | 0 | active | 2026-08-01 |
| 93 | 1824.6 | WildCard10 (`g56-50`) — Portugal — João III | ±185.5 | 1 | 0 | active | 2026-08-01 |
| 94 | 1824.5 | TheHeir (`advanced_evolved`) — Persia — Cyrus | ±191.6 | 1 | 0 | active | 2026-08-01 |
| 95 | 1824.4 | JackOfAllTrades (`advanced`) — Australia — John Curtin | ±185.5 | 1 | 1 | active | 2026-08-01 |
| 96 | 1822.5 | TheHeir (`advanced_evolved`) — Mapuche — Lautaro | ±138.4 | 7 | 1 | active | 2026-08-01 |
| 97 | 1822.4 | Maverick2 (`g20-21`) — Georgia — Tamar | ±185.6 | 1 | 0 | active | 2026-08-01 |
| 98 | 1821.0 | JackKnife (`g44-41`) — Sumeria — Gilgamesh | ±185.5 | 1 | 1 | active | 2026-08-01 |
| 99 | 1820.8 | TheHeir (`advanced_evolved`) — Khmer — Jayavarman VII | ±175.4 | 2 | 1 | active | 2026-08-01 |
| 100 | 1820.7 | TheHeir (`advanced_evolved`) — Aztec — Montezuma | ±185.8 | 1 | 0 | active | 2026-08-01 |
| 101 | 1820.6 | OldGuard (`advanced_v1`) — Portugal — João III | ±184.9 | 1 | 0 | active | 2026-08-01 |
| 102 | 1820.1 | WildCard6 (`g48-43`) — Kongo — Mvemba a Nzinga | ±168.4 | 2 | 0 | active | 2026-08-01 |
| 103 | 1820.0 | WildCard2 (`g28-27`) — China — Qin Shi Huang | ±54.3 | 23 | 9 | retired | 2026-07-23 |
| 104 | 1819.2 | TheHeir (`advanced_evolved`) — Gaul — Ambiorix | ±159.3 | 3 | 0 | active | 2026-08-01 |
| 105 | 1817.9 | DarkHorse5 (`g40-38`) — Rome — Trajan | ±92.5 | 6 | 3 | retired | 2026-07-23 |
| 106 | 1817.9 | FreeSpirit6 (`g48-44`) — Germany — Frederick Barbarossa | ±168.8 | 2 | 0 | active | 2026-08-01 |
| 107 | 1817.3 | JackKnife (`g44-41`) — Rome — Trajan | ±56.3 | 21 | 9 | active | 2026-08-01 |
| 108 | 1816.8 | Opportunist3 (`g28-28`) — China — Qin Shi Huang | ±49.2 | 34 | 14 | active | 2026-08-01 |
| 109 | 1815.8 | FreeSpirit6 (`g48-44`) — Kongo — Mvemba a Nzinga | ±170.0 | 2 | 0 | active | 2026-08-01 |
| 110 | 1815.7 | FreeSpirit6 (`g48-44`) — Cree — Poundmaker | ±186.2 | 1 | 1 | active | 2026-08-01 |
| 111 | 1815.4 | Opportunist3 (`g28-28`) — Brazil — Pedro II | ±186.0 | 1 | 0 | active | 2026-08-01 |
| 112 | 1812.7 | OldGuard (`advanced_v1`) — Aztec — Montezuma | ±184.2 | 1 | 0 | active | 2026-08-01 |
| 113 | 1811.8 | WildCard6 (`g48-43`) — Maori — Kupe | ±185.2 | 1 | 0 | active | 2026-08-01 |
| 114 | 1811.3 | WildCard10 (`g56-50`) — Germany — Frederick Barbarossa | ±169.2 | 2 | 0 | active | 2026-08-01 |
| 115 | 1811.3 | WildCard6 (`g48-43`) — France — Catherine de Medici | ±155.2 | 3 | 1 | active | 2026-08-01 |
| 116 | 1811.1 | FreeSpirit6 (`g48-44`) — Byzantium — Basil II | ±185.7 | 1 | 0 | active | 2026-08-01 |
| 117 | 1810.8 | JackKnife (`g44-41`) — Portugal — João III | ±185.4 | 1 | 0 | active | 2026-08-01 |
| 118 | 1810.7 | WildCard10 (`g56-50`) — Kongo — Mvemba a Nzinga | ±186.6 | 1 | 0 | active | 2026-08-01 |
| 119 | 1808.5 | WildCard6 (`g48-43`) — America — Abraham Lincoln | ±186.2 | 1 | 1 | active | 2026-08-01 |
| 120 | 1808.1 | OldGuard (`advanced_v1`) — Byzantium — Basil II | ±183.6 | 1 | 0 | active | 2026-08-01 |
| 121 | 1806.9 | OldGuard (`advanced_v1`) — Brazil — Pedro II | ±156.4 | 3 | 0 | active | 2026-08-01 |
| 122 | 1806.7 | JackKnife (`g44-41`) — Inca — Pachacuti | ±168.1 | 2 | 1 | active | 2026-08-01 |
| 123 | 1806.1 | Opportunist3 (`g28-28`) — India — Gandhi | ±169.0 | 2 | 1 | active | 2026-08-01 |
| 124 | 1806.1 | TheHeir (`advanced_evolved`) — America — Abraham Lincoln | ±169.5 | 2 | 0 | active | 2026-08-01 |
| 125 | 1805.9 | Maverick2 (`g20-21`) — Korea — Seondeok | ±185.3 | 1 | 0 | active | 2026-08-01 |
| 126 | 1804.0 | DarkHorse (`g4-10`) — Rome — Trajan | ±43.8 | 43 | 21 | retired | 2026-07-23 |
| 127 | 1803.4 | WildCard6 (`g48-43`) — Phoenicia — Dido | ±138.8 | 5 | 0 | active | 2026-08-01 |
| 128 | 1803.1 | WildCard6 (`g48-43`) — Germany — Frederick Barbarossa | ±169.0 | 2 | 0 | active | 2026-08-01 |
| 129 | 1802.4 | JackKnife (`g44-41`) — Khmer — Jayavarman VII | ±185.6 | 1 | 0 | active | 2026-08-01 |
| 130 | 1802.1 | JackOfAllTrades (`advanced`) — Korea — Seondeok | ±185.6 | 1 | 1 | active | 2026-08-01 |
| 131 | 1802.0 | WildCard6 (`g48-43`) — Greece — Pericles | ±65.8 | 15 | 5 | active | 2026-08-01 |
| 132 | 1800.4 | Maverick2 (`g20-21`) — Australia — John Curtin | ±167.1 | 2 | 0 | active | 2026-08-01 |
| 133 | 1799.4 | Maverick2 (`g20-21`) — China — Qin Shi Huang | ±42.4 | 72 | 33 | active | 2026-08-01 |
| 134 | 1799.0 | JackKnife (`g44-41`) — Sweden — Kristina | ±185.2 | 1 | 0 | active | 2026-08-01 |
| 135 | 1798.6 | WildCard6 (`g48-43`) — Rome — Trajan | ±51.3 | 23 | 10 | active | 2026-08-01 |
| 136 | 1798.5 | WildCard6 (`g48-43`) — Maya — Lady Six Sky | ±186.0 | 1 | 0 | active | 2026-08-01 |
| 137 | 1798.0 | JackKnife (`g44-41`) — Persia — Cyrus | ±185.6 | 1 | 0 | active | 2026-08-01 |
| 138 | 1796.6 | JackKnife (`g44-41`) — Hungary — Matthias Corvinus | ±169.0 | 2 | 1 | active | 2026-08-01 |
| 139 | 1796.4 | FreeSpirit6 (`g48-44`) — Maori — Kupe | ±185.4 | 1 | 0 | active | 2026-08-01 |
| 140 | 1796.2 | Maverick2 (`g20-21`) — Mongolia — Genghis Khan | ±185.7 | 1 | 0 | active | 2026-08-01 |
| 141 | 1796.1 | WildCard10 (`g56-50`) — Babylon — Hammurabi | ±170.1 | 2 | 0 | active | 2026-08-01 |
| 142 | 1794.2 | WildCard6 (`g48-43`) — Scotland — Robert the Bruce | ±183.3 | 1 | 0 | active | 2026-08-01 |
| 143 | 1794.0 | JackKnife (`g44-41`) — Ethiopia — Menelik II | ±169.7 | 2 | 1 | active | 2026-08-01 |
| 144 | 1793.5 | Maverick6 (`g56-49`) — Egypt — Cleopatra | ±115.9 | 5 | 0 | retired | 2026-07-23 |
| 145 | 1793.1 | Opportunist3 (`g28-28`) — France — Catherine de Medici | ±156.7 | 3 | 0 | active | 2026-08-01 |
| 146 | 1792.9 | JackKnife (`g44-41`) — Arabia — Saladin | ±182.6 | 1 | 0 | active | 2026-08-01 |
| 147 | 1792.4 | WildCard10 (`g56-50`) — Canada — Wilfrid Laurier | ±185.8 | 1 | 0 | active | 2026-08-01 |
| 148 | 1792.0 | Maverick2 (`g20-21`) — Canada — Wilfrid Laurier | ±183.8 | 1 | 0 | active | 2026-08-01 |
| 149 | 1790.2 | WildCard10 (`g56-50`) — Gaul — Ambiorix | ±185.1 | 1 | 0 | active | 2026-08-01 |
| 150 | 1789.9 | OldGuard (`advanced_v1`) — Ethiopia — Menelik II | ±156.8 | 3 | 1 | active | 2026-08-01 |
| 151 | 1789.4 | WildCard10 (`g56-50`) — Mongolia — Genghis Khan | ±185.9 | 1 | 0 | active | 2026-08-01 |
| 152 | 1788.5 | TheHeir (`advanced_evolved`) — Japan — Hojo Tokimune | ±194.3 | 1 | 0 | active | 2026-08-01 |
| 153 | 1788.3 | TheHeir (`advanced_evolved`) — Germany — Frederick Barbarossa | ±187.0 | 1 | 0 | active | 2026-08-01 |
| 154 | 1788.2 | JackKnife (`g44-41`) — Japan — Hojo Tokimune | ±170.5 | 2 | 0 | active | 2026-08-01 |
| 155 | 1787.2 | Opportunist3 (`g28-28`) — Byzantium — Basil II | ±156.3 | 3 | 0 | active | 2026-08-01 |
| 156 | 1787.1 | Opportunist3 (`g28-28`) — Indonesia — Gitarja | ±186.4 | 1 | 0 | active | 2026-08-01 |
| 157 | 1786.1 | TheHeir (`advanced_evolved`) — Russia — Peter | ±175.2 | 2 | 1 | active | 2026-08-01 |
| 158 | 1785.9 | Maverick2 (`g20-21`) — Gran Colombia — Simón Bolívar | ±184.5 | 1 | 0 | active | 2026-08-01 |
| 159 | 1785.6 | WildCard10 (`g56-50`) — Scotland — Robert the Bruce | ±185.4 | 1 | 0 | active | 2026-08-01 |
| 160 | 1785.0 | Maverick2 (`g20-21`) — Byzantium — Basil II | ±167.9 | 2 | 0 | active | 2026-08-01 |
| 161 | 1785.0 | WildCard10 (`g56-50`) — Vietnam — Ba Trieu | ±169.2 | 2 | 0 | active | 2026-08-01 |
| 162 | 1783.7 | WildCard (`g24-24`) — China — Qin Shi Huang | ±95.8 | 5 | 2 | retired | 2026-07-23 |
| 163 | 1782.8 | TheHeir (`advanced_evolved`) — Maya — Lady Six Sky | ±156.6 | 3 | 1 | active | 2026-08-01 |
| 164 | 1781.7 | Maverick2 (`g20-21`) — Cree — Poundmaker | ±186.8 | 1 | 0 | active | 2026-08-01 |
| 165 | 1781.4 | WildCard6 (`g48-43`) — Aztec — Montezuma | ±155.9 | 3 | 0 | active | 2026-08-01 |
| 166 | 1781.3 | Opportunist6 (`g40-37`) — China — Qin Shi Huang | ±54.4 | 19 | 7 | retired | 2026-07-23 |
| 167 | 1780.6 | JackOfAllTrades (`advanced`) — Rome — Trajan | ±41.4 | 49 | 15 | active | 2026-08-01 |
| 168 | 1780.5 | WildCard6 (`g48-43`) — Australia — John Curtin | ±168.6 | 2 | 1 | active | 2026-08-01 |
| 169 | 1780.3 | HolyRoller (`g12-15`) — Rome — Trajan | ±94.0 | 5 | 3 | retired | 2026-07-23 |
| 170 | 1780.3 | JackOfAllTrades (`advanced`) — Russia — Peter | ±184.2 | 1 | 0 | active | 2026-08-01 |
| 171 | 1779.9 | FreeSpirit6 (`g48-44`) — Khmer — Jayavarman VII | ±168.6 | 2 | 1 | active | 2026-08-01 |
| 172 | 1779.7 | WildCard4 (`g40-36`) — Rome — Trajan | ±61.2 | 15 | 4 | retired | 2026-07-23 |
| 173 | 1779.0 | Opportunist3 (`g28-28`) — Rome — Trajan | ±41.8 | 54 | 19 | active | 2026-08-01 |
| 174 | 1778.7 | WildCard10 (`g56-50`) — Khmer — Jayavarman VII | ±185.8 | 1 | 0 | active | 2026-08-01 |
| 175 | 1778.0 | JackOfAllTrades (`advanced`) — Japan — Hojo Tokimune | ±185.9 | 1 | 0 | active | 2026-08-01 |
| 176 | 1777.5 | WildCard8 (`g52-46`) — Rome — Trajan | ±72.8 | 9 | 3 | retired | 2026-07-23 |
| 177 | 1777.4 | FreeSpirit5 (`g36-34`) — Greece — Pericles | ±69.6 | 10 | 3 | retired | 2026-07-23 |
| 178 | 1777.0 | JackKnife (`g44-41`) — Canada — Wilfrid Laurier | ±154.0 | 3 | 0 | active | 2026-08-01 |
| 179 | 1776.3 | Opportunist3 (`g28-28`) — Scotland — Robert the Bruce | ±146.4 | 4 | 0 | active | 2026-08-01 |
| 180 | 1776.0 | JackOfAllTrades (`advanced`) — Indonesia — Gitarja | ±155.5 | 3 | 1 | active | 2026-08-01 |
| 181 | 1775.0 | JackOfAllTrades (`advanced`) — Mongolia — Genghis Khan | ±185.7 | 1 | 0 | active | 2026-08-01 |
| 182 | 1774.6 | JackOfAllTrades (`advanced`) — Sumeria — Gilgamesh | ±146.8 | 4 | 1 | active | 2026-08-01 |
| 183 | 1773.4 | WildCard6 (`g48-43`) — Indonesia — Gitarja | ±185.2 | 1 | 0 | active | 2026-08-01 |
| 184 | 1772.8 | DarkHorse (`g4-10`) — China — Qin Shi Huang | ±42.5 | 47 | 22 | retired | 2026-07-23 |
| 185 | 1772.3 | JackKnife (`g44-41`) — Poland — Jadwiga | ±168.9 | 2 | 0 | active | 2026-08-01 |
| 186 | 1771.8 | OldGuard (`advanced_v1`) — Inca — Pachacuti | ±182.4 | 1 | 0 | active | 2026-08-01 |
| 187 | 1771.8 | FreeSpirit6 (`g48-44`) — Australia — John Curtin | ±169.7 | 2 | 0 | active | 2026-08-01 |
| 188 | 1771.5 | Maverick2 (`g20-21`) — Phoenicia — Dido | ±183.5 | 1 | 0 | active | 2026-08-01 |
| 189 | 1770.8 | JackOfAllTrades (`advanced`) — Hungary — Matthias Corvinus | ±185.3 | 1 | 0 | active | 2026-08-01 |
| 190 | 1770.6 | Maverick6 (`g56-49`) — Greece — Pericles | ±95.5 | 5 | 2 | retired | 2026-07-23 |
| 191 | 1770.4 | JackKnife (`g44-41`) — Phoenicia — Dido | ±130.8 | 6 | 1 | active | 2026-08-01 |
| 192 | 1769.9 | WildCard10 (`g56-50`) — Poland — Jadwiga | ±155.7 | 3 | 0 | active | 2026-08-01 |
| 193 | 1769.7 | JackKnife (`g44-41`) — Brazil — Pedro II | ±186.4 | 1 | 0 | active | 2026-08-01 |
| 194 | 1769.2 | Maverick5 (`g48-42`) — Greece — Pericles | ±61.8 | 15 | 2 | retired | 2026-07-23 |
| 195 | 1769.0 | TheHeir (`advanced_evolved`) — Georgia — Tamar | ±187.6 | 1 | 0 | active | 2026-08-01 |
| 196 | 1768.3 | TheHeir (`advanced_evolved`) — Portugal — João III | ±161.4 | 3 | 0 | active | 2026-08-01 |
| 197 | 1768.2 | Maverick2 (`g20-21`) — Russia — Peter | ±145.1 | 4 | 0 | active | 2026-08-01 |
| 198 | 1767.5 | WildCard7 (`g52-45`) — Egypt — Cleopatra | ±87.9 | 7 | 2 | retired | 2026-07-23 |
| 199 | 1766.9 | Opportunist3 (`g28-28`) — Persia — Cyrus | ±168.2 | 2 | 0 | active | 2026-08-01 |
| 200 | 1766.4 | TheHeir (`advanced_evolved`) — Sweden — Kristina | ±157.6 | 3 | 0 | active | 2026-08-01 |
| 201 | 1766.3 | WildCard10 (`g56-50`) — Greece — Pericles | ±70.3 | 10 | 1 | active | 2026-08-01 |
| 202 | 1766.2 | Maverick3 (`g24-25`) — Rome — Trajan | ±67.9 | 10 | 5 | retired | 2026-07-23 |
| 203 | 1766.0 | OldGuard (`advanced_v1`) — Netherlands — Wilhelmina | ±156.0 | 3 | 0 | active | 2026-08-01 |
| 204 | 1765.1 | WildCard2 (`g28-27`) — Greece — Pericles | ±47.0 | 30 | 7 | retired | 2026-07-23 |
| 205 | 1764.6 | FreeSpirit2 (`g16-19`) — Rome — Trajan | ±45.5 | 30 | 10 | retired | 2026-07-23 |
| 206 | 1764.5 | FreeSpirit6 (`g48-44`) — Gaul — Ambiorix | ±153.5 | 3 | 0 | active | 2026-08-01 |
| 207 | 1764.5 | JackOfAllTrades (`advanced`) — Babylon — Hammurabi | ±169.0 | 2 | 0 | active | 2026-08-01 |
| 208 | 1764.2 | Maverick2 (`g20-21`) — Scotland — Robert the Bruce | ±157.0 | 3 | 0 | active | 2026-08-01 |
| 209 | 1763.9 | TheHeir (`advanced_evolved`) — Nubia — Amanitore | ±170.8 | 2 | 0 | active | 2026-08-01 |
| 210 | 1763.8 | Opportunist3 (`g28-28`) — Babylon — Hammurabi | ±168.5 | 2 | 0 | active | 2026-08-01 |
| 211 | 1763.4 | OldGuard (`advanced_v1`) — Rome — Trajan | ±41.8 | 62 | 20 | active | 2026-08-01 |
| 212 | 1763.4 | JackKnife (`g44-41`) — Russia — Peter | ±185.2 | 1 | 0 | active | 2026-08-01 |
| 213 | 1762.8 | FreeSpirit6 (`g48-44`) — Rome — Trajan | ±66.6 | 14 | 3 | active | 2026-08-01 |
| 214 | 1762.5 | FreeSpirit6 (`g48-44`) — Mali — Mansa Musa | ±186.1 | 1 | 0 | active | 2026-08-01 |
| 215 | 1761.8 | WildCard6 (`g48-43`) — Cree — Poundmaker | ±184.7 | 1 | 0 | active | 2026-08-01 |
| 216 | 1761.5 | Opportunist3 (`g28-28`) — Phoenicia — Dido | ±182.3 | 1 | 0 | active | 2026-08-01 |
| 217 | 1760.3 | WildCard6 (`g48-43`) — Sumeria — Gilgamesh | ±167.0 | 2 | 0 | active | 2026-08-01 |
| 218 | 1760.2 | WildCard5 (`g44-40`) — Greece — Pericles | ±104.5 | 5 | 1 | retired | 2026-07-23 |
| 219 | 1759.8 | Maverick5 (`g48-42`) — China — Qin Shi Huang | ±74.1 | 9 | 4 | retired | 2026-07-23 |
| 220 | 1758.4 | JackKnife (`g44-41`) — Mapuche — Lautaro | ±185.0 | 1 | 0 | active | 2026-08-01 |
| 221 | 1756.6 | FreeSpirit6 (`g48-44`) — Babylon — Hammurabi | ±169.1 | 2 | 1 | active | 2026-08-01 |
| 222 | 1755.9 | JackOfAllTrades (`advanced`) — India — Gandhi | ±156.6 | 3 | 0 | active | 2026-08-01 |
| 223 | 1755.7 | Maverick2 (`g20-21`) — Indonesia — Gitarja | ±182.5 | 1 | 0 | active | 2026-08-01 |
| 224 | 1755.2 | TheHeir (`advanced_evolved`) — Spain — Philip II | ±186.3 | 1 | 0 | active | 2026-08-01 |
| 225 | 1754.9 | OldGuard (`advanced_v1`) — Zulu — Shaka | ±167.8 | 2 | 1 | active | 2026-08-01 |
| 226 | 1753.7 | FreeSpirit6 (`g48-44`) — France — Catherine de Medici | ±168.4 | 2 | 0 | active | 2026-08-01 |
| 227 | 1753.0 | OldGuard (`advanced_v1`) — Indonesia — Gitarja | ±186.2 | 1 | 0 | active | 2026-08-01 |
| 228 | 1752.7 | OldGuard (`advanced_v1`) — Scotland — Robert the Bruce | ±137.0 | 5 | 0 | active | 2026-08-01 |
| 229 | 1752.2 | WildCard6 (`g48-43`) — Nubia — Amanitore | ±168.8 | 2 | 0 | active | 2026-08-01 |
| 230 | 1752.1 | JackKnife (`g44-41`) — Netherlands — Wilhelmina | ±169.6 | 2 | 0 | active | 2026-08-01 |
| 231 | 1751.8 | WildCard10 (`g56-50`) — China — Qin Shi Huang | ±120.0 | 3 | 0 | active | 2026-07-23 |
| 232 | 1751.3 | WildCard6 (`g48-43`) — Georgia — Tamar | ±186.6 | 1 | 0 | active | 2026-08-01 |
| 233 | 1750.4 | Maverick6 (`g56-49`) — Rome — Trajan | ±89.8 | 6 | 1 | retired | 2026-07-23 |
| 234 | 1749.7 | FreeSpirit2 (`g16-19`) — Greece — Pericles | ±55.7 | 16 | 4 | retired | 2026-07-23 |
| 235 | 1749.1 | FreeSpirit (`g16-18`) — Rome — Trajan | ±68.3 | 11 | 5 | retired | 2026-07-23 |
| 236 | 1748.7 | Maverick2 (`g20-21`) — Arabia — Saladin | ±186.4 | 1 | 0 | active | 2026-08-01 |
| 237 | 1747.9 | JackOfAllTrades (`advanced`) — Persia — Cyrus | ±186.0 | 1 | 0 | active | 2026-08-01 |
| 238 | 1747.9 | WildCard6 (`g48-43`) — Ottomans — Suleiman | ±183.0 | 1 | 0 | active | 2026-08-01 |
| 239 | 1747.4 | OldGuard (`advanced_v1`) — America — Abraham Lincoln | ±184.5 | 1 | 0 | active | 2026-08-01 |
| 240 | 1746.9 | OldGuard (`advanced_v1`) — Scythia — Tomyris | ±185.7 | 1 | 0 | active | 2026-08-01 |
| 241 | 1746.3 | Maverick4 (`g36-35`) — Rome — Trajan | ±146.3 | 2 | 0 | retired | 2026-07-23 |
| 242 | 1745.7 | Maverick2 (`g20-21`) — Gaul — Ambiorix | ±184.4 | 1 | 0 | active | 2026-08-01 |
| 243 | 1745.4 | TheHeir (`advanced_evolved`) — Rome — Trajan | ±193.7 | 1 | 0 | active | 2026-08-01 |
| 244 | 1745.4 | FreeSpirit2 (`g16-19`) — China — Qin Shi Huang | ±45.9 | 30 | 10 | retired | 2026-07-23 |
| 245 | 1745.1 | FreeSpirit6 (`g48-44`) — Mongolia — Genghis Khan | ±169.4 | 2 | 1 | active | 2026-08-01 |
| 246 | 1744.8 | Opportunist3 (`g28-28`) — Sweden — Kristina | ±156.0 | 3 | 0 | active | 2026-08-01 |
| 247 | 1744.2 | WildCard8 (`g52-46`) — Greece — Pericles | ±66.6 | 12 | 2 | retired | 2026-07-23 |
| 248 | 1744.2 | JackKnife (`g44-41`) — Maori — Kupe | ±170.2 | 2 | 0 | active | 2026-08-01 |
| 249 | 1744.0 | WildCard5 (`g44-40`) — Rome — Trajan | ±90.6 | 6 | 3 | retired | 2026-07-23 |
| 250 | 1743.3 | WildCard10 (`g56-50`) — Nubia — Amanitore | ±158.3 | 3 | 1 | active | 2026-08-01 |
| 251 | 1741.6 | WildCard10 (`g56-50`) — Zulu — Shaka | ±168.7 | 2 | 0 | active | 2026-08-01 |
| 252 | 1740.3 | FreeSpirit (`g16-18`) — China — Qin Shi Huang | ±65.2 | 12 | 4 | retired | 2026-07-23 |
| 253 | 1740.1 | WildCard3 (`g32-31`) — Greece — Pericles | ±98.5 | 5 | 0 | retired | 2026-07-23 |
| 254 | 1740.0 | JackOfAllTrades (`advanced`) — Maori — Kupe | ±184.9 | 1 | 0 | active | 2026-08-01 |
| 255 | 1739.8 | Opportunist3 (`g28-28`) — Australia — John Curtin | ±156.2 | 3 | 0 | active | 2026-08-01 |
| 256 | 1739.7 | JackKnife (`g44-41`) — Korea — Seondeok | ±167.9 | 2 | 0 | active | 2026-08-01 |
| 257 | 1739.4 | Opportunist3 (`g28-28`) — Greece — Pericles | ±43.1 | 43 | 12 | active | 2026-07-23 |
| 258 | 1739.4 | FreeSpirit6 (`g48-44`) — Ethiopia — Menelik II | ±155.8 | 3 | 0 | active | 2026-08-01 |
| 259 | 1739.0 | WildCard6 (`g48-43`) — Arabia — Saladin | ±154.6 | 3 | 0 | active | 2026-08-01 |
| 260 | 1738.8 | JackOfAllTrades (`advanced`) — Netherlands — Wilhelmina | ±137.0 | 5 | 0 | active | 2026-08-01 |
| 261 | 1738.6 | Opportunist3 (`g28-28`) — Poland — Jadwiga | ±154.5 | 3 | 0 | active | 2026-08-01 |
| 262 | 1738.5 | Opportunist3 (`g28-28`) — Ottomans — Suleiman | ±168.5 | 2 | 0 | active | 2026-08-01 |
| 263 | 1738.4 | JackOfAllTrades (`advanced`) — Sweden — Kristina | ±169.3 | 2 | 1 | active | 2026-08-01 |
| 264 | 1737.7 | WildCard6 (`g48-43`) — Gaul — Ambiorix | ±156.0 | 3 | 0 | active | 2026-08-01 |
| 265 | 1737.6 | WildCard4 (`g40-36`) — China — Qin Shi Huang | ±51.9 | 21 | 2 | retired | 2026-07-23 |
| 266 | 1737.4 | FreeSpirit6 (`g48-44`) — Vietnam — Ba Trieu | ±186.5 | 1 | 0 | active | 2026-08-01 |
| 267 | 1737.0 | Opportunist3 (`g28-28`) — Maori — Kupe | ±186.2 | 1 | 0 | active | 2026-08-01 |
| 268 | 1737.0 | FreeSpirit6 (`g48-44`) — Brazil — Pedro II | ±167.9 | 2 | 0 | active | 2026-08-01 |
| 269 | 1735.5 | FreeSpirit6 (`g48-44`) — Portugal — João III | ±185.6 | 1 | 0 | active | 2026-08-01 |
| 270 | 1735.2 | JackOfAllTrades (`advanced`) — Portugal — João III | ±185.8 | 1 | 0 | active | 2026-08-01 |
| 271 | 1734.5 | Maverick2 (`g20-21`) — Babylon — Hammurabi | ±186.2 | 1 | 0 | active | 2026-08-01 |
| 272 | 1734.5 | OldGuard (`advanced_v1`) — Khmer — Jayavarman VII | ±170.3 | 2 | 0 | active | 2026-08-01 |
| 273 | 1734.0 | FreeSpirit6 (`g48-44`) — Aztec — Montezuma | ±168.4 | 2 | 0 | active | 2026-08-01 |
| 274 | 1733.7 | WildCard10 (`g56-50`) — Mali — Mansa Musa | ±168.6 | 2 | 0 | active | 2026-08-01 |
| 275 | 1733.5 | Opportunist3 (`g28-28`) — Egypt — Cleopatra | ±44.0 | 44 | 16 | active | 2026-08-01 |
| 276 | 1733.4 | FreeSpirit6 (`g48-44`) — Netherlands — Wilhelmina | ±138.4 | 5 | 0 | active | 2026-08-01 |
| 277 | 1732.4 | WildCard10 (`g56-50`) — Spain — Philip II | ±186.7 | 1 | 0 | active | 2026-08-01 |
| 278 | 1732.0 | JackKnife (`g44-41`) — Macedon — Alexander | ±169.7 | 2 | 0 | active | 2026-08-01 |
| 279 | 1731.8 | FreeSpirit6 (`g48-44`) — America — Abraham Lincoln | ±156.0 | 3 | 0 | active | 2026-08-01 |
| 280 | 1731.7 | WildCard6 (`g48-43`) — Zulu — Shaka | ±121.1 | 8 | 1 | active | 2026-08-01 |
| 281 | 1731.3 | Opportunist3 (`g28-28`) — Russia — Peter | ±168.0 | 2 | 0 | active | 2026-08-01 |
| 282 | 1730.7 | OldGuard (`advanced_v1`) — Babylon — Hammurabi | ±169.3 | 2 | 0 | active | 2026-08-01 |
| 283 | 1730.4 | JackKnife (`g44-41`) — Indonesia — Gitarja | ±168.5 | 2 | 0 | active | 2026-08-01 |
| 284 | 1730.1 | WildCard2 (`g28-27`) — Rome — Trajan | ±43.6 | 36 | 12 | retired | 2026-07-23 |
| 285 | 1730.1 | JackKnife (`g44-41`) — Spain — Philip II | ±166.9 | 2 | 0 | active | 2026-08-01 |
| 286 | 1729.6 | TheHeir (`advanced_evolved`) — Maori — Kupe | ±187.6 | 1 | 0 | active | 2026-08-01 |
| 287 | 1728.9 | WildCard10 (`g56-50`) — Gran Colombia — Simón Bolívar | ±185.9 | 1 | 0 | active | 2026-08-01 |
| 288 | 1728.7 | Opportunist3 (`g28-28`) — Khmer — Jayavarman VII | ±185.7 | 1 | 0 | active | 2026-08-01 |
| 289 | 1727.0 | DarkHorse3 (`g20-22`) — China — Qin Shi Huang | ±102.5 | 5 | 3 | retired | 2026-07-23 |
| 290 | 1726.8 | WildCard4 (`g40-36`) — Greece — Pericles | ±60.9 | 14 | 4 | retired | 2026-07-23 |
| 291 | 1726.6 | OldGuard (`advanced_v1`) — Canada — Wilfrid Laurier | ±168.1 | 2 | 0 | active | 2026-08-01 |
| 292 | 1725.8 | TheHeir (`advanced_evolved`) — Norway — Harald Hardrada | ±186.8 | 1 | 0 | active | 2026-08-01 |
| 293 | 1725.6 | Maverick2 (`g20-21`) — Mali — Mansa Musa | ±186.4 | 1 | 0 | active | 2026-08-01 |
| 294 | 1724.3 | OldGuard (`advanced_v1`) — Spain — Philip II | ±185.8 | 1 | 0 | active | 2026-08-01 |
| 295 | 1723.6 | JackKnife (`g44-41`) — Aztec — Montezuma | ±145.3 | 4 | 0 | active | 2026-08-01 |
| 296 | 1723.2 | JackKnife (`g44-41`) — Georgia — Tamar | ±185.8 | 1 | 0 | active | 2026-08-01 |
| 297 | 1723.0 | FreeSpirit6 (`g48-44`) — Norway — Harald Hardrada | ±167.7 | 2 | 0 | active | 2026-08-01 |
| 298 | 1722.7 | JackOfAllTrades (`advanced`) — America — Abraham Lincoln | ±184.3 | 1 | 0 | active | 2026-08-01 |
| 299 | 1722.6 | WildCard6 (`g48-43`) — Mongolia — Genghis Khan | ±167.3 | 2 | 0 | active | 2026-08-01 |
| 300 | 1722.5 | FreeSpirit5 (`g36-34`) — Egypt — Cleopatra | ±75.4 | 8 | 2 | retired | 2026-07-23 |
| 301 | 1722.0 | Opportunist3 (`g28-28`) — Norway — Harald Hardrada | ±156.9 | 3 | 0 | active | 2026-08-01 |
| 302 | 1721.7 | OldGuard (`advanced_v1`) — Sweden — Kristina | ±182.8 | 1 | 0 | active | 2026-08-01 |
| 303 | 1720.6 | FreeSpirit4 (`g36-33`) — China — Qin Shi Huang | ±78.4 | 9 | 3 | retired | 2026-07-23 |
| 304 | 1719.8 | FreeSpirit6 (`g48-44`) — Egypt — Cleopatra | ±54.9 | 19 | 4 | active | 2026-08-01 |
| 305 | 1719.4 | TheHeir (`advanced_evolved`) — Australia — John Curtin | ±188.1 | 1 | 0 | active | 2026-08-01 |
| 306 | 1719.3 | DarkHorse2 (`g16-20`) — China — Qin Shi Huang | ±47.8 | 25 | 7 | retired | 2026-07-23 |
| 307 | 1718.1 | JackOfAllTrades (`advanced`) — Greece — Pericles | ±43.2 | 61 | 14 | active | 2026-08-01 |
| 308 | 1717.3 | OldGuard (`advanced_v1`) — France — Catherine de Medici | ±181.5 | 1 | 0 | active | 2026-08-01 |
| 309 | 1717.2 | OldGuard (`advanced_v1`) — Greece — Pericles | ±42.3 | 50 | 9 | active | 2026-08-01 |
| 310 | 1716.6 | Opportunist3 (`g28-28`) — Ethiopia — Menelik II | ±137.8 | 5 | 0 | active | 2026-08-01 |
| 311 | 1716.2 | WildCard10 (`g56-50`) — Macedon — Alexander | ±168.4 | 2 | 0 | active | 2026-08-01 |
| 312 | 1715.4 | FreeSpirit6 (`g48-44`) — Ottomans — Suleiman | ±183.2 | 1 | 0 | active | 2026-08-01 |
| 313 | 1715.2 | WildCard6 (`g48-43`) — Russia — Peter | ±184.3 | 1 | 0 | active | 2026-08-01 |
| 314 | 1715.1 | Opportunist3 (`g28-28`) — Gran Colombia — Simón Bolívar | ±184.7 | 1 | 0 | active | 2026-08-01 |
| 315 | 1714.1 | WildCard10 (`g56-50`) — Scythia — Tomyris | ±168.7 | 2 | 0 | active | 2026-08-01 |
| 316 | 1712.2 | Maverick2 (`g20-21`) — Mapuche — Lautaro | ±156.2 | 3 | 0 | active | 2026-08-01 |
| 317 | 1709.5 | JackKnife (`g44-41`) — Babylon — Hammurabi | ±168.9 | 2 | 0 | active | 2026-08-01 |
| 318 | 1707.7 | JackOfAllTrades (`advanced`) — Cree — Poundmaker | ±185.3 | 1 | 0 | active | 2026-08-01 |
| 319 | 1707.0 | TheHeir (`advanced_evolved`) — Scythia — Tomyris | ±187.7 | 1 | 0 | active | 2026-08-01 |
| 320 | 1706.2 | DarkHorse3 (`g20-22`) — Greece — Pericles | ±108.8 | 4 | 1 | retired | 2026-07-23 |
| 321 | 1706.2 | WildCard6 (`g48-43`) — Spain — Philip II | ±155.0 | 3 | 0 | active | 2026-08-01 |
| 322 | 1706.1 | JackKnife (`g44-41`) — Gran Colombia — Simón Bolívar | ±138.8 | 5 | 0 | active | 2026-08-01 |
| 323 | 1704.6 | FreeSpirit6 (`g48-44`) — Macedon — Alexander | ±147.6 | 4 | 1 | active | 2026-08-01 |
| 324 | 1704.3 | WildCard6 (`g48-43`) — Brazil — Pedro II | ±158.2 | 3 | 0 | active | 2026-08-01 |
| 325 | 1704.3 | DarkHorse2 (`g16-20`) — Rome — Trajan | ±47.7 | 26 | 5 | retired | 2026-07-23 |
| 326 | 1704.1 | JackOfAllTrades (`advanced`) — Arabia — Saladin | ±147.9 | 4 | 0 | active | 2026-08-01 |
| 327 | 1703.7 | JackKnife (`g44-41`) — Scythia — Tomyris | ±170.3 | 2 | 0 | active | 2026-08-01 |
| 328 | 1703.7 | FreeSpirit5 (`g36-34`) — Rome — Trajan | ±60.4 | 14 | 3 | retired | 2026-07-23 |
| 329 | 1703.6 | Maverick2 (`g20-21`) — Egypt — Cleopatra | ±44.3 | 53 | 16 | active | 2026-08-01 |
| 330 | 1703.1 | DarkHorse (`g4-10`) — Greece — Pericles | ±47.0 | 31 | 5 | retired | 2026-07-23 |
| 331 | 1702.3 | TheHeir (`advanced_evolved`) — Babylon — Hammurabi | ±179.1 | 3 | 0 | active | 2026-08-01 |
| 332 | 1701.6 | WildCard6 (`g48-43`) — Portugal — João III | ±168.5 | 2 | 0 | active | 2026-08-01 |
| 333 | 1701.4 | WildCard6 (`g48-43`) — Mali — Mansa Musa | ±185.8 | 1 | 0 | active | 2026-08-01 |
| 334 | 1701.1 | Maverick2 (`g20-21`) — Sweden — Kristina | ±185.9 | 1 | 0 | active | 2026-08-01 |
| 335 | 1701.0 | JackKnife (`g44-41`) — Nubia — Amanitore | ±168.0 | 2 | 0 | active | 2026-08-01 |
| 336 | 1700.6 | WildCard6 (`g48-43`) — Scythia — Tomyris | ±186.4 | 1 | 0 | active | 2026-08-01 |
| 337 | 1700.1 | OldGuard (`advanced_v1`) — Cree — Poundmaker | ±181.3 | 1 | 0 | active | 2026-08-01 |
| 338 | 1699.5 | FreeSpirit3 (`g32-30`) — China — Qin Shi Huang | ±96.5 | 6 | 2 | retired | 2026-07-23 |
| 339 | 1699.5 | JackKnife (`g44-41`) — Cree — Poundmaker | ±184.5 | 1 | 0 | active | 2026-08-01 |
| 340 | 1699.0 | OldGuard (`advanced_v1`) — Poland — Jadwiga | ±168.2 | 2 | 0 | active | 2026-08-01 |
| 341 | 1699.0 | Opportunist3 (`g28-28`) — Georgia — Tamar | ±169.7 | 2 | 0 | active | 2026-08-01 |
| 342 | 1698.6 | Maverick2 (`g20-21`) — Greece — Pericles | ±44.8 | 50 | 11 | active | 2026-08-01 |
| 343 | 1698.5 | WildCard10 (`g56-50`) — Brazil — Pedro II | ±186.2 | 1 | 0 | active | 2026-08-01 |
| 344 | 1698.2 | JackOfAllTrades (`advanced`) — Zulu — Shaka | ±168.4 | 2 | 0 | active | 2026-08-01 |
| 345 | 1696.1 | JackOfAllTrades (`advanced`) — Phoenicia — Dido | ±168.5 | 2 | 0 | active | 2026-08-01 |
| 346 | 1696.1 | Maverick3 (`g24-25`) — China — Qin Shi Huang | ±82.0 | 7 | 2 | retired | 2026-07-23 |
| 347 | 1696.1 | Maverick2 (`g20-21`) — Sumeria — Gilgamesh | ±185.3 | 1 | 0 | active | 2026-08-01 |
| 348 | 1694.8 | JackKnife (`g44-41`) — Scotland — Robert the Bruce | ±130.4 | 6 | 0 | active | 2026-08-01 |
| 349 | 1694.5 | FreeSpirit6 (`g48-44`) — Persia — Cyrus | ±185.6 | 1 | 0 | active | 2026-08-01 |
| 350 | 1692.7 | JackOfAllTrades (`advanced`) — Nubia — Amanitore | ±156.5 | 3 | 0 | active | 2026-08-01 |
| 351 | 1691.7 | Maverick2 (`g20-21`) — Maori — Kupe | ±186.8 | 1 | 0 | active | 2026-08-01 |
| 352 | 1691.7 | TheHeir (`advanced_evolved`) — Mongolia — Genghis Khan | ±201.0 | 1 | 0 | active | 2026-08-01 |
| 353 | 1691.2 | OldGuard (`advanced_v1`) — Japan — Hojo Tokimune | ±181.8 | 1 | 0 | active | 2026-08-01 |
| 354 | 1690.9 | Maverick2 (`g20-21`) — Brazil — Pedro II | ±186.9 | 1 | 0 | active | 2026-08-01 |
| 355 | 1690.9 | Opportunist3 (`g28-28`) — Germany — Frederick Barbarossa | ±184.7 | 1 | 0 | active | 2026-08-01 |
| 356 | 1690.9 | Opportunist5 (`g32-32`) — Greece — Pericles | ±81.4 | 7 | 2 | retired | 2026-07-23 |
| 357 | 1690.7 | Maverick5 (`g48-42`) — Rome — Trajan | ±65.1 | 12 | 4 | retired | 2026-07-23 |
| 358 | 1689.9 | TheHeir (`advanced_evolved`) — Egypt — Cleopatra | ±160.5 | 5 | 0 | active | 2026-08-01 |
| 359 | 1689.1 | Maverick2 (`g20-21`) — Germany — Frederick Barbarossa | ±155.2 | 3 | 0 | active | 2026-08-01 |
| 360 | 1689.0 | Opportunist3 (`g28-28`) — Spain — Philip II | ±183.2 | 1 | 0 | active | 2026-08-01 |
| 361 | 1687.7 | WildCard6 (`g48-43`) — Poland — Jadwiga | ±186.1 | 1 | 0 | active | 2026-08-01 |
| 362 | 1687.4 | Maverick2 (`g20-21`) — Japan — Hojo Tokimune | ±168.2 | 2 | 0 | active | 2026-08-01 |
| 363 | 1687.2 | JackOfAllTrades (`advanced`) — China — Qin Shi Huang | ±41.7 | 64 | 17 | active | 2026-08-01 |
| 364 | 1686.3 | HolyRoller (`g12-15`) — Greece — Pericles | ±114.6 | 4 | 2 | retired | 2026-07-23 |
| 365 | 1686.1 | FreeSpirit6 (`g48-44`) — Scythia — Tomyris | ±185.0 | 1 | 0 | active | 2026-08-01 |
| 366 | 1684.0 | OldGuard (`advanced_v1`) — Ottomans — Suleiman | ±170.6 | 2 | 0 | active | 2026-08-01 |
| 367 | 1683.7 | WildCard10 (`g56-50`) — Arabia — Saladin | ±185.5 | 1 | 0 | active | 2026-08-01 |
| 368 | 1682.9 | JackOfAllTrades (`advanced`) — Mali — Mansa Musa | ±168.2 | 2 | 0 | active | 2026-08-01 |
| 369 | 1682.1 | OldGuard (`advanced_v1`) — Egypt — Cleopatra | ±42.8 | 55 | 15 | active | 2026-08-01 |
| 370 | 1681.9 | Maverick2 (`g20-21`) — Netherlands — Wilhelmina | ±155.8 | 3 | 0 | active | 2026-08-01 |
| 371 | 1681.7 | JackOfAllTrades (`advanced`) — Kongo — Mvemba a Nzinga | ±185.9 | 1 | 0 | active | 2026-08-01 |
| 372 | 1681.4 | TrainingWheels (`basic`) — Rome — Trajan | ±44.5 | 41 | 7 | active | 2026-07-23 |
| 373 | 1680.8 | FreeSpirit6 (`g48-44`) — Spain — Philip II | ±184.1 | 1 | 0 | active | 2026-08-01 |
| 374 | 1680.6 | JackOfAllTrades (`advanced`) — Khmer — Jayavarman VII | ±183.4 | 1 | 0 | active | 2026-08-01 |
| 375 | 1679.7 | FreeSpirit6 (`g48-44`) — Zulu — Shaka | ±170.2 | 2 | 0 | active | 2026-08-01 |
| 376 | 1677.8 | JackKnife (`g44-41`) — Egypt — Cleopatra | ±54.2 | 25 | 6 | active | 2026-08-01 |
| 377 | 1677.6 | FreeSpirit6 (`g48-44`) — Sumeria — Gilgamesh | ±167.8 | 2 | 0 | active | 2026-08-01 |
| 378 | 1675.4 | Opportunist3 (`g28-28`) — Japan — Hojo Tokimune | ±185.7 | 1 | 0 | active | 2026-08-01 |
| 379 | 1674.6 | WildCard10 (`g56-50`) — India — Gandhi | ±186.5 | 1 | 0 | active | 2026-08-01 |
| 380 | 1674.0 | WildCard10 (`g56-50`) — Phoenicia — Dido | ±170.3 | 2 | 0 | active | 2026-08-01 |
| 381 | 1673.8 | WildCard5 (`g44-40`) — China — Qin Shi Huang | ±85.0 | 6 | 1 | retired | 2026-07-23 |
| 382 | 1673.7 | FreeSpirit3 (`g32-30`) — Rome — Trajan | ±77.0 | 8 | 2 | retired | 2026-07-23 |
| 383 | 1673.4 | Opportunist2 (`g20-23`) — China — Qin Shi Huang | ±98.8 | 5 | 1 | retired | 2026-07-23 |
| 384 | 1672.1 | WildCard6 (`g48-43`) — Norway — Harald Hardrada | ±185.6 | 1 | 0 | active | 2026-08-01 |
| 385 | 1671.4 | OldGuard (`advanced_v1`) — Hungary — Matthias Corvinus | ±184.5 | 1 | 0 | active | 2026-08-01 |
| 386 | 1671.3 | Maverick2 (`g20-21`) — Nubia — Amanitore | ±146.7 | 4 | 0 | active | 2026-08-01 |
| 387 | 1670.6 | Maverick4 (`g36-35`) — China — Qin Shi Huang | ±109.3 | 4 | 1 | retired | 2026-07-23 |
| 388 | 1670.0 | OldGuard (`advanced_v1`) — Gran Colombia — Simón Bolívar | ±145.9 | 4 | 0 | active | 2026-08-01 |
| 389 | 1669.3 | JackKnife (`g44-41`) — America — Abraham Lincoln | ±168.2 | 2 | 0 | active | 2026-08-01 |
| 390 | 1669.3 | WildCard10 (`g56-50`) — Russia — Peter | ±186.4 | 1 | 0 | active | 2026-08-01 |
| 391 | 1669.2 | WildCard10 (`g56-50`) — Indonesia — Gitarja | ±185.4 | 1 | 0 | active | 2026-08-01 |
| 392 | 1668.7 | WildCard10 (`g56-50`) — Aztec — Montezuma | ±170.4 | 2 | 0 | active | 2026-08-01 |
| 393 | 1668.5 | WildCard6 (`g48-43`) — Vietnam — Ba Trieu | ±158.4 | 3 | 0 | active | 2026-08-01 |
| 394 | 1668.4 | OldGuard (`advanced_v1`) — Mongolia — Genghis Khan | ±145.2 | 4 | 0 | active | 2026-08-01 |
| 395 | 1667.9 | Opportunist3 (`g28-28`) — Macedon — Alexander | ±186.0 | 1 | 0 | active | 2026-08-01 |
| 396 | 1665.4 | WildCard4 (`g40-36`) — Egypt — Cleopatra | ±64.8 | 13 | 3 | retired | 2026-07-23 |
| 397 | 1663.8 | JackOfAllTrades (`advanced`) — Scotland — Robert the Bruce | ±169.2 | 2 | 0 | active | 2026-08-01 |
| 398 | 1662.6 | FreeSpirit6 (`g48-44`) — Inca — Pachacuti | ±155.6 | 3 | 0 | active | 2026-08-01 |
| 399 | 1661.9 | WildCard6 (`g48-43`) — Inca — Pachacuti | ±185.8 | 1 | 0 | active | 2026-08-01 |
| 400 | 1661.9 | WildCard6 (`g48-43`) — Japan — Hojo Tokimune | ±186.4 | 1 | 0 | active | 2026-08-01 |
| 401 | 1661.4 | WildCard9 (`g56-48`) — Egypt — Cleopatra | ±65.6 | 11 | 2 | retired | 2026-07-23 |
| 402 | 1660.3 | FreeSpirit6 (`g48-44`) — Maya — Lady Six Sky | ±169.4 | 2 | 0 | active | 2026-08-01 |
| 403 | 1659.8 | OldGuard (`advanced_v1`) — Persia — Cyrus | ±186.2 | 1 | 0 | active | 2026-08-01 |
| 404 | 1659.1 | FreeSpirit4 (`g36-33`) — Greece — Pericles | ±69.5 | 13 | 0 | retired | 2026-07-23 |
| 405 | 1658.7 | FreeSpirit6 (`g48-44`) — China — Qin Shi Huang | ±55.9 | 16 | 3 | active | 2026-07-23 |
| 406 | 1658.6 | FreeSpirit6 (`g48-44`) — Poland — Jadwiga | ±169.3 | 2 | 0 | active | 2026-08-01 |
| 407 | 1657.7 | Maverick4 (`g36-35`) — Greece — Pericles | ±67.1 | 11 | 2 | retired | 2026-07-23 |
| 408 | 1657.0 | JackKnife (`g44-41`) — Greece — Pericles | ±56.6 | 19 | 2 | active | 2026-08-01 |
| 409 | 1654.5 | Opportunist3 (`g28-28`) — Portugal — João III | ±185.8 | 1 | 0 | active | 2026-08-01 |
| 410 | 1654.1 | DarkHorse2 (`g16-20`) — Greece — Pericles | ±47.2 | 25 | 4 | retired | 2026-07-23 |
| 411 | 1652.8 | WildCard3 (`g32-31`) — China — Qin Shi Huang | ±121.9 | 3 | 0 | retired | 2026-07-23 |
| 412 | 1652.7 | Opportunist4 (`g28-29`) — Rome — Trajan | ±73.6 | 9 | 3 | retired | 2026-07-23 |
| 413 | 1652.3 | DarkHorse (`g4-10`) — Egypt — Cleopatra | ±46.0 | 31 | 7 | retired | 2026-07-23 |
| 414 | 1650.5 | Opportunist3 (`g28-28`) — Arabia — Saladin | ±169.3 | 2 | 0 | active | 2026-08-01 |
| 415 | 1649.7 | DarkHorse4 (`g24-26`) — Egypt — Cleopatra | ±93.4 | 5 | 1 | retired | 2026-07-23 |
| 416 | 1648.8 | JackOfAllTrades (`advanced`) — Mapuche — Lautaro | ±169.0 | 2 | 0 | active | 2026-08-01 |
| 417 | 1647.6 | WildCard6 (`g48-43`) — Gran Colombia — Simón Bolívar | ±170.1 | 2 | 0 | active | 2026-08-01 |
| 418 | 1647.3 | FreeSpirit6 (`g48-44`) — Mapuche — Lautaro | ±155.7 | 3 | 0 | active | 2026-08-01 |
| 419 | 1645.2 | JackOfAllTrades (`advanced`) — Ethiopia — Menelik II | ±185.3 | 1 | 0 | active | 2026-08-01 |
| 420 | 1645.1 | WildCard7 (`g52-45`) — China — Qin Shi Huang | ±130.8 | 3 | 1 | retired | 2026-07-23 |
| 421 | 1642.8 | ScoreKeeper (`g8-14`) — Rome — Trajan | ±69.4 | 9 | 0 | retired | 2026-07-23 |
| 422 | 1642.7 | WildCard2 (`g28-27`) — Egypt — Cleopatra | ±43.2 | 37 | 5 | retired | 2026-07-23 |
| 423 | 1642.5 | JackOfAllTrades (`advanced`) — Scythia — Tomyris | ±168.8 | 2 | 0 | active | 2026-08-01 |
| 424 | 1638.3 | WildCard5 (`g44-40`) — Egypt — Cleopatra | ±119.5 | 4 | 0 | retired | 2026-07-23 |
| 425 | 1636.2 | Opportunist5 (`g32-32`) — Rome — Trajan | ±87.1 | 6 | 1 | retired | 2026-07-23 |
| 426 | 1635.6 | Opportunist2 (`g20-23`) — Greece — Pericles | ±80.6 | 8 | 1 | retired | 2026-07-23 |
| 427 | 1632.4 | Opportunist3 (`g28-28`) — Zulu — Shaka | ±139.4 | 5 | 0 | active | 2026-08-01 |
| 428 | 1631.8 | WildCard6 (`g48-43`) — Mapuche — Lautaro | ±169.1 | 2 | 0 | active | 2026-08-01 |
| 429 | 1631.6 | Opportunist4 (`g28-29`) — China — Qin Shi Huang | ±107.8 | 4 | 0 | retired | 2026-07-23 |
| 430 | 1627.8 | OldGuard (`advanced_v1`) — Norway — Harald Hardrada | ±155.4 | 3 | 0 | active | 2026-08-01 |
| 431 | 1627.8 | FreeSpirit6 (`g48-44`) — Georgia — Tamar | ±146.8 | 4 | 0 | active | 2026-08-01 |
| 432 | 1625.6 | Maverick6 (`g56-49`) — China — Qin Shi Huang | ±88.0 | 7 | 0 | retired | 2026-07-23 |
| 433 | 1625.4 | JackOfAllTrades (`advanced`) — Maya — Lady Six Sky | ±184.6 | 1 | 0 | active | 2026-08-01 |
| 434 | 1625.1 | Opportunist5 (`g32-32`) — Egypt — Cleopatra | ±104.2 | 4 | 0 | retired | 2026-07-23 |
| 435 | 1625.0 | WildCard6 (`g48-43`) — Netherlands — Wilhelmina | ±170.6 | 2 | 0 | active | 2026-08-01 |
| 436 | 1625.0 | Maverick4 (`g36-35`) — Egypt — Cleopatra | ±110.7 | 4 | 1 | retired | 2026-07-23 |
| 437 | 1624.4 | OldGuard (`advanced_v1`) — Georgia — Tamar | ±157.7 | 3 | 0 | active | 2026-08-01 |
| 438 | 1624.3 | WildCard8 (`g52-46`) — Egypt — Cleopatra | ±68.8 | 12 | 2 | retired | 2026-07-23 |
| 439 | 1624.1 | TheHeir (`advanced_evolved`) — Kongo — Mvemba a Nzinga | ±169.5 | 3 | 0 | active | 2026-08-01 |
| 440 | 1623.6 | OldGuard (`advanced_v1`) — Maori — Kupe | ±145.8 | 4 | 0 | active | 2026-08-01 |
| 441 | 1622.5 | OldGuard (`advanced_v1`) — Russia — Peter | ±185.8 | 1 | 0 | active | 2026-08-01 |
| 442 | 1621.2 | OldGuard (`advanced_v1`) — Arabia — Saladin | ±167.7 | 2 | 0 | active | 2026-08-01 |
| 443 | 1620.4 | DarkHorse2 (`g16-20`) — Egypt — Cleopatra | ±48.1 | 27 | 7 | retired | 2026-07-23 |
| 444 | 1620.3 | FreeSpirit (`g16-18`) — Egypt — Cleopatra | ±52.4 | 20 | 4 | retired | 2026-07-23 |
| 445 | 1618.9 | Opportunist6 (`g40-37`) — Greece — Pericles | ±57.9 | 16 | 3 | retired | 2026-07-23 |
| 446 | 1618.5 | FreeSpirit6 (`g48-44`) — Arabia — Saladin | ±170.6 | 2 | 0 | active | 2026-08-01 |
| 447 | 1617.5 | WildCard8 (`g52-46`) — China — Qin Shi Huang | ±74.3 | 10 | 2 | retired | 2026-07-23 |
| 448 | 1615.7 | Opportunist6 (`g40-37`) — Egypt — Cleopatra | ±68.8 | 11 | 3 | retired | 2026-07-23 |
| 449 | 1613.8 | FreeSpirit (`g16-18`) — Greece — Pericles | ±50.8 | 20 | 2 | retired | 2026-07-23 |
| 450 | 1613.5 | DarkHorse4 (`g24-26`) — Greece — Pericles | ±69.6 | 9 | 1 | retired | 2026-07-23 |
| 451 | 1611.4 | OldGuard (`advanced_v1`) — Macedon — Alexander | ±170.7 | 2 | 0 | active | 2026-08-01 |
| 452 | 1606.9 | WildCard10 (`g56-50`) — Ethiopia — Menelik II | ±140.5 | 5 | 0 | active | 2026-08-01 |
| 453 | 1603.8 | FreeSpirit6 (`g48-44`) — Nubia — Amanitore | ±167.7 | 2 | 0 | active | 2026-08-01 |
| 454 | 1603.4 | Maverick3 (`g24-25`) — Egypt — Cleopatra | ±60.7 | 14 | 1 | retired | 2026-07-23 |
| 455 | 1597.6 | JackOfAllTrades (`advanced`) — Ottomans — Suleiman | ±141.6 | 5 | 0 | active | 2026-08-01 |
| 456 | 1596.7 | Opportunist2 (`g20-23`) — Egypt — Cleopatra | ±119.4 | 3 | 0 | retired | 2026-07-23 |
| 457 | 1594.7 | WildCard3 (`g32-31`) — Egypt — Cleopatra | ±87.8 | 6 | 0 | retired | 2026-07-23 |
| 458 | 1590.9 | WildCard3 (`g32-31`) — Rome — Trajan | ±87.2 | 6 | 0 | retired | 2026-07-23 |
| 459 | 1587.7 | FreeSpirit5 (`g36-34`) — China — Qin Shi Huang | ±80.6 | 10 | 2 | retired | 2026-07-23 |
| 460 | 1575.5 | FreeSpirit2 (`g16-19`) — Egypt — Cleopatra | ±48.0 | 29 | 2 | retired | 2026-07-23 |
| 461 | 1570.5 | FreeSpirit3 (`g32-30`) — Greece — Pericles | ±109.7 | 4 | 0 | retired | 2026-07-23 |
| 462 | 1568.5 | FreeSpirit4 (`g36-33`) — Egypt — Cleopatra | ±66.6 | 12 | 1 | retired | 2026-07-23 |
| 463 | 1564.0 | DarkHorse3 (`g20-22`) — Rome — Trajan | ±72.0 | 9 | 1 | retired | 2026-07-23 |
| 464 | 1559.3 | WildCard (`g24-24`) — Greece — Pericles | ±71.7 | 9 | 2 | retired | 2026-07-23 |
| 465 | 1557.0 | DarkHorse6 (`g44-39`) — Egypt — Cleopatra | ±89.7 | 6 | 0 | retired | 2026-07-23 |
| 466 | 1555.5 | WildCard7 (`g52-45`) — Greece — Pericles | ±86.0 | 7 | 2 | retired | 2026-07-23 |
| 467 | 1541.3 | Opportunist5 (`g32-32`) — China — Qin Shi Huang | ±108.4 | 5 | 1 | retired | 2026-07-23 |
| 468 | 1540.6 | DarkHorse6 (`g44-39`) — Greece — Pericles | ±149.2 | 2 | 0 | retired | 2026-07-23 |
| 469 | 1540.5 | Opportunist4 (`g28-29`) — Egypt — Cleopatra | ±99.8 | 5 | 1 | retired | 2026-07-23 |
| 470 | 1539.6 | TrainingWheels (`basic`) — Greece — Pericles | ±44.8 | 58 | 0 | active | 2026-07-23 |
| 471 | 1533.5 | ScoreKeeper (`g8-14`) — Greece — Pericles | ±70.6 | 11 | 1 | retired | 2026-07-23 |
| 472 | 1526.0 | ScoreKeeper (`g8-14`) — Egypt — Cleopatra | ±69.7 | 11 | 0 | retired | 2026-07-23 |
| 473 | 1517.4 | Maverick3 (`g24-25`) — Greece — Pericles | ±67.9 | 11 | 0 | retired | 2026-07-23 |
| 474 | 1517.2 | WildCard (`g24-24`) — Egypt — Cleopatra | ±101.9 | 5 | 1 | retired | 2026-07-23 |
| 475 | 1512.9 | FreeSpirit3 (`g32-30`) — Egypt — Cleopatra | ±97.7 | 5 | 0 | retired | 2026-07-23 |
| 476 | 1512.8 | WildCard6 (`g48-43`) — Egypt — Cleopatra | ±60.5 | 23 | 2 | active | 2026-08-01 |
| 477 | 1508.7 | JackOfAllTrades (`advanced`) — Egypt — Cleopatra | ±47.7 | 49 | 7 | active | 2026-08-01 |
| 478 | 1508.1 | DarkHorse4 (`g24-26`) — China — Qin Shi Huang | ±110.8 | 4 | 0 | retired | 2026-07-23 |
| 479 | 1475.2 | ScoreKeeper (`g8-14`) — China — Qin Shi Huang | ±70.9 | 10 | 0 | retired | 2026-07-23 |
| 480 | 1469.8 | Maverick5 (`g48-42`) — Egypt — Cleopatra | ±98.4 | 6 | 0 | retired | 2026-07-23 |
| 481 | 1468.7 | DarkHorse5 (`g40-38`) — Egypt — Cleopatra | ±113.1 | 5 | 1 | retired | 2026-07-23 |
| 482 | 1467.0 | Opportunist2 (`g20-23`) — Rome — Trajan | ±95.5 | 6 | 0 | retired | 2026-07-23 |
| 483 | 1431.5 | DarkHorse5 (`g40-38`) — China — Qin Shi Huang | ±142.9 | 4 | 0 | retired | 2026-07-23 |
| 484 | 1414.2 | HolyRoller (`g12-15`) — China — Qin Shi Huang | ±102.5 | 5 | 0 | retired | 2026-07-23 |
| 485 | 1396.4 | DarkHorse5 (`g40-38`) — Greece — Pericles | ±98.7 | 6 | 0 | retired | 2026-07-23 |
| 486 | 1373.8 | WildCard9 (`g56-48`) — Greece — Pericles | ±147.1 | 4 | 0 | retired | 2026-07-23 |
| 487 | 1370.3 | WildCard9 (`g56-48`) — China — Qin Shi Huang | ±216.5 | 1 | 0 | retired | 2026-07-23 |
| 488 | 1356.2 | TrainingWheels (`basic`) — China — Qin Shi Huang | ±54.5 | 53 | 1 | active | 2026-07-23 |
| 489 | 1315.9 | DarkHorse3 (`g20-22`) — Egypt — Cleopatra | ±134.8 | 3 | 0 | retired | 2026-07-23 |
| 490 | 1312.4 | DarkHorse6 (`g44-39`) — Rome — Trajan | ±123.9 | 6 | 0 | retired | 2026-07-23 |
| 491 | 1300.3 | SilverTongue2 (`g52-47`) — Greece — Pericles | ±112.8 | 6 | 0 | retired | 2026-07-23 |
| 492 | 1269.3 | SilverTongue2 (`g52-47`) — China — Qin Shi Huang | ±118.6 | 7 | 0 | retired | 2026-07-23 |
| 493 | 1246.5 | SilverTongue2 (`g52-47`) — Egypt — Cleopatra | ±132.7 | 4 | 0 | retired | 2026-07-23 |
| 494 | 1221.1 | HolyRoller (`g12-15`) — Egypt — Cleopatra | ±108.7 | 6 | 0 | retired | 2026-07-23 |
| 495 | 1211.1 | TrainingWheels (`basic`) — Egypt — Cleopatra | ±60.4 | 60 | 0 | active | 2026-07-23 |
| 496 | 1166.2 | SilverTongue2 (`g52-47`) — Rome — Trajan | ±158.9 | 4 | 0 | retired | 2026-07-23 |

## Strategies without a civilization/leader Elo

These roster strategies have no `leader_elo` entries. Their global Glicko-2 rating is
shown in descending order, but it is deliberately not mixed into the exact
civilization/leader ranking above.

| Global Elo | Player (strategy) | RD | Games | Wins | Status |
|---:|---|---:|---:|---:|---|
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
