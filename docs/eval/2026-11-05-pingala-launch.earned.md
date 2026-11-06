# Pingala earns a committed launch: experimental policy

Native Indonesia game `civvis-20261105T184951Z-campus-sea-pilot-v2-03-on`
completed its Spaceport at 141 and satellite at 152. At the loss on 166 its
Moon Landing held 686 of 750 production, with a native quote of two turns.
Pingala acquired Space Initiative by 134 and remained in the original capital;
the launch city held Magnus, including Industrialist. This diagnoses an unused
promotion, not an avoided loss or a counterfactual victory.

The existing `pingala-follows-research-2` cannot handle this case by itself:
it requires the temporary Science posture, excludes all major wars, and keeps
specialist Magnus in place. The original game enabled version one.

`pingala-earns-the-launch` is an independent **default-off** policy. On an
explicit Science target it considers a current space **project**, before title
spending, so the existing native governor-exchange continuation can preserve
the return leg. Other target lanes retain their behavior. An unrelated war
or temporary Conquest/Diplomacy posture does not alone prohibit the exchange;
Recovery, local danger, loyalty protection, disabled governors, immature posts,
same-turn repeat requests, Curator works and the current Spaceport post do.
Only an ordinary or specialist Magnus can be displaced; defensive and other
specialist occupants remain in place.

The forecast uses the native remaining-turn quote as its baseline. During
establishment both moving governors' effects are absent. After establishment,
relative modeled production includes Space Initiative, all existing production
multipliers, and Magnus's lost plant/regional effects. The move must save a
full quoted turn. A separate experimental budget limits the empire's lost
science plus half its culture to 5% over the projected completion interval.
This budget is a hypothesis, not an optimized tradeoff. Great Person points,
future projects and subsequent changes to the economy are not valued by this
forecast; native strength validation remains necessary.

The shipped Gathering Storm rules are the authority for the effect:
`DLC/Expansion2/Data/Expansion1_Governors.xml:446-447` attaches
`EDUCATOR_FASTER_SPACE_RACE_PRODUCTION` to Space Initiative;
`:743-744` defines `MODIFIER_SINGLE_CITY_ADJUST_SPACE_RACE_PROJECTS_PRODUCTION`;
`:1178-1180` sets `Amount` to **30**. A Spaceport district under construction
does not receive this project bonus. Establishment is budgeted using the
existing native five-turn lower bound and the duration table.

Thirty local governor regressions passed, including eleven new controls for
this policy. Fourteen append-point regressions and the gene metadata check
also passed. Tests used the CI profile with optimization disabled for the
local build; the full required CI gate remains pending.

Captured-input verification is in progress. Native residence is observed
across boards: a cold snapshot cannot establish sufficient tenure. The
planned probes use two declared prior observations, with an independent audit
of stable native posts in every turn105–153. They are bounded observation
histories, not reproduction of the original private/controller history.

No captured-input firing result, native actuation,
city survival, production throughput, or win-rate improvement is claimed yet.
The operator's native seat restriction remains in force.

## First captured-input result: negative

Frozen source `d7567bb7443c8231b539f37e8fd2fe1313aec306`, executable SHA256
`06a4eb229057278f2ad544abb860448809a5f32567cfeada485b635c91ee26ad`,
completed twelve target queries plus twenty-four prior-observation queries.
All636 actual registered reads matched the declared fixed frontiers; both
repeated arms matched, and their effective genomes differed only by this flag.
All orders were identical at134/0,142/0 and153/0. No governor assignment was
emitted by either arm. The results are retained as a negative proposal test.

Passing the local fixtures did not establish activation on these native states.
The next source revision adds guard/forecast journal details for diagnosis;
strategic thresholds are unchanged. This candidate is not ready for promotion
or a native strength trial.
