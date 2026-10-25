# Existing-war Domination campaign handover

## Hypothesis and limits

The assigned-victory branch can choose Expansion during an existing major
war before the generic already-at-war Conquest branch is reached. Its normal
four-city handover also requires the elective-war power margin. A major war
that already exists need not be treated as a decision to open a new one.

Native Gran Colombia Emperor run `civvis-20261025T011023Z`, source
`7b6b83d0e5f9232fdaa955d00f5b493a237bf2f5`, first logs Conquest at turn 81.
It ultimately loses to team 2's Science victory at turn 190, with one city
taken and three lost. This motivates investigation, not a causal conclusion:
modernization and economic delivery also lag. The natural-speed clock cap is
already implemented; the text about the first half is not proof of a
650-turn-cap timing bug.

## Preregistered treatment

`domination-war-hands-over` is a new default-off opt-in. Only an effective
assigned Domination lane with at least the existing four-city opening band,
an already-active living major war, and no bounded-raid-only war may skip
Expansion for Conquest. Earlier Recovery, emergency, and imminent-victory
decisions retain priority. Peace, minors, barbarians, dead rivals, other lanes,
and prewar declaration/readiness thresholds remain unchanged.

The initial scaffold registers the flag and tests without adding its strategy
branch. Positive integration tests must first fail on Expansion versus
Conquest, not on compilation or fixture setup. Only valid failures count as
RED. Fixture amendments, if needed, remain unit-only and are recorded before
the behavioral hook is added.

## Validation and evaluation plan

1. Run an identical-binary recorded-history control through turn 70 of the
   native run before interpreting any replay treatment. Use the original
   immutable native binary and its actual 283 recorded forced policies.
   Require the replay tool's complete-reply/native control gate; do not
   replace a failed earliest control with a later matching frame.
2. Run the new strategy integration tests against the no-op scaffold, then
   with the hook, covering active major war, multiple wars with a bounded
   raid, peace, Recovery, small openings, minor/barbarian/dead opponents, and
   non-Domination lanes. Run existing handover tests, the full CI-profile
   suite, Rust quality, and generated-manifest checks.
3. For policy replay, use one candidate binary with the flag off/on and the
   same event prefixes. An executable wrapper may add the opt-in only to the
   treated process; preserve its text and both real binary hashes. A current-
   source untreated control must match the recorded native decisions before
   attributing a difference to the new flag. Replay measures decisions, not
   counterfactual survival or native wins.
4. Run the fixed same-binary focal-policy simulator comparison on Immortal
   seeds `59002500` through `59002503` and Deity seeds `59002600` through
   `59002603`: 16 complete executions, eight maps, both existing evaluator
   arms. Keep its four-major Gran Colombia, adaptive CIVVIS rivals (not
   Firaxis AI), Tiny Pangaea, six minors, Online, all victories, and natural
   250-turn clock unchanged. Record complete outcomes, capital/major-city
   milestones, and full focal journals. Count the new handover reason.
5. A batch with no active-major-war handovers is a coverage failure, not
   evidence of safety or strength. Do not provoke extra wars or alter the
   game conditions merely to make this probe activate. These are development
   cases, not a fresh holdout. No native strength claim is allowed without
   actual host victories under a registered experimental arm.
6. Keep published defaults, the deployment genome, forced-policy file, live
   pins, and native settings unchanged during this investigation. Any native
   experiment or promotion needs its own accurate registration and readback.
   Ship only a sound, truthfully evaluated change; do not call the broader
   highest-difficulty Domination objective complete from passing unit tests.

## Results

### Recorded-history observer control

The identical original-binary control finishes 190 frames through turn 70
in 8.07 seconds, with zero differences between its two replay arms. Request
identity matches all frames, but only 174/190 complete native replies match.
The native-history gate therefore fails and the command exits 2.

The first mismatch is turn 14, frame 0: the recorded and replayed scout routes
differ. Sixteen frames differ from native overall, all between turns 14 and
29. This is not a treatment failure or evidence for the new policy. It means
this history cannot support the preregistered native replay attribution.
Do not rescue the study by selecting a later matching frame, relaxing the
control, or interpreting a treated replay against this rejected baseline.

Original binary SHA-256:
`d04a2b39f80e9f87d36b3e5586d256cebcae8c44959518907f189e2747de09af`.
Recorded 283-policy force-file SHA-256:
`d7ac5367dda9dc54093ebc55f59d3630c908c577fe84309bbe5378cecec3de55`.

No-op integration tests and subsequent policy evaluation remain pending.
No new default selection, earlier capture, or win-rate improvement is claimed.
