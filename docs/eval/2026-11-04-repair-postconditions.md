# Repair postconditions must distinguish pillaged predecessors

The production verifier accepted an owned district or building as evidence that its production order succeeded. An already present, still pillaged item therefore earned `order_verified` even when the following state showed another item in the queue. `city_holds` now requires an unpillaged district or building. A repair named by the current queue still proves that it is queued; an observed intact item proves the resulting infrastructure is present. An intact unfinished district continues to prove placement.

A discovery scan of 17 completed runs (recent Standard Immortal/Deity games and the motivating older Australia run) found four examples in two games where an existing complete district remained pillaged, the next turn produced another item, and the old ledger recorded `order_verified`:

| Run | Ordered turn | City | Requested district | Next-turn queue |
|---|---:|---|---|---|
| `civvis-20261103T070446Z` | 160 | Toowoomba | `DISTRICT_THEATER` | `BUILDING_FACTORY` |
| `civvis-20261103T070446Z` | 160 | Adelaide | `DISTRICT_CAMPUS` | `BUILDING_SEWER` |
| `civvis-20261103T222149Z` | 290 | Pydna | `DISTRICT_CAMPUS` | `DISTRICT_NEIGHBORHOOD` |
| `civvis-20261103T222149Z` | 301 | Alexandria | `DISTRICT_INDUSTRIAL_ZONE` | `DISTRICT_NEIGHBORHOOD` |

These are postcondition failures, not proof that the host never accepted a transient order. Subsequent queue overrides or renewed damage can obscure what happened. The verifier compares observed states; it is not an operation acknowledgement. Do not use mere district ownership as a repair execution receipt.

The same discovery includes a positive native control: Canberra's pillaged Neighborhood at turn152 of `civvis-20261103T070446Z` is intact at153, with its queue cleared. The new fixture retains that successful repair alongside the four negative cases. `tools/fixtures/native_repair_postconditions.json` contains the original selected city fields, old verdicts and source-event SHA-256 values; unrelated city fields are omitted. It is not a complete replay input.

Firaxis's `Base/Assets/UI/Panels/ProductionPanel.lua:1996` reads `cityDistricts:IsPillaged(row.Index)` for a district's Repair status; line2075 reads the building equivalent. Those are already exported as district `pillaged` and city `pillaged_buildings` and are the fields used by this check.

Ten focused tests cover the native fragments, a still pillaged building, queued district and building repairs, an observed building repair after its queue clears, ordinary unfinished district placement, and a pillaged building that cannot by itself prove a purchase. No repair-policy activation, counterfactual survival or strength result is established by these verifier fixtures.

A known building repair whose later pillage-status list is absent is `Unverifiable` unless its queue proves it started. Missing status preserves the historical presence evidence for an ordinary new building that was not a known repair. Tests cover both paths.
