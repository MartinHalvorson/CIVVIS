# Culture forecast version two

`culture-lane-forecast-2` projects secular and religious Tourism through each living rival's current `international_tourism_multiplier`. The original forecast gives every rival a full-strength market. Version two therefore responds to open borders, trade, different governments, incoming modifiers, and religious penalties, and excludes teammates from both its market and rival bar.

Both versions remain selectable. Enabling one disables the other. Version two is off in deployment; all 139 existing deployment tags, their prior selection, and 39 hysteresis-held decisions are retained.

This remains a constant-rate forecast. It does not predict future diplomatic changes or concerts and retains the original omission of Film Studio and cultural-alliance additions. The existing forecast's rates and counters still define the projection; the change is how current international modifiers price those rates.

## Validation

Seven focused culture forecast tests pass. The new cases cover exclusive/reversible version selection, religious market penalties, secular tourism unaffected by different religions, open borders and expiry, and teammate exclusion. All 260 relevant Python registry, append-point, evidence, evaluation-manifest, and genome-cost tests pass.

The short standard screen declared and completed six Emperor games, seeds 9136500–9136505: 36 complete seats and six winners, verified against the analysis with `continuous_screen_status.py`. Both family versions were screened, all other genes held at the deployment defaults, with the standard independent family draw (0.45 probability of version one, 0.30 of version two, 0.25 off). The header was read back and checked for the standard six-player, 74×46 Continents, nine-city-state, Online configuration, native competitions, and the frozen binary's source and hash.

| Family state | Seats | Wins |
|---|---:|---:|
| Off | 14 | 4 |
| Version one | 15 | 2 |
| Version two | 7 | 0 |

This is execution evidence and a runtime smoke test. Seven version-two seats cannot establish an improvement or justify moving a default; its marginal win-rate resolution was ±51.6 percentage points. The artifact satisfies the repository's nonzero-screen-evidence gate without entering the deployment ledger's source or reporting-batch lists. A twenty-second sample of this smoke process was used to locate CPU hotspots; its timing is not a performance benchmark.

The frozen screen executable was built from clean `27d0d27b79dda1a08193f18d1a3f3161f5cf8ee8`, SHA-256 `5c911c21e47703d8902d384def359864e23c2dd4800857a6e533ec0eb03900e6`, with compiled-gene fingerprint `d97338b845aaf9ac1abb5166c3ac1edefe5ab14396d229a596b63d92a83fdeb2`. Its unmodified analysis is committed at `docs/gene_screens/fires/culture-lane-forecast-2.json`. Raw rows, the checked header, and the run plan remain under `civvis-runs/0913-culture-v2-*`.
