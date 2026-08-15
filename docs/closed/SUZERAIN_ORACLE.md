# Suzerainty oracle causal contract

Status: **contract correction frozen before implementation and before any new
Suzerainty oracle map is run**. This changes a diagnostic oracle, never a
shipped controller or game rule.

## Historical result and the contract error

The merged Suzerainty oracle raised the granted seat from 22.7% control wins to
56.7% over 300 confirmation cells after a 100-cell screen. That remains strong
evidence that the bundle of Envoy acquisition, allocation, threshold yields,
and city-state control can matter. It does **not** isolate allocation quality.

`Grant::Suzerain` did not conserve the focal empire's Envoy stock. At every met
city-state where the focal effective count was below the strict-lead target, it
wrote a larger count directly into the focal player's raw Envoy table without
decrementing `envoys_free` or removing a placement elsewhere. The grant
therefore created a resource and allocated that resource perfectly. The later
post-action census could not close the allocation half of that bundle because
it sampled after `advanced_envoys`, whose loop spends the free pool by
construction. The conserved-stock observer on draft PR #609 is the separate
instrument for that question.

The historical implementation also mixed raw and effective counts when Amani
was established in the target city-state. It computed the required target from
each rival's effective `envoys_at`, then stored that effective target as the
focal **raw** count. The engine subsequently added Messenger's virtual Envoys
and applied Puppeteer again. A target of six could therefore become eight with
Messenger or sixteen with Messenger plus Puppeteer, even though the source
comment promised the minimum strict lead. This can grant extra 1/3/6 threshold
yields and makes the exact historical effect size an intentionally generous,
but mislabelled, compound ceiling.

No historical seed will be reinterpreted as if it used the corrected code. A
future causal estimate from `Grant::Suzerain` must use a disjoint prospectively
registered seed and identify the corrected source commit. This task starts no
simulator while the Strategic Expansion oracle owns the shared capacity.

## Corrected grant

The grant remains deliberately cheating and is labelled accordingly. It asks
what the whole acquisition-plus-allocation outcome could be worth; it is not a
playable policy and is not a conserved-stock allocation treatment.

At the start of the focal major's turn, for each met, living city-state:

1. Read every living rival major's **effective** representation through the
   engine's `envoys_at` method.
2. Set the required effective target to `max(3, best_rival + 1)`.
3. If the focal seat already reaches that target, do nothing.
4. Otherwise find the smallest nondecreasing focal **raw** count whose engine
   effective count reaches the target. This search must use `envoys_at`, not a
   private copy of Messenger or Puppeteer arithmetic.
5. Change only that focal raw placement. Leave `envoys_free`, rival placements,
   Governors, Gold, Faith, and every other game field untouched.

Because an effective multiplier is discrete, the minimum reachable effective
count can occasionally exceed the target; the grant may not add another raw
Envoy beyond that minimum. The oracle records both changed city-states and the
number of raw Envoys it created so future reports cannot call the intervention
resource-free.

## Deterministic validation

Focused tests must establish all of the following before the correction is
considered complete:

- without Amani, a rival effective count of five produces exactly six focal
  raw/effective Envoys;
- established Messenger uses two virtual Envoys, so the same target requires
  four raw Envoys rather than six;
- established Messenger plus Puppeteer reaches an effective six from one raw
  Envoy and is not written as six raw / sixteen effective;
- the focal empire controls the city-state after every firing;
- `envoys_free`, every rival raw placement, Gold, and Faith remain identical;
  and
- the raw-Envoy provenance counter equals the actual increase in focal stored
  stock.

The full locked CI suite and the repository's final integration checks remain
required before landing. Merge order is #584 first, then this correction on
latest `main`; their edits occupy separate grant branches of `src/oracle.rs`.
