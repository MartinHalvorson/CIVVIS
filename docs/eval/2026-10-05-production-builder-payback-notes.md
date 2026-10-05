# Rejected early worked-production Builder investment

The previous Builder reservation repair corrected a diagnosed same-frame
handoff, but all eight fresh paired games were byte-identical. It did not
raise average early production. This experiment tested an opt-in reservation to train an idle city
a Builder when its currently worked, unlocked productive jobs forecast enough
production to repay the actual remaining cost.

Both prototypes failed coverage and their runtime API, policies, diagnostics,
and tests were removed. The final native source is byte-identical to its
merged main baseline. The historical probe intentionally references the removed
API and reproduces the experiment using the archived prototype libraries; it
is not a probe for the final main library.

During evaluation the candidate was opt-in through
`enable_builder_payback_reserve`; production defaults were unchanged. Existing named-lane workforce
limits are retained. Nearby active charges cover jobs first and another
queued Builder prevents a second reservation. Two distinct uncovered worked
land jobs are required, in a city below the existing 8-production foundation.
The forecast uses the ordinary router's preferred legal improvement and
`Game::improvement_yield_change`, including standing improvement yields and
feature removal. Jobs that lose food are excluded. It credits at most the
new Builder's native charge count, includes build time plus local travel and
operation delays, and discounts production by 25% for uncertainty.

The investment window is 80 standard turns (40 Online), bounded by the actual
remaining clock. Investment ends after 160 standard turns (80 Online).
Research, local defense, growth, trade income, and amenity reservations retain
priority. A qualified unstarted Builder survives a later frame through the
same investment forecast; explicit refusal and siege handling retain priority.

## Frozen evaluation

The protocol and probe were written before treatment play. Control and
candidate used the same frozen normal optimized CI-profile binary; only
the candidate opts in. Four Emperor and four Deity pairs screen the candidate
before the separate sixteen-pair-per-difficulty confirmation. Primary is
turn-75 production, with death or an early end counted as zero. Confirmation
requires improvement at both difficulties without lower survival or more than
10% Science loss. No Firaxis parity claim follows from this native evaluation.

## Validation progress

- Initial fast fixture suite: four passed, zero failed; unoptimized CI override
  used only for feedback, not game comparisons or final validation.
- Strengthened feature-removal fixture: four focused tests passed, including
  two legal improvements with exactly zero net Production gain.
- Treatment append-point suite: fourteen passed.
- Prototype-one GitHub full Cargo gate passed on source `159b212a5`.
- Prototype-one local Rust quality passed for all four changed Rust files.
- Final Rust quality passed for the historical probe after runtime removal;
  final changed native-source set is empty. `git diff --check` passed.
- A new five-test fast suite passes after the shared turn-driver reservation;
  this includes a full native delegated turn with the opening already complete
  and both required Scouts present. A disabled control chooses another item.
- Final normal optimized local Cargo suite: 4,591 passed, zero failed, 54 ignored; four test threads. Native source is identical to merged main.
- Prototype-one fresh pilot: sixteen executions, all zero exit; all eight
  paired action logs and final worlds byte-identical. No worked-production
  Builder reservation was recorded. Emperor mean turn-75 Production 53.675
  in both arms; Deity 36.875 in both arms. Gate failed.
- Confirmation: unplayed.

Before native edits, all open PR file lists and each actual patch changing
advanced.rs were inspected. The reservation field/default and industrial
pre-discretionary queue hunk do not rewrite another task's hunks. The
Builder commitment expression is above #3938's separate receipt insertion;
its preservation disjunctions are unchanged here. Shared-file ownership is
recorded by the launcher in the draft PR.

## Second prototype rejected after consumed replay

The first prototype's city-local reservation only runs inside the strategic
governor. The turn driver also has a delegated route, so the revised opt-in
adds a shared reservation before either governor and retains the city-local
check for direct production previews. It respects prior emergency, strategic,
recon, and income claims, requires the existing military census to cover the
city count, and reserves at most one new Builder. Deferral reasons are recorded
in the planning journal for diagnosis. This is a routing coverage change, not
a finding that delegation caused every null pilot result.

The full-turn fixture initially still owed its opening Scout, then its second
recon Scout; those legitimate prior claims were corrected before the five-test
passing run. The initial narrow filter also executed zero tests before the exact
full test name was used. These attempts provide no successful route validation.
A missing macro import was caught and corrected before the passing suite.
All logs are retained.

Before any new fresh screen, the second prototype must change useful Builder
work on consumed diagnostic worlds. The first protocol's confirmation remains
unplayed. The first probe and normal optimized library remain frozen under
`civvis-production-evidence/2026-10-04/payback/artifacts`; all 88 raw pilot files
and their hashes remain in the adjacent `pilot` archive.


The second prototype replayed all eight already-consumed pilot maps, with
sixteen executions and zero failures. All eight paired action logs and final
worlds were byte-identical, and no reservation was accepted. Deferral buckets
showed existing city-output/alarm, workforce/insolvency, uncovered-job, and
age/recovery/threat guards excluding the investment. Each bucket groups several
conditions and does not isolate one cause. The shared driver did execute its
checks, so a passing route fixture cannot establish useful treatment coverage.

The second prototype includes the merged #3938 Builder receipt repair. Its
control world can therefore differ from the first prototype's control; those
cross-version differences are not treatment gains. Exact same-source arm pairs
remain the comparison. All 136 replay files and hashes are archived under
`civvis-production-evidence/2026-10-04/payback/diagnostic-v2`. Both native patches,
optimized libraries, dependencies, compiler commands, and probe hashes remain
in adjacent `artifacts` and `artifacts-v2` directories.

The coverage gate failed. No fresh second-prototype screen was run. The first
protocol's confirmation seeds remain unplayed. No average early-production
improvement or Firaxis parity was established. The next independent experiment
will test the public opening order to make a Builder available earlier.

Journal ring eviction counters are recorded separately from actual lost
observations: the drained thought IDs were contiguous, with no reset or
truncated-turn events. The first fresh pilot's 88 files and all sixteen exit
codes remain in its result manifest. Every assigned map was reported.
