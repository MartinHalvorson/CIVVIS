# A holding Inquisitor can defend its post

Native verification game `civvis-20261009T064953Z` ended in a Religious
defeat at turn 92. Its recorded revision was
`05d4067de91e613346e7bb5fdad5771e67245df0`, with executable SHA-256
`a052864f0426154acedaa06f66c428cac8a0e544e3af79f45604bb4bd5d3a9a0`.
The fixed seat was Emperor, four players, Tiny Pangaea, Online, Gran Colombia,
all native victory types enabled and a domination target. Settings and the
production retirement rule remain fixed.

At turn 90, native Inquisitor `3407900` stood in Cuenca with 100 HP and all
three charges. Cuenca was the sole faithful Shrine city from which replacement
religious units could be purchased. The reconstructed first frame offered two
legal theological attacks: an adjacent Missionary and an adjacent Apostle.
All three actual controller frames issued no action for this defender. The
fresh controller also issued none. These are decision observations, not proof
that a particular attack would have prevented defeat.

`advanced_religious_step` returned immediately when `inquisitor_veto_step`
returned `Some(false)`. Holding a source or finding no worthwhile cleansing
target therefore prevented the common theological combat selection below it
from running. The repair lets that existing selection run before returning
the hold. A successful cleansing or movement still returns immediately. The
existing health threshold and wounded-target exception are retained, and a
held defender does not proceed to the later movement or hunting logic.

The host executes theological combat through the ordinary attack path.
`Base/Assets/Text/en_US/Civilopedia_Concepts_Text.xml:636` says to attack one
religious unit with another. The shipped melee input path is
`Base/Assets/UI/Civ6Common.lua:152-163`; our existing adapter uses that
`MOVE_TO` operation with its attack modifier. No host mechanics or adapter
operation changes are part of this repair.

Four regressions cover a healthy guard attacking without leaving its post or
spending a charge, the existing wounded-unit hold against a fresh enemy,
the wounded-target exception, and cleansing taking precedence over combat.
The unchanged native controller's red run and the corrected candidate's
validation are recorded in the PR before readiness. Native actuation and
winning effect require subsequent fixed-seat verification. No win-rate gain
is established by a replay or unit test.
