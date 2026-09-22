# Connect safe fuel for the first Bomber

The Domination resource-purchase exception previously considered only existing melee and siege units with researched upgrades. A first Bomber has no such predecessor. Add its resource demand when its technology is researched, its required district family has an unpillaged completed instance, and the visible resource has zero stock and income. Keep this air demand during Recovery and rival-victory counter plans; preserve the existing Conquest/Expansion restrictions for ground upgrades.

Emergency purchases still run first. Purchases still require a legal plot, no usable owned deposit, an affordable quote with 40 Gold plus six turns of deficit reserved, a charged Builder within four route steps, and safe destination risk. Home threats veto the exception. No resource, production or host legality is bypassed.

## Native evidence and limits

Native083707 knew Advanced Flight from turn 151 and had a completed Aerodrome, but Aluminum remained unavailable through the Aztecs' Culture victory on continuation turn 235. The revealed deposit at native (23,15), within three hexes of Cumaná, initially looked promising from turn-start cash and Builder snapshots. Replays disproved an immediate safe purchase opportunity: turns 197/201/203 had no legal purchase quote; turn 228 had 136 Gold against a 170 quote; turn 230 had 174 against 170, below the 40 Gold reserve. Turn 231 entered Recovery with 228 Gold against 175 and a charged Builder three route steps away, but destination risk was 66.4 against the limit of 30. The revised helper correctly refuses it. Later in that turn the treasury held only 63 Gold.

Paired replay compares baseline 2903a06b0437e46f0188d25be00e4ec0e262a810 with implementation 1db883ce82d25b7b4717f24baf2c76fb77a25310, using identical native inputs and arguments. Both native runs originally used a7; the comparison isolates the new purchase logic on the newer baseline. All 151 continuation replies and all 339 independent religious-loss control replies are exactly equal, including exported orders and internal actions. Forced genome, treatments and withheld ledger match each native recording. Temporary diagnostic logging also reproduced all 151 replies exactly and was removed. Replay binary SHA-256: 688a0b76d7fc63bb270d48698760b8b2954d2c6d7975461a2019374dde01b007.

This is a tested capability for safe resource opportunities, not a demonstrated improvement to either recorded outcome. No Aluminum connection, Bomber completion, faster capture, win-rate gain or native Domination victory is claimed. The recorded loss still requires a way to secure resource access or convert the existing army into capital captures before the rival victory.

## Validation

Eleven focused tests pass, including first-Bomber supply, absent technology/field, pillaged field, existing stock, Builder charges, treasury, home defense, other victory targets, Recovery/Diplomacy air demand and unchanged ground restrictions. Full pre-integration suite: 4,206 passed, 53 ignored. Changed-line quality, append-point checks, release build and eight four-player 180-turn soaks (seeds 373100–373107) pass. These soaks test runtime stability, not Domination success.

Merged main a621748873383c60dc7938793283903ab7843ffd once. Integrated source 8616a20575e3897b40203c6646e08d7a916f5816: 4209 tests passed, 53 ignored; changed-line quality, append-point checks, release build and the same eight-game soak were rerun and passed.
