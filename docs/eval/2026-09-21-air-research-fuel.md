# Supplied air research and unavailable ground fuel

Investigation in progress; this proposal has not passed a paired replay.

Native run `civvis-20260921T203333Z`, pinned to `525be833e`, requested
Combustion at turn 175 while an appointed air offensive still needed Steam
Power, Flight, Radio, and Advanced Flight. At turn 179 it had learned
Combustion but still held zero Oil and received zero Oil income. The native
Cuirassier `8978444` exposed the upgrade refusal: “Insufficient Resources.
1 [ICON_RESOURCE_OIL] Oil required to upgrade this type of unit.” All cities
omitted Tank from their buildable offers. Aluminum stock was 10 with income 2.

The one observed owned Oil tile, offset `(13, 11)`, first appeared in the tile
export at turn 176. It is Coast and unimproved. Offshore Oil needs Plastics:
the installed `Base/Assets/Gameplay/Data/Improvements.xml:48` names
`IMPROVEMENT_OFFSHORE_OIL_RIG` with `PrereqTech="TECH_PLASTICS"` and
`Domain="DOMAIN_SEA"`. This is not a missed immediately legal Oil Well.

The candidate should preserve a supplied Domination air beeline when the
otherwise nearer ground upgrade requires already-revealed fuel with no stock,
no income, and no currently connectable owned source. Unknown fuel, stock or
income, repairable/legally improvable deposits, resource-free upgrades, and a
home emergency must retain the existing nearer-upgrade priority. The ordinary
wartime modernization selector need not change.

Frozen input through turn 185:
`/tmp/civvis-air-research-fuel-replay/native-events.jsonl`, SHA-256
`4e792137582233352d24ff15d3616dfb44278e542af312fc1eeacbc5f10b60b8`.
Native resource and refusal evidence is in that directory's
`native-evidence.json`. A changed research order in a recorded-board replay
would not prove an earlier completed technology, aircraft, capture, or win.
