# Unit preservation candidate

Request: anticipate enemy replies one or two turns ahead, withdraw valuable
units, and let them heal instead of repeatedly buying or producing replacements.

`unit-preservation` is an opt-in candidate. It validates the complete ordinary
turn before the observed-player adapters receive its orders. The live finishing
volley uses the same policy before applying its early attacks. The default
controller remains the paired control until validation and deployment finish.

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

Validation is in progress. The first candidate passed all 4,691 Rust tests
in CI, including its eight preservation regressions. The final candidate adds
correct pre-clamp lower damage rolls, uncertain host-kill coverage, shared
movement forecasts, a reserve on the second reply, and retreat ranking on the
board after vacating the origin. Its ten focused tests and full CI rerun remain
pending. Local Python checks passed: 14 append-point tests, 21 evaluation
manifest tests, and 185 gene metadata tests.

A completed firing probe varied only `unit-preservation` over six games
(seeds 761004–761009), 12 independent major seats, two players, 32×22 Pangaea,
Online speed, Emperor majors, no city-states, and a 100-turn clock. Three seats
drew the policy on and nine drew it off. The committed analysis is
`docs/gene_screens/fires/unit-preservation.json`; `gene_fires.py --max 0`
passes. This small probe establishes registration/firing evidence only: it is
not a paired casualty comparison or a win-rate/deployment recommendation.

No live casualty reduction is claimed. Native recorded-history replay and
runtime/deployment verification remain required. The native build retains its
observed wall-tier and defeated-player rules before the forecast and uses the
same explicit host-rule settings when replaying accepted actions.

This uses observed enemies and known movement/combat rules. It does not predict
unseen reinforcements, random future hazards, or the opponent's chosen order.
Aircraft retain the immediate-reply check; their interception damage is applied
by the speculative mission and they do not use a land escape flood.
