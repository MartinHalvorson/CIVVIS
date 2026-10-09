# Value Páez's retained army aura before retirement

Native verification `civvis-20261009T075708Z` retired every one of its five
Comandantes. Each activation has a later verified receipt. At turn 136 it
retired Páez, its only Comandante, while two Crossbowmen, a Man-at-Arms, two
Trebuchets and a Knight stood within two tiles. The recorded intermediate
state immediately before activation supplies these positions. This game
retired at turn 150 in production rank 4; it is a non-win.

The shipped `DLC/GranColombia_Maya/Data/GranColombia_Maya_GreatPeople.xml`
grants every Comandante its land-unit aura at lines 39–48, defines eligible
unit tags at 94–107, and sets its strength to +5 at 299–302. Páez's
retirement grants the cavalry ability at 194–199 and sets its strength to
+4 at 329–337. His shared Llanero healing effect is defined at 66–75 and
275–280. These are native rules; this change chooses when to use them.

The bridge currently requests immediate activation whenever the host says
it is legal. The fallback Lua driver also activates a legal person, so
omitting a Rust order alone cannot preserve the general. The existing
Comandante spending-policy helper already serves state export, explicit
activation, highlight filtering and the fallback driver. Extend that
helper for Páez instead of adding a second source of spending decisions.

Reserve Páez when retiring would remove the aura from at least two nearby
eligible, unembarked land units and their aggregate +5 strength exceeds
the nearby cavalry's aggregate +4 retirement strength. A second unspent
Comandante covering a troop removes that troop from the lost-aura count.
Healing an injured nearby Llanero takes precedence. Unreadable host data
keeps existing activation behavior. The comparison is a local decision
heuristic, not a claim that summing strength establishes combat outcomes
or globally optimal retirement timing.

The unchanged driver fails seven intended reservation checks, while its
existing Urdaneta controls and the retirement controls pass. The corrected
suite exercises both explicit and fallback orders, the shared bridge
policy, nearby/distant/spent replacement commanders, wounded/healthy
Llaneros, out-of-range and embarked troops, and unreadable authority.
No test simulates native aura application.

Native deployment and winning effect remain unverified. Preserve the
fixed verification policy, all completed attempts and every retirement
in the non-win denominator. No improved win rate is claimed.
