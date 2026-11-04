# Where the completed native wars failed to reach capitals

This is a descriptive audit of the same eight completed Emperor Domination
attempts published in [the material trial](2026-10-25-native-material-screen.md).
They produced zero wins and no observed held foreign major original capital.
It is not a new strength experiment or evidence for promoting a policy.

The final census uses the first native state each turn. It replaces city
rosters each state and whole plot records each tile update. A capital counts
only when its **original owner** occurs in the observed major roster; a
city-state capital subsequently conquered by a major does not count. Unknown
founders are excluded rather than guessed to be major. Our own lost original
capital, when reported under a rival, is included because Domination also
requires recovering it. This is a census of observed capitals, not hidden cities.

## The physical bottlenecks

“Ready” below means native HP >=50, moves >0, attacks remaining >0. Range uses
each aircraft's native range and the offset-to-axial cylindrical distance in
`src/hex.rs`. It is geometry, not host attack permission. A base is an observed
own city center, complete unpillaged Aerodrome, owned unpillaged airstrip or
Carrier. Base geometry does not establish a free slot, safe approach, legal
rebase or adequate supplies. The report separately records whether city loyalty
survives two ticks and preserves missing readings.

| Native stem suffix | War-capital turns | Two ready Bombers, no base geometry in range | Two ready Bombers currently in range | In range, visible and healthy land taker within 8 |
|---|---:|---:|---:|---:|
| 123652 | 59 | 32 | 0 | 0 |
| 124842 | 67 | 0 | 24 | 1 |
| 130400 | 26 | 0 | 0 | 0 |
| 131248 | 142 | 0 | 0 | 0 |
| 132534 | 137 | 0 | 0 | 0 |
| 134257 | 0 | 0 | 0 | 0 |
| 134959 | 118 | 38 | 6 | 6 |
| 140632 | 80 | 0 | 0 | 0 |
| **Total** | **629** | **70** | **30** | **7** |

These are capital-turns; simultaneous wars can contribute more than one per
turn. The taker test is only distance and native health, not a safe route or
capture calculation. Last exported plot visibility is likewise an observation,
not an inferred line of sight. There were 128 war-capital turns with two ready
Bombers anywhere, and only three of eight games supplied any such turns.
The issued orders contain twelve AIR_ATTACKs on major original capital plots,
all Rio, across the entire eight-game cohort. Orders alone do not establish
host execution, capital damage or an eventual capture.

This does not support weakening assault safety gates. Most capital positions
never presented the wing, sight and capture body together. Research, production,
forward basing, scouting and capture movement need to deliver that combination.

## Brasília was not a viable base without a loyalty rescue

In `civvis-20261025T134959Z`, Brasília at native (31,19) was captured at
turn 156. The actual captured decision at frame 3 reports our new city ID
786443, population 6, loyalty 50 and **native total loyalty -31.3438/turn**.
The decision issues RAZE, the host reports a successful request, and the next
native state contains neither our city nor Brazil's old city there. The visible
plot subsequently loses its City Center. Keeping the city without changing
that total would leave less than two ticks before revolt, shorter than the
existing two-tick air-base arrival/operating guard.

The journal's -20 population-pressure estimate is not the native total. Nor was
there a persistent foreign-city export bug: the first exploratory inspection
retained an old roster entry. Direct raw-state comparison disproved it. The
six audit controls include roster replacement and captured city-state identity
so neither mistake can silently drive the final census.

At turn 160 Panamá's center is 11 tiles from Rio, beyond a Bomber's native range
10. During 24 blocked Rio turns (157–180), narrow owned, visible, flat,
resource-free sites within range exist **only after removing a feature**.
There are no immediately empty owned sites under that filter, and no Engineer
in those states. At the reconstructed turn-157 decision no city reports an
Armory or a trainable Military Engineer. An airstrip policy would therefore
need the full preparation chain, not just an Improve action. The terrain
screen is not native build permission or evidence that the chain would finish
in time. A doomed captured city cannot substitute for that preparation.

## A separate visibility candidate

In `124842`, Istanbul has two or more ready Bombers currently in range on 24
war turns, but only one also passes the visible/near-taker screen (turn 195).
No AIR_ATTACK on its capital plot is issued. The native input for that frame
has three ready aircraft in range and a healthy land taker five tiles away;
that still does not prove a legal, safe or winning assault.

Spies can supply sight: the installed primary database row
`Base/Assets/Gameplay/Data/Units.xml:769` declares UNIT_SPY with
`BaseSightRange="3"` and `TeamVisibility="true"`. Existing engine visibility
also reads established spies. A separate default-off candidate will test whether
one spare idle spy can maintain sight of an active capital that the wing
actually reaches. It must preserve running missions, native travel permission,
home defense and higher-priority race denial, and demonstrate changed actions
on actual captured inputs. This audit does not claim that candidate helps.

## Reproduction and retained corrections

Run `2026-10-25-capital-assault-audit.tests.py`, then the census with `--control`
and a new `--out` path. The witness tool takes `--tools tools` as well. It
reconstructs **all 71 read calls**, comprising five distinct captured reads,
for turn/frame 156/3, 157/0 and 160/0 in game 7 and 195/0 in game 2. Every
reconstructed last state equals the unique native event state. The witness
report retains before/after states, actual decisions and intervening events.

Raw native events, decisions and captured bytes/indexes already reside in the
committed `2026-10-25-native-material-screen.cohort-01` through `cohort-08`
archives. In particular, cohort-02 and cohort-07 reconstruct the witnesses;
restore their run directories and point capture descriptor paths to that
restored root. Exact input hashes and read multiplicities are recorded. Large
reconstructed prefix copies remain locally and are not duplicated in this PR.

The evidence archive retains the initial v1/v2 reports, which incorrectly
included original city-state capitals, the corrected v3 report, the final
reproducible census, raw and final controls, witnesses, source and native script
search. Only the final report is authoritative. Six controls pass. No Rust,
Lua, runtime setting or deployment genome changes in this PR.

`2026-10-25-capital-assault-audit.evidence.tar.xz`: **313,712 bytes**, SHA-256
`0841cd64de8f266c8efe6e5f5d399dbb6efc6c8703ae287dc2830b5259b41343`.
All 17 archive members were verified against their source bytes.
