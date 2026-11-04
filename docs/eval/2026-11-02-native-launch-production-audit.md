# Native launch production audit, 2026-11-02

The two latest retained Emperor Science losses do not establish that earlier
Satellites would advance their launches. The first never completed Earth
Satellite; the second knew Satellites seven turns before completing its
Spaceport. This does not reject the research treatment in PR #4337, whose
strength screen remains separate, but it makes launch production and earlier
economic development the next investigation priorities.

## Native first-observed milestones

| Run suffix | Industrialization | Chemistry | Rocketry | Satellites | Spaceport | Earth Satellite | Moon Landing | Lost |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 161612Z | 128 | 163 | 171 | 196 | 195 | unobserved | unobserved | 208 |
| 165603Z | 112 | 141 | 152 | 167 | 174 | 184 | 197 | 198 |

Full tags are `civvis-20261102T161612Z` and `civvis-20261102T165603Z`.
Both ladder records report a loss to Technology. Times come from native
`events.jsonl` states, not research-choice journal lines. Missing launches
are unobserved, not completion at the turn limit or zero-duration launches.
The attached JSON binds each complete event source with SHA-256 and retains
the first state's launch-city census for each turn.

The first observed Spaceport queue was t176 at 61 Production in 161612Z,
and t153 at 42 Production in 165603Z. The second city's Spaceport cost was
900 production in the native board: at t160 it held 327 progress and showed
14 turns remaining at 42 Production. This is consistent with the observed
long construction interval; it does not prove a queue-stall bug. Its first
launch completed ten turns after the pad, and Moon Landing thirteen turns
after that. Earlier technology does not eliminate those construction costs.

## Two tempting interventions the native evidence does not support

**An own Factory is not the same as a missing Factory yield.** At t152 the
165603Z launch city, Majapahit, did not list `BUILDING_FACTORY` among its own
buildings and could build it for 165 production, about four turns. However,
the native production source text already reports:

```text
+2 from Outgoing Trade Routes
+6 from Districts
 +6 from Industrial Zone
+12 from Buildings
 +6 from Factory
 +3 from Workshop
 +1 from Water Mill
 +2 from Palace
+22 from Worked Tiles
```

A regional Factory already contributes. Do not price another local Factory
as six newly gained Production. Whether an own copy is worthwhile depends
on a follow-on plant or another concrete effect; it needs a native measured
counterfactual before a new forced-building rule. The UI attribution is
retained verbatim, not used to invent a stacking model.

**Buildable power is not demonstrated powered output.** Ternate offered a
Coal Power Plant by t150 and both Coal and Oil plants by t160. Yet the
exported Coal/Oil stocks and income at t152, t160 and t174 were zero.
Oil income was observed at +6 by t184. The journal starts Ternate's Oil
plant at t187 and Tuban's Coal plant at t190. Thus an earlier forced plant
alone has no demonstrated fuel supply during the initial pad construction.
The stock and income fields are observations, not a complete power-flow
model: do not infer exact powered status, import feasibility or marginal
Science merely from their absence or a building's printed yield.

## Next useful decisions

1. Keep PR #4337's independent strength screen and its win/survival gates.
   Its captured-board proposal activation is real; these two cases show why
   activation alone cannot establish a faster victory.
2. Inspect earlier fuel acquisition, resource connections and trade offers
   alongside regional power coverage before pricing an early plant. A valid
   treatment must secure usable production or Science, not just a building.
3. Investigate earlier usable University development independently. In
   165603Z Education was already owned at t64, but t100 showed four Campuses,
   three Libraries and only one University; by t125 all five Campuses had
   Universities. That is an observed development window, not proof that a
   forced University would beat its displaced build. Check production offers
   and queue commitments before changing weights or priorities.

The observations above are a two-run diagnosis. They are not a fleet-wide
effect estimate and do not authorize a default promotion.

## Reproduce

```sh
python3 tools/civ6_launch_production_audit.py \
  /Users/martbot-mbp-m5-max-128/civvis-civ6-runs/control/civvis-20261102T165603Z/events.jsonl \
  --out /tmp/native-launch-production-audit.json
python3 -m unittest discover -s tools -p test_civ6_launch_production_audit.py
```

Four focused tests verify regional-yield attribution without inventing an own
building, unknown-versus-zero semantics, incomplete-pad handling, and the
first-state-per-turn census. No controller, engine or default is changed.
