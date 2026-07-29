# Water resource connections

Status: **implemented and locally validated; merge remains ordered behind #584**.

## Observation

An exploratory audit of the same 20 completed production final saves used by
the repair-routing study, from `20260729T130510.846199Z` through
`20260729T150656.019296Z`, found 73 active Offshore Oil Rigs and 76 active
Fishing Boats on Amber. The affected improvements appeared in 83 of 149
surviving major-civilization seat-games. In 59 seat-games, an affected water
improvement was the empire's only native improved copy of Oil or Amber that
was not pillaged.

The improvements are legal and data-defined for those resources:

- `offshore_oil_rig.resources` contains `oil`; and
- `fishing_boats.resources` contains `amber`.

The placement path already treats either an improvement's `resources` list or
the resource's single default `improvement` field as a valid match. The
`improve_resource` boost predicate does the same. Resource accounting does not:
several paths compare only with `ResourceSpec.improvement`, which is `oil_well`
for Oil and `mine` for Amber. Consequently, legal water copies are built and
shown on the map but do not provide native Oil accumulation, Amber access,
monopoly control, destination strategic-copy effects, Grand Bazaar resource
effects, or city-state resource access.

This save census measures exposure, not gameplay effect. The defect itself is
deterministic: the data admits an improvement-resource pair and the connection
predicate silently applies a narrower rule.

## Frozen semantic contract

Introduce one internal predicate for whether an improvement connects the
resource on its tile. It is true exactly when at least one term holds:

1. the improvement equals the resource's default `improvement`;
2. the improvement's data-defined `resources` list contains the resource; or
3. the improvement is an Industry or Corporation, whose resource identity is
stored by the underlying tile rather than a fixed improvement list.

A resource-bearing tile supplies a live connection only when it is not
pillaged and either is the owning City Center or has an improvement satisfying
that predicate. An absent, unknown, unrelated, or pillaged improvement does not
connect anything.

Use this same contract in every shipped accounting path that currently repeats
the narrower comparison:

- destination-city connected strategic copies;
- Grand Bazaar city luxury amenities;
- owned strategic-resource rate, including policy and Grand Bazaar additions;
- Amani Foreign Investor strategic access;
- the all-resource connected census; and
- the per-resource connected count used by access, trade, monopolies,
  corporations, and city-state propagation.

No resource placement, visibility, unlock, tile yield, accumulation amount,
stockpile cap, trade price, monopoly threshold, Industry placement rule, or AI
policy changes. The broader predicate must not make a mismatched improvement
connect a resource merely because both exist on one tile.

## Verification contract

Focused regression tests must establish all of the following before merge:

- an active Offshore Oil Rig on owned Oil counts as one connected copy and
  supplies the stock three Oil per turn;
- an active Fishing Boats improvement on owned Amber counts as one connected
  luxury copy;
- pillaging either alternate removes the connection and repairing it restores
  the same connection;
- stock default improvements retain their existing behavior;
- an improvement whose `resources` list does not contain the tile resource
  remains disconnected; and
- the one-pass connected census and the per-resource counter agree for the
  alternate improvements.

The full release suite must remain green. This is a deterministic rules fix, so
no randomized gameplay batch or outcome threshold can validate it more directly
than the data contract and executable regression tests. Production outcomes may
be observed after deployment but cannot rescue a failing correctness test.

## Ownership and ordering

This task claims only `src/game.rs` and this document. The path overlap with
#584 is explicitly coordinated: that PR changes one visibility line in a
distant section and owns an already-frozen evaluator. This fix must not merge
ahead of or alter #584's measurement boundary without explicit coordination.
It starts no simulator process and consumes no place in the heavy-job queue.

## Implementation and validation

Commit `01d302f` introduces a single improvement/resource predicate and a
single live-tile wrapper, then routes each preregistered accounting endpoint
through them. The production change is confined to `src/game.rs`; placement,
unlock, yields, accumulation amounts, AI, and simulator code are unchanged.

The regression surface now checks the complete frozen contract:

- every `ImprovementSpec.resources` pair and every nonempty stock
  `ResourceSpec.improvement` pair satisfies the shared predicate;
- unrelated improvements remain disconnected, while Industries and
  Corporations preserve their existing tile-resource identity;
- Offshore Oil Rigs and Amber Fishing Boats agree between the one-pass census
  and the per-resource counter;
- active, pillaged, and Builder-repaired states affect native access, empire
  Luxuries, destination strategic copies, and stock Oil accumulation exactly
  once, without spending a Builder charge;
- Grand Bazaar Luxury amenities and strategic accumulation use the same live
  connection, including Resource Management and Corporate Libertarianism
  additions; and
- the existing Amani/Puppeteer runtime test now exercises an Offshore Oil Rig,
  covering Foreign Investor and ordinary Suzerainty propagation end to end.

Local release validation on the implementation commit and its documentation
follow-up:

- `cargo test --release --locked --lib strategic_resource_tests -j1`: 8
  passed, 0 failed;
- `cargo test --release --locked --lib
  amani_executes_messenger_resources_puppeteer_and_emissary -j1`: 1 passed,
  0 failed; and
- `cargo test --release --locked --lib -j1 -- --test-threads=1`: 1,173 passed,
  0 failed, 15 intentionally ignored.

`git diff --check` is clean, and a static scan finds none of the replaced
default-only predicates in the preregistered call sites. No randomized batch,
frozen evaluator seed, or additional simulator process was used.
