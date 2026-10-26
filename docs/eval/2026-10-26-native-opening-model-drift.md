# Fixed Native opening-model drift audit

All **83 registered passive reads complete successfully**. The 219 matched city-read records cover 207 distinct city-turns from the fixed sixteen-game Immortal Domination cohort. Every city matches by Native coordinates; every terrain-worker set matches. All six city yields remain within the registered 0.05 absolute tolerance before per-city host correction.

This narrows the search for the cohort’s early Production deficit: these snapshots provide no evidence of a material city-yield import error or a different imported terrain-worker allocation. It does not validate future growth forecasts or establish a strategy improvement.

## Scope and identities

Use exactly the prior opening census’s eighty cases at frame-zero turns 10, 20, 30, 40 and 50, preserving all eighty-three actual captured reads. The selection, protocol, reader and six semantic controls were published before queries at `206fdfeb203e4c43148da474f13c35c39b251dab`. Reconstructed prefix lengths and SHA-256 hashes must match the census before each query. No later events are added. All three same-frame alternate reads produce identical comparison objects, including the case whose same-turn plot availability differs.

The frozen actual Native adapter is built from `0b66913516fce803117c153c7125ba6c2144c613`, SHA-256 `85d5a904f8df01055d13775e5285fe41737a7a02d96abe61dfe3105ab0f53a09`. Each command uses a fresh disposable mirror and passive `--dump-mirror`; it returns before AI planning. No Native orders are generated or issued.

Compare `city.yields` from the captured host state against diagnostic `model_yields`, not the corrected `yields` display. The model already includes host-derived per-plot corrections and imported worked-tile choices. Therefore this is a snapshot consistency check after those imports, not an independent reconstruction of every terrain rule or a test of citizen optimization. The diagnostic twins and correction subtraction are visible in `src/bin/civvis_orders.rs:10200–10246` at the frozen source revision.

## Yield and worker comparisons

| Yield | Known city-read records | Differences above 0.05 | Mean absolute difference | Maximum absolute difference |
| --- | ---: | ---: | ---: | ---: |
| Food | 219 | 0 | 0 | 0 |
| Production | 219 | 0 | 0.000581 | 0.003120 |
| Gold | 219 | 0 | 0.000509 | 0.003120 |
| Science | 219 | 0 | 0.000763 | 0.003125 |
| Culture | 219 | 0 | 0.001486 | 0.006090 |
| Faith | 219 | 0 | 0.000096 | 0.003120 |

There are no missing or duplicate city coordinates, unknown yield readings, or terrain-worker set differences. Corrected displays also match within tolerance, but that agreement is calibration rather than additional fidelity evidence.

## Housing and amenity residuals

Raw model housing equals the host in 187 records, exceeds it by 0.5 in thirty, and exceeds it by 1 in two. Flooring fractional model housing to the integral Native reading removes all thirty half-point differences. This arithmetic compatibility does not establish the host’s hidden fractional housing or its rounding implementation.

The two remaining housing differences are **Cuenca at turns 40 and 50 of `civvis-20261026T000859Z`**. The host reports housing 2 and water housing 2; the raw model reports housing 3 and water housing 3. All other reported housing source categories agree. The corrected total is 2. The source of this one-point water residual remains unresolved; it is not grounds for a gameplay-rule rewrite.

Two raw amenity totals differ at turn 50 of `civvis-20261026T014409Z`: **Panamá host 3 versus model 2**, and **Popayán host 1 versus model 2**. The signed differences cancel but each city remains separately flagged. The host source ledger reports Panamá with 2 luxury amenities and 1 civic amenity, and Popayán with 1 luxury amenity. These facts do not identify the cause of the model’s allocation difference. Corrected totals follow the host. No negative or unavailable accessor is silently converted to agreement; this sample happens to have 219 known housing and amenity pairs.

## Validation and evidence

Six semantic controls pass: coordinate matching, raw-versus-corrected yields, missing/nonfinite readings, fractional housing, ambiguous minus-one amenities versus other negative readings, and separate variants. The summarizer regenerates every comparison from the retained raw diagnostic stdout and the exact census state. Repeated summaries are byte-identical. All 83 diagnostic exits are zero; no query error is omitted.

The evidence archive retains all stdout/stderr, commands, comparison objects, selection, protocol, reader, controls, summary source, logs and source identities. Its per-file manifest records byte hashes and independent read times. The original full captured prefixes are already retained in the completed Slinger cohort archives; the companion census archive contains the exact selected states. The actual adapter bytes are already retained in the prospective forward-airfield activation archive. This packet references their exact identities rather than duplicating them.

No gameplay, runtime policy, trial assignment, default or ledger changes follow from this audit.

The sealed archive has **265 verified members, 118,480 bytes**, SHA-256 `98e4e6f1f0758be93c63f146f5f883713ca7b138cb3b1a3ab621395325a683ea`.
