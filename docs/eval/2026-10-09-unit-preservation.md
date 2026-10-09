# Unit preservation candidate

Request: anticipate enemy replies one or two turns ahead, withdraw valuable
units, and let them heal instead of repeatedly buying or producing replacements.

`unit-preservation` is an opt-in candidate. It validates the complete ordinary
turn before the observed-player adapters receive its orders. The live finishing
volley uses the same policy before applying its early attacks. The public default remains off; the native deployment explicitly enables this
policy after its recorded-history and runtime checks.

The reply budget counts every visible hostile source once at its upper combat
roll. Later blows are priced at the defender's reduced health. Friendly targets
do not divide this survival budget. The next-turn escape test refreshes the
survivor's movement and projects each visible enemy's possible movement followed
by its second-turn strike reach. A unit must survive the first reply with a
health reserve and have a reachable stand that survives the following reply.

Unsafe unit sequences are removed to a fixed point, because removing a friendly
kill restores its victim's threat to other units. Favorable sampled kills are
also withheld from the forecast unless their lower damage budget proves the
victim gone. Recovery orders prefer survivable stands, then lower incoming
damage and actual healing rate. Existing battle recovery memory is retained
until 100 HP and already participates in native host-ID remapping.

The ten-test candidate passed all 4,693 Rust tests in CI. The final revision
adds failed-city-capture outcomes, records the exact approach after earlier
movement, and reduces stale host preview strength by prior possible wounds.
The focused suite now has twelve tests, including an unsafe uncertain capture
and a minimum-damage capture that still reaches safety. Final-head CI is the
required integration gate. Local Python checks passed: 14 append-point tests,
21 evaluation manifest tests, and 185 gene metadata tests.

A local full-suite run on the earlier ten-test candidate passed 4,412 tests
and failed five unrelated map-setup/server assertions under heavy concurrent
load (50 ignored). The CI full suite on that same candidate passed. These local
failures are retained here rather than reported as a clean full-suite result.

A completed firing probe varied only `unit-preservation` over six games
(seeds 761004–761009), 12 independent major seats, two players, 32×22 Pangaea,
Online speed, Emperor majors, no city-states, and a 100-turn clock. Three seats
drew the policy on and nine drew it off. The committed analysis is
`docs/gene_screens/fires/unit-preservation.json`; `gene_fires.py --max 0`
passes. This small probe establishes registration/firing evidence only: it is
not a paired casualty comparison or a win-rate/deployment recommendation.

Recorded native-history replay of the ten-test release candidate covered 416
frames through turn 150. All input artifacts remained unchanged, the baseline
matched at the earliest changed complete native reply, and the replay gate
passed. Orders changed on 234 frames. Withdrawals included scouts, warriors,
and siege units. The final revision must pass the same replay before arming.
This compares decisions on recorded boards, not played counterfactual survival;
no live casualty reduction is claimed.

The earlier release replay took 131.9 seconds total: candidate decision time
104.3 seconds versus baseline 26.4 seconds. Median decision time was 45.2 ms
versus 39.6 ms, with a candidate maximum of 5.37 seconds. The total cost increase
is larger than the median increase because difficult late-game boards require
more forecasts. The native build retains observed wall-tier, defeated-player,
Food/Faith quality, and host OCR rules.

This uses observed enemies and known movement/combat rules. It does not predict
unseen reinforcements, random future hazards, or the opponent's chosen order.
Aircraft retain the immediate-reply check; their interception damage is applied
by the speculative mission and they do not use a land escape flood.
