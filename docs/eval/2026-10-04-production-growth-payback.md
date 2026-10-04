# Early housing growth and production: preregistered screen

The goal is faster production growth against high-level rivals. The preceding
industrial-chain experiment (#3892) did not produce broad early catch-up and was
reverted. In its fresh confirmation control, Deity turn-75 production averaged
32.688 from 3 cities and 13.375 population, only 0.171 of the strongest rival's
production. The recorded King game also had housing limits in five of six cities
at turn 75. These observations motivate an early growth screen; they do not prove
that housing investment will repay its cost.

The first candidate is the existing, deployment-off
`first-granary-reserve-2` policy. It reserves a legal Granary only when its housing
lift forecasts the next citizen within a speed-scaled 30 Standard turns and
advances that arrival by at least 2 Standard turns. It excludes a locally
threatened or recently attacked city and uses the current food yield. It ignores
the building's additional food and any change in worked tiles. No runtime source,
gene default, rules, or ledger is changed for this screen.

## Frozen comparison

- Engine source: `afba14116415131f730a462dff4e1d7811aced93`; the task's claim
  commit has identical `src`, `data`, ledger, and Cargo manifests. Reuse its clean,
  completed production release library, recording its hash before play.
- One probe binary, two explicit arms. Both use the deployment gene ledger;
  only the focal candidate calls `enable_first_granary_reserve_2` afterward.
- Four majors, 60 by 38 Pangaea, six city-states, Online speed, all victories and
  barbarians, turn cap 150. Gran Colombia is focal, rivals are randomized.
- Focal seat is exempt from difficulty bonuses, explicitly targets Domination,
  and opponents use the unchanged `AdvancedAi::fleet` seats.
- Pilot: Emperor seeds 61006000–61006003 and Deity 61006100–61006103, both arms.
  Freeze the policy and probe before running any of these games.
- Reserved, unrun confirmation: Emperor 61006200–61006211 and Deity
  61006300–61006307. Use only if the unchanged pilot supports further testing.
- Record all final outcomes, including early world endings and eliminations.
  Compare only checkpoints observed in both arms, explicitly list missing
  checkpoints, and never carry a final value forward as an observed later turn.
- At turns 25, 50, 75, 100, 125, and 150, observe production, strongest-rival
  production, population, cities, Granaries, housing-limited cities with and
  without Granaries, amenity shortages, science, culture, Gold, Builders, and
  military power. The rival population/city counts belong to the rival with the
  greatest production at that checkpoint. Cumulative production is the sum of
  observed city yields, not production spent or credited to construction.
- Early production, population, and production relative to the strongest rival
  determine whether the hypothesis addresses the goal. Expansion, science,
  culture, military power, and outcomes reveal costs. A late conditional gain
  alone is insufficient evidence of fast catch-up. A pilot is not promotion
  evidence or a claim of actual Firaxis performance.

## Preliminary read-only controller replay

Before the native screen, the archived #3892 control controller replayed turns
1–100 of the same recorded King game's concatenated prefix with persistent AI
history. `--with first-granary-reserve-2` is an existing supported treatment arm;
both arms use the same archived binary and recordings. The candidate first
changes a production proposal on turn 40 and proposes Granaries in seven cities
through turn 100, while control proposes none. All 100 responses completed in
both arms. The observations continue to come from the original controller, so
these are counterfactual proposals, not actuated decisions or candidate growth
outcomes. The replay engine predates the unrelated #3895 spy mirror change;
the native screen uses the current engine specified above.

Results and final validation are pending.
