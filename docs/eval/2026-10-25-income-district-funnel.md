# Eligible income districts through the production selector

The fixed post-hoc native census through2026-10-25T18:05Z includes all9
completed Immortal run segments from161417Z:all losses,none with a completed
Commercial Hub at their first t100 frame. Four later show bankruptcy amenities;
five do not. Currency is observed by t47–64. There are171 idle city/turn frames
at t60–100 with a native Commercial Hub quote within20 turns, before other
policy gates. The exact census, native quotes, selected frames, production
claims and file digests are retained in `native-context.json.gz` (SHA256
cef9d907c082044c030e368a3947538ad06dff17f256560b20b1cda3117054fc). These are offered
opportunities, not proof that the final controller could or should build171
Hubs. Do not attribute all losses to this helper or to bankruptcy.

## Registered eligibility check before changing production code

`commercial_hub_item` currently ranks every unthreatened city with a spatial
site, then checks `can_produce` and the20-turn limit only for the city being
asked. Hypothesis:a city failing either of those existing requirements can
still occupy one of the preferred ranks, preventing an eligible lower-ranked
city from building a wanted Hub. This is a selector eligibility defect; the
native caller contribution remains to be traced independently.

Add two actual-production regressions to the existing commercial step suite
while keeping production code unchanged. In the same existing six-city
fixture, the top3 city ranks respectively have native21-turn quotes or explicit
production refusals; the other3 offer eligible Hubs within20 turns. Assert
spatial/legal/quote preconditions, then require each eligible city to choose
and successfully apply a real Produce action without reissuing a refused or
slow candidate. Existing commercial-step tests are the unchanged controls.
Derive their names/count from source and the built test catalog rather than a
manual counter. Fixture/compiler failures are retained but are not RED.

If both intended assertions fail on unchanged code and all existing controls
pass, move those same legality/time tests before city ranking. Keep the
existing desired number of Hubs, production ranking, tie ordering, sites,
window, threat, Granary, Settler, Market and Trader priorities. Add no new
strategy/default or native setting. Repeat the exact focused suite GREEN,
run complete required validation and check the full controller path before
claiming any native improvement. Broader queue-priority rewrites need their
own observed caller evidence and evaluation.

The first fixture build fails at two district-site calls: `&str` does not
implement this API's typed `AsName` parameter. That compiler output and the
original test source remain in `compile-error.json.gz`; no logical assertion
runs and no production patch is qualified. Only those calls change to typed
`Name` values before the complete unchanged-production suite is rerun.

The next complete attempt has6 passing controls and the intended refusal
assertion, but the slow-quote case stops at its spatial-site precondition:
one random coastal city has no free legal Hub site. Its complete source,
log and catalog remain in `fixture-error.json.gz`, not a complete RED. Keep
both original seeds and cases, clear irrelevant features/resources/water from
owned free fixture tiles, assert each site, and recompute the production rank
before rerunning unchanged production. No game or experimental map is replaced.

## Genuine regression result

After only the fixture API/terrain corrections, all6 original controls pass
and both new cases reach their intended eligible-city assertion on unchanged
production code. The exact8 source names match the built catalog. Complete
RED logs, source and catalog are retained in `red.json.gz`. No fixture error
is counted as one of these two logical failures.

A separate post-hoc menu-only native sanity scan finds0 top-three slow-quote
versus lower idle/fast-offer patterns in the fixed9-run t60–100 corpus. Its
inputs/limitations remain in `menu-sanity.json.gz`. It does not reconstruct
model spatial sites, refusals, held foundations, plan threats or actual helper
execution; the generic selector defect is not established as the reason those
native Hubs were absent. The higher-priority queue funnel still needs a caller
trace. Correct only the existing eligibility order, without changing priorities
or claiming a native benefit from the synthetic regressions.

## Focused correction result

The same8 tests now pass, including all6 original controls and the2 actual
Produce regressions. The selector validates each city's existing legality
and20-turn limit before that city takes a preferred rank, retaining its chosen
site with the rank. The wanted Hub count, Production/city tie order, threat
checks, window and Granary/Settler/Trader/Market priorities remain. There is
no new weight, strategy, default or host setup. Complete output, exact source,
controller/catalog provenance and binary digests remain in `green.json.gz`.

The first hosted regression checkpoint still has the retained spatial-fixture
failure, so its complete failed log is retained as development history. The
registered genuineRED comes from the later corrected fixture on unchanged
production, not from that hosted failure. Full CI-profile strategy tests,
merged-source checking and changed-line quality remain required before this
correction can integrate; a local8-case pass is not substituted for them.

## Final merged fixture validation

Main `91a282bad` merges cleanly as `774cd3871`, including the completed
defender-purchase correction. The producer file remains byte-identical to the
focused GREEN source; `cargo check --locked` passes on this merged tree.
Changed-line quality then identifies one test-fixture copy expression: replace
`iter().copied().collect()` with `to_vec()`. The exact source amendment and
failed quality output remain in `fixture-copy-amendment.json.gz`.

After that mechanical copy change, all8 focused tests pass again on the
merged source, with the same6 controls and2 actual-Produce cases. The final
test file equals the previous qualified source except for that one copy
expression. Complete final source, logs, controller receipts and merged check
remain in `final-green.json.gz`. The final committed tree still requires a
passing changed-line gate and the complete hosted CI-profile strategy suite
before integration; no broad local result is invented for this small repair.

Final local changed-line quality passes on committed `5cc23dd5b`, including
the corrected test copy. Complete output, exit status and source fingerprints
remain in `integration.json.gz`. The final source is now ready for the required
hosted CI-profile strategy/performance gates and integration. The focused
8-case result and compile check retain their exact scopes stated above.
