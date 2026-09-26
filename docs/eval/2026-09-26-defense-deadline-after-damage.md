# Defense deadlines after small hits

**WITHHELD FROM DEPLOYMENT.** The deadline correction passes its regressions
and assigns native relief earlier, but all202 emitted native decisions remain
identical. The fixed Prince pilot gives no wins in either source and a mixed
first seed. Do not merge this candidate as a retention repair. The follow-up
must make assigned relief affect commands and then receive fresh registered
evaluation.

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

## Candidate and results

Frozen candidate `9de4a7053fa34418bdcd6415c7e94246f72d90aa` takes the minimum
of the existing observed-hostile approach estimate and the measured damage
deadline. With no observed hostile nearby it preserves the damage estimate.
Two regressions fail first on `fec690a08`; all28 objective-board tests then
pass, including earlier defense-priority protection. Full local validation:
4,337 passed, zero failed, 53 existing ignores. Changed-line Rust quality
passes. This is AI scheduling, with no engine rule change.

The complete native replay changes the target's force from none to one unit
at73, none to three at74, and none to three at75. Deadline21 at73 and5 at75
both become2. All202 full decision objects remain identical across baseline
and candidate, as do the membership-only diagnostic's decisions. Reassignment
alone is therefore not an effective native retention repair.

The diagnostic identifies native Archer917510 (axial6,21) as the initial
reassigned defender. Its turn73 combat against Hungary's Heavy Chariot at
axial7,22 and movement away from the city persist. At74 the nearby native
Man-at-Arms2555923,62HP, belongs to the Settler escort rather than the defense
pool. Both the remote combat choice and civilian guard reservation require
causal follow-up; this candidate changes neither. No city survival is proved.

All eight registered paired-arm records completed for each source:

| Seed | Toggle | Before → after ending | Before → after score | Major cities ever held | Foreign original capitals ever held |
| --- | --- | --- | --- | --- | --- |
| 37910000 | off | Score250 → Score250 | 931 → 812 | 0 → 2 | 0 → 1 |
| 37910000 | on | Culture187 → Culture150 | 375 → 343 | 0 → 0 | 0 → 0 |
| 37910001 | off | Culture208 → Culture208 | 844 → 844 | 0 → 0 | 0 → 0 |
| 37910001 | on | Culture208 → Culture208 | 844 → 844 | 0 → 0 | 0 → 0 |
| 37910002 | off | Culture171 → Culture171 | 523 → 523 | 0 → 0 | 0 → 0 |
| 37910002 | on | Culture171 → Culture171 | 523 → 523 | 0 → 0 | 0 → 0 |
| 37910003 | off | Diplomatic245 → Diplomatic245 | 891 → 891 | 0 → 0 | 0 → 0 |
| 37910003 | on | Diplomatic245 → Diplomatic245 | 891 → 891 | 0 → 0 | 0 → 0 |

Every focal seat loses; no Domination win occurs. The original home capital
remains held in every arm. The foreign original capital captured by seed
37910000 off is lost again before the end (foreign capitals held at end:0). The two legs report equal outcomes on the last
three seeds; they are still paired observations, not additional independent
seeds. Profile, civilizations and forced bundle match for each source pair.
This all-controller comparison changes every simulator AI, so the first
seed's mixed result cannot identify which controller benefited or suffered.

Binary SHA256 receipts:

| Source | civvis_orders | victory_eval |
| --- | --- | --- |
| baseline de6a184d2 | 319859d31971f2f6d95e1cf1cbce896aa9a2a30d48b2a04a8a223bc68ef98eef | a7dd4662115996973f290e88b83172ba47764bc2c549bbe15bf4c9afd75a244b |
| candidate9de4a7053 | db3753fff40f412da8701703e67f5752be9e5a21e9cffba7459ef6f569435a08 | e22117f2d3a8d6fd637fcca96b7d9bfbc9c26dce7d6b6a05ce824f350c570b57 |

The complete raw results, profile checks, exact replay commands, diagnostic
patch and binary hashes are preserved in the evidence folder. Sequential
replay wall times were collected under concurrent build/pilot load and are
not a controlled cost comparison; the repository's paired-cost CI is the
performance gate. Neither simulator outcomes nor green tests justify a
native promotion or deployment of this incomplete repair.
