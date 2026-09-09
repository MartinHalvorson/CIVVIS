# `science-threat-denial` — deny the rival's finish, not only speed ours

*2026-09-09 · opt-in gene, off by default · `src/ai/advanced/science_threat_denial.rs`*

## The evidence

The Emperor ladder loses the space race it is running. On the rung the fleet
plays, the rivals complete the science victory at standard turns **213–228**
while this seat stands at a Spaceport with **one to three of four launches**
done. `docs/civvis-the-empire-builds-forts-not-science.md` records the shape
of the gap underneath that result: three to nine times more defensive than
science buildings in every deep game, and **12–33 technologies behind** at the
end. On the Immortal and Deity rungs the rival carries a **+24 %** and
**+32 %** yield handicap on top of that, so the rival *will* launch first.

Every science gene in the controller pushes our own launch forward —
`science-victory-drive` picks a launch city, `science-endgame` reserves the
production, the boost genes buy the technologies. **None of them is aimed at
the rival's Spaceport.** Speeding our own launch is a race we lose by
construction once the handicap is large enough; the finish has to be taken
away from them as well.

The one piece of denial that already exists is `deny_leaders`' espionage
bonus, `SCIENCE_ROCKETRY_DENIAL_PRIORITY` in `src/ai/advanced.rs`. It arms at
`SCIENCE_ROCKETRY_DENIAL_PROGRESS = 65` in `rival_victory_pressure`'s
currency, and 65 is *the Mars colony already landed* — the third of four
stages. By that point the seat has one launch left to stop and a spy that has
not yet travelled. It is also gated on `plan.strategy == Science`, so a seat
playing any other plan never denies at all.

## The design

### The threat

`AdvancedAi::science_threats` returns the threatening rivals, most pressing
first. The guards are `culture_trade_threats`': met, alive, a living major,
not a teammate, the science victory enabled, `victory_planning` on. Two
admissions:

- **the physical race** — we have explored a tile carrying a Spaceport of
  theirs, or they have completed any of the four space projects; or
- **the pace** — they lead our technology count by `SCIENCE_THREAT_TECH_LEAD`
  (**6**) after standard turn `SCIENCE_THREAT_TECH_LEAD_TURN` (**120**).

Six technologies is half the smallest deficit any lost Emperor game finished
with, and turn 120 is where a lead stops being a hut or a first-to-meet bonus.
Ranking is by stages landed, then by the technology lead, then by seat, so the
order is total and stable across turns.

`science_projects` on a player is *every* completed non-repeatable project —
`manhattan_project` and `operation_ivy` land there too — so the stage count
filters against the explicit four-name array `victory_races` uses rather than
taking the set's length.

### Rung 1 — diplomacy

- **No sale of passage and no sale of a Great Work to a threat**
  (`science_denial_deal_allowed`, wired into the four quick-deal filters in
  `strategic_bilateral_trade` beside `culture_deal_allowed`). Those are the
  two sales that pay a rival's space race directly. Purchases from the threat
  and every other sale stay open: starving our own treasury does not slow
  their launch.
- **No alliance and no research agreement with a threat**
  (`science_denial_refuses_alliance`, wired into `propose_strategic_alliance`'s
  partner filter). A research agreement hands a science threat exactly the
  yield it is winning with, and *any* alliance makes rung 4's war illegal for
  its whole term.
- **A denunciation, once per turn**, of the most pressing threat. It costs the
  rival the friendship and alliance routes to our market and starts the Formal
  War clock rung 4 reads.

The denunciation runs on `AdvancedAi::denounce_most_pressing`, factored out of
`culture-threat-early`'s `culture_threat_denunciation` (#3278) and now shared
by both: the caller supplies a ranked list, the primitive reads the engine's
own legality out of the diplomacy family (met, at peace, not friends or
allied, not already denounced) and denounces the first that applies. The
culture path's behaviour is unchanged — filtering to legal and then sorting by
pressure picks the same seat as sorting and then finding the first legal.

### Rung 2 — espionage

The model's launch sabotage is **`disrupt_rocketry`**. It is gated on the
`spaceport` district family, and `Game::apply_spy_mission_effect` pillages the
pad *and* zeroes the city's banked spaceport-project progress. That is the
sabotage-of-production this lane wants — `sabotage_production` itself is gated
on `industrial_zone` and only pillages that district's buildings — so
`disrupt_rocketry` is what the gene sends.

Two bonuses, both zero when the gene is off:

- `DENIAL_SPY_ASSIGN_PRIORITY` (**340**) on a spy assignment whose destination
  is a threat's launch city. The stock Science table pays 150 for any
  Spaceport and 180 for the war plan's target; 340 outranks both together, so
  the pad that is actually going to launch is the posting.
- `DENIAL_SPY_MISSION_PRIORITY` (**700**) on `disrupt_rocketry` run against a
  threat, at our threshold rather than at Mars and under any grand strategy.

700 is not a round number chosen for feel. The spy pass scores a mission as
`strategic × spy_success_chance`, and `disrupt_rocketry`'s base chance is
**0.20** against `steal_tech_boost`'s **0.35**. At the stock values,
`290 × 0.20 = 58` against `320 × 0.35 = 112`: the disruption is never
selected. `(290 + 700) × 0.20 = 198` clears it. A test pins that arithmetic so
the constant cannot be tuned into inertness.

### Rung 3 — the raid

While we are at war with a threat whose pad we have seen and which is not
already pillaged, the `DENIAL_RAID_SIZE` (**2**) nearest mobile soldiers within
`DENIAL_RAID_REACH` (**8**) of the pad walk onto it and pillage it. A pillaged
Spaceport runs no space project until it is repaired.

The party never includes the lone garrison of one of our cities: that is
`AdvancedAi::lone_garrison`, the floor `opportunistic_war` already keeps for
the same question — the answer to a raid is a counter-raid, and an empty city
is its prize. A soldier holding a city under pressure is likewise left alone,
on `raid_prize_step`'s own guard and radius.

The step is wired into the unit ladder immediately ahead of `raid_prize_step`,
because it is the objective a war was declared for. It returns `None` whenever
the unit has no part in the raid, so the rest of the ladder is untouched.

### Rung 4 — the war

`science_denial_war_diplomacy` runs in `advanced_diplomacy` just ahead of
`opportunistic_war_diplomacy`, with the same "a declaration here is the turn's
one declaration" contract.

A threat is a war target when it is inside `DENIAL_WAR_LAUNCH_HORIZON` (**40**
standard turns) of finishing, we are **not** inside a shorter horizon of our
own finish, its pad is one we can see, and the two war gates that already
exist admit it: `war_is_affordable` (`war-needs-a-treasury`) and
`one_war_holds_declaration` (`one-war-at-a-time`).

The horizon is projected by `science_denial_turns_to_finish`: each space
project still to build is priced at the launch city's own rate with
`science_project_build_turns` — the helper the science drive already uses to
rank its own pads — and a launched expedition's remaining flight at the
distance left over `Game::exoplanet_speed`. It reads the same public
victory-screen shape `rival_victory_pressure` does. 40 turns is deliberately
longer than the war itself: the declaration, the march and the pillage all
have to land before the last project completes.

The opening is `preferred_war_opening` — the cheapest legal casus belli, else
the surprise war once a denouncement stands. When that function answers with a
denouncement instead (the Formal War clock has not run), the denouncement is
taken and **no** declaration is spent that turn.

**The war must be able to reach its own objective.** The first probe of this
gene opened **13 wars across 14 on-seats and pillaged 2 pads**: the
declaration read only whether we could *see* a Spaceport, never whether the
raid it exists for could get to one, so it bought grievances and no denial.
`science_denial_raid_can_reach` now requires a soldier that is not a lone
garrison to be within `DENIAL_MARCH_SHARE` (**0.6**) of `DENIAL_WAR_MAX_TURNS`
of marching to the pad. The route is read on a **speculative board with the
declaration already applied**, for the reason `opportunistic-war` version two
reads it there: before the war, the target's closed borders make every route
into its interior look impassable, and a distance on the map is not a route
across an ocean at all.

Peace is offered as soon as the pad is pillaged or gone, or after
`DENIAL_WAR_MAX_TURNS` (**25**) standard turns, whichever comes first, and
never before the engine's own minimum war length (`RAID_PEACE_EARLIEST`).

## Off

With `science_threat_denial` false every entry point returns before it reads
the board: `science_threats` is empty, so the two diplomatic filters and both
espionage bonuses are identities, the raid step is `None`, and the war pass
clears its own state and returns `false`. A test walks every one of them.

## Tests

22 focused tests in `src/ai/advanced/science_threat_denial/tests.rs`:

- the registry row is opt-in and ships off in both controllers
- a pad or a landed project makes a rival a threat; a pad we have not explored
  does not; a non-space project is not a stage; unmet, dead, minor, teammate,
  own seat, victory disabled and `legacy()` all read empty
- a technology lead is a threat only from the pace turn, and one technology
  short of the bar is nobody's threat
- threats rank by stages, then by the technology lead
- passage and Great Work sales to a threat are refused; buying, bystanders and
  luxury sales are not; off, every deal is allowed
- alliances and research agreements with a threat are refused, bystanders' are
  not
- the most pressing threat is denounced, once, and then the next; an active
  denouncement is never repeated; a friend is never denounced
- the pad is the spy posting and the disruption is promoted only in a threat's
  city; the bonus arithmetic clears the success-chance gap
- the raid party is two soldiers and never a lone garrison
- the raid pillages the pad it stands on (recording `denial_pillages`), stands
  down once it is pillaged, and marches on it otherwise
- the raid needs both the war and the gene
- no war is declared for a pad no soldier can walk to, and the lone garrison
  of our own city does not count as the march
- the projection reads the launch city and shortens as the race is banked
- the war opens only inside the horizon and only behind our own finish, and is
  refused by `war-needs-a-treasury` and `one-war-at-a-time`
- the declaration waits for the Formal War clock and then opens, recording the
  pad as its objective
- the war closes when the pad is pillaged, and when it has run its course
- a concluded war is forgotten, and the gene off clears the state
- off, no entry point reads the board

## Fires probe

<!-- PROBE -->

## Instrumentation

The two expensive rungs left no trace a screen could read, so the first probe
could not say whether either reached the board — the "a claim is not a check"
failure `AGENTS.md` names. `denial_wars` and `denial_pillages` are now counted
on the seat and exported by `gene_screen` beside `raid_wars`, which is how the
13-wars-for-2-pillages defect above was found at all.

## What this does not answer

- **The probe is not a price.** `docs/GENE_SCREEN.md` §"A probe's win Δ is not
  a measurement of the gene" — twelve games cannot separate a gene from the
  seed. Pricing belongs to the continuous screen.
- **The four rungs are screened together.** Nothing here says which of the
  four pays. If the joint gene prices positive, the natural follow-up is to
  split rungs 3–4 into their own tag the way `raid-pillage-prizes` splits the
  pillage half off `opportunistic-war`.
- **The pace admission is untested against a real board.** Six technologies at
  turn 120 is argued from the ladder's end-of-game deficits, not measured at
  turn 120. A screen that varies `SCIENCE_THREAT_TECH_LEAD` would say whether
  the number is right.
- **The projection reads rival internals.** `science_denial_turns_to_finish`
  reads the rival's banked production and city yields directly, as
  `rival_victory_pressure` reads their `science_projects` and
  `exoplanet_distance`. That is house style for this controller, but it is not
  fog-honest, and `docs/civvis-the-fog-honest-agent-loses-by-not-landing-orders.md`
  is the standing caution in both directions.
