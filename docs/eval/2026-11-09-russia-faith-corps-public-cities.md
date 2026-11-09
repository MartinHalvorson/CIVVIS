# Russia sizes its religious corps from observed public city totals

Native `civvis-20261109T183445Z` played Russia/Peter on Deity, Gathering Storm,
Online speed, six-player Small Continents. Its frozen controller was
`c591f8ec9fdd9ac74711ac98c479f4948a536123`, SHA-256
`6ae3850fa4f7c9b045afc619f1f8f21c2c04ff8b8544f7f8e23d6ef298f5267e`.
`religion-seat-fields-apostles` was reported active by its runtime genome.

At t100, frame 0, the seat had four Apostles of its own faith, 720 Faith,
and native Apostle price quotes of 230 Faith. The located foreign cities and
the host's already-exported public city totals disagree substantially:

| Rival | Located cities | Public city count | Located cities positively following Buddhism |
| --- | ---: | ---: | ---: |
| Japan | 7 | 7 | 2 |
| Gran Colombia | 0 | 10 | 0 |
| Ottomans | 0 | 9 | 0 |
| Poland | 3 | 5 | 0 |

Each rival's reported majority followed another faith on this frame. The
former corps helper counts only located cities not known to follow our faith:
eight cities, giving `2 + ceil(8 / 6) = 4` Apostles. It ignores the public
counts of the other 21 cities. Missing faith observations are not proof of an
unconverted city; they are part of the heuristic's possible remaining work.

The same helper body is present in the original native revision and this
task's source base. The new regression reproduces these located/public counts
and exercises the actual cap helper, rather than a separate copy of its math.

## Policy

The existing Apostle treatment adds a living major's publicly counted but
unlocated cities to its workload estimate. Positively known cities of our faith
are still subtracted. For the captured configuration, this estimates up to
29 cities that may need conversion, and reaches the existing ceiling of six.
The ceiling, unit mix, purchase gates, Faith reserves and victory test do not
change. A Religious victory still needs civilization majorities, not every
city in this workload estimate.

A native majority verdict already following our faith contributes no extra
unseen work. Its known holdouts keep their existing contribution. An absent
majority verdict remains unresolved. Missing or stale public totals never
reduce the known-city workload. Our own cities, dead majors, city-states and
barbarians contribute nothing. No hidden city objects, locations or religions
are manufactured.

With the treatment off, on another victory target, before founding, or without
the offensive posture, the caller retains its shipped cap. Without public
city-count observations, the known-city heuristic retains its old result.
This is a repair to an existing opt-in, with no new gene or default promotion.

## Evidence and verification

The actual native read captured for t100, frame 0, is 25,666,911 bytes,
SHA-256 `11cacfeffc0bbaacb63b29847efe3c013a7bf695bb96b6d799789f6e197057a9`.
Sealed input, original decision and capture descriptor are retained under
`~/civvis-tactics-results/2026-11-09/russia-faith-corps-public-cities/`.

The existing native export's `publicEmpireStats` iterates
`subject:GetCities():Members()` and increments `tally.city_count`; its result
is carried in each met rival's `public_stats`. The confirmed-majority
readback uses the shipped Firaxis
`Base/Assets/UI/PartialScreens/WorldRankings.lua:2049`,
`GetReligionInMajorityOfCities`. This policy consumes those existing
observations and adds no host queries.

Focused coverage includes the captured workload, missing/stale counts,
confirmed and reverted native majorities, unresolved majorities, excluded
players, and the existing treatment/target/posture/founding gates. Native
resumed one-shot planning can verify the imported estimate, but cannot establish
host execution or improved victory outcomes. The original run lost to a rival
Diplomatic victory at t166. No religious win-rate improvement is established.
