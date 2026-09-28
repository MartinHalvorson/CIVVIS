# Regional loyalty governor candidate — 2026-09-28

Status: experimental, default off, not demonstrated to improve whole-game outcomes.

## Recorded loss

`civvis-20260928T032649Z/events.jsonl` (SHA-256
`e49a8d3b4b8eecb57c8522cbed4c738dae1715f56aed1935757ccd13fcb46534`):
Guayaquil, native city 327684, rebelled on turn 120 without city or wall damage.
It already had Magnus and a Monument. Titles arrived separately at t102 and
t108 and funded Pingala Researcher and Connoisseur. The empire left its Dark
Age at t121. Holding observed changes fixed, adding four loyalty from t111
would leave about 33.67 at t120; that sensitivity calculation is not a native
replay or evidence of later survival.

## Decision replay that exposed the integration defect

Binary built from `e17569a46`; source log prefixes retain first decision frames,
not the later frames after the title was spent. Replay reads snapshots and
prints orders; no orders are sent to Civilization VI.

| Case | Control | Regional candidate at e17569a46 |
| --- | --- | --- |
| t102 frame 0, source through line 7735 | Promote Pingala Researcher | Journal says save; outbound order appoints Liang |
| t108 frame 0, source through line 8392 | Promote Pingala Connoisseur | Journal says save; outbound order appoints Liang |
| t108 with the earlier title restored and Researcher removed | Promote Pingala Researcher | Appoint Victor in Caracas (native city 131073) |
| t108 after a simulated successful Victor assignment to Caracas | Not evaluated | Promote Victor Garrison Commander |

The two modified t108 cases hold all other observed state fixed, including
research previously gained. They are decision counterfactuals, not replays of
the intervening native turns or evidence that native operations succeeded.

The governor-only unit tests passed while the actual pipeline still consumed
the saved title in BasicAi's ancillary research pass. A regression through
`AdvancedAi::take_turn` failed with zero titles retained instead of one.
Therefore the reservation must also suppress that fallback governor spending.

## Replay after the fallback correction

Rebuilt revision `96f6ff3d0f69005ee9e4cb565b36cac0d0e766a6`; executable SHA-256
`013437c1f2b210cbfe585afe5221e9f1891c057b5a34e3c8c5bfa55c3c01308f`. All eight case/arm invocations exited successfully.
The controls still promote Pingala. With the regional gene enabled:

- Observed t102 and t108 first frames emit **no governor order**: the first
  title is now retained instead of being consumed by a fallback Liang appointment.
- The t108 saved-title counterfactual appoints Victor in Caracas (131073).
- The simulated post-appointment t108 frame promotes Garrison Commander.

This verifies the controller's complete offline decision/output path on the
recorded inputs. It does not verify native operation execution or subsequent
city survival. No live game was modified by these replays.

## Native governor evidence

Installed Gathering Storm `DLC/Expansion2/Data/Expansion1_Governors.xml:249-250`
connects Garrison Commander to `PRESTIGE_IDENTITY_PRESSURE_TO_DOMESTIC_CITIES`;
lines 873-880 specify `Amount=4` and `DomesticCities=true`.

`civvis-20260927T225002Z-cont5/events.jsonl:31` reports Online speed and Gathering
Storm; line 36 reports Victor's `turns_to_establish=3`. SHA-256:
`860a26e2996f8a601c6d36493d79722d55dce90c85b1197ae675f40692be24c8`.

The tooltip in `DLC/Expansion2/Text/en_US/Expansion1_InGameText.xml:533` says
other cities within nine tiles. The simulator explicitly includes Victor's own
city. Two native, same-turn promotion transitions support the inclusive model:

- `civvis-20260927T225002Z-cont4`, lines 10275 -> 10363, t154: Victor remains
  established in Panama (131073), gains Garrison Commander, and that city's
  loyalty rate rises 29 -> 33. Its other changed exported city field is the
  predicted religion-conversion timer. Source SHA-256:
  `b72851012c90098a8a459de5e6add95f96f640b590e53420f7abecbdda664884`.
- `civvis-20260927T203347Z-cont3`, lines 12311 -> 12425, t236: Victor remains
  established in Bogota (65536), gains the promotion, and loyalty rate rises
  29 -> 33. Other exported city changes are culture yield and defense. Source
  SHA-256: `5240ced524d6231db92a18b3ecf6ebe67293c7283aeeb1b05685417c2f0439d2`.

These were natural transitions with policy changes in the same frame, not
isolated interventions. Neither changed policy set introduces a loyalty card;
city amenities, population, buildings and current religion stay unchanged.
Retain the existing inclusive simulation rule; do not reverse it from tooltip
wording alone. This PR changes no simulation mechanic.

## Remaining validation

The focused suite now passes all nine tests, including a complete turn retaining
the reserved title and a subsequent controlled-player turn releasing it after
pressure recovers. The ancillary pass continues its other work and suppresses
only fallback governor spending while the reservation is active. The 191 existing
BasicAi tests also pass (two ignored), and all three registry append-point tests
pass. The rebuilt mirror replay verifies the outbound orders as recorded above.

A six-game exploratory simulation probe is running from code revision
`96f6ff3d0`, with only this gene varied, seeds 38450000–38450005 and three workers.
The small probe is not evidence of a win-rate improvement.

Natural firing evidence, opportunity-cost evaluation, generated gene metadata,
the full suite, and integration checks remain pending. No live or simulation
win-rate improvement is established by the focused tests or these replays.
