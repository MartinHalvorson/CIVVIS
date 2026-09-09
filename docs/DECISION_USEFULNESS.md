# Choose the most useful contribution

The operator's direction is to ask, for every unit and city, **which feasible
choice contributes most to winning from this position?** This applies to all
roles and all production categories. Activity, damage, yields, unit counts,
and score are intermediate measures; none is the objective by itself.

## Decision contract

1. Name the empire need this unit or city can satisfy. Consider survival,
   expansion, economic growth, information, and the current victory path.
2. Compare feasible alternatives, including continuing the current task,
   waiting, healing, guarding, and producing an enabling investment. A
   locally available action does not get priority merely because it is first
   in the dispatch code.
3. Value the **additional** benefit over what already exists or is already
   committed. Count other units, queues, reservations, and diminishing returns.
   When comparing replacements for one queue, exclude that queue from both
   sides' supply baseline. Include its remaining completion cost and saved
   progress separately.
4. Price when the benefit arrives, whether it lasts long enough to matter,
   the risk of losing the unit or city, maintenance, scarce charges and
   resources, and the work forgone by committing this unit or city.
5. Allocate jointly where choices interact. An escort and civilian agree on
   an executable destination; two builders do not claim one job; several
   cities do not all answer a shortage of one unit. Prefer the assignment
   that leaves the best useful alternatives for everyone else.
6. Reconsider after material changes. Keep a commitment when the alternatives
   are effectively tied; avoid oscillation that delays all benefits. Recheck
   legality and safety against the observed board before acting.
7. Explain a decision with its intended payoff and strongest feasible
   alternative. Measure whether the predicted benefit happened. A score is
   an estimate to test, not proof that an action is optimal.

## What is implemented, and what remains

The controller is a mixture of ranked choices and priority dispatch. It is
**not an exhaustive global optimizer**. The military Objective Board and
requisition system already model shared needs, but their deployment depends
on the gene ledger's measured results. This direction is not a reason to
enable a poorly performing treatment or to relabel heuristic scores as win
probabilities.

The named victory governor now reviews occupied queues as well as idle ones.
A replacement must improve the estimated value by more than 25% (or the
explicitly configured margin); economic recovery and survival overrides keep
their existing authority. It compares city production against the empire
without the reconsidered city's queue, refreshed after each city's orders.
Builder job rankings subtract the value of a standing improvement. Before
spending a charge on useful local work, a builder compares the existing
travel-priced job ranking and attempts a strictly better available job.
Existing safety, repair, project-contribution, and reserved guard obligations
still run first. Route attempts are bounded; inaccessible alternatives leave
the local action available. The adaptive genome retains its screened policy.

This is a first correction to two concrete violations, not complete coverage
of the contract. Further work must explicitly compare discretionary city
reservations with alternatives, reassess pinned jobs when their payoff
changes, and extend shared allocation beyond existing escort and military
reservations. Unit classes with specialist actions need their own feasible
candidate generation, benefit estimates, and observations of outcomes.

## Evidence required for further changes

- A scenario where the old action is less useful and the replacement achieves
  a concrete benefit; test the actual dispatch path, not just a score helper.
- Cases where the apparently attractive alternative is unsafe, unreachable,
  already covered, too late, or would break a more valuable commitment.
- Deterministic ties and bounded planning cost, including large empires.
- Matched game evaluations for strategic policy changes, followed by live
  observation after deployment. Passing unit tests verifies behavior; improved
  win rate requires game evidence.

Do not claim “maximally optimized” from a higher internal score alone. The
standard is better decisions and stronger outcomes, with their limits stated.

## Initial comparison: 2026-09-09

A small matched check completed 12 pairs on fixed seeds 91011–91022. Seat 0
won **3/12 with either policy**. There were both gains and losses on individual
seeds. This is a neutral, small native comparison, not evidence of improved
win rate or of Firaxis performance. No margin was tuned against these seeds.

The baseline tree was `7ea36b7b9`; the candidate was `703841b2f`, before the
unrelated scout-role integration. Both used `Game::new(4, 32, 24, seed, 150, 0)`
and four Science-targeted major controllers. For measurement only, an isolated
candidate build applied the new policy to seat 0: the queue-census condition
required `pid == 0`, `production_review_margin` required `g.current == 0`,
and `marginal_improvement_value` and `more_useful_builder_job` returned their
old behavior for `pid != 0`. These evaluation-only gates are not deployed.
Opponents therefore retained the baseline policy in both arms.

Runs alternated baseline/candidate order by seed and continued through the
engine's winner declaration (turn 151 for a 150-turn score cap). An earlier
150-turn snapshot had not reached that declaration and was not counted as a
completed-game result. Full tests on the subsequently merged revision also
passed: 3,342 passed, 50 ignored, including nine new usefulness regressions.

| Seed | Previous winner | New winner | Seat 0 score, previous → new | Seat 0 cities, previous → new |
|---|---:|---:|---:|---:|
| 91011 | 1 | 2 | 87 → 66 | 2 → 1 |
| 91012 | 1 | 1 | 156 → 119 | 5 → 3 |
| 91013 | 1 | 0 | 116 → 153 | 2 → 4 |
| 91014 | 0 | 0 | 138 → 160 | 4 → 4 |
| 91015 | 2 | 2 | 69 → 86 | 2 → 3 |
| 91016 | 1 | 1 | 113 → 115 | 3 → 4 |
| 91017 | 0 | 3 | 150 → 93 | 4 → 2 |
| 91018 | 3 | 3 | 115 → 104 | 3 → 4 |
| 91019 | 0 | 0 | 179 → 186 | 4 → 4 |
| 91020 | 3 | 3 | 111 → 141 | 3 → 4 |
| 91021 | 1 | 1 | 131 → 101 | 3 → 2 |
| 91022 | 1 | 1 | 114 → 123 | 3 → 5 |

Winner IDs are zero-based. Reproduction harness (link against each policy's
`cargo build --profile ci --locked --lib` output):

```rust
use civvis::ai::{AdvancedAi, Ai, VictoryTarget};
use civvis::game::{Action, Game};
fn main() {
    let seed: u64 = std::env::args().nth(1).unwrap().parse().unwrap();
    let mut g = Game::new(4, 32, 24, seed, 150, 0);
    let mut fleet = AdvancedAi::fleet(&g);
    for pid in 0..4 { fleet[pid] = AdvancedAi::targeting(VictoryTarget::Science); }
    let mut turns = 0;
    while g.winner.is_none() {
        let pid = g.current;
        fleet[pid].take_turn(&mut g, pid);
        if g.winner.is_none() && g.current == pid { g.apply(pid, &Action::EndTurn).unwrap(); }
        turns += 1;
        assert!(turns < 100000 && g.turn <= 152);
    }
    let seat_score = g.score(0);
    let seat_cities = g.player_city_ids(0).len();
    let cities: usize = (0..4).map(|p| g.player_city_ids(p).len()).sum();
    let score: i64 = (0..4).map(|p| g.score(p)).sum();
    let production: f64 = (0..4).flat_map(|p|g.player_city_ids(p)).map(|id|g.city_yields(id).production).sum();
    let improved = g.map.tiles.values().filter(|tile|tile.improvement.is_some()).count();
    println!("seed={seed} turn={} winner={:?} seat_score={seat_score} seat_cities={seat_cities} cities={cities} score={score} production={production:.1} improved={improved}",g.turn,g.winner);
}
```
