# Fund the defensive Inquisition before repeated Missionaries

Native run `civvis-20260921T114215Z`, pinned to `895b1f7e0`, lost to a rival's
religious victory at turn 114. A native KEEP verdict confirms the conquest of
Istanbul and elimination of its owner at 106, eight turns before the loss.
The player founded Buddhism at 84. Bogotá had
its Shrine by 90 and its Temple by 100, and repeatedly purchased Missionaries.
At 106 all six cities still followed Buddhism; by 113 three of seven followed
Orthodoxy. The last pre-victory frame at 114 showed three Buddhist cities,
three Orthodox cities and one city with no majority. The victory event occurred
after this observation; it does not establish a majority-rule discrepancy.

The defensive purchase loop set the Apostle cap to zero and omitted it from
the normal defensive priorities. Its Inquisitor priority only became active
after an Inquisition already existed. Thus the existing Apostle-to-Inquisition
action had no purchase path to supply it. Faith was 80 at 90, 145 at 100,
130 at 105 and 25 at 110 while replacements consumed the balance.

The change prepares one Inquisition for Domination founders with religious
victory enabled, an owned holy city, unlocked Apostles and a working Temple
following their own faith. It first allows an initial spreader or an empire
with at least half its cities following the founder's religion, then reserves
religious spending for one Apostle and the first Inquisitor. An existing Apostle
banks Faith for the defender while it travels. The Apostle launches even while
all home cities still follow its religion. Once a charged Inquisitor exists,
ordinary spending resumes. Other victory lanes keep their existing priorities.
Actual purchases still require the engine or host's price and legality.

The native mirror also lacked Holy City identity and Inquisition status. The
control mod now reads the Holy City's coordinates and the player's actual
Inquisition boolean. The mirror maps coordinates after placing cities and applies
observed true or false on both rebuild and synchronization. Different native
players can have the same city ID, so coordinates prevent a rival city from
being mistaken for the Holy City. A regression covers that collision. Older
observations do not invent a Holy City or successful Inquisition.

Native interface evidence: shipped `Base/Assets/UI/ReligionScreen.lua:795` calls
`CityManager.GetCity(playerReligion:GetHolyCityID())`. The installed
`GameCore_XP2.dll` registers `HasLaunchedInquisition` and
`lHasLaunchedInquisition`; the PlayerReligion API catalog also lists the
[instance method](https://sukritact.github.io/Civilization-VI-Modding-Knowledge-Base/PlayerReligion).
Each export is independently protected when an accessor is unavailable.

The full Rust suite passed 4,037 tests (53 ignored), including seven focused
AI regressions and three mirror regressions. All 54 discovered Lua regression
suites passed under Lua 5.1, along with syntax checks, fourteen treatment-append
tests, formatting and diff checks. Eight four-player simulator games completed
(seeds 0–7, turn limit 180, four jobs).

The original 299-frame replay lacks the new host
fields and produces no changed orders. A separate diagnostic replay supplies
Bogotá's coordinates, inferred from its being the sole completed Holy Site at
the first founded-faith observation (85), and an unlaunched Inquisition. These
are explicit added premises, not fields recorded by the historical mod. In that
matched 299-frame comparison, Missionary purchases fall from five to two: the
87/0 and 90/0 purchases remain, while 100/1, 106/3 and 109/0 are withheld.
Those are the only three actionable changed frames (six including synthetic
receipts, three internal). No Apostle or Inquisitor is purchased because the
historical Faith balances and menus retain the original spending. Baseline and
candidate elapsed times were 40.37s and 48.11s, unpaired and built with different
optimization profiles; only the required paired CI gate measures performance.
Recorded-board replays cannot accumulate hypothetical saved Faith in subsequent
historical frames or prove a prevented loss. No native Domination victory has
been verified.
