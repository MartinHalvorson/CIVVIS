# Neutral forward-airfield capabilities

Computer: MacBook Pro

The Native Bomber campaign in `civvis-20261015T120650Z` fielded two Bombers
at turn 155 while the closest existing base stood twelve hexes from known
Solokha. The visible, empty, flat, resource-free neutral plot (24,21) stood
three hexes from an owned city and ten from Solokha. A new Airstrip there
could improve nominal reach, subject to construction, travel, safety, fuel
and actual native legality. No Engineer was fielded or offered for production
in that history; the Armory prerequisite is material. No counterfactual city
capture or win is claimed.

A read-only census of that world, the prior `090922Z` pilot and the unrelated
`101527Z` domination victory finds 82 city-turns with such geometric sites.
Only the winning history offers an Engineer in those cases. The census
filters current visible empty flat land within eight hexes of a city; it is
not a path, placement quote, costed infrastructure plan or independent
strength sample. Existing zero-damage air previews include successful
pillage: the turn-154 Kurgan at (32,20) becomes pillaged in native frame 3.
They are not evidence that air transport failed.

The installed shipped rules are explicit. Base
`Assets/Gameplay/Data/Improvements.xml:50` declares Airstrip
`CanBuildOutsideTerritory="true"`, `AirSlots="3"` and `TECH_FLIGHT`.
The Gathering Storm `CityBannerManager.lua:2800` reads
`plot:GetImprovementOwner()` separately from territorial ownership; lines
403-409 count the Airstrip's aircraft and slots. Current `valid_improvements`
returns before considering a tile without an owning city. Current
`air_capacity_at` counts Airstrips only through that city. Native plot exports
carry territorial owner and purchase city but omit improvement ownership.

The first checkpoint adds two production-path controls without the repair:
an Engineer can construct an Airstrip on neutral flat land, and a mirrored
neutral Airstrip whose native improvement owner is local player 1 supplies
three slots to modeled seat 0. Fixture preconditions distinguish that owner
from land ownership and from native-to-model player mapping. Baseline
`c178eaa6b` compiles all targets locally. GitHub `38802849666` reaches the
Engineer neutral-placement assertion after its fixture preconditions and
fails it. The job passes 3,749 other tests and stops early at 3,750/6,082;
the mirror ownership control has not run in that fail-fast job. Its synthetic
PR merge includes the concurrent frontier-walls change. Neither a complete
baseline suite nor two executed red controls is claimed.

The repair carries `CanBuildOutsideTerritory` from all six stock Gathering
Storm improvement rows. A neutral plot can offer only an improvement with
that permission; ordinary Builder work, Missile Silos, foreign territory,
stale city references, unsafe unattributed native ground, terrain, resources,
features, unlocks, unit capabilities and charges retain their guards. Building
records improvement ownership without inventing an owning city. Active neutral
Airstrips supply three slots to their known owner; owned legacy Airstrips keep
their territorial fallback. Missing owner observations grant no neutral base.

Native `Plot:GetImprovementOwner()` is exported as `io`, cached per viewer
only while visible, and included in tile delta identity. The mirror maps
native owner IDs to modeled seats separately from territorial ownership and
clears stale owners. New controls cover construction and rebase capacity,
foreign/unknown owners, ownership changes, pillage, removal, save compatibility
and hidden-state redaction. The new field defaults to absent on older saves.
Existing exhaustive plot fixtures initialize `io: None`.

All 107 mod scripts parse under Lua 5.1.5 and all 102 discovered mod tests
pass, including the new export/fog/delta controls. All 30 fidelity-tool tests
pass. Both the original and expanded improvement audits against installed
Gathering Storm data report zero divergences. The compiled cache on this
host currently lacks Gathering Storm rows and is not used as that reference.
Full Rust validation, engine soak, cost measurement and native execution
remain pending. No planner or default selection is promoted in this change.

The intended continuation carries the shipped outside-territory rule and
improvement ownership through placement, execution, saves, native export,
mirroring and airbase capacity. A separate strategy experiment must cost the
Armory/Engineer chain, find a safe reachable site, execute construction and
show native rebase/use before making a conquest-strength claim. Global play
pins, verification policy and force-on selections remain untouched.
