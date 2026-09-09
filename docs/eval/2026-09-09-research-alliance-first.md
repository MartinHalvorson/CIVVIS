# research-alliance-first: the alliance an Emperor handicap cannot deny us

Opt-in gene, `Kind::OptIn`, off by default. Off it is byte-identical.
Code: `src/ai/advanced/research_alliance.rs`; tests in
`src/ai/advanced/research_alliance/tests.rs`; fires probe in
`docs/gene_screens/fires/2026-09-09-research-alliance-first.json`.

**Read the probe section before this gene is priced.** The 36-seat probe is
negative on score share in every version that was run, and the win axis is
noise at this size. This round is a design and a measurement, not a case for
turning the gene on.

## The alliance model, as this simulator actually has it

Every claim below is a line in this repository, not a memory of Civ VI.

- **Kinds are strings, not an enum.** `AllianceState { kind: String, points:
  f64, level: i32, ends: u32 }` (`src/game.rs:3766-3772`), held symmetrically
  in `Player.alliances: BTreeMap<usize, AllianceState>` (`src/game.rs:4777`).
  The five legal kinds are `research | cultural | economic | military |
  religious` (`src/game/actions.rs:8819-8822`). An empire holds at most one
  alliance of each kind.
- **Legality.** `Game::do_propose_deal` (`src/game/actions.rs:8818-8841`)
  requires a valid kind, the civic `civil_service` on **both** trees, the kind
  free on both sides, peace, no denouncement, and — for `research` only —
  `tree_effect(pid, "research_agreements") > 0.0` on both, which comes from
  the tech `scientific_theory` (`data/tree_effects.json:20`).
- **A declared friendship is NOT an engine prerequisite.** Nothing in
  `do_propose_deal` reads `friends_until`. The friendship gate is the
  controllers': `src/ai.rs:8511` makes the Basic controller wait for
  `are_friends` before attaching an alliance kind, and the Advanced
  controller bundles `friendship: true` into every alliance proposal
  (`src/ai/advanced.rs:16455-16470`). This gene keeps the two-step because
  the operator's design asks for it; whether the bundled one-step lands more
  often is an open question this round did not answer.
- **Levels come from points, and routes feed them.**
  `Game::process_diplomacy` (`src/game/actions.rs:11034-11112`), once per
  pair per turn: `points += 1.0 + 0.25 (an outgoing route to a partner city)
  + 0.25 (an incoming one) + 0.25 per side for Democracy or Wisselbanken +
  0.5 same secret society + 0.5 a Sumerian joint war`. Then `level = 3 if
  points >= 240, 2 if >= 80, else 1`. So one route each way is up to **50
  percent faster levelling**.
- **What a Research Alliance pays.** Level 2, every
  `standard_duration(30)` turns: `share_research_alliance_boosts`
  (`src/game/actions.rs:8042`) hands the partner's cheapest unboosted tech to
  whichever side lacks it. Level 3: `sci += 10 percent of the sum of the
  ally's city science` (`src/game.rs:32180-32189`, the Cultural tier's twin
  at `:32171`).
- **Routes to an ally are already priced.**
  `trade_route_destination_value_from` (`src/ai/advanced.rs:33601-33625`)
  adds a research ally's `+2` Science, a flat `45.0` for the first connection
  in a direction, and `+18.0` for a cultural ally at level 2 — there is no
  level-aware twin on the research arm.
- **Two cards pay science on an international route.**
  `trade_confederation` (`+1`) and `market_economy` (`+2`), through
  `international_trade_science`, read at `src/game/city.rs:4082` and owned by
  no other card in the tree, so no domestic route earns it.

**What the genome did not price.** `propose_strategic_alliance`
(`src/ai/advanced.rs:16315`) picks its kind from the grand strategy on a
twelve-turn cadence (`g.turn % 12 != pid % 12`), and its ranking term for a
research partner is the count of techs the partner holds and we do not — a
measure of what we can copy once, not of the science still flowing at level
3. `coalition.rs` proposes alliances early, but only military ones and only
in front of a war.

## The design

1. **Ranking.** Met, living majors, not at war, not denounced either way,
   grievance under `ALLY_GRIEVANCE_CEILING` (75), not already allied with us,
   and neither the current culture threat (`culture_trade_threats`) nor the
   current science threat. Score = `ALLY_SCIENCE_WEIGHT` x the partner's
   science share of ours + `ALLY_FRIENDSHIP_READY` (180) if a friendship
   stands + `ALLY_RESEARCH_LEGAL` (60) if a Research Alliance is legal, minus
   our grievance. `ALLY_MIN_SCIENCE_SHARE` (0.6) keeps trivial partners out:
   at level 3 the share is 10 percent of the partner's output, so a partner
   at 0.6 of us is worth 6 percent of our own science and one at 0.2 is worth
   2 — inside the noise of a single Library.
2. **The science threat.** No twin of `culture_trade_threats` exists on main
   (`grep science_threat` hits one test name). `research_alliance_science_
   threat` is built from the same public signal the denial layer reads:
   `rival_victory_pressure(g, rival).strategy == Science && progress >=
   ALLY_SCIENCE_THREAT_PROGRESS (30)` — deliberately
   `CULTURE_THREAT_PRESSURE_EARLY`'s bar.
3. **Sequence.** One proposal a turn. The partner is **sticky**: chosen once
   and kept until they stop being a candidate at all, because a ranking
   re-read every turn canvasses every major, and a declared friendship is not
   free (it forbids denouncing and war, and breaking one carries grievance).
   Friendship first; then, once Civil Service stands on both trees, the
   Research Alliance; and only if that is unavailable the free kind whose
   model yield is closest to science (`ALLY_FALLBACK_KINDS = [cultural,
   economic, religious, military]`, ordered by the yields above), journalled.
   A refusal is answered by the `ALLY_RETRY_TURNS` (10) cool-down, not by
   moving on — the model exposes no refusal event. Once the Research Alliance
   stands the desk stops proposing entirely.
4. **Routes.** `ALLY_ROUTE_PREMIUM` (30) on the first route to an ally whose
   level is still below 3 — the real accrual is what it buys, so it stops at
   the second route (the `+0.25` is already collected) and at level 3 (a
   further point buys nothing). Capped to `ALLY_ROUTE_PREMIUM_OPENING_CAP`
   (10) while the empire holds fewer than `ALLY_ROUTE_OPENING_BAND_CITIES`
   (6) cities, so an internal food or production route in the opening band is
   not displaced.
5. **Card.** The best offered card of `[market_economy,
   trade_confederation]`, spliced to the front of the desired deck exactly as
   `culture_defense_cards` is, while a non-threat alliance stands.
6. **Guard.** A culture or science threat is never proposed to, never carries
   the route premium, and never justifies the card. A standing alliance with
   a partner who becomes a threat is **not broken** — breaking one costs
   grievance and the science is still real; only the objective is dropped.

## ⚠ Civil Service lands past turn 140, and it is the binding constraint

Measured on this model at the screen's own size (6 majors, 74x46, Standard,
250 turns, seed 26081900, whole game played through `run_game_observed`): our
own `civil_service` at **turn 143**, the first rival's at **144**. The typed
alliance the engine will not seat without it therefore has about a hundred
turns left. At `+1.25` points a turn with one route each way that reaches
level 2 (80 points) and **never** level 3 (240). On a 250-turn clock this
gene buys the shared tech boosts and not the 10 percent science share.

That measurement came out of a defect this round nearly shipped. The first
version gated the *whole* ranking on Civil Service, so the desk's first ask
was turn 144 and the probe was flat. A unit test that proposes on a
hand-built board proves only that the board was built right;
`the_desk_reaches_a_real_game_and_asks` plays a whole game at the screen's
size and requires a counter to have moved. Its first form used 4 majors on
44x30 over 160 turns — where **nobody ever reaches Civil Service** — and it
failed, which is how the gate was found. Lifting Civil Service off the
friendship stage moved the desk's first ask from turn 144 to **turn 16**.

A second finding from the same instrument: in that whole game no seat ever
ended holding a `research` alliance. `scientific_theory` is late enough that
the fallback kinds take the sequence in practice. A gene called
`research-alliance-first` that mostly seats cultural and religious alliances
is doing something, but not the thing it is named for.

## Fires probe — not a measurement

`gene_screen --games 12 --jobs 4 --genes research-alliance-first --p-on 0.5
--difficulty emperor --rivals firaxis-mix --handicap rivals --rival-chairs 3`
(12 games, 36 measured seats, 14 on / 22 off, seeds 26081900..26081911). This
run resolves a win delta of +/-27.6 pp at 80 percent power, so the win axis
carries no information at all here (`docs/GENE_SCREEN.md`, "A probe's win
delta is not a measurement of the gene"). Three versions were run on the
same seeds:

| version | win delta | z | share delta | z | techs@end | science/turn |
|---|---:|---:|---:|---:|---:|---:|
| v1 (Civil Service gated the ranking; canvassing) | -11.0 pp | -1.12 | **-3.66 pp** | **-3.00** | -6.65 (z -2.05) | -72.2 (z -2.06) |
| v2 (friendship ungated; canvassing) | +7.8 pp | +0.68 | **-3.36 pp** | **-2.12** | -3.78 (z -0.83) | -49.0 (z -1.32) |
| v3 (shipped: friendship ungated; sticky partner) | -15.6 pp | -1.43 | **-3.59 pp** | **-3.29** | -3.96 (z -1.15) | -20.6 (z -0.66) |

The gene fires (`tools/gene_fires.py --max 0` green, 279 reachable genes
covered, this gene not among the eight zero-width rows).

**The honest read.** The win column swings 23 pp between versions that differ
in one rule — that is the +/-27.6 pp resolution talking, and no version's
interval excludes zero. The score-share column does not swing: **every**
version costs about 3.5 points of score share at |z| between 2 and 3.3. That
is the one signal in this probe with any consistency, and it is negative.
Pricing belongs to the continuous screen, but a reviewer should treat this
gene as "shows a persistent share cost, unexplained" and not as neutral.

Three hypotheses for the share cost, none tested here:
1. The desk runs *before* `propose_strategic_alliance` in `advanced_diplomacy`
   and its pending deal blocks the stock proposal for that partner, so the
   gene may be displacing the stock alliance rather than adding to it.
2. Unlike `propose_strategic_alliance`, the desk honours neither
   `denied_partner` (the victory-denial exclusion) nor that function's
   `rival_victory_pressure(other).progress < 82` ceiling. It can ally a
   runaway.
3. The card splice puts an *economic*-slot card at the head of the desired
   deck for the rest of the game; on a Science plan that evicts something.

## Tests

16 in `src/ai/advanced/research_alliance/tests.rs`: registry row opt-in and
toggles are twins; the science share as a ratio, including a silent empire;
the route premium's cap and its two stopping conditions, pure; every ranking
exclusion (science floor, war, denouncement both ways, grievance ceiling,
unmet, already allied) and the deliberate *non*-exclusion of Civil Service;
the culture and science threat guards, the bar, and the lane switch; ranking
by science and the friendship's weight; friendship-then-alliance with the
cool-down; the friendship leading and waiting for Civil Service; a refusal
answered by the cool-down and a released partner's place going to the
next-best; the fallback ladder by model yield, for a taken slot and for a
missing tech; the objective dropped without the alliance being broken; the
route premium reaching the real valuation and zero off; the card only while a
non-threat alliance stands; the off path inert; and a whole 250-turn
screen-size game in which the desk must actually ask somebody.

## Validation

- `cargo test --profile ci --locked` — exit 0 (3177 lib tests, 47 ignored;
  all other targets green)
- `tools/genes.py check`, `tools/eval_manifest.py --check`,
  `tools/genome_cost.py check`, `tools/gene_fires.py --max 0`,
  `tools/test_genes.py`, `tools/test_eval_manifest.py`,
  `tools/test_gene_fires.py`, `tools/test_treatment_append_points.py` — green
- `tools/rust_quality.py` — "the changed lines are formatted and
  warning-free"
- `docs/gene_ledger.json` regenerates with churn unrelated to this gene (main
  has drifted from the checked-in file), so it is left at `origin/main`.
