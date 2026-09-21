# Supplied air research and unavailable ground fuel

The paired native replay changes the intended research order at turn 175.
This is order-level evidence, not a verified earlier aircraft or victory.

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

The change preserves a supplied Domination air beeline when the
otherwise nearer ground upgrade requires already-revealed fuel with no stock,
no income, and no currently connectable owned source. Unknown fuel, stock or
income, repairable/legally improvable deposits, resource-free upgrades, and a
home emergency retain the existing nearer-upgrade priority. The ordinary
wartime modernization selector is unchanged.

Frozen input through turn 185:
`/tmp/civvis-air-research-fuel-replay/native-events.jsonl`, SHA-256
`4e792137582233352d24ff15d3616dfb44278e542af312fc1eeacbc5f10b60b8`.
Native resource and refusal evidence is in that directory's
`native-evidence.json`. A changed research order in a recorded-board replay
would not prove an earlier completed technology, aircraft, capture, or win.


## Validation and replay

Source `1459acc1b` was integrated with parent
`6448ed0bd7ecb60449f30a6ef0a19f37f9862d95` as
`63edd088ae8fca064bf356533b07dd55c0fb412e`. The baseline binary comes from
`6ef99cca6`, whose production Rust, data, and Cargo files are identical to that
parent; the only Rust difference is the additional Scout continuation test.
Binary hashes and exact input provenance are in the replay directory's
`provenance.json`.

Both sides complete all 537 frames. Exactly one internal decision changes:
turn 175 requests `TECH_STEAM_POWER` toward Advanced Flight instead of
`TECH_COMBUSTION`. The baseline first requests Steam Power at turn 179.
Exactly two exported frames change: that research order and its turn-176
receipt. The recorded next board still researches Combustion, so the candidate
receipt reports a mismatch. This is a counterfactual consequence of replaying
unchanged future boards, not an observed host rejection of the new order.
All other internal actions remain identical. Results:
`/tmp/civvis-3693-comparison.json`; replay durations 82.31/96.69 seconds are
not a controlled performance measurement.

Seven focused tests pass. The new primary case fails on the old code, then
passes with the actual research selection checked. Guard cases cover offshore
Oil without/with Plastics, stock, income, legal land deposits, repairable wells,
unrevealed Oil, absent aircraft fuel, another victory target, and a threatened
home city. The existing tests retain resource-free ground modernization.

`cargo test --profile ci --locked`: 4,124 passed, zero failed, 53 ignored
across six suites. All 14 append-point checks pass. Eight four-player soak games
complete with a 180-turn limit, seeds 369300–369307. Scoped rustfmt and
`git diff --check` pass.

The native game's second continuation later lost to the Netherlands' science
victory at turn 233, with no enemy original capital held. Its first observed
Bomber appeared at turn 217. That continuation uses the old AI and does not
validate this patch. Earlier completed research, aircraft, captures, and a
Domination victory remain unverified.
