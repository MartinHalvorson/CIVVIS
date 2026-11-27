# Offensive campaign

The operator's checklist (2026-11-27), verbatim. It is the specification
this document audits the AI against.

```
Offensive Campaign Tactics Strategy
- Need to know the high-level strategy. Are we pushing in the next 20 turns?
- Our rough total and the rough opposing total
- Any allies (city states or Civs) we can rope in.
- What is the Mission? We pretty much play to dominate most mission.
- Plan out keeping a city. Can we hold loyalty? Can we chop? Can we afford a builder? How many turns to chop?
- Can we peace out after 8 turns?
- How much can I trade (take) from the AI before going to war. (War cancels trade deals)
- Need to produce units and position units
- Time everything to attack at once
- Plan out pillaging roughly too
- Leading units go to the backside (open borders is very cheap compared to a war)
- Can move around a few turns based on the opportunities that present themselves (taking a settler)
Peace deal
- What can we get from a peace deal?
- Once peaced out, immediately trade as much as I can with the AI.
- Favor more cash now if we are planning on going to war soon.
Understand attrition odds
- Civ VI - play to eliminate units
- Advance Wars - damage dealt / taken
```

## Verdict

- **At Deity the AI never runs an offensive campaign.** In 611 live Deity
  games (2026-11-18 to 11-27) it declared war on a major 38 times, in 48 of
  the games. 19 of the 26 declarations that were replayed through the
  decider were a heretic strike (`faith-strike-wipes-the-spreaders`). Two
  were launcher wars (`launcher-war-serves-every-lane`). The other five did
  not reproduce in a single-turn replay; they include the one game's two
  conquests. None of the 21 that reproduced was a campaign to take a city.
- **Those wars took nothing and cost units.** A city of the target was struck
  in 6 of the 38 wars. Cities were taken in 2 of them, both in one game (11
  cities, all held to the end). The other 25 that ended did so in peace with
  nothing taken, after a median of 28 turns. Over the 38 wars we killed 182
  units and lost 299.
- **Held cities track score.** Our score over the best rival's is 0.37 in
  games where we end holding no foreign city, 0.42 with one, 0.49 with two
  and 0.60 with three or more (485 games that reached turn 150). That is
  correlation, not proof that captures cause the higher score.
- **The missing link this PR adds is a plan for the war we are already in.**
  `city-campaign-2`, the live version of the campaign planner, returns
  without a plan as soon as any major war is open. So once a war has started
  and its cost is paid (grievances, cancelled deals), nothing names a city
  the army can take. Version 3, `city-campaign-3`, does:
  - **Go:** it names one holdable city, within 12 tiles, whose whole bill the
    field army carries today.
  - **Attack at once:** it gives every unit that one objective.
  - **Clock:** the city must fall within 20 turns of the war, or the war
    reads no-go. A no-go stops the army walking into walls one unit at a
    time.
  - **Peace:** it offers peace once the city is ours.

## The live record

**Source.**
- The `ledger` branch, fetched 2026-11-27 05:20Z: run summaries plus the
  event logs of the 48 games in which we declared war.
- Deity games only, since 2026-11-18, continuation sittings excluded: 611
  games, 539 of which reached turn 100.

**How each war was read.**
- The start of a war is our `war` event (`source: civvis`). Its end is the
  first `state` frame after it with `at_war` false for that rival.
- A unit is "near" a city when it is a land unit with combat strength, not
  embarked, within N hexes of any target city in our own frame. Distance uses
  Civ VI odd-row offset converted to axial, with the map wrap.
- A strike on a city is a `combat` event with `ours: true` and a defender of
  type `district` or `city` owned by the target.
- A capture is a `city_occupation` with `ours_now: true`.
- Deals are `deal_closed` events with that rival.

| Deity, all games since 11-18 | value |
|---|---|
| games / reached turn 100 | 611 / 539 |
| games where we declared any war | 48 (59 declarations: 38 on majors, 21 on city-states) |
| declarations by lane | Religious 46, Culture 8, Science 5, Diplomatic 0 |
| foreign cities held at turn 100 | 32 in 541 games (0.06 a game; 30 games hold any) |
| foreign cities held at turn 150 | 93 in 486 games (0.19 a game, sd 0.43) |
| kills / losses, all wars (ours and theirs) | 15,453 / 8,919 (1.73) |
| damage dealt / taken, all combat | 1,780,842 / 1,879,188 (0.95) |
| peace requests sent / deals accepted / refused | 1,939 in 373 games / 502 / 1,318 |
| moves refused because the step "would declare war" | 3,998 in 447 games |
| our score / best rival's, median | 0.37 (0.52 in our wins) |

| The 38 declarations on majors | count |
|---|---:|
| a target city visible in our frame when we declared | 31 |
| ≥3 of our land units within 3 tiles of a target city at the declaration | 4 |
| no land unit of ours within 6 tiles of any target city at the declaration | 19 |
| ≥3 within 3 tiles at any point in the war's first 10 turns | 10 |
| a siege unit of ours within 6 tiles in the first 15 turns | 16 |
| no siege unit owned at the declaration | 15 |
| any strike on a target city | 6 |
| wars that took a city / cities taken / still held at game end | 2 / 11 / 11 |
| kills / losses with the target | 182 / 299 (0.61) |
| ended in peace / of which with a capture | 25 / 0 (median 28 turns) |
| a closed deal with the target on the declaration turn or the one before | 0 |
| a closed deal with the target within 5 turns after peace | 5 of 25 |
| our peace requests to them that carried a concession | 0 |
| median declaration turn / our military over theirs | 100.5 / 1.6× |

The 21 city-state wars took no city. 9 of them struck one.

**Why we declared.** Each declaration was replayed through
`civvis_orders --mirror <run> --turn <T> --explain`, at turns T−1 and T,
with the run's own `--with` arm (the `genome_treatments.forced` list in its
summary).
- **Religious seat:** the journal on the turn before reads "Holding the
  heretic strike on X | faith-strike-wipes-the-spreaders: N spreader(s)
  stand near our cities but too few can be reached and condemned this turn."
  This held in 18 of the 21 Religious declarations replayed, and in the one
  Culture declaration.
- **Science seat:** the two launcher wars reproduced exactly: "Declaring war
  on Spain | 1404 power against their 756, launcher-war-ignores-the-edge:
  past its Moon with a pad standing, the war raids its pads rather than
  besieging a city". The same happened against Hungary.
- **Neither kind is a campaign.** The heretic strike exists so the units at
  home can condemn spreaders. The launcher war raids pads.

**What happens inside those wars.** Mid-war replays show that the Objective
Board does raise a Siege row on the war's objective. The force behind it is
short, and the grand strategy is still Expansion:

- civvis-20261121T081054Z, turn 95: "Siege Kraków (value 680, need 178,
  force #4 of 7) … short: 2 Siege"; "Grand strategy: expansion | the first
  half is reserved for expansion and defense". The war lasted 87 turns and
  cost 41 units for 7 kills.
- civvis-20261121T062751Z, turn 95: "Siege Râ-Kedet (need 354, force #1 of
  11) … short: 6 Siege"; "Holding the capture of Râ-Kedet while its siege
  train gathers on the staging ring". That war lasted 80 turns, cost 34
  units for 14 kills, and struck the city 0 times.

**The one conquest** was civvis-t7-20261123T105628Z (Religious seat,
Ethiopia).
- Against Scotland from turn 105: three cities, 2.1× their power. The first
  strike on a city came 42 turns in. We struck cities 55 times and took 8
  cities.
- Against the Maori from turn 176: 12 of our units within 6 tiles at the
  declaration, a strike on a city the same turn, 44 strikes in all, 3 cities
  taken.
- That game ended with our score at 1,586 against the best rival's 1,470,
  though a rival won by Science.

## The checklist, item by item

"Live" means the tag is in the `forced` genome recorded by 600 of the 611
runs (the 11 others carry no arm). That recorded list differs from
`deploy/live-force-on.txt` (see "Arming" below).

| item | what the AI does today | live | the gap | evidence |
|---|---|---|---|---|
| Are we pushing in the next 20 turns? | An elective war needs the Conquest posture (`advanced/assessment.rs`). It comes from a standing `CampaignPlan` (`city-campaign-2`) or from the empire ratio, and the empire-ratio branch is off on the live seat (`no_elective_war`). `city-campaign-2` plans only at peace, only when a Conquest objective already has its whole bill staged, and only for 10 turns. `WarPlan` (`war_plans.rs`: research → mobilize → stage → strike → exploit) is the forward plan; it is not a gene, only the evaluator constructor `AdvancedAi::timing_attack()` turns it on. | v2 yes; `WarPlan` no | No forward go/no-go anywhere. Version 2 never plans while a war is open. **This PR:** a 20-turn go/no-go inside the war (`city-campaign-3`). A forward plan at peace is still missing. | 38 declarations, none a campaign; 0 plans at Emperor (`city_campaign.rs` 2026-09-10 note) |
| Our rough total and theirs | `appraise_neighbour`: public military power and tech count. `campaign_city_requirement`: defenders within 6 tiles, city strength, walls and the tier factor, times the 1.5 spare. `assault_forecast` reads a siege's odds and changes no decision. | yes (`assault-forecast-is-read`) | Read only at peace. **This PR** reads the same bill at war, at the city, against the whole field army. | our/their power at our declarations: median 1.6× |
| Allies we can rope in | `coalition-before-war` (versions 1–3) invites the target's neighbours before declaring. `military-alliance-shield-2` and `befriend-the-strongest` buy safety. The mod arm `EnvoyLevy` levies city-state armies. | coalition no (0/611); shield and befriend yes | Our own declarations ask nobody to join. | 0 coalition invitations on the live seat |
| What is the mission? | Victory lanes (`victory_target`). At Deity the seat plays Science (334 games), Religious (169), Culture (85) and Diplomatic (12), never Domination. | yes | "We pretty much play to dominate most missions": no lane runs a conquest. | lane counts above |
| Keep the city: loyalty, chop, a Builder | The bill's `holdable` test: projected loyalty at or above −4 a turn (`CAMPAIGN_HOLD_LOYALTY_FLOOR`) and not deferred by the capital-first rule. Also `raze-a-doomed-capture`, `bleeding-capital-loyalty`, `capture-hold-chain`, `loyalty-rate-alarm`. `chop-for-expansion` chops for Settlers only. | yes, apart from the chop | Nothing sends a Builder or a chop to a captured city, and no governor plan is made for it. | 11 of 11 captured cities held to the end. Our own cities that flip to Free Cities are agent D's lane (`docs/DEFENSE_TACTICS.md`). |
| Peace out after 8 turns | The host forbids peace for 10 turns after a declaration (`peace-waits-for-the-host-minimum`). Peace when the war stalls: `peace-when-the-war-does-not-pay`, `unchosen-war-takes-the-first-peace`, `score-seat-takes-what-pays`. `city_campaign_diplomacy` offers peace once the planned cities are taken. | yes | Peace is a reaction to a stall, never planned. **This PR:** capture → peace offer, and 20 turns without a capture → no-go. | 25 peaces, median turn 28, 0 with a capture |
| Take what we can before the war | `BasicAi::war_eve_liquidation` (`ai/diplomacy_turn.rs`, `Game::war_eve_deals`) sells luxuries, Open Borders and Gold-per-turn riders for lump Gold on the declaration turn, on every declaration path. On the bridge it crosses as a `sell` order. | yes (not a gene) | **No such deal ever closed.** The sale and the war go out on the same turn. A host deal session is answered asynchronously, so a sale sent beside the declaration plausibly arrives after the war has cancelled it. That is unverified: the single-turn replays did not reproduce a war-eve sale. | 0 of 38 |
| Produce and position units | `siege_train.rs` (the train, breakers, `siege-shortfall-claims-the-projects`, `war-reinforcement`, `muster-walks-the-road`, `stage-musters-out-of-reach`) and the Objective Board's Siege rows. | yes | The lane wars open with the army at home. Mid-war boards show the Siege row short (2–6 bodies) while the strategy is still Expansion. | ≥3 units within 3 tiles at the declaration: 4 of 38; no siege unit owned: 15 of 38 |
| Time everything to attack at once | `campaign_staged_for_war`: 3 bodies on the 3–5 ring before an elective declaration. `capture-waits-on-the-march` and `siege-rally-holds` hold the strike until the train arrives. The lane wars skip staging by design. | yes | Inside a war the board can raise a row on any city, so units arrive in ones. **This PR:** one Siege row per war (the plan's city), and none in a no-go war. | city struck in 6 of 38 wars; 182 kills for 299 losses |
| Plan the pillage | `campaign-pillage`: spare movement pillages. `war-raids-the-pads` raids spaceports. `raids-cut-tourism`. | campaign-pillage no; the other two yes | The campaign's pillage is unarmed. | — |
| Leading units to the backside (Open Borders) | `campaign_staging_position` never stands inside the target's territory, even with its Open Borders. `siege-buys-the-passage` buys passage through third parties. `foreign-site-buys-the-open-border` (Great Scientists only). | passage yes; the target's own Open Borders never | We never buy the target's Open Borders in order to stage behind its border. | 3,998 moves refused "would declare war" |
| Move a few turns for an opportunity (a Settler) | `opportunistic-war` (versions 1–2) declares on a weak, exposed target. | no (0/611) | Unarmed. | — |
| What can we get from a peace deal | `peace-asks-a-city` asks for a city while we hold the upper hand. | yes | We never hold the upper hand: 0 requests carried a concession. | 0 of the peace requests to the targets of our 38 wars |
| Trade right after peace | `strategic_bilateral_trade` runs every turn. Nothing is triggered by a peace. | yes (not keyed on peace) | Missing. | 5 of 25 peaces were followed by a deal within 5 turns |
| More cash now if war is soon | `war_eve_liquidation` takes lump Gold over Gold per turn. | yes | It never closes (row above). | 0 of 38 |
| Play to eliminate units / dealt over taken | `battle_planner` (`battle-planner-2`), `docs/BATTLE_SOLVER.md` (`battle-solver`, `solver-plays-the-host`), `risk-is-priced`, `war-kills-the-bands-2`. | planner yes; solver and risk no | The planner prices one turn against one reply. The solver plays the fight forward and is unarmed. Per-unit attrition is already built; it is not this PR's work. | 1.73 kills per loss overall, but 0.61 in our own wars; damage 0.95 |

**Ranked by the cities each gap loses:**
1. **The war in hand is never planned.** This is the largest gap: every
   declaration in the record is a war that takes nothing. This PR closes it.
2. **The muster stays short while the strategy reads Expansion.**
3. **We never stage behind the target's border** (its Open Borders).
4. **The pre-war sale never closes.** It costs Gold, not cities.
5. **Peace terms and trading after peace.**

## What this PR builds: `city-campaign-3`, the war in hand

Version 3 of the `city-campaign` family (`advanced/city_campaign.rs`). Only
one version of the family plays: enabling one clears the others. Version 3
keeps version two's staged plan at peace unchanged, and adds:

- **Go/no-go at war.** `maintain_city_campaign` runs at the start of every
  turn. While a major war is open, `plan_city_campaign_3` reads each war in
  hand. A rival qualifies when it is living, at war with us and a legal
  target, is not the launcher war (`launcher_keeps_the_war` /
  `launcher_war_opens`; that front is the Science race's), and the seat is
  not on the Domination lane (its war desk picks capitals itself). For each
  such rival, `war_in_hand_pick` takes the cheapest of its cities that
  meets all of these:
  - within `CAMPAIGN_V2_REACH` (12) tiles of one of ours;
  - holdable;
  - its whole spare-included bill is carried by the field army today:
    strength, bodies, and a capturer.

  The best pick across the wars becomes a one-city `CampaignPlan`, already
  declared. Counters: `campaign:planned` and `campaign:war_planned`. Journal:
  "Planning the war in hand against X".
- **Attack at once.** A standing plan already puts its city on the Objective
  Board as a Siege row. With the gene on, the board leaves out the generic
  objective of the same rival when it is a different city
  (`campaign_withholds_the_siege`). The army therefore gathers on one city
  instead of two.
- **No-go.** A war in hand where no city of the rival clears the bill is
  recorded no-go for the turn (`campaign_war_no_go`). The board raises no
  Siege row against that rival. The army holds and fights what comes to it
  (the Destroy rows stand), instead of feeding a siege that is short of its
  bill.
- **The clock.** A plan that has taken nothing `CAMPAIGN_V3_CLOCK` (20)
  standard turns into its war is dropped. The rival stays no-go for the
  cooldown (`CAMPAIGN_REPEAT_COOLDOWN`, 15 turns), and no new plan is drawn
  in that time. Counter: `campaign:clock_expired`.
- **Peace.** Once the plan's city is ours, `city_campaign_diplomacy`
  (unchanged) offers peace that turn. If the rival refuses and the war goes
  on, the next turn plans the war's next city. A plan's war ending in peace
  starts the cooldown, as before.

With the gene off, nothing reads the new code: every new branch checks
`city_campaign_3`, and the no-go map stays empty.

**Tests** (`city_campaign.rs`):
- `the_war_in_hand_gets_a_plan_that_version_two_never_draws`: version 2 at
  war returns `None`; version 3 names the neighbour's city, already
  declared. This is the defect, and the test fails without the change.
- `a_war_none_of_whose_cities_the_army_can_take_is_a_no_go`: no plan, the
  Siege row withheld, and peace clears the verdict.
- `the_plan_holds_the_army_on_one_city_of_theirs`.
- `the_clock_turns_an_untaken_war_into_a_no_go_and_a_capture_into_peace`.
- `city_campaign_3_is_a_native_opt_in_and_one_version_plays`.

**What it does not do.**
- It does not plan a war at peace 20 turns ahead (item 1's forward half).
- It does not switch production to the bill (rank 2).
- It does not buy the target's Open Borders (rank 3).
- It does not move the pre-war sale a turn earlier (rank 4).
- It does not re-trade after peace (rank 5).

Those are the next links, in that order.

## What it should move, and how it is read

**The ledger numbers it should move:**
- **Foreign cities held at turn 150 (primary): up.** Today 0.19 a game
  (sd 0.43, 486 games), from `conquest_marks.at.150.cities` in each run's
  summary.
- At turn 100: today 0.06.
- Our score over the best rival's: today a median of 0.37 (sd 0.135).
- **Units lost in the wars we are in: down,** through the no-go.

**Mechanism checks, from the event logs with the reading rules above.** Take
every war of 10 turns or more, ours or theirs. Two shares should rise: wars
in which a target city is struck, and wars that end with a capture. Today
those are 6 of 38 and 2 of 38 for our own declarations. Kills over losses in
those wars should rise from 0.61 towards the overall 1.73.

**Games needed.**
- To detect +0.15 foreign cities held at turn 150 at 80% power, two-sided
  5%: about 130 games per arm, roughly three days of the live host at 90
  games a day with the arms alternating.
- The score ratio needs about 180 per arm to detect +0.04.
- A Deity win rate moved by 5 points needs more than 1,000 per arm. That is
  not a verdict this gene can get on its own.

**Arming.**
- `deploy/live-force-on.txt` now names `city-campaign-3` where it named
  `city-campaign-2`. Version 3 contains version 2.
- That file is not what the live host plays. The recorded arm (473 tags in
  `genome_treatments.forced`) has 295 tags the repository list does not
  name. The supervisor therefore resolves a local override
  (`~/.civvis-live-force-on` or `CIVVIS_WITH_FILE`) ahead of this file, and
  the repository change reaches the seat only once that override does the
  same.
- **The operator's action**, one of:
  - replace `city-campaign-2` with `city-campaign-3` in the host's override;
  - better, for a verdict, write `city-campaign-3` to
    `~/.civvis-live-screen-gene` when the current screen
    (`religion-seat-takes-the-faith-beliefs`) ends, so that each game is
    dealt on or off (`docs/LIVE_SCREEN.md`).
- Arming is confirmed when the next run summary's `forced` list carries
  `city-campaign-3` and not `city-campaign-2`.
