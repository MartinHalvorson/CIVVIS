# Overwhelming conversion: fighting the war the agent already declared

Status: **three default-off live-bridge treatments (`war-economy`,
`war-reinforcement`, `war-patience`), shipped with mechanism evidence;
outcome measurement pending on the live Settler ladder**. The tournament
controllers, the `advanced` entrant, and the frozen `advanced_v1` anchor are
untouched: every new branch short-circuits on a flag only
`enable_live_bridge` sets, and the source-contract re-pin in `src/main.rs`
records why the anchor's decisions are byte-identical.

## The live defect

The 2026-08-07/08 ten-game Settler campaign solved survival (4/4 full-length
after the garrison treatments) and lost every game on score at the 250-turn
cap — 639 against 1317 on the first complete game, 457 at three cities on
`civvis-20260808T033223Z`. Across the fifteen retained live logs of that
window the ledger records **four war declarations and zero city captures**.
The games are decided on score, captures are the largest single score swing
available (the taker gains a city and the loser loses one), and it is the one
lever never pulled.

This is not the refuted aggression lane. `docs/RUSH.md` and
`docs/closed/WAR_TIMING.md` measured *creating* wars — ancient rushes and appointed
midgame timing attacks — and rejected them on wins. The live agent has the
opposite problem: it reaches its own Conquest posture and its own declaration
(`plan strategy=conquest` held for 20 assessments of the last full game) and
then fails to fight the war it started. Three separate mechanisms fail it, in
sequence:

1. **The war is fought on a peacetime economy.** The production routing sends
   Recovery, targeted lanes, and appointed `WarPlan`s through
   `advanced_production`, but an *adaptive* Conquest plan falls through to the
   Basic governor: army target `mil_per_city × cities` (≈1.4 per city on the
   deployed g56-48 genome), no enemy weighting, no siege appetite, none of the
   wartime production bonuses. `docs/RUSH.md` §"measured trap" proved the
   routing from the other side: raising the army inside `production_value`
   was twice byte-identical to a no-op, because the plan never reaches it.
2. **Reinforcements never reach the front.** `campaign_staging_step`
   assembles the army on the objective's 3..=5 ring *before* the declaration
   and then deliberately stands down. From then on force groups are cliques
   at `command_radius`, so fresh production and released garrisons form one-
   and two-body groups at home whose local strength at the objective can
   never clear `LOCAL_SUPERIORITY_FLOOR`. They hold forever. Measured on
   `civvis-20260808T033223Z`, t217–t225: land forces of one, two and three
   against the same objective, every one journalled "too weak locally to
   advance", while the empire fielded 10–14 units.
3. **A slow siege sues itself out.** Fatigue (war age ≥ 24 with no campaign
   progress in 12 turns) both offers peace and accepts any white peace at
   +320 — a stalled wall-grind reads identically to a lost war — and peace
   sets a 30-turn re-declaration lockout. The measured live pattern is one
   declaration per game and no second attempt.

## The treatments

All three are flags on `AdvancedAi`, off by default, set by
`enable_live_bridge`, each with its own `live_without_*` evaluator arm and
`--without` tag so the live harness can hold any one off against the deployed
composite.

- **`war-economy`** adds one arm to the production routing: an adaptive
  Conquest plan reaches `advanced_production`, exactly the shape adaptive
  Recovery already has (`delegated_cities` still fills the queues the war
  path leaves alone).
- **`war-reinforcement`** adds `wartime_reinforcement_step`: a unit whose
  group is standing still (Hold/Muster) anchored more than
  `REINFORCEMENT_FRONT_RADIUS` (8) from the campaign objective routes to the
  objective's 3..=5 staging ring — the same road, and the same ring, the
  pre-war assembly uses. It declines, in order: any threatened home city
  (empire-wide stand-down; the homeland keeps first claim), the front group
  and any group already moving (their measured posture logic is untouched),
  a unit in enemy contact (the tactical layer owns it), and a unit below
  `withdraw_hp` (it heals where it stands). Groups are rebuilt from the
  board every turn, so an arrival merges into the front clique and takes its
  orders. This deliberately does **not** touch the measured-and-rejected
  levers: postures are not bypassed (`docs/RUSH.md` — ignoring
  `relieving`/`Muster` made captures *fall* 9/12 → 6/12), `focus_target` is
  not pinned, and the front group is never ordered onto the ring.
- **`war-patience`** scopes the fatigue clause: while the empire holds
  `OVERWHELMING_WAR_RATIO` (2.5×) empire-wide power over its own campaign
  target, the stall clause stands down on both the offer and the accept side,
  and the existing denied-partner guard (−260) keeps refusing the target's
  white peace. Everything else about peace keeps its shape: the outmatched
  trigger (power < 0.62× theirs), the Recovery trigger, and fatigue against
  any player that is *not* the campaign target are untouched, so the moment
  the advantage is gone, so is the patience. 2.5 sits far above the 1.32
  elective-declaration ratio: patience is only ever extended to a war the
  declaration logic already called a walkover.

## Why this is not the refuted lane

Every prior aggression arm *created* wars and was rejected on wins while
winning terminal score (`advanced_rush` 47.1% paired at the live profile;
timing attacks v1–v3 all rejected; `advanced_target_domination` 0 victories
in 60 games against adaptive). The conversion trio creates no war: it changes
nothing about when Conquest is assessed or when a declaration fires. It only
makes the war the shipped gating already chose behave like a war — an army
sized for it, reinforced toward it, and not abandoned while it is being won.
On the live cell the outcome currency *is* terminal score, the one axis the
aggression arms reliably improved.

## Measurement

Headless screens cannot price these mechanisms in wins from the tournament
seat — the `advanced` entrant never carries the flags, and `live` vs
`live_without_*` selfplay historically resolves live-bridge mechanisms at
±0 because several fire orders of magnitude more often under the real
bridge (`src/bin/civvis_orders.rs` records the 20-axis composite at +9 Elo,
CI −53..+71). The evidence standard here is the one the other 28 bridge
treatments shipped under: deterministic mechanism tests (four in
`src/ai/advanced.rs`: the trio's flag contract, the routing arm, the
patience scope on both deal sides, and the rear march delivering a unit to
the staging ring), plus instrumented live runs.

Preregistered live expectations, checkable in any post-treatment run
directory:

- `why.log` shows `Reinforcing the campaign with ...` entries and the
  objective's force count rising above the historical 1–3;
- `Pressing the war on ...` journal entries replacing stall-peace offers
  while the power readout is overwhelming;
- **first live city capture** — the ledger's zero is the number this
  package exists to move — and with it the score gap at the cap;
- survival unchanged: 4/4 full-length games stays the floor, since home
  defense, garrisons, and every threatened-city path retain first claim.

If the ladder runs read no captures and no force concentration after a fair
sample, these flags come back out rather than sitting unpriced — the same
contract `district-coverage` carries.
