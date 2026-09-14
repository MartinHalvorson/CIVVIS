# `science-threat-denial` and `science-denial-war` — deny the rival's finish, not only speed ours

*2026-09-09 · two opt-in genes, off by default · `src/ai/advanced/science_threat_denial.rs`*

Two tags, priced apart. **`science-threat-denial`** is rungs 1 and 2 below —
the diplomatic refusals, the denunciation and one spy's disruption of the
launch pad: no unit, no grievance, no war. **`science-denial-war`** is rungs 3
and 4 — the pad raid and the bounded war that opens it. The war tag *requires*
the base and arms it (`enable_science_denial_war` sets both flags), so it
fires on its own in a screen and a seat that draws the war without the base
plays the whole ladder; the raid and the war read
`science_denial_war_active`, which is both flags together. The split was made
after the first probe of all four rungs as one tag (below, "The first probe")
opened 13 wars for 2 pillaged pads: the war rung was the whole of the cost.

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

### Rung 1 — diplomacy (`science-threat-denial`)

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
  its whole term. **An existing alliance is never touched**: the partner
  filter sits in `propose_strategic_alliance`, which already skips a standing
  ally (`alliance_with(..).is_none()`), so the refusal only ever declines a
  *new* one.
- **A denunciation, once per turn**, of the most pressing threat. It costs the
  rival the friendship and alliance routes to our market and starts the Formal
  War clock rung 4 reads. An allied threat is left out of the ranking: the
  engine's `do_denounce` refuses an ally in any case ("cannot denounce that
  player"), so the alliance could not be broken here, and leaving it in would
  only spend the turn's one denunciation on a refused apply. The seat counts
  each denunciation as `denial_denunciations`.

The denunciation runs on `AdvancedAi::denounce_most_pressing`, factored out of
`culture-threat-early`'s `culture_threat_denunciation` (#3278) and now shared
by both: the caller supplies a ranked list, the primitive reads the engine's
own legality out of the diplomacy family (met, at peace, not friends or
allied, not already denounced) and denounces the first that applies. The
culture path's behaviour is unchanged — filtering to legal and then sorting by
pressure picks the same seat as sorting and then finding the first legal.

### Rung 2 — espionage (`science-threat-denial`)

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

**One pad takes one spy, and the network is released when the threat ends.**
The posting bonus is withheld once any spy of ours has `Spy::city` equal to
the pad city — the engine sets that field to the destination the turn the
order is given (`do_assign_spy`), so one read covers both the posted and the
travelling agent, and because the spy pass mutates the board between spies a
second idle spy the same turn already sees the first's order. Without this
every idle spy would have been sent to the same pad: `legal_spy_actions`
excludes only the spy's *own* current city. Both bonuses are zero the turn a
rival stops being a threat, which hands the spy back to the stock table, and
the engine lists `disrupt_rocketry` only against an *active* pad
(`active_spy_target_position` → `district_is_active`, i.e. not pillaged), so
a pad already pillaged is never disrupted twice. The seat counts each posting
to a threat's pad as `denial_spy_posts`.

### Rung 3 — the raid (`science-denial-war`)

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

### Rung 4 — the war (`science-denial-war`)

`science_denial_war_diplomacy` runs in `advanced_diplomacy` just ahead of
`opportunistic_war_diplomacy`, with the same "a declaration here is the turn's
one declaration" contract.

A threat is a war target when it is inside `DENIAL_WAR_LAUNCH_HORIZON` (**40**
standard turns) of finishing, our own finish is **neither sooner than theirs
nor inside that same horizon**, its pad is one we have explored, and the two
war gates that already exist admit it: `war_is_affordable`
(`war-needs-a-treasury`) and `one_war_holds_declaration`
(`one-war-at-a-time`). The own-horizon gate is the reviewer's addition: a
seat 40 turns from its own launch keeps its production for the last project
rather than a war, whoever is ahead.

The horizon is projected by `science_denial_turns_to_finish`, which now takes
the launch city explicitly: for a rival it is **the city whose pad we have
explored** (`ScienceThreat::pad`), for ourselves `science_drive_pick_launch_city`.
Each space project still to build is priced at that city's own rate with
`science_project_build_turns` — the helper the science drive already uses to
rank its own pads — and a launched expedition's remaining flight at the
distance left over `Game::exoplanet_speed`. 40 turns is deliberately longer
than the war itself: the declaration, the march and the pillage all have to
land before the last project completes.

**What the projection reads past the fog, stated rather than assumed.** The
pad is a tile in `players[pid].explored`; the landed projects, the
expedition's distance and the technology count are the public victory and
score screens, the same shape `rival_victory_pressure` reads. The one reach
past what a player could see is the pad city's production and its banked
project progress, which a real player learns from a spy posted there. It is
confined to that one city — the first version picked the rival's launch city
by scanning *every* city of theirs, which is gone — and there is no
fog-honest production estimate in the controller to replace it with;
`docs/civvis-the-fog-honest-agent-loses-by-not-landing-orders.md` is the
standing caution in both directions.

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
clears its own state and returns `false`. A test walks every one of them. With
the base on and `science_denial_war` false, rungs 3 and 4 are inert and a
test walks those two.

The `denounce_most_pressing` refactor is behaviour-preserving for
`culture-threat-early` (#3278): the old path filtered the threats to the
legal ones and then took the first by pressure; the new one sorts by pressure
and takes the first legal, which is the same element, and
`rival_culture_pressures` is a pure read so computing it before the empty
check changes nothing.

## Tests

26 focused tests in `src/ai/advanced/science_threat_denial/tests.rs`:

- both registry rows are opt-in and ship off in both controllers; the war
  tag arms the base, turning it off leaves the base as it was, and turning
  the base off leaves the war rung inert
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
- the most pressing threat is denounced, once (and counted), and then the
  next; an active denouncement is never repeated; a friend is never
  denounced; an ally is left out and the next threat taken, and the alliance
  stands
- the pad is the spy posting and the disruption is promoted only in a threat's
  city; the bonus arithmetic clears the success-chance gap; one spy per pad,
  the posted spy keeps its own bonus, a foreign spy binds nothing
- the raid party is two soldiers and never a lone garrison
- the raid pillages the pad it stands on (recording `denial_pillages`), stands
  down once it is pillaged, and marches on it otherwise
- the raid needs the war, and the war tag: the base alone never raids
- no war is declared for a pad no soldier can walk to, and the lone garrison
  of our own city does not count as the march
- the projection reads the given launch city, refuses a city that is not
  theirs, and shortens as the race is banked
- the war opens only inside the horizon, only behind our own finish, and
  never inside the horizon of our own; it is refused by `war-needs-a-treasury`
  and `one-war-at-a-time`
- the declaration waits for the Formal War clock and then opens, recording the
  pad as its objective; the base tag alone, on the same board, never declares
- the war closes when the pad is pillaged, and when it has run its course
- a concluded war is forgotten; the war tag off clears the state, and so does
  the base off
- off, no entry point reads the board

## The first probe — all four rungs as one tag

`gene_screen --games 12 --jobs 4 --genes science-threat-denial --p-on 0.5
--difficulty emperor --rivals firaxis-mix --handicap rivals --rival-chairs 3`
before the split — 12 games, 36 seats, 14 on / 22 off. 7 of 12 games ended in
a science victory at median standard turn 199; 50 % of measured seats lost to
a rival's science finish. **13 denial wars and 2 pillaged pads** on, zero of
both off; win −3.9 ± 11.5 pp (14.3 % on, 18.2 % off), share −2.77 pp, on-seats
lost to a rival's science finish 57.1 % against 45.5 % off. Thirteen wars
bought two pillages: the war rung paid the full price of a war and converted
one time in six. That is why the rungs are two tags.

## Fires probes — the two tags separately

Same command, once per tag, on the split build (commit `b25f62ca`), same
seeds (`26081900`–`26081911`), 12 games, 36 seats, 14 on / 22 off each.
`docs/gene_screens/fires/2026-09-09-science-threat-denial.json` and
`docs/gene_screens/fires/2026-09-09-science-denial-war.json`;
`gene_fires.py --max 0` is green on both.

**The regime is the right one, both times.** 7 of 12 games end in a science
victory (median standard turn 204 / 201), 3 in a religious one, 1 in a
culture one, 1 on score; `lost_to.science` is **50.0 %** of measured seats in
both batches (religious 19.4 %, culture 8.3 %, diplomatic 0 %).

**Both tags fire.** The counters are on the seat, exported by `gene_screen`:

| counter | `science-threat-denial` on / off | `science-denial-war` on / off |
|---|---:|---:|
| `denial_denunciations` | **197** / 0 | 193 / 0 |
| `denial_spy_posts` | **31** / 0 | 25 / 0 |
| `denial_wars` | 0 / 0 | **9** / 0 |
| `denial_pillages` | 0 / 0 | **0** / 0 |

**And the win column is the same in both, and the same as the first probe.**

| axis | `science-threat-denial` | `science-denial-war` |
|---|---:|---:|
| win on / off | 14.3 % / 18.2 % | 14.3 % / 18.2 % |
| win Δ | −3.9 ± 11.5 pp (z −0.34) ~ | −3.9 ± 11.5 pp (z −0.34) ~ |
| score share Δ | −2.97 pp (z −1.80) | −2.95 pp (z −1.69) |
| **on-seats lost to a rival's science** | **57.1 %** (8 of 14) vs 45.5 % off (10 of 22) | **57.1 %** (8 of 14) vs 45.5 % off (10 of 22) |
| techs at end Δ | −2.47 ± 3.85 | −2.97 ± 3.77 |
| techs at std t150 Δ | −0.48 ± 1.12 | −0.48 ± 1.12 |
| science / turn at end Δ | −27.4 ± 48.3 | −45.4 ± 38.5 |
| games finished Δ | −4.73 ± 4.28 pp | −5.58 ± 4.10 pp |
| seats forgotten Δ | +1.01 ± 0.60 pp | +1.16 ± 0.54 pp |

Read honestly. With the same seeds the same fourteen seats draw the gene in
every run, and **the same two of them win and the same eight lose to a
rival's science finish whether the gene is the base, the base plus the war,
or the original four rungs**. Every declaration, denunciation and posting in
these twelve games landed on a seat whose outcome it did not change. The
−3.9 pp and the +11.7 pp of science losses are therefore the *draw* — which
seats happened to be on — and not the gene; the columns that do move with
the tag are the ones that read what it does to the seat itself: the war tag
costs about **18 more science per turn** at the end than the base
(−45.4 against −27.4, the same seats) and half a technology more, which is
the production and the units nine wars take. The base tag's own cost on
those columns is indistinguishable from the draw at this size.

**The war rung, gated harder, still does not convert.** The reach gate and
the own-horizon gate cut the declarations from 13 to 9, and the pillages from
2 to 0. Nine wars for no pillage is worse than thirteen for two. Whatever
loses these wars still happens after the declaration — the party is two
soldiers picked by map distance each turn, the pad sits inside a defended
city's ring, and peace comes at `DENIAL_WAR_MAX_TURNS` whether or not the
march arrived — and it is not diagnosed here. That is the reason the war is
its own tag: the continuous screen can now turn it off without losing the
base, and the ledger's default rule (a negative pooled Diff vetoes) will
decide it on its own numbers.

A 12-game probe is not a measurement (`docs/GENE_SCREEN.md` §"A probe's win Δ
is not a measurement of the gene"): these say the tags fire and show *how*
they fire, not what either is worth.

## Instrumentation

Four counters on the seat, exported by `gene_screen` beside `raid_wars`, one
per rung that reaches the board: `denial_denunciations` and
`denial_spy_posts` for the base tag, `denial_wars` and `denial_pillages` for
the war tag. The first probe of the split had only the last two and could
say nothing about whether the base tag had reached the board at all — the
"a claim is not a check" failure `AGENTS.md` names — which is why the other
two exist.

## What this does not answer

- **Neither probe is a price.** Twelve games on one seed set cannot separate
  a gene from the draw, and here they visibly did not. Pricing belongs to the
  continuous screen, tag by tag.
- **Why nine wars did not reach a pad is not diagnosed.** The counters say
  it happened; nothing says whether the party never formed, never arrived, or
  died on the ring. A `denial_marched` counter, or a journal read of one of
  the nine, would say which — and that is the measurement that decides
  whether rung 3 is fixable or whether `science-denial-war` should simply
  leave the code from the bottom of the table.
- **197 denunciations across 14 seats is a lot of denouncing.** Once a
  denouncement lapses the threat is denounced again the next turn it is
  legal; the diplomatic cost of standing denounced by us for the whole late
  game, on a rival we are not going to fight, is not priced here.
- **The pace admission is untested against a real board.** Six technologies
  at turn 120 is argued from the ladder's end-of-game deficits, not measured
  at turn 120.
- **The projection's one reach past the fog is documented, not removed.** See
  rung 4 above.
