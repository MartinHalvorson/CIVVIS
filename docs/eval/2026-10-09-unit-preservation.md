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

Validation is in progress. Focused regressions cover concentrated fire, a
survivable first reply inside an inescapable second-turn trap, an open escape,
an executed withdrawal, full-health recovery, safe finishing kills, and the
real turn entry point. No live casualty reduction or performance claim is made
yet. Full tests, a runtime cost measurement, and native replay/deployment
verification remain required before completion.

This uses observed enemies and known movement/combat rules. It does not predict
unseen reinforcements, random future hazards, or the opponent's chosen order.
Aircraft retain the immediate-reply check; their interception damage is applied
by the speculative mission and they do not use a land escape flood.
