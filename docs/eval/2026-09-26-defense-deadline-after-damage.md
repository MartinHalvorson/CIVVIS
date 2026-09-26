# Defense deadlines after small hits

## Native diagnosis

Prince run `civvis-20260926T222220Z` used source
`3d8d41e43c982b92bd703c1d503c6e86e1a081e5`, Simón Bolívar / Gran Colombia,
Tiny Pangaea, four players, Online, Expansion 2, all victory conditions enabled.
Székesfehérvár (native city262147, offset19,17) was captured on41 and lost to
Samarkand's army on76, absent from our roster at77. Loyalty stayed100.

Persistent replay of all202 observed state frames through77 using the exact
native runtime binary reproduces the allocation failure: Defend Székesfehérvár
has no force at73 (deadline21) and75 (deadline5), then four defenders at76.
Force IDs differ from the live log; the target, need, deadline and delayed
assignment agree. Other cities and the active Szeged siege compete for units.

The board records183 HP and9 recent damage at73. Its damage branch computes
ceil(183/9)=21 and skips the nearby-hostile arrival estimate used for an
undamaged city. Small initial damage can therefore postpone relief. This
mechanism is reproduced, but a better allocation and survival are not yet
established.

Walls were absent from native buildable items through73, available74 with a
five-turn estimate, selected75. An earlier70 wall order is not established as
legal. Samarkand was at peace through69 and hostile70; the commercial hub
selected68 predates that confirmed hostility.

## Preregistered evaluation

Before any policy change, retain the clean baseline source `de6a184d24208a4c19bf64c0c02004cc8d2aaebf`
and its binaries. Candidate and baseline replay the same complete observed
prefix, SHA256 `afd1616cbcdca0d5e88a470d992807524e8dca70e53800482dd423dd01b2d778`,
with the recorded20-policy force bundle and one persistent decider. Replay
orders cannot change the archived future states; this is decision evidence,
not a counterfactual city survival result.

Four Prince paired seeds37910000–37910003 run to completion for each source:

```text
victory_eval --domination-pair early-conquest-opening --difficulty prince --games 4 --start-seed 37910000 --out <fresh-file>
```

Compare all registered arms and reported outcomes, captures, city losses,
original foreign capitals and decision cost. All controllers use the selected
source; opponents are CIVVIS, not Firaxis. The off/on toggle is a within-source
pair; it does not create eight independent source-comparison seeds. Retain
negative results. No source tuning after inspecting outcomes. A harmed pilot
requires withholding the candidate until explained or resolved with new
preregistered evidence. Promotion requires native domination outcomes.

Evidence is retained outside task worktrees at
`~/civvis-war-evidence-20260926/prince-city-retention/`.
