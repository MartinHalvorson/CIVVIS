# Solvent production and core defence

The science recovery guard now runs without the optional `war-economy` gene
(#3222). This follow-up also banks existing upkeep-heavy civilian, building,
and district queues during recovery. Income infrastructure, Spaceports,
victory projects, repairs and local defenders retain their commitments.
The ordinary scorer cannot buy another Spy through the undermanned-war
exception. Queue progress is banked by the engine's existing item key.

Before discretionary spending, targeted Science can modernize one existing
land defender within three tiles of a developed city. More developed cities
rank first, then strength gained per Gold. The legal upgrade quote must leave
100 Gold plus 25 per city and its incremental maintenance must fit current
income. Appointed offensive war packages retain their own upgrade pass.
This is a bounded core investment, not an attempt to match every rival's army.

Regression tests cover the bankrupt Spy-to-Trader handoff and saved production,
preserved defenders/income/victory commitments, the wartime Spy veto, and core
upgrades with and without sufficient recurring income. The unassigned legacy
controller retains its existing decisions. Native tournaments still record
all six seats from each game.

The retained live baseline lost its original capital and four other developed
cities. The subsequent run spent 68 of its first 180 observed turns at zero
Gold with negative income. These changes address those failure mechanisms;
native soak completion is not evidence of a higher live Emperor win rate.
