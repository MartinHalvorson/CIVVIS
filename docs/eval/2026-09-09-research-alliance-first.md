# research-alliance-first: the alliance an Emperor handicap cannot deny us

Opt-in gene, `Kind::OptIn`, off by default. Off it is byte-identical: every
hook the stock desk and the route valuation call returns the identity
(`off_the_gene_is_inert`). Code: `src/ai/advanced/research_alliance.rs`;
tests in `src/ai/advanced/research_alliance/tests.rs`; fires probe in
`docs/gene_screens/fires/2026-09-09-research-alliance-first.json`, with
its raw rows and the paired off-run beside it (`.jsonl`).

**This round is two things.** A gene, and a diagnosis of why its first
version cost 3.5 points of score share — a cost that turned out to be two
unrelated things stacked, one of them the screen's own seat draw. Read the
diagnosis before pricing anything alliance-shaped.

## The alliance model, as this simulator actually has it

Every claim below is a line in this repository, not a memory of Civ VI.

- **Kinds are strings**, `research | cultural | economic | military |
  religious` (`src/game/actions.rs:8819-8822`), state `AllianceState { kind,
  points, level, ends }` (`src/game.rs:3766-3772`), held in
  `Player.alliances: BTreeMap<usize, AllianceState>` — **one alliance per
  partner**, of at most one kind each per empire.
- **Legality** (`Game::do_propose_deal`, `src/game/actions.rs:8774-8841`): a
  valid kind, `civil_service` on **both** trees, the kind free on both sides,
  peace, no denouncement, and for `research` only
  `tree_effect(pid, "research_agreements") > 0.0` on both — the tech
  `scientific_theory` (`data/tree_effects.json:20`).
- **A declared friendship is NOT an engine prerequisite.** Nothing in
  `do_propose_deal` reads `friends_until`. The stock Advanced desk bundles
  `friendship: true` into the alliance proposal itself.
- **Points and levels** (`Game::process_diplomacy`,
  `src/game/actions.rs:11034-11112`): `+1.0` a turn, `+0.25` per route each
  way, `+0.25` a side for Democracy/Wisselbanken; level 2 at `80.0`, level 3
  at `240.0`. One route each way is up to 50 percent faster levelling.
- **What Research pays.** Level 2: `share_research_alliance_boosts` every
  `standard_duration(30)` turns (`src/game/actions.rs:8042`). Level 3:
  `sci += 10%` of the ally's city science (`src/game.rs:32180-32189`).
- **What the Basic controller answers.** `src/ai.rs:8319`: a friendship,
  alliance or passage proposal is accepted whenever grievance is under 75 and
  the proposer is not 1.8× its power. The desk's proposals land.

## The design (what shipped after review)

The whole gene runs **through** `propose_strategic_alliance`, the stock
desk, so the desk's own exclusions — `denied_partner`, the
`rival_victory_pressure(other).progress < 82` ceiling, grievance under 75, an
open proposal either way — apply unchanged. On:

1. **The kind is Research**, whatever the grand strategy names, while
   `research_alliance_lane` says one is worth waiting for: none held, and
   level 2 still reachable before the turn limit
   (`level_two_reachable`: `80 / 1.5` = 54 turns left). While our own tree
   lacks `scientific_theory` the desk **waits** — proposes no other kind — so
   the best partner's one slot is not spent on a cultural or economic
   alliance that would make the Research Alliance impossible with them.
   Past the horizon the stock kind resumes.
2. **The twelve-turn cadence is bypassed** for the research ask; a refused
   partner is not re-asked for `ALLY_RETRY_TURNS` (10).
3. **The partner score gains `ALLY_SCIENCE_WEIGHT` (110) × the partner's
   science share of ours.** The stock terms stay.
4. **A culture threat is barred** (the proposal bundles passage, +25 percent
   tourism against us).
5. **The first route to the research ally carries `ALLY_ROUTE_PREMIUM` (30)**
   below level 3, capped to `ALLY_ROUTE_PREMIUM_OPENING_CAP` (10) under
   `ALLY_ROUTE_OPENING_BAND_CITIES` (6). The cap is applied in
   `route_premium`, pure, and the premium reaches
   `trade_route_destination_value_from` next to the stock `45.0` first-
   connection term (`the_route_premium_reaches_the_valuation_for_a_research_
   ally_only`).

Removed from the first version, with the reason: the **declared friendship
ahead of the alliance** (the whole share cost — below); the **fallback
kinds** (they consumed the partner's only slot, which is why the first
version never once held a Research Alliance); the **sticky partner** (it was
not sticky: a seated fallback alliance released it, and the desk went on to
befriend and ally the next major, ending with three alliances of three
kinds and four friendships); the **science-threat bar at 30 percent** (a
research alliance with the science leader hands the *follower* the larger
share and the boosts — the stock 82 percent ceiling is the right bar); the
**card splice** (`market_economy` is already in the Science deck at position
8; splicing it to the head put a +2-per-route card ahead of `rationalism`);
the **minimum science share** (a weak partner is the stock desk's business).

## ⚠⚠ The diagnosis: what −3.5 pp of share actually was

The junior's probe (12 games, 36 seats, seeds 26081900..26081911) read a
score-share cost of about −3.5 pp at |z| 2–3.3 in all three versions. It was
two things stacked.

### 1. The early friendship deleted the opening's campaign target

An ablation on seeds 26081900/26081901 (seat 0 on, every other major stock,
default rung) with temporary switches inside the module:

| variant | seat 0 share (seed 900 / 901) |
|---|---:|
| off | 0.178 / 0.150 |
| full first version | 0.120 / 0.092 |
| friendships only, no alliances | 0.120 / 0.092 |
| alliances, no card, no premium | 0.120 / 0.092 |
| no early friendship | **0.178 / 0.150 — byte-identical to off** |

The friendship alone was the whole cost. A per-turn observer put the first
divergence at **turn 20**: the warrior moves `(0,29)→(−1,29)` off and
`(0,29)→(1,28)` on, the world otherwise identical. Cloning the turn-20 world,
stripping `friends_until` from one copy and replaying a *fresh stock*
controller on both reproduced the split — and printed why:

```
strip_friendship=false  plan strategy=Expansion target=None    city=None     legal3=false
strip_friendship=true   plan strategy=Expansion target=Some(3) city=Some(56) legal3=true
```

`campaign_target_legal` (`src/ai/advanced.rs:12071`) refuses a friend. The
t16 friendship with the highest-science neighbour — the nearest strong civ —
removed the opening plan's only campaign target, and the opening posture
hangs off it: the befriended seat sat at **one city until past turn 80**
(off: two at t40, three at t100, four at t150), science at t150 10.7 vs 24.2,
and ended 19 techs to 29, 12 civics to 17, 21 buildings to 44.

### 2. The rest was the seat draw

The redesigned gene, on the junior's exact probe command, still reads
**share −3.66 pp, z −1.99** unpaired. Rerunning the same 12 seeds with
`--p-on 0.01` (every seat off) and pairing each measured seat with itself:

| paired, same seed and seat, n=14 on-seats | Δ | se | z |
|---|---:|---:|---:|
| score share | **−0.56 pp** | 0.71 | −0.78 |
| techs at end | +0.57 | 1.20 | +0.48 |
| science at end | −16.5 | 21.0 | −0.78 |
| cities | −0.21 | 0.91 | −0.23 |
| wins | 2 of 14 on, 3 of 14 off | | |

The 22 off-seats moved −0.05 pp (z −0.19) between the runs. The unpaired
−3.66 is the 14 drawn seats standing ~3 pp below the other 22 *with the gene
off too* — `--p-on 0.5` draws the same seats from the same seeds in every
version, which is why three different designs all read "≈ −3.5". **A probe's
unpaired share column is not a measurement of the gene when the seat draw is
fixed by the seed**; pair it against an off-run of the same seeds.

The gene does act: 9 of the 12 games diverge between the on-run and the
off-run (26081902, 26081905 and 26081907 are identical).

## Whole-game instrument: is a Research Alliance reached at all?

`research_alliance_whole_game_instrument` (`#[ignore]`) plays the screen's
own setup — 6 majors, 74×46, 250 turns, Emperor, seats 0-2 measured with the
gene on and exempt from the handicap, seats 3-5 stock rivals with it — and
reports per major the turn `civil_service` and `scientific_theory` landed,
the first turn a Research Alliance stood, and the desk's counter.

Three seeds, nine measured seats:

| seed | measured seat | `civil_service` | `scientific_theory` | Research Alliance |
|---|---|---:|---:|---|
| 26081900 | Gran Colombia | 181 | — | — (military L1, a rival's ask) |
| 26081900 | England | 189 | — | — (military L1, a rival's ask) |
| 26081900 | **France** | 114 | **217** | **asked once, stood from t231 with Babylon, L1 at t251** |
| 26081901 | Rome / Gaul / Netherlands | 149 / 129 / 229 | — | — |
| 26081902 | Canada / Ottomans / Egypt | 130 / — / — | — | — (game ended t172) |

The rivals, playing Emperor's bonuses, reached `scientific_theory` at
t169, t174 and t195. **A Research Alliance is reached — in 1 of 9 measured
seats, twenty turns before the clock, at level 1.** On a 250-turn clock it
pays nothing: level 2 needs 54 turns with a route each way. The binding
constraint is not the desk but our own pace to `scientific_theory` (best
t217 against the rivals' t169), which is the `science_pace` axis every
science gene here is fighting. The gene's payoff, if it has one, is on the
live clock; the screen can say only that it does not hurt.

⚠ The first version's instrument (and my first rerun of it) played
default-rung, all-Advanced worlds, where `scientific_theory` reaches 2 of 18
majors by turn 250 and a Research Alliance is unreachable. That is not the
screen: the screen's Emperor rivals push a science victory at t155–t243, and
its seats hold 46–77 techs at the end. An instrument that does not mirror the
screen's difficulty measures a different game.

## Tests (`src/ai/advanced/research_alliance/tests.rs`, 14 + 1 ignored)

Registry row opt-in and toggles are twins; the science share as a clamped
ratio; the route premium's cap and its two stopping conditions, pure; the
level-2 horizon, pure; the lane (`Stock`/`Research`/`Wait`, and `Stock`
again when held or past the horizon); the desk asking for Research off
cadence whatever the plan says, with the counter and the cool-down recorded;
the desk waiting for Scientific Theory on the cadence turn where the stock
desk seats an economic alliance, and resuming the stock kind past the
horizon; the science term picking the partner with the science where stock
breaks the tie by id; a refusal answered by the cool-down and the next-best
asked meanwhile; a culture threat barred and `denied_partner` honoured under
the gene; a held Research Alliance ending the gene's asking; the premium
reaching the valuation for a research ally only, zero for a cultural one and
zero off; every hook the identity off; and the whole-game instrument.

## Validation

- `cargo test --profile ci --locked` — exit 0 (3212 lib tests, 48 ignored;
  every other target green)
- `tools/rust_quality.py --base <merge-base> --head HEAD` — "the changed
  lines are formatted and warning-free"
- `tools/genes.py check`, `tools/eval_manifest.py --check`,
  `tools/genome_cost.py check`, `tools/gene_fires.py --max 0`,
  `tools/test_genes.py`, `tools/test_treatment_append_points.py` — green
