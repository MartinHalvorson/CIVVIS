# Inquisitor cover for the final religious purchase source — experiment

Native run `civvis-20260921T221854Z-cont1` lost by religion to Cree at176.
Cuenca was the only city with a completed Holy Site. It followed Buddhism
at172–173, became neutral at174 and Hindu at175. The Inquisitor bought at171
left Cuenca, fell to14HP, and used a charge at Popayán. At175 the empire
held143Faith but had lost its Buddhist religious purchase source.

The existing veto pass sends an Inquisitor toward the largest foreign-pressure
share, without valuing replacement-unit production. A regression fixture
reproduces departure from a sole faithful purchase source while a charged
foreign Missionary approaches. The old code fails the position assertion.

The prototype assigns one charged own-faith Inquisitor to the sole remaining
faithful active Holy Site with a Shrine or Temple when a foreign charged
spreader is within four hexes. If no faithful source remains and there is only
one active source, it prioritizes restoration there. A healthy nearby defender
gets the assignment; other Inquisitors retain ordinary conversion cleanup.
Existing immediate Remove Heresy actions retain priority. No passive spread
blocking is assumed: holding position preserves the defender for subsequent
removal/restoration actions. Other victory lanes and religious-victory-disabled
games retain the existing behavior.

Focused tests and native paired replay are pending. This is not evidence of
an actually saved source, prevented religious victory, or Domination win.
Another author's PR3701 improves emergency Missionary purchases earlier in
this recording; this experiment changes Inquisitor movement instead.
