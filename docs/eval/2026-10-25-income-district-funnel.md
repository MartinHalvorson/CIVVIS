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
